use std::time::{Duration, Instant};

use alacritty_terminal::grid::{Dimensions, Indexed, Scroll};
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::selection::SelectionRange;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::cell::{Cell as TermCell, Flags};
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::term::{Term, TermDamage, TermMode};
use alacritty_terminal::vte::ansi::{Color, CursorShape, CursorStyle};
use eframe::egui::Color32;

use super::colors;
use super::screen_content::content_rows;
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
    /// History rows carried above the viewport, as negative rows, so an anchored pane shows them
    /// where the screen's empty last rows would otherwise leave a gap; 0 when not anchored.
    pub band_rows: usize,
    /// Whether the last capture was anchored to the pane's bottom edge.
    anchored: bool,
    /// Counts captures, so whatever is built from the cells knows when it is out of date.
    pub generation: u64,
    /// Changed by something the terminal's own damage tracking doesn't see (the theme).
    dirty: bool,
    /// What the last capture saw of the state alacritty doesn't track as damage.
    seen: Seen,
    captured_at: Option<Instant>,
}

/// Terminal state a capture depends on that alacritty doesn't count as damage, compared
/// against the live terminal to tell an unchanged screen from a changed one.
#[derive(Default, PartialEq)]
struct Seen {
    cursor: Point,
    cursor_style: Option<CursorStyle>,
    /// The cell under the cursor, which alacritty reports damaged on every call.
    under: Option<(char, Color, Color, Flags)>,
    mode: Option<TermMode>,
    selection: Option<SelectionRange>,
}

impl Seen {
    fn of(term: &Term<EventProxy>) -> Self {
        let cursor = term.grid().cursor.point;
        let cell = &term.grid()[cursor];
        Self {
            cursor,
            cursor_style: Some(term.cursor_style()),
            under: Some((cell.c, cell.fg, cell.bg, cell.flags)),
            mode: Some(*term.mode()),
            selection: term.selection.as_ref().and_then(|s| s.to_range(term)),
        }
    }
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
            band_rows: 0,
            anchored: false,
            generation: 0,
            dirty: true,
            seen: Seen::default(),
            captured_at: None,
        }
    }
}

impl Frame {
    /// Recaptures the frame unless the PTY thread is mid-parse, in which case the previous frame
    /// is kept for up to `MAX_STALE`. Returns false when the frame is stale.
    /// With `anchored`, history fills in above the viewport for every empty row at the bottom of
    /// the screen, so drawn that much lower the content sits against the pane's bottom edge.
    pub fn refresh(&mut self, term: &FairMutex<Term<EventProxy>>, theme: &Theme, anchored: bool) -> bool {
        let mut term = match term.try_lock_unfair() {
            Some(term) => term,
            None if self.captured_at.is_some_and(|at| at.elapsed() < MAX_STALE) => return false,
            None => term.lock(),
        };
        // Full-screen programs lay the whole screen out themselves; decided from the live mode so
        // the frame that switches screens is already captured the right way.
        let anchored = anchored && !term.mode().contains(TermMode::ALT_SCREEN);
        let seen = Seen::of(&term);
        // Unchanged screens keep their cells; the band only moves with content or size, which
        // alacritty reports as damage.
        let changed = self.dirty || anchored != self.anchored || seen != self.seen || damaged(&mut term);
        if changed {
            let band = if anchored { self.settle_band(&mut term) } else { 0 };
            self.capture_into(&term, theme, band);
            self.generation = self.generation.wrapping_add(1);
            self.seen = seen;
            self.dirty = false;
            self.anchored = anchored;
        }
        term.reset_damage();
        self.captured_at = Some(Instant::now());
        true
    }

    /// Recaptures next time even if the terminal reports no change, e.g. after a theme change.
    pub fn invalidate(&mut self) {
        self.dirty = true;
    }

    /// The band for this capture, with the view moved so it reads the same: while scrolled into
    /// history, output filling the screen's empty rows (or a program freeing them) would shrink
    /// or grow the band and move every line on view with it, so the view scrolls by the
    /// difference, as the terminal itself does for output that scrolls the screen. A resize
    /// changes the band for its own reasons and is left alone. Either way the view stops where
    /// the band reaches the first line of history, which a pane anchored while scrolled up (the
    /// setting switched on, a session starting) would otherwise be past.
    fn settle_band(&self, term: &mut Term<EventProxy>) -> usize {
        let band = term.screen_lines().saturating_sub(content_rows(term).max(1));
        let offset = term.grid().display_offset() as i32;
        let still = self.anchored && offset != 0 && term.screen_lines() == self.screen_lines;
        let delta = if still { self.band_rows as i32 - band as i32 } else { 0 };
        let top = term.grid().history_size().saturating_sub(band) as i32;
        let target = (offset + delta).clamp(0, top);
        if target != offset {
            term.scroll_display(Scroll::Delta(target - offset));
        }
        band
    }

    /// Refills the frame from the terminal, reusing the cell buffer across frames, with `band`
    /// rows of history above the viewport.
    fn capture_into(&mut self, term: &Term<EventProxy>, theme: &Theme, band: usize) {
        let content = term.renderable_content();
        let offset = content.display_offset as i32;
        let cursor_point = content.cursor.point;
        let (selection, palette) = (content.selection, content.colors);
        let cells = &mut self.cells;
        cells.clear();
        let mut under = None;
        let mut blank = true;
        let screen_lines = term.screen_lines() as i32;

        // The band, plus the row above it that shows partially while scrolling between lines.
        let above = (-offset - 1 - band as i32..=-offset - 1).flat_map(|line| edge_row(term, line));
        let below = edge_row(term, screen_lines - offset);
        for indexed in above.chain(content.display_iter).chain(below) {
            let cell = indexed.cell;
            if cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                continue;
            }
            blank &= cell.c == ' ';
            let row = indexed.point.line.0 + offset;
            let selected = selection.as_ref().is_some_and(|s| s.contains(indexed.point));
            let (fg, bg) = cell_colors(cell, palette, theme, selected);
            if indexed.point == cursor_point {
                under = Some((cell.c, bg, cell.flags.contains(Flags::BOLD)));
            }
            cells.push(Cell {
                row,
                col: indexed.point.column.0,
                c: cell.c,
                zerowidth: cell.zerowidth().map(|z| z.to_vec().into_boxed_slice()),
                fg,
                bg,
                flags: cell.flags,
                width: if cell.flags.contains(Flags::WIDE_CHAR) { 2 } else { 1 },
            });
        }
        let cursor_row = cursor_point.line.0 + offset;
        self.cursor =
            Cursor { row: cursor_row, col: cursor_point.column.0, shape: content.cursor.shape, under };
        self.mode = content.mode;
        self.display_offset = offset as usize;
        self.history_size = term.grid().history_size();
        self.screen_lines = term.screen_lines();
        self.has_selection = term.selection.is_some();
        self.blank = blank;
        self.band_rows = band;
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

/// Whether the terminal's own damage tracking saw a change since the last capture: output,
/// a resize, scrolling and palette changes all count. The cursor's cell is reported damaged on
/// every call, so it alone doesn't; [`Seen`] covers the cursor.
fn damaged(term: &mut Term<EventProxy>) -> bool {
    let cursor = term.grid().cursor.point;
    let (line, col) = (cursor.line.0 as usize, cursor.column.0);
    match term.damage() {
        TermDamage::Full => true,
        TermDamage::Partial(mut lines) => lines.any(|l| l.line != line || l.left != col || l.right != col),
    }
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
