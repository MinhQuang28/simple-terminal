use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::Line;
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::{Term, TermMode};
use alacritty_terminal::vte::ansi::{Color, NamedColor};

use super::session::EventProxy;

/// Rows of the screen, from its top through the last one with anything on it or the visible
/// cursor. Measured on the screen itself rather than the scrolled viewport, so scrolling into
/// history doesn't change it.
pub fn content_rows(term: &Term<EventProxy>) -> usize {
    let grid = term.grid();
    let lines = term.screen_lines() as i32;
    let last_used = (0..lines).rev().find(|&l| grid[Line(l)].into_iter().any(is_visible)).unwrap_or(-1);
    // A hidden cursor paints nothing, so where a program parks it doesn't extend the content.
    let cursor = if term.mode().contains(TermMode::SHOW_CURSOR) { grid.cursor.point.line.0 } else { -1 };
    (last_used.max(cursor) + 1) as usize
}

/// Whether the cell shows anything: a glyph, a decoration (drawn even on spaces), or a background
/// that differs from the pane's.
fn is_visible(cell: &Cell) -> bool {
    cell.c != ' '
        || cell.zerowidth().is_some()
        || cell.flags.intersects(Flags::INVERSE | Flags::ALL_UNDERLINES | Flags::STRIKEOUT)
        || cell.bg != Color::Named(NamedColor::Background)
}

#[cfg(test)]
#[path = "screen_content_tests.rs"]
mod tests;
