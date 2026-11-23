//! A unified diff as the rows the diff view draws: what each line is, and its line number.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    /// An `@@ -a,b +c,d @@` header opening a hunk.
    Hunk,
    Added,
    Removed,
    Context,
    /// Anything else: "\ No newline at end of file", or git's message when there is no diff.
    Meta,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub kind: RowKind,
    /// Line number in the new file, or in the old one for a removed line.
    pub number: Option<u32>,
}

/// The rows of a diff from its first hunk on; the file header before it is left out.
#[derive(Debug, Default)]
pub struct DiffRows {
    /// Lines of the diff text left out before the first row.
    pub skip: usize,
    pub rows: Vec<Row>,
    /// The file's own text of each row, without the `+`/`-` column: one line per row, empty for
    /// rows that aren't code.
    pub code: String,
}

/// Old and new start lines of a hunk header such as `@@ -25,6 +28,9 @@ fn main()`.
fn hunk_starts(line: &str) -> Option<(u32, u32)> {
    let mut parts = line.split(' ').skip(1);
    let start = |part: &str, sign: char| part.strip_prefix(sign)?.split(',').next()?.parse().ok();
    Some((start(parts.next()?, '-')?, start(parts.next()?, '+')?))
}

pub fn parse(text: &str) -> DiffRows {
    let lines: Vec<&str> = text.lines().collect();
    let skip = lines.iter().position(|l| l.starts_with("@@")).unwrap_or(0);
    let (mut old, mut new) = (0, 0);
    let mut in_hunk = false;
    let mut rows = Vec::with_capacity(lines.len() - skip);
    let mut code = String::with_capacity(text.len());
    for line in &lines[skip..] {
        let (kind, number, text) = match line.as_bytes().first() {
            Some(b'@') => {
                (old, new) = hunk_starts(line).unwrap_or((old, new));
                in_hunk = true;
                (RowKind::Hunk, None, "")
            }
            Some(b'+') if in_hunk => {
                new += 1;
                (RowKind::Added, Some(new - 1), &line[1..])
            }
            Some(b'-') if in_hunk => {
                old += 1;
                (RowKind::Removed, Some(old - 1), &line[1..])
            }
            Some(b' ') if in_hunk => {
                (old, new) = (old + 1, new + 1);
                (RowKind::Context, Some(new - 1), &line[1..])
            }
            _ => (RowKind::Meta, None, ""),
        };
        if !rows.is_empty() {
            code.push('\n');
        }
        code.push_str(text);
        rows.push(Row { kind, number });
    }
    DiffRows { skip, rows, code }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &str = "diff --git a/a.rs b/a.rs\nindex 1..2 100644\n--- a/a.rs\n+++ b/a.rs\n@@ -25,3 +28,4 @@ fn main() {\n one\n-two\n+2\n+3\n four\n\\ No newline at end of file";

    #[test]
    fn numbers_rows_from_the_hunk_header() {
        let diff = parse(DIFF);
        assert_eq!(diff.skip, 4);
        let rows: Vec<_> = diff.rows.iter().map(|r| (r.kind, r.number)).collect();
        let expected = [
            (RowKind::Hunk, None),
            (RowKind::Context, Some(28)),
            (RowKind::Removed, Some(26)),
            (RowKind::Added, Some(29)),
            (RowKind::Added, Some(30)),
            (RowKind::Context, Some(31)),
            (RowKind::Meta, None),
        ];
        assert_eq!(rows, expected);
        assert_eq!(diff.code, "\none\ntwo\n2\n3\nfour\n");
        assert_eq!(diff.code.lines().count() + 1, diff.rows.len(), "one code line per row");
    }

    #[test]
    fn text_that_is_not_a_diff_is_all_meta() {
        let diff = parse("Cannot run git: not found\n+not an added line");
        assert_eq!(diff.skip, 0);
        assert!(diff.rows.iter().all(|r| r.kind == RowKind::Meta));
    }
}
