use eframe::egui::{Align, Key, Layout, Modifiers, RichText, TextEdit, Ui};

/// Longest list of matches kept; past it the count shows as "9999+".
const MAX_MATCHES: usize = 10_000;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BarKind {
    Find,
    GoToLine,
}

/// A range of the text, in characters, for the editor to select and scroll to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Jump {
    pub start: usize,
    pub end: usize,
}

/// A match: its character range and the zero-based line it starts on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Found {
    pub start: usize,
    pub end: usize,
    pub line: usize,
}

/// The find or go-to-line bar shown above a file's text.
pub struct EditorBar {
    pub kind: BarKind,
    pub query: String,
    current: usize,
    matches: Vec<Found>,
    /// The query and text length `matches` were found for.
    searched: Option<(String, usize)>,
    opened: bool,
}

impl EditorBar {
    pub fn new(kind: BarKind) -> Self {
        Self { kind, query: String::new(), current: 0, matches: Vec::new(), searched: None, opened: true }
    }

    /// Matches in text order, with the current one's index.
    pub fn matches(&self) -> (&[Found], usize) {
        (&self.matches, self.current)
    }

    /// Searches again when the query or the text changed; returns true when it did.
    fn search(&mut self, text: &str) -> bool {
        let key = (self.query.clone(), text.len());
        if self.searched.as_ref() == Some(&key) {
            return false;
        }
        self.matches = find_all(text, &self.query);
        self.current = 0;
        self.searched = Some(key);
        true
    }
}

/// Shows the bar; returns where the editor should jump, and whether the bar closed.
pub fn show(ui: &mut Ui, bar: &mut EditorBar, text: &str) -> (Option<Jump>, bool) {
    let mut jump = None;
    let mut close = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let hint = match bar.kind {
            BarKind::Find => "Find",
            BarKind::GoToLine => "Line[:column]",
        };
        let edit = ui.add(TextEdit::singleline(&mut bar.query).hint_text(hint).desired_width(220.0));
        if std::mem::take(&mut bar.opened) {
            edit.request_focus();
        }
        let (shift, enter, escape) = ui.input_mut(|i| {
            (i.modifiers.shift, i.consume_key(Modifiers::NONE, Key::Enter), i.key_pressed(Key::Escape))
        });
        close = escape && edit.has_focus();
        match bar.kind {
            BarKind::Find => {
                let changed = bar.search(text);
                let mut step = 0;
                if edit.has_focus() && enter {
                    step = if shift { -1 } else { 1 };
                }
                let count = if bar.matches.len() >= MAX_MATCHES {
                    format!("{MAX_MATCHES}+")
                } else {
                    bar.matches.len().to_string()
                };
                let label = if bar.matches.is_empty() {
                    if bar.query.is_empty() { String::new() } else { "No matches".into() }
                } else {
                    format!("{} of {count}", bar.current + 1)
                };
                ui.label(RichText::new(label).size(11.5).weak());
                if ui.small_button("‹").clicked() {
                    step = -1;
                }
                if ui.small_button("›").clicked() {
                    step = 1;
                }
                if !bar.matches.is_empty() && (changed || step != 0) {
                    let len = bar.matches.len() as i64;
                    bar.current = ((bar.current as i64 + step).rem_euclid(len)) as usize;
                    let Found { start, end, .. } = bar.matches[bar.current];
                    jump = Some(Jump { start, end });
                }
            }
            BarKind::GoToLine => {
                if edit.has_focus() && enter {
                    let (line, col) = parse_position(&bar.query);
                    let index = char_index(text, line, col);
                    jump = Some(Jump { start: index, end: index });
                    close = true;
                }
            }
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.small_button("×").clicked() {
                close = true;
            }
        });
    });
    (jump, close)
}

/// Where `query` occurs in `text`; all-lowercase queries ignore case.
fn find_all(text: &str, query: &str) -> Vec<Found> {
    if query.is_empty() {
        return Vec::new();
    }
    let sensitive = query.chars().any(char::is_uppercase);
    let fold = |c: char| if sensitive { c } else { c.to_lowercase().next().unwrap_or(c) };
    let needle: Vec<char> = query.chars().map(fold).collect();
    let hay: Vec<char> = text.chars().map(fold).collect();
    let mut matches = Vec::new();
    let (mut i, mut line) = (0, 0);
    while i + needle.len() <= hay.len() && matches.len() < MAX_MATCHES {
        if hay[i..i + needle.len()] == needle[..] {
            matches.push(Found { start: i, end: i + needle.len(), line });
            line += hay[i..i + needle.len()].iter().filter(|c| **c == '\n').count();
            i += needle.len();
        } else {
            line += usize::from(hay[i] == '\n');
            i += 1;
        }
    }
    matches
}

/// `12` or `12:4` as a one-based line and column; anything else is line 1.
fn parse_position(query: &str) -> (usize, usize) {
    let mut parts = query.trim().splitn(2, ':');
    let line = parts.next().and_then(|l| l.trim().parse().ok()).unwrap_or(1);
    let col = parts.next().and_then(|c| c.trim().parse().ok()).unwrap_or(1);
    (line, col)
}

/// Character offset of a one-based line and column, clamped to the text.
pub fn char_index(text: &str, line: usize, col: usize) -> usize {
    let mut index = 0;
    for (i, row) in text.split('\n').enumerate() {
        let chars = row.chars().count();
        if i + 1 == line.max(1) {
            return index + col.max(1).min(chars + 1) - 1;
        }
        index += chars + 1;
    }
    text.chars().count()
}

/// Zero-based line the character at `index` is on.
pub fn line_of(text: &str, index: usize) -> usize {
    text.chars().take(index).filter(|c| *c == '\n').count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_with_smart_case_in_character_offsets() {
        let ranges =
            |text, query| find_all(text, query).iter().map(|f| (f.start, f.end, f.line)).collect::<Vec<_>>();
        assert_eq!(ranges("Ünïcode\nünïcode", "ünï"), [(0, 3, 0), (8, 11, 1)]);
        assert_eq!(ranges("Ünïcode ünïcode", "Ünï"), [(0, 3, 0)]);
        assert_eq!(ranges("aaaa", "aa"), [(0, 2, 0), (2, 4, 0)]);
        assert!(find_all("text", "").is_empty());
    }

    #[test]
    fn parses_line_and_column() {
        assert_eq!(parse_position("12"), (12, 1));
        assert_eq!(parse_position(" 12:4 "), (12, 4));
        assert_eq!(parse_position("x"), (1, 1));
        assert_eq!(char_index("ab\ncd", 2, 2), 4);
        assert_eq!(line_of("ab\ncd", 4), 1);
    }
}
