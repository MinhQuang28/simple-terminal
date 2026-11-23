use eframe::egui::{Align, Key, Layout, Modifiers, TextEdit, Ui};
use regex::Regex;

use crate::find_box::{self, FindOptions, pattern};

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
    options: FindOptions,
    current: usize,
    matches: Vec<Found>,
    /// The query, its switches and the text length `matches` were found for.
    searched: Option<(String, FindOptions, usize)>,
    opened: bool,
}

impl EditorBar {
    pub fn new(kind: BarKind) -> Self {
        Self {
            kind,
            query: String::new(),
            options: FindOptions::default(),
            current: 0,
            matches: Vec::new(),
            searched: None,
            opened: true,
        }
    }

    /// Matches in text order, with the current one's index.
    pub fn matches(&self) -> (&[Found], usize) {
        (&self.matches, self.current)
    }

    /// Searches again when the query or the text changed; returns true when it did.
    fn search(&mut self, text: &str) -> bool {
        let key = (self.query.clone(), self.options, text.len());
        if self.searched.as_ref() == Some(&key) {
            return false;
        }
        self.matches = find_all(text, &self.query, self.options);
        self.current = 0;
        self.searched = Some(key);
        true
    }
}

/// Shows the bar; returns where the editor should jump, and whether the bar closed.
pub fn show(ui: &mut Ui, bar: &mut EditorBar, text: &str) -> (Option<Jump>, bool) {
    let mut jump = None;
    let mut close = false;
    ui.horizontal(|ui| match bar.kind {
        BarKind::Find => {
            let changed = bar.search(text);
            let status = match bar.matches.len() {
                _ if bar.query.is_empty() => String::new(),
                0 => "No results".into(),
                MAX_MATCHES.. => format!("{} of {MAX_MATCHES}+", bar.current + 1),
                count => format!("{} of {count}", bar.current + 1),
            };
            let found = find_box::show(ui, &mut bar.query, &mut bar.options, &status);
            if std::mem::take(&mut bar.opened) {
                found.edit.request_focus();
            }
            if found.changed {
                // The count shown was for the query before this edit.
                ui.ctx().request_repaint();
            }
            close = found.close;
            if !bar.matches.is_empty() && (changed || found.step != 0) {
                let len = bar.matches.len() as i64;
                bar.current = ((bar.current as i64 + i64::from(found.step)).rem_euclid(len)) as usize;
                let Found { start, end, .. } = bar.matches[bar.current];
                jump = Some(Jump { start, end });
            }
        }
        BarKind::GoToLine => {
            ui.spacing_mut().item_spacing.x = 6.0;
            let edit =
                ui.add(TextEdit::singleline(&mut bar.query).hint_text("Line[:column]").desired_width(220.0));
            if std::mem::take(&mut bar.opened) {
                edit.request_focus();
            }
            let (enter, escape) =
                ui.input_mut(|i| (i.consume_key(Modifiers::NONE, Key::Enter), i.key_pressed(Key::Escape)));
            close = escape && edit.has_focus();
            if edit.has_focus() && enter {
                let (line, col) = parse_position(&bar.query);
                let index = char_index(text, line, col);
                jump = Some(Jump { start: index, end: index });
                close = true;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                close |= ui.small_button("×").clicked();
            });
        }
    });
    (jump, close)
}

/// Where `query` matches in `text` under `options`, as character offsets.
fn find_all(text: &str, query: &str, options: FindOptions) -> Vec<Found> {
    let Some(regex) =
        Some(query).filter(|q| !q.is_empty()).and_then(|q| Regex::new(&pattern(q, options)).ok())
    else {
        return Vec::new();
    };
    let newlines = |part: &str| part.bytes().filter(|b| *b == b'\n').count();
    let mut matches = Vec::new();
    // The regex reports bytes; the editor counts characters, so both advance together.
    let (mut byte, mut chars, mut line) = (0, 0, 0);
    for found in regex.find_iter(text).filter(|m| !m.is_empty()).take(MAX_MATCHES) {
        let before = &text[byte..found.start()];
        chars += before.chars().count();
        line += newlines(before);
        let len = found.as_str().chars().count();
        matches.push(Found { start: chars, end: chars + len, line });
        chars += len;
        line += newlines(found.as_str());
        byte = found.end();
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
    fn finds_in_character_offsets_under_each_switch() {
        let ranges = |text, query, options| {
            find_all(text, query, options).iter().map(|f| (f.start, f.end, f.line)).collect::<Vec<_>>()
        };
        let plain = FindOptions::default();
        assert_eq!(ranges("Ünïcode\nünïcode", "ünï", plain), [(0, 3, 0), (8, 11, 1)]);
        let case = FindOptions { case: true, ..plain };
        assert_eq!(ranges("Ünïcode ünïcode", "Ünï", case), [(0, 3, 0)]);
        assert_eq!(ranges("aaaa", "aa", plain), [(0, 2, 0), (2, 4, 0)]);
        assert_eq!(ranges("a.b axb", "a.b", plain), [(0, 3, 0)], "a dot is a dot unless regex is on");
        let regex = FindOptions { regex: true, ..plain };
        assert_eq!(ranges("a.b axb", "a.b", regex), [(0, 3, 0), (4, 7, 0)]);
        let word = FindOptions { word: true, ..plain };
        assert_eq!(ranges("cat concat", "cat", word), [(0, 3, 0)]);
        assert!(find_all("text", "", plain).is_empty());
        assert!(find_all("text", "(", regex).is_empty(), "an unfinished regex finds nothing");
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
