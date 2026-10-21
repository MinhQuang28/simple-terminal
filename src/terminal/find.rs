use std::sync::{Arc, Mutex};

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Direction, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::Term;
use alacritty_terminal::term::search::{Match, RegexSearch};
use eframe::egui::{
    Align, Frame, Key, Layout, Margin, Modifiers, Rect, Response, RichText, TextEdit, Ui, pos2, vec2,
};

use super::render::Grid;
use super::session::{EventProxy, TerminalSession};
use super::style::TerminalStyle;

const MAX_COUNTED: usize = 999;
const BAR_WIDTH: f32 = 320.0;

/// In-pane search; the current match becomes the terminal selection so ⌘C copies it.
#[derive(Clone, Default)]
pub struct FindState {
    pub open: bool,
    query: String,
    compiled: Option<Arc<Mutex<(String, RegexSearch)>>>,
    current: Option<Match>,
    total: usize,
    /// Set by a keystroke, cleared once the matches were counted for it.
    unsearched: bool,
}

impl FindState {
    /// Opens the bar (or re-focuses it) when ⌘F is pressed with the pane focused.
    pub fn shortcut(&mut self, ui: &Ui, pane_focused: bool) {
        if pane_focused && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::F)) {
            self.open = true;
        }
    }

    fn regex(&mut self) -> Option<Arc<Mutex<(String, RegexSearch)>>> {
        let pattern = pattern_for(&self.query);
        let fresh = self.compiled.as_ref().is_some_and(|c| c.lock().map(|c| c.0 == pattern).unwrap_or(false));
        if !fresh {
            let regex = RegexSearch::new(&pattern).ok()?;
            self.compiled = Some(Arc::new(Mutex::new((pattern, regex))));
        }
        self.compiled.clone()
    }
}

/// Smart case: all-lowercase queries match case-insensitively. Regex syntax is passed through.
fn pattern_for(query: &str) -> String {
    if query.chars().any(char::is_uppercase) { query.to_string() } else { format!("(?i){query}") }
}

/// Draws the find bar and match highlights over `rect`; handles Enter/⇧Enter/Esc.
pub fn overlay(
    ui: &mut Ui,
    session: &mut TerminalSession,
    state: &mut FindState,
    rect: Rect,
    grid: Grid,
    pane: &Response,
    style: &TerminalStyle<'_>,
) {
    let bar =
        Rect::from_min_size(pos2(rect.right() - BAR_WIDTH - 10.0, rect.top() + 6.0), vec2(BAR_WIDTH, 30.0));
    let mut step = None;
    let mut changed = false;
    let mut close = false;
    ui.scope_builder(
        eframe::egui::UiBuilder::new().max_rect(bar).layout(Layout::left_to_right(Align::Center)),
        |ui| {
            Frame::popup(ui.style()).inner_margin(Margin::symmetric(8, 4)).show(ui, |ui| {
                let edit = ui.add(
                    TextEdit::singleline(&mut state.query).hint_text("Find (regex)").desired_width(170.0),
                );
                // Count once per edit, deferred until the box has a query; this used to re-count
                // every frame whenever the box lost focus with no match.
                state.unsearched |= edit.changed();
                if state.unsearched && !state.query.is_empty() {
                    changed = true;
                    state.unsearched = false;
                }
                if ui.memory(|m| m.focused().is_none()) {
                    edit.request_focus();
                }
                let (shift, enter, escape) =
                    ui.input(|i| (i.modifiers.shift, i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
                if edit.has_focus() && enter {
                    step = Some(if shift { Direction::Left } else { Direction::Right });
                }
                close = escape;
                let count = if state.total >= MAX_COUNTED {
                    format!("{MAX_COUNTED}+")
                } else {
                    state.total.to_string()
                };
                ui.label(RichText::new(count).size(11.5).weak());
                if ui.small_button("‹").clicked() {
                    step = Some(Direction::Left);
                }
                if ui.small_button("›").clicked() {
                    step = Some(Direction::Right);
                }
                if ui.small_button("×").clicked() {
                    close = true;
                }
            });
        },
    );
    if close {
        state.open = false;
        state.current = None;
        session.term.lock().selection = None;
        pane.request_focus();
        return;
    }
    let Some(regex) = state.regex() else { return };
    let Ok(mut regex) = regex.lock() else { return };
    let mut term = session.term.lock();
    if changed {
        state.current = None;
        state.total = count_matches(&term, &mut regex.1);
        step = Some(Direction::Right);
    }
    if let Some(direction) = step.filter(|_| !state.query.is_empty()) {
        state.current = next_match(&term, &mut regex.1, state.current.as_ref(), direction);
        if let Some(m) = &state.current {
            term.scroll_to_point(*m.start());
            let mut selection = Selection::new(SelectionType::Simple, *m.start(), Side::Left);
            selection.update(*m.end(), Side::Right);
            term.selection = Some(selection);
        }
    }
    if !state.query.is_empty() {
        let color = style.theme.accent().gamma_multiply(0.35);
        for m in visible_matches(&term, &mut regex.1) {
            if state.current.as_ref() == Some(&m) {
                continue;
            }
            for (row, cols) in rows_of(&term, &m) {
                ui.painter().rect_filled(grid.cell_rect(row, cols.0, cols.1 - cols.0), 2.0, color);
            }
        }
    }
}

fn next_match(
    term: &Term<EventProxy>,
    regex: &mut RegexSearch,
    current: Option<&Match>,
    direction: Direction,
) -> Option<Match> {
    let (origin, side) = match (current, direction) {
        (Some(m), Direction::Right) => (*m.end(), Side::Right),
        (Some(m), Direction::Left) => (*m.start(), Side::Left),
        (None, _) => (Point::new(Line(-(term.grid().display_offset() as i32)), Column(0)), Side::Left),
    };
    term.search_next(regex, origin, direction, side, None)
}

fn count_matches(term: &Term<EventProxy>, regex: &mut RegexSearch) -> usize {
    let mut origin = Point::new(term.topmost_line(), Column(0));
    let mut count = 0;
    while count < MAX_COUNTED {
        let Some(m) = term.search_next(regex, origin, Direction::Right, Side::Left, None) else { break };
        if count > 0 && *m.start() <= origin {
            break;
        }
        count += 1;
        origin = *m.end();
        if origin.column.0 + 1 < term.columns() {
            origin.column.0 += 1;
        } else if origin.line < term.bottommost_line() {
            origin = Point::new(origin.line + 1, Column(0));
        } else {
            break;
        }
    }
    count
}

fn visible_matches(term: &Term<EventProxy>, regex: &mut RegexSearch) -> Vec<Match> {
    let offset = term.grid().display_offset() as i32;
    let top = Line(-offset);
    let bottom = Line(term.screen_lines() as i32 - 1 - offset);
    let mut origin = Point::new(top, Column(0));
    let mut matches = Vec::new();
    while matches.len() < 500 {
        let Some(m) =
            term.search_next(regex, origin, Direction::Right, Side::Left, Some(term.screen_lines()))
        else {
            break;
        };
        if m.start().line > bottom || (!matches.is_empty() && *m.start() <= origin) {
            break;
        }
        origin = *m.end();
        if origin.column.0 + 1 < term.columns() {
            origin.column.0 += 1;
        } else {
            origin = Point::new(origin.line + 1, Column(0));
        }
        matches.push(m);
    }
    matches
}

/// Viewport rows and column ranges a match covers.
fn rows_of(term: &Term<EventProxy>, m: &Match) -> Vec<(i32, (usize, usize))> {
    let offset = term.grid().display_offset() as i32;
    let (start, end) = (m.start(), m.end());
    (start.line.0..=end.line.0)
        .map(|line| {
            let from = if line == start.line.0 { start.column.0 } else { 0 };
            let to = if line == end.line.0 { end.column.0 + 1 } else { term.columns() };
            (line + offset, (from, to))
        })
        .collect()
}
