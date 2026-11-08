//! VS Code's line commands on a text and a selection, both in character indices: move or copy the
//! selected lines up or down, delete them, toggle their comments. Each returns the new text and
//! where the selection lands.

/// A selection as (anchor, cursor) character indices.
pub type Sel = (usize, usize);

/// Start (character index) of every line, plus one past the end.
fn line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (i, c) in text.chars().enumerate() {
        if c == '\n' {
            starts.push(i + 1);
        }
    }
    starts.push(text.chars().count() + 1);
    starts
}

/// The lines `sel` touches, first and last (zero-based). A selection ending at a line's very
/// start leaves that line out, as VS Code does.
fn lines_of(starts: &[usize], sel: Sel) -> (usize, usize) {
    let (lo, hi) = (sel.0.min(sel.1), sel.0.max(sel.1));
    let line = |at: usize| starts.partition_point(|s| *s <= at) - 1;
    let first = line(lo);
    let last = line(hi);
    let last = if hi > lo && last > first && starts[last] == hi { last - 1 } else { last };
    (first, last)
}

fn lines(text: &str) -> Vec<&str> {
    text.split('\n').collect()
}

/// Moves the selected lines one up (`up`) or down, swapping them with their neighbour.
pub fn move_lines(text: &str, sel: Sel, up: bool) -> Option<(String, Sel)> {
    let starts = line_starts(text);
    let (first, last) = lines_of(&starts, sel);
    let mut rows = lines(text);
    if (up && first == 0) || (!up && last + 1 >= rows.len()) {
        return None;
    }
    let (neighbour, shift) = if up { (first - 1, -1) } else { (last + 1, 1) };
    let moved = rows.remove(neighbour);
    let at = if up { last } else { first };
    rows.insert(at, moved);
    let delta = moved.chars().count() + 1;
    let by = |i: usize| if shift < 0 { i - delta } else { i + delta };
    Some((rows.join("\n"), (by(sel.0), by(sel.1))))
}

/// Copies the selected lines above (`up`) or below themselves; the selection follows the copy
/// going down and stays on the copy going up, as VS Code's.
pub fn copy_lines(text: &str, sel: Sel, up: bool) -> (String, Sel) {
    let starts = line_starts(text);
    let (first, last) = lines_of(&starts, sel);
    let mut rows = lines(text);
    let block: Vec<&str> = rows[first..=last].to_vec();
    let size: usize = block.iter().map(|r| r.chars().count() + 1).sum();
    for (i, row) in block.iter().enumerate() {
        rows.insert(last + 1 + i, row);
    }
    let sel = if up { sel } else { (sel.0 + size, sel.1 + size) };
    (rows.join("\n"), sel)
}

/// Deletes the selected lines; the cursor lands on the line that took their place.
pub fn delete_lines(text: &str, sel: Sel) -> (String, Sel) {
    let starts = line_starts(text);
    let (first, last) = lines_of(&starts, sel);
    let mut rows = lines(text);
    let col = sel.1 - starts[lines_of(&starts, (sel.1, sel.1)).0];
    rows.drain(first..=last);
    if rows.is_empty() {
        return (String::new(), (0, 0));
    }
    let line = first.min(rows.len() - 1);
    let start: usize = rows[..line].iter().map(|r| r.chars().count() + 1).sum();
    let at = start + col.min(rows[line].chars().count());
    (rows.join("\n"), (at, at))
}

/// Comments the selected lines with `prefix` (and `suffix` for block-only languages such as CSS),
/// or uncomments them when every non-blank one already is. The comment goes at the block's
/// shallowest indentation, as VS Code puts it; the selection stays on the same text.
pub fn toggle_comment(text: &str, sel: Sel, prefix: &str, suffix: &str) -> (String, Sel) {
    let starts = line_starts(text);
    let (first, last) = lines_of(&starts, sel);
    let mut rows: Vec<String> = lines(text).into_iter().map(str::to_string).collect();
    let filled = |r: &str| !r.trim().is_empty();
    let block: Vec<usize> = (first..=last).filter(|i| filled(&rows[*i])).collect();
    let commented =
        |r: &str| r.trim().starts_with(prefix.trim_end()) && r.trim().ends_with(suffix.trim_start());
    let uncomment = !block.is_empty() && block.iter().all(|i| commented(&rows[*i]));
    let indent = block.iter().map(|i| chars_before_text(&rows[*i])).min().unwrap_or(0);
    // Per edited line: where its marker sits (column), how many characters the marker added
    // (negative when removed), and how many the whole line gained.
    let mut edits: Vec<(usize, usize, isize, isize)> = Vec::new();
    for &line in &block {
        let row = &rows[line];
        let old = row.chars().count() as isize;
        let (at, new_row) = if uncomment {
            let at = chars_before_text(row);
            let split = row.char_indices().nth(at).map_or(row.len(), |(i, _)| i);
            let body = &row[split..];
            let body =
                body.strip_prefix(prefix).or_else(|| body.strip_prefix(prefix.trim_end())).unwrap_or(body);
            let body =
                body.strip_suffix(suffix).or_else(|| body.strip_suffix(suffix.trim_start())).unwrap_or(body);
            (at, format!("{}{body}", &row[..split]))
        } else {
            let split = row.char_indices().nth(indent).map_or(row.len(), |(i, _)| i);
            (indent, format!("{}{prefix}{}{suffix}", &row[..split], &row[split..]))
        };
        let marker = if uncomment {
            let body = row.chars().skip(at).collect::<String>();
            let stripped = if body.starts_with(prefix) { prefix } else { prefix.trim_end() };
            -(stripped.chars().count() as isize)
        } else {
            prefix.chars().count() as isize
        };
        edits.push((line, at, marker, new_row.chars().count() as isize - old));
        rows[line] = new_row;
    }
    let shift = |end: usize| {
        let line = starts.partition_point(|s| *s <= end) - 1;
        let col = end - starts[line];
        let mut moved = end as isize;
        for &(l, at, marker, gained) in &edits {
            if l < line {
                moved += gained;
            } else if l == line && col >= at {
                // Inside a removed marker the end goes to where the marker was.
                moved += marker.max(-((col - at) as isize));
            }
        }
        moved.max(0) as usize
    };
    (rows.join("\n"), (shift(sel.0), shift(sel.1)))
}

/// Columns of indentation before the line's text.
fn chars_before_text(row: &str) -> usize {
    row.chars().take_while(|c| c.is_whitespace()).count()
}

/// The line comment markers for `ext`'s language: a prefix, and a suffix where a language only
/// has block comments.
pub fn comment_markers(path: &std::path::Path) -> (&'static str, &'static str) {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_ascii_lowercase();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    match ext.as_str() {
        _ if matches!(name.as_str(), "dockerfile" | "makefile" | "gnumakefile") => ("# ", ""),
        "py" | "sh" | "bash" | "zsh" | "fish" | "rb" | "yaml" | "yml" | "toml" | "ini" | "conf" | "cfg"
        | "r" | "pl" | "ex" | "exs" | "nix" | "tf" | "env" | "gitignore" | "dockerignore" => ("# ", ""),
        "sql" | "lua" | "hs" | "elm" => ("-- ", ""),
        "css" => ("/* ", " */"),
        "html" | "htm" | "xml" | "svg" | "md" | "markdown" | "vue" | "svelte" | "astro" | "plist" => {
            ("<!-- ", " -->")
        }
        "liquid" => ("{% comment %} ", " {% endcomment %}"),
        "clj" | "cljs" | "lisp" | "el" | "scm" => (";; ", ""),
        _ if name.starts_with(".env") => ("# ", ""),
        _ => ("// ", ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "one\ntwo\nthree";

    #[test]
    fn moves_lines_with_the_cursor() {
        // Cursor on "two", column 1.
        assert_eq!(move_lines(TEXT, (5, 5), true), Some(("two\none\nthree".into(), (1, 1))));
        assert_eq!(move_lines(TEXT, (5, 5), false), Some(("one\nthree\ntwo".into(), (11, 11))));
        assert_eq!(move_lines(TEXT, (1, 1), true), None, "first line can't go up");
        // A selection over "one" and "two" moves as a block.
        assert_eq!(move_lines(TEXT, (0, 6), false), Some(("three\none\ntwo".into(), (6, 12))));
    }

    #[test]
    fn a_selection_ending_at_a_line_start_leaves_that_line() {
        // "one\n" selected whole: only "one" moves.
        assert_eq!(move_lines(TEXT, (0, 4), false), Some(("two\none\nthree".into(), (4, 8))));
    }

    #[test]
    fn copies_and_deletes_lines() {
        assert_eq!(copy_lines(TEXT, (5, 5), false), ("one\ntwo\ntwo\nthree".into(), (9, 9)));
        assert_eq!(copy_lines(TEXT, (5, 5), true), ("one\ntwo\ntwo\nthree".into(), (5, 5)));
        assert_eq!(delete_lines(TEXT, (5, 5)), ("one\nthree".into(), (5, 5)));
        assert_eq!(delete_lines(TEXT, (10, 10)), ("one\ntwo".into(), (6, 6)), "last line: cursor goes up");
        assert_eq!(delete_lines("solo", (2, 2)), (String::new(), (0, 0)));
    }

    #[test]
    fn toggles_comments_at_the_shallowest_indent() {
        let text = "fn a() {\n    x();\n        y();\n}";
        let (commented, _) = toggle_comment(text, (13, 30), "// ", "");
        assert_eq!(commented, "fn a() {\n    // x();\n    //     y();\n}");
        let (back, _) = toggle_comment(&commented, (13, 33), "// ", "");
        assert_eq!(back, text);
        let (css, _) = toggle_comment("a { b: c; }", (0, 0), "/* ", " */");
        assert_eq!(css, "/* a { b: c; } */");
        assert_eq!(toggle_comment(&css, (0, 0), "/* ", " */").0, "a { b: c; }");
    }

    #[test]
    fn comment_keeps_the_cursor_on_its_text() {
        // Cursor after "x" in "    x();" (col 5).
        let (_, sel) = toggle_comment("    x();", (5, 5), "// ", "");
        assert_eq!(sel, (8, 8));
        let (_, sel) = toggle_comment("    // x();", (8, 8), "// ", "");
        assert_eq!(sel, (5, 5));
    }
}
