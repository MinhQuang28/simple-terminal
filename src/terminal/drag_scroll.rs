//! Dragging a selection past the top or bottom edge of a pane scrolls the scrollback, so a
//! selection can reach beyond what is on screen.

use alacritty_terminal::term::TermMode;
use eframe::egui::{Response, Ui};
use std::time::Duration;

use super::render::Grid;
use super::scroll::ScrollState;
use super::session::TerminalSession;

/// How often a held drag scrolls another step.
const STEP: Duration = Duration::from_millis(50);

/// When the last step ran, in egui time.
#[derive(Default)]
pub struct DragScroll {
    last_step: f64,
}

impl DragScroll {
    /// Scrolls one step toward the pointer while a selection drag is outside the pane's rows: one
    /// line plus one per row-height the pointer is past the edge, so farther goes faster.
    pub fn step(
        &mut self,
        ui: &Ui,
        response: &Response,
        session: &TerminalSession,
        scroll: &mut ScrollState,
        grid: Grid,
    ) {
        if !response.dragged() || session.frame.mode.contains(TermMode::ALT_SCREEN) {
            return;
        }
        let Some(pos) = ui.input(|i| i.pointer.latest_pos()) else { return };
        let rect = response.rect;
        let past = if pos.y < rect.top() {
            rect.top() - pos.y
        } else if pos.y > rect.bottom() {
            -(pos.y - rect.bottom())
        } else {
            return;
        };
        ui.ctx().request_repaint_after(STEP);
        let now = ui.input(|i| i.time);
        if now - self.last_step < STEP.as_secs_f64() {
            return;
        }
        self.last_step = now;
        let lines = 1.0 + (past.abs() / grid.cell.y).floor();
        let band = session.frame.band_rows;
        let mut term = session.term.lock();
        scroll.clear_pixel_offset();
        scroll.scroll_lines(&mut term, lines.copysign(past), band);
        // This frame was captured before the scroll; show it now rather than a step late.
        ui.ctx().request_repaint();
    }
}
