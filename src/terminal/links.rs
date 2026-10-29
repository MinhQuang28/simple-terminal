use std::ops::Range;
use std::path::{Path, PathBuf};

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::grid::Row;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::Term;
use alacritty_terminal::term::cell::{Cell, Flags};

use super::session::EventProxy;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Link {
    Url(String),
    File { path: PathBuf, line: Option<u32>, col: Option<u32> },
}

/// A link and the viewport cells it spans: a column range on each row it covers, as a link the
/// terminal wrapped carries on into the rows below.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub rows: Vec<(i32, Range<usize>)>,
    pub link: Link,
}

const URL_SCHEMES: [&str; 3] = ["http://", "https://", "file://"];
/// Most rows one wrapped line is followed across, each way, looking for a link.
const MAX_WRAPPED_ROWS: i32 = 32;
const TRAILING_PUNCTUATION: &[char] = &['.', ',', ';', ':', '!', '?', ')', ']', '}', '\'', '"', '>'];

/// The link under viewport cell `(row, col)`: an OSC 8 hyperlink if the cell has one, else a
/// URL or an existing file path (optionally `:line:col`) written in the text of the line, rows
/// the terminal wrapped included.
pub fn at(term: &Term<EventProxy>, row: i32, col: usize, cwd: &Path) -> Option<Span> {
    let line = Line(row - term.grid().display_offset() as i32);
    // A row above the viewport may lie past the start of history in a fresh pane.
    if line < term.grid().topmost_line() {
        return None;
    }
    let grid_row = &term.grid()[line];
    if let Some(hyperlink) = grid_row[Column(col)].hyperlink() {
        let same = |c: usize| grid_row[Column(c)].hyperlink().as_ref() == Some(&hyperlink);
        let start = (0..=col).rev().take_while(|c| same(*c)).last().unwrap_or(col);
        let end = (col..grid_row.len()).take_while(|c| same(*c)).last().unwrap_or(col) + 1;
        let link = Link::Url(hyperlink.uri().to_string());
        return Some(Span { rows: vec![(row, start..end)], link });
    }
    let (text, cells) = line_text(term, line);
    let offset = term.grid().display_offset() as i32;
    let index = cells.iter().position(|&(l, c)| (l.0 + offset, c) == (row, col))?;
    let (token, range) = token_at(&text, index)?;
    let link = parse(&token, cwd)?;
    Some(Span { rows: row_ranges(&cells[range], offset), link })
}

/// The characters of the line `line` belongs to, following the rows the terminal wrapped it
/// across, plus the grid cell each character starts at.
fn line_text(term: &Term<EventProxy>, line: Line) -> (Vec<char>, Vec<(Line, usize)>) {
    let grid = term.grid();
    let wraps = |l: Line| grid[l][Column(grid.columns() - 1)].flags.contains(Flags::WRAPLINE);
    let mut first = line;
    while first > grid.topmost_line() && line.0 - first.0 < MAX_WRAPPED_ROWS && wraps(first - 1) {
        first -= 1;
    }
    let mut last = line;
    while last < grid.bottommost_line() && last.0 - line.0 < MAX_WRAPPED_ROWS && wraps(last) {
        last += 1;
    }
    let (mut text, mut cells) = (Vec::new(), Vec::new());
    for l in first.0..=last.0 {
        row_text(&grid[Line(l)], Line(l), &mut text, &mut cells);
    }
    (text, cells)
}

/// Appends the row's characters and the cell each one starts at.
fn row_text(row: &Row<Cell>, line: Line, text: &mut Vec<char>, cells: &mut Vec<(Line, usize)>) {
    for (col, cell) in row.into_iter().enumerate() {
        if cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
            continue;
        }
        text.push(cell.c);
        cells.push((line, col));
    }
}

/// The viewport rows a run of cells covers, with the columns on each.
fn row_ranges(cells: &[(Line, usize)], display_offset: i32) -> Vec<(i32, Range<usize>)> {
    let mut rows: Vec<(i32, Range<usize>)> = Vec::new();
    for &(line, col) in cells {
        let row = line.0 + display_offset;
        match rows.last_mut() {
            Some((r, cols)) if *r == row => cols.end = col + 1,
            _ => rows.push((row, col..col + 1)),
        }
    }
    rows
}

/// The whitespace-delimited token containing `index`, trimmed of surrounding quotes/brackets.
fn token_at(text: &[char], index: usize) -> Option<(String, Range<usize>)> {
    let boundary = |c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | '`');
    if boundary(text[index]) {
        return None;
    }
    let mut start = (0..index).rev().find(|i| boundary(text[*i])).map_or(0, |i| i + 1);
    let end = (index..text.len()).find(|i| boundary(text[*i])).unwrap_or(text.len());
    while start < index && matches!(text[start], '(' | '[' | '{') {
        start += 1;
    }
    let mut token: String = text[start..end].iter().collect();
    let mut trimmed = 0;
    while token.ends_with(TRAILING_PUNCTUATION) && !is_url(&token) {
        token.pop();
        trimmed += 1;
    }
    // Trailing `)` and `.` end sentences more often than URLs.
    while is_url(&token) && (token.ends_with(['.', ',', ')']) || token.ends_with(':')) {
        token.pop();
        trimmed += 1;
    }
    (!token.is_empty()).then_some((token, start..end - trimmed))
}

fn is_url(token: &str) -> bool {
    URL_SCHEMES.iter().any(|s| token.starts_with(s))
}

pub fn parse(token: &str, cwd: &Path) -> Option<Link> {
    if is_url(token) {
        return Some(Link::Url(token.to_string()));
    }
    let (path, line, col) = split_position(token);
    let expanded = match path.strip_prefix("~/") {
        Some(rest) => std::env::var_os("HOME").map(|h| PathBuf::from(h).join(rest))?,
        None => PathBuf::from(path),
    };
    let path = if expanded.is_absolute() { expanded } else { cwd.join(expanded) };
    path.is_file().then_some(Link::File { path, line, col })
}

/// Splits `src/a.rs:12:3` into the path and its optional line and column.
fn split_position(token: &str) -> (&str, Option<u32>, Option<u32>) {
    let mut parts = token.rsplitn(3, ':');
    let last = parts.next().unwrap_or(token);
    let Ok(last_number) = last.parse::<u32>() else { return (token, None, None) };
    let Some(middle) = parts.next() else { return (token, None, None) };
    match (middle.parse::<u32>(), parts.next()) {
        (Ok(line), Some(path)) if !path.is_empty() => (path, Some(line), Some(last_number)),
        _ => {
            let path = &token[..token.len() - last.len() - 1];
            (path, Some(last_number), None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_line_and_column_suffixes() {
        assert_eq!(split_position("src/a.rs:12:3"), ("src/a.rs", Some(12), Some(3)));
        assert_eq!(split_position("src/a.rs:12"), ("src/a.rs", Some(12), None));
        assert_eq!(split_position("src/a.rs"), ("src/a.rs", None, None));
        assert_eq!(split_position("12:3"), ("12", Some(3), None));
    }

    #[test]
    fn finds_tokens_and_trims_punctuation() {
        let text: Vec<char> = "see (src/main.rs:4). https://x.io/a, ok".chars().collect();
        assert_eq!(token_at(&text, 6).unwrap().0, "src/main.rs:4");
        assert_eq!(token_at(&text, 22).unwrap(), ("https://x.io/a".to_string(), 21..35));
        assert!(token_at(&text, 3).is_none());
    }

    #[test]
    fn follows_a_url_the_terminal_wrapped() {
        use alacritty_terminal::vte::ansi::{Processor, StdSyncHandler};
        let mut term = EventProxy::test_term(10, 4);
        let mut parser: Processor<StdSyncHandler> = Processor::new();
        parser.advance(&mut term, b"go https://a.io/xyz ok\r\nnext");
        let url = Link::Url("https://a.io/xyz".into());
        let span = Span { rows: vec![(0, 3..10), (1, 0..9)], link: url };
        assert_eq!(at(&term, 1, 4, Path::new("/")), Some(span.clone()));
        assert_eq!(at(&term, 0, 5, Path::new("/")), Some(span));
        assert_eq!(at(&term, 2, 1, Path::new("/")), None, "a row after the line ended");
    }

    #[test]
    fn parses_urls_and_existing_files() {
        let dir = std::env::temp_dir().join(format!("repodeck-links-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/main.rs"), "").unwrap();
        assert_eq!(parse("https://a.b/c", &dir), Some(Link::Url("https://a.b/c".into())));
        assert_eq!(
            parse("src/main.rs:7:2", &dir),
            Some(Link::File { path: dir.join("src/main.rs"), line: Some(7), col: Some(2) })
        );
        assert_eq!(parse("src/missing.rs", &dir), None);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
