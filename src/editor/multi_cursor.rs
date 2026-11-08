//! VS Code's multiple cursors over a plain text, in character indices: adding the next match of
//! the selection (⌘D), every match (⇧⌘L), a cursor on the line above or below (⌥⌘↑↓), undoing the
//! last one added (⌘U), and typing, deleting and moving at all of them at once.

pub use super::line_ops::Sel;

fn lo(s: Sel) -> usize {
    s.0.min(s.1)
}

fn hi(s: Sel) -> usize {
    s.0.max(s.1)
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The word around character `at`, if there is one.
pub fn word_at(text: &str, at: usize) -> Option<Sel> {
    let chars: Vec<char> = text.chars().collect();
    let touches = |i: usize| chars.get(i).is_some_and(|c| is_word(*c));
    let at = if touches(at) {
        at
    } else if at > 0 && touches(at - 1) {
        at - 1
    } else {
        return None;
    };
    let start = (0..=at).rev().take_while(|i| touches(*i)).last()?;
    let end = (at..chars.len()).take_while(|i| touches(*i)).last()? + 1;
    Some((start, end))
}

fn slice(chars: &[char], s: Sel) -> String {
    chars[lo(s)..hi(s)].iter().collect()
}

/// Character indices where `needle` starts, in order.
fn matches(chars: &[char], needle: &[char]) -> Vec<usize> {
    if needle.is_empty() || needle.len() > chars.len() {
        return Vec::new();
    }
    (0..=chars.len() - needle.len()).filter(|i| chars[*i..*i + needle.len()] == *needle).collect()
}

/// ⌘D: with nothing selected, selects the word at the cursor; otherwise adds the next match of
/// the selection after the last cursor, wrapping around. Returns the new primary selection; the
/// old primary joins `extra`.
pub fn select_next(text: &str, primary: Sel, extra: &mut Vec<Sel>) -> Option<Sel> {
    if primary.0 == primary.1 {
        return word_at(text, primary.1);
    }
    let chars: Vec<char> = text.chars().collect();
    let needle: Vec<char> = slice(&chars, primary).chars().collect();
    let taken = |at: usize| lo(primary) == at || extra.iter().any(|s| lo(*s) == at);
    let found = matches(&chars, &needle);
    let after = found.iter().find(|at| **at > lo(primary) && !taken(**at));
    let next = after.or_else(|| found.iter().find(|at| !taken(**at)))?;
    extra.push(primary);
    Some((*next, next + needle.len()))
}

/// ⇧⌘L: every match of the selection (or of the word at the cursor) becomes a cursor; the
/// primary stays where it was.
pub fn select_all(text: &str, primary: Sel, extra: &mut Vec<Sel>) -> Option<Sel> {
    let primary = if primary.0 == primary.1 { word_at(text, primary.1)? } else { primary };
    let chars: Vec<char> = text.chars().collect();
    let needle: Vec<char> = slice(&chars, primary).chars().collect();
    extra.clear();
    extra.extend(
        matches(&chars, &needle)
            .into_iter()
            .filter(|at| *at != lo(primary))
            .map(|at| (at, at + needle.len())),
    );
    Some(primary)
}

/// Line and column of character `at`.
fn line_col(chars: &[char], at: usize) -> (usize, usize) {
    let before = &chars[..at.min(chars.len())];
    let line = before.iter().filter(|c| **c == '\n').count();
    let col = before.iter().rev().take_while(|c| **c != '\n').count();
    (line, col)
}

/// Character index of `line`'s column `col`, clamped to the line's end; `None` past the text.
fn index_of(chars: &[char], line: usize, col: usize) -> Option<usize> {
    let mut start = 0;
    for _ in 0..line {
        start += chars[start..].iter().position(|c| *c == '\n')? + 1;
    }
    let len = chars[start..].iter().position(|c| *c == '\n').unwrap_or(chars.len() - start);
    Some(start + col.min(len))
}

/// ⌥⌘↑/↓: a cursor on the line above the topmost cursor, or below the bottommost, at the same
/// column; it becomes the primary.
pub fn add_line(text: &str, primary: Sel, extra: &mut Vec<Sel>, up: bool) -> Option<Sel> {
    let chars: Vec<char> = text.chars().collect();
    let all = extra.iter().chain([&primary]);
    let edge = if up { all.min_by_key(|s| s.1) } else { all.max_by_key(|s| s.1) }.copied()?;
    let (line, col) = line_col(&chars, edge.1);
    let target = if up { line.checked_sub(1)? } else { line + 1 };
    let at = index_of(&chars, target, col)?;
    extra.push(primary);
    Some((at, at))
}

/// ⌘U: drops the cursor added last; the one before it becomes the primary again.
pub fn undo_last(extra: &mut Vec<Sel>) -> Option<Sel> {
    extra.pop()
}

pub enum Edit {
    Insert(String),
    Backspace,
    Delete,
    /// A line break carrying the line's indentation over.
    Newline,
    Move {
        dir: Dir,
        extend: bool,
    },
}

#[derive(Clone, Copy)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
}

/// Applies `edit` at every cursor in `cursors`, returning the new text when it changed. Cursors
/// that end up on the same spot merge.
pub fn apply(text: &str, cursors: &mut Vec<Sel>, edit: &Edit) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    if let Edit::Move { dir, extend } = edit {
        for s in cursors.iter_mut() {
            *s = moved(&chars, *s, *dir, *extend);
        }
        dedup(cursors);
        return None;
    }
    // Each cursor's range in the original text and what replaces it, left to right.
    let mut order: Vec<usize> = (0..cursors.len()).collect();
    order.sort_by_key(|i| lo(cursors[*i]));
    let mut out: Vec<char> = Vec::with_capacity(chars.len() + cursors.len());
    let mut copied = 0;
    for i in order {
        let s = cursors[i];
        let (mut start, end, insert): (usize, usize, Vec<char>) = match edit {
            _ if s.0 != s.1 && !matches!(edit, Edit::Newline | Edit::Insert(_)) => (lo(s), hi(s), Vec::new()),
            Edit::Insert(t) => (lo(s), hi(s), t.chars().collect()),
            Edit::Newline => {
                let line_start = chars[..lo(s)].iter().rposition(|c| *c == '\n').map_or(0, |p| p + 1);
                let indent = chars[line_start..].iter().take_while(|c| **c == ' ' || **c == '\t');
                (lo(s), hi(s), std::iter::once('\n').chain(indent.copied()).collect())
            }
            Edit::Backspace => (s.1.saturating_sub(1), s.1, Vec::new()),
            Edit::Delete => (s.1, (s.1 + 1).min(chars.len()), Vec::new()),
            Edit::Move { .. } => unreachable!("handled above"),
        };
        // Ranges of neighbouring cursors that overlap are cut where the last one ended.
        start = start.max(copied);
        let end = end.max(start);
        out.extend_from_slice(&chars[copied..start]);
        out.extend(insert);
        cursors[i] = (out.len(), out.len());
        copied = end;
    }
    out.extend_from_slice(&chars[copied..]);
    dedup(cursors);
    Some(out.into_iter().collect())
}

fn moved(chars: &[char], s: Sel, dir: Dir, extend: bool) -> Sel {
    let head = match dir {
        // Without Shift, a selection collapses to the side the arrow points to.
        Dir::Left if !extend && s.0 != s.1 => return (lo(s), lo(s)),
        Dir::Right if !extend && s.0 != s.1 => return (hi(s), hi(s)),
        Dir::Left => s.1.saturating_sub(1),
        Dir::Right => (s.1 + 1).min(chars.len()),
        Dir::Up | Dir::Down => {
            let (line, col) = line_col(chars, s.1);
            let target = if matches!(dir, Dir::Up) { line.checked_sub(1) } else { Some(line + 1) };
            target.and_then(|l| index_of(chars, l, col)).unwrap_or(if matches!(dir, Dir::Up) {
                0
            } else {
                chars.len()
            })
        }
        Dir::Home => chars[..s.1].iter().rposition(|c| *c == '\n').map_or(0, |p| p + 1),
        Dir::End => s.1 + chars[s.1..].iter().position(|c| *c == '\n').unwrap_or(chars.len() - s.1),
    };
    if extend { (s.0, head) } else { (head, head) }
}

fn dedup(cursors: &mut Vec<Sel>) {
    let mut seen = Vec::with_capacity(cursors.len());
    cursors.retain(|s| {
        let fresh = !seen.contains(s);
        seen.push(*s);
        fresh
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_next_walks_matches_and_wraps() {
        let text = "foo bar foo baz foo";
        let mut extra = Vec::new();
        let word = select_next(text, (1, 1), &mut extra).unwrap();
        assert_eq!((word, extra.len()), ((0, 3), 0), "first press selects the word");
        let second = select_next(text, word, &mut extra).unwrap();
        assert_eq!(second, (8, 11));
        let third = select_next(text, second, &mut extra).unwrap();
        assert_eq!((third, extra.clone()), ((16, 19), vec![(0, 3), (8, 11)]));
        assert_eq!(select_next(text, third, &mut extra), None, "every match taken");
        assert_eq!(undo_last(&mut extra), Some((8, 11)));
    }

    #[test]
    fn select_all_takes_every_match() {
        let mut extra = Vec::new();
        assert_eq!(select_all("ab x ab y ab", (0, 0), &mut extra), Some((0, 2)));
        assert_eq!(extra, vec![(5, 7), (10, 12)]);
    }

    #[test]
    fn adds_cursors_on_neighbouring_lines() {
        let text = "abcd\nab\nabcdef";
        let mut extra = Vec::new();
        let below = add_line(text, (3, 3), &mut extra, false).unwrap();
        assert_eq!(below, (7, 7), "clamped to the shorter line's end");
        let below = add_line(text, below, &mut extra, false).unwrap();
        assert_eq!(below, (10, 10), "from the bottom cursor, at its column");
        assert_eq!(add_line(text, (1, 1), &mut Vec::new(), true), None);
    }

    #[test]
    fn types_and_deletes_at_every_cursor() {
        let mut cursors = vec![(0, 3), (8, 11)];
        let text = apply("foo bar foo", &mut cursors, &Edit::Insert("qux".into())).unwrap();
        assert_eq!(text, "qux bar qux");
        assert_eq!(cursors, vec![(3, 3), (11, 11)]);
        let text = apply(&text, &mut cursors, &Edit::Backspace).unwrap();
        assert_eq!((text.as_str(), cursors.clone()), ("qu bar qu", vec![(2, 2), (9, 9)]));
        let mut cursors = vec![(6, 6)];
        assert_eq!(apply("    ab", &mut cursors, &Edit::Newline).unwrap(), "    ab\n    ");
    }

    #[test]
    fn moves_every_cursor_and_merges_them() {
        let mut cursors = vec![(1, 1), (6, 6)];
        apply("ab\ncd\nef", &mut cursors, &Edit::Move { dir: Dir::Up, extend: false });
        assert_eq!(cursors, vec![(0, 0), (3, 3)], "the first can't go up: to the text's start");
        apply("ab\ncd\nef", &mut cursors, &Edit::Move { dir: Dir::Home, extend: false });
        assert_eq!(cursors, vec![(0, 0), (3, 3)]);
        let mut cursors = vec![(0, 0), (1, 1)];
        apply("ab", &mut cursors, &Edit::Move { dir: Dir::Home, extend: false });
        assert_eq!(cursors, vec![(0, 0)], "merged");
    }
}
