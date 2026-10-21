use std::time::{Duration, Instant};

use alacritty_terminal::grid::{Dimensions, Indexed};
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::cell::{Cell as TermCell, Flags};
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::term::{Term, TermMode};
use alacritty_terminal::vte::ansi::CursorShape;
use eframe::egui::Color32;

use super::colors;
use super::session::EventProxy;
use crate::theme::Theme;

/// Longest a pane keeps showing its previous frame while the PTY thread holds the terminal.
/// Parsing one synchronized update (up to 2 MiB, e.g. Claude Code replaying a resumed
/// conversation) holds the lock for over 10 ms; waiting on it every frame stalls the whole window.
const MAX_STALE: Duration = Duration::from_millis(50);

/// A visible cell with colors already resolved against palette, theme and selection.
pub struct Cell {
    pub row: i32,
    pub col: usize,
    pub c: char,
    pub zerowidth: Option<Box<[char]>>,
    pub fg: Color32,
    pub bg: Color32,
    pub flags: Flags,
    pub width: usize,
}

pub struct Cursor {
    pub row: i32,
    pub col: usize,
    pub shape: CursorShape,
    /// Glyph, background and weight under the cursor, for redrawing it inverted.
    pub under: Option<(char, Color32, bool)>,
}

/// Everything the view needs, copied out of the terminal so its lock is held only while
/// copying rather than through text layout, and taken once per frame.
pub struct Frame {
    pub cells: Vec<Cell>,
    pub cursor: Cursor,
    pub mode: TermMode,
    /// Lines the view is scrolled up into history.
    pub display_offset: usize,
    pub history_size: usize,
    pub screen_lines: usize,
    pub has_selection: bool,
    /// Nothing but spaces on screen, e.g. while a program is still starting.
    pub blank: bool,
    captured_at: Option<Instant>,
}

impl Default for Frame {
    fn default() -> Self {
        let cursor = Cursor { row: 0, col: 0, shape: CursorShape::Hidden, under: None };
        Self {
            cells: Vec::new(),
            cursor,
            mode: TermMode::empty(),
            display_offset: 0,
            history_size: 0,
            screen_lines: 0,
            has_selection: false,
            blank: true,
            captured_at: None,
        }
    }
}

impl Frame {
    /// Recaptures the frame unless the PTY thread is mid-parse, in which case the previous frame
    /// is kept for up to `MAX_STALE`. Returns false when the frame is stale.
    pub fn refresh(&mut self, term: &FairMutex<Term<EventProxy>>, theme: &Theme) -> bool {
        let term = match term.try_lock_unfair() {
            Some(term) => term,
            None if self.captured_at.is_some_and(|at| at.elapsed() < MAX_STALE) => return false,
            None => term.lock(),
        };
        self.capture_into(&term, theme);
        self.captured_at = Some(Instant::now());
        true
    }

    /// Refills the frame from the terminal, reusing the cell buffer across frames.
    fn capture_into(&mut self, term: &Term<EventProxy>, theme: &Theme) {
        let content = term.renderable_content();
        let offset = content.display_offset as i32;
        let cursor_point = content.cursor.point;
        let (selection, palette) = (content.selection, content.colors);
        let cells = &mut self.cells;
        cells.clear();
        let mut under = None;
        let mut blank = true;

        let above = edge_row(term, -offset - 1);
        let below = edge_row(term, term.screen_lines() as i32 - offset);
        for indexed in above.chain(content.display_iter).chain(below) {
            let cell = indexed.cell;
            if cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                continue;
            }
            blank &= cell.c == ' ';
            let selected = selection.as_ref().is_some_and(|s| s.contains(indexed.point));
            let (fg, bg) = cell_colors(cell, palette, theme, selected);
            if indexed.point == cursor_point {
                under = Some((cell.c, bg, cell.flags.contains(Flags::BOLD)));
            }
            cells.push(Cell {
                row: indexed.point.line.0 + offset,
                col: indexed.point.column.0,
                c: cell.c,
                zerowidth: cell.zerowidth().map(|z| z.to_vec().into_boxed_slice()),
                fg,
                bg,
                flags: cell.flags,
                width: if cell.flags.contains(Flags::WIDE_CHAR) { 2 } else { 1 },
            });
        }
        self.cursor = Cursor {
            row: cursor_point.line.0 + offset,
            col: cursor_point.column.0,
            shape: content.cursor.shape,
            under,
        };
        self.mode = content.mode;
        self.display_offset = offset as usize;
        self.history_size = term.grid().history_size();
        self.screen_lines = term.screen_lines();
        self.has_selection = term.selection.is_some();
        self.blank = blank;
    }
}

fn cell_colors(cell: &TermCell, palette: &Colors, theme: &Theme, selected: bool) -> (Color32, Color32) {
    let mut fg = colors::resolve(cell.fg, palette, theme);
    let mut bg = colors::resolve(cell.bg, palette, theme);
    if cell.flags.contains(Flags::INVERSE) {
        std::mem::swap(&mut fg, &mut bg);
    }
    if cell.flags.contains(Flags::DIM) {
        fg = fg.gamma_multiply(0.66);
    }
    if cell.flags.contains(Flags::HIDDEN) {
        fg = bg;
    }
    if selected {
        bg = theme.selection.color();
    }
    (fg, bg)
}

/// The grid row just outside the viewport, which shows partially while scrolling between lines.
fn edge_row(term: &Term<EventProxy>, line: i32) -> impl Iterator<Item = Indexed<&TermCell>> {
    let grid = term.grid();
    let exists = line >= grid.topmost_line().0 && line < grid.screen_lines() as i32;
    exists
        .then(|| grid[Line(line)].into_iter().enumerate())
        .into_iter()
        .flatten()
        .map(move |(col, cell)| Indexed { point: Point::new(Line(line), Column(col)), cell })
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
