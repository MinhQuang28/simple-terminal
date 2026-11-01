//! A read-only look at a pane drawn somewhere other than its own place in the layout: the
//! peek overlay. The pane keeps its size, so the program inside doesn't reflow; what doesn't fit
//! is cut off at the right and the top, keeping the bottom rows (an agent's latest output and
//! prompt) in view.

use eframe::egui::{Rect, Ui, pos2};

use super::render::{self, CursorState, Grid};
use super::session::{Presence, TerminalSession};
use super::style::TerminalStyle;
use super::view::anchored;

/// How soon to look again while the terminal is busy being written.
const REFRESH_RETRY: std::time::Duration = std::time::Duration::from_millis(8);

pub fn show(ui: &mut Ui, session: &mut TerminalSession, style: &TerminalStyle<'_>, rect: Rect) {
    session.set_presence(Presence::Visible);
    session.sync_theme(style.theme);
    if let Some(text) = session.process_events() {
        ui.ctx().copy_text(text);
    }
    if !session.frame.refresh(&session.term, style.theme, anchored(session, style)) {
        ui.ctx().request_repaint_after(REFRESH_RETRY);
    }
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, style.theme.background.color());
    let (cell, text_offset) = render::metrics(&painter, style);
    let frame = &session.frame;
    // Rows below `screen_lines - band` are the screen's empty bottom, pushed off as the pane does.
    let shown_rows = frame.screen_lines.saturating_sub(frame.band_rows) as f32;
    let origin = pos2(rect.left() + style.padding, rect.bottom() - style.padding - shown_rows * cell.y);
    let grid = Grid { origin, cell, text_offset };
    let cursor = CursorState { focused: false, visible: true };
    let mut paint = std::mem::take(&mut session.view.paint);
    render::paint(&painter, grid, style, frame, &mut paint, cursor, None);
    session.view.paint = paint;
}
