use std::time::Instant;

use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::term::{Term, TermMode};
use eframe::egui::{
    Align2, Color32, CornerRadius, Event, FontId, Id, MouseWheelUnit, Pos2, Rect, Sense, Stroke, StrokeKind,
    Ui, pos2, vec2,
};

use super::render::Grid;
use super::session::{EventProxy, TerminalSession};
use crate::ui::widgets::accent;

const SCROLLBAR_WIDTH: f32 = 6.0;
const SCROLLBAR_MARGIN: f32 = 3.0;
const MIN_THUMB: f32 = 24.0;

/// Arrow keys sent for alt-screen scrolling come from a token bucket: a wheel notch's lines go out
/// at once, but a trackpad flick is paced. Claude Code takes eight same-direction arrows within
/// 100 ms as a scroll wheel misconfigured to send keys and shows a warning, so a burst plus the
/// refill over 100 ms stays below that.
const ARROW_BURST: f32 = 3.0;
const ARROW_REFILL_PER_SEC: f32 = 40.0;

/// Fractional scroll position between two grid lines, so trackpad scrolling moves pixel by pixel
/// while the terminal itself only knows whole lines. Positive means scrolled up into history.
#[derive(Clone, Copy, Default)]
pub struct ScrollState {
    pub remainder: f32,
    /// Fractional lines of wheel movement not yet forwarded to the program as a whole event.
    carry: f32,
    arrow_tokens: f32,
    arrows_refilled: Option<Instant>,
}

impl ScrollState {
    /// Takes as many of `lines` arrow presses as the bucket allows right now; the rest are dropped.
    fn take_arrows(&mut self, lines: i32, now: Instant) -> i32 {
        let elapsed = self.arrows_refilled.map_or(f32::MAX, |t| now.duration_since(t).as_secs_f32());
        self.arrow_tokens = (self.arrow_tokens + elapsed * ARROW_REFILL_PER_SEC).min(ARROW_BURST);
        self.arrows_refilled = Some(now);
        let allowed = lines.unsigned_abs().min(self.arrow_tokens as u32) as i32;
        self.arrow_tokens -= allowed as f32;
        allowed * lines.signum()
    }

    /// Pixel shift of the grid for the sub-line part of the scroll position.
    pub fn pixel_offset(&self, line_height: f32) -> f32 {
        self.remainder * line_height
    }

    pub fn snap_to_bottom(&mut self, term: &mut Term<EventProxy>) {
        term.scroll_display(Scroll::Bottom);
        self.remainder = 0.0;
    }

    /// Drops any sub-line offset; alt-screen programs draw whole rows and must not be shifted.
    pub fn clear_pixel_offset(&mut self) {
        self.remainder = 0.0;
    }

    /// Scrolls the view by fractional lines, applying whole lines to the terminal and keeping the
    /// rest, clamped so the view never shows past the first or last line.
    pub fn scroll_lines(&mut self, term: &mut Term<EventProxy>, delta: f32) {
        self.remainder += delta;
        let lines = self.remainder.trunc() as i32;
        if lines != 0 {
            term.scroll_display(Scroll::Delta(lines));
            self.remainder -= lines as f32;
        }
        let offset = term.grid().display_offset();
        let at_bottom = offset == 0;
        let at_top = offset >= term.grid().history_size();
        if (at_bottom && self.remainder < 0.0) || (at_top && self.remainder > 0.0) {
            self.remainder = 0.0;
        }
    }
}

/// Routes mouse-wheel input: mouse reports for programs that asked for them, arrow keys for
/// full-screen programs, scrollback otherwise.
pub fn handle_wheel(
    ui: &Ui,
    session: &mut TerminalSession,
    state: &mut ScrollState,
    grid: Grid,
    pointer: Pos2,
) {
    let (mode, rows) = (session.frame.mode, session.frame.screen_lines);
    let forwarded = mode.intersects(TermMode::MOUSE_MODE)
        || mode.contains(TermMode::ALT_SCREEN | TermMode::ALTERNATE_SCROLL);
    if !forwarded {
        let delta = ui.input(|i| i.smooth_scroll_delta.y);
        if delta != 0.0 {
            state.scroll_lines(&mut session.term.lock(), delta / grid.cell.y);
        }
        return;
    }

    // Programs pace their own wheel acceleration from event timing, so they get each wheel event
    // as the OS delivered it rather than egui's smoothed per-frame delta (which would turn one
    // mouse notch into a run of small events).
    let events: Vec<f32> = ui.input(|i| {
        i.events
            .iter()
            .filter_map(|event| match event {
                Event::MouseWheel { unit, delta, .. } => Some(match unit {
                    MouseWheelUnit::Point => delta.y / grid.cell.y,
                    MouseWheelUnit::Line => delta.y,
                    MouseWheelUnit::Page => delta.y * rows as f32,
                }),
                _ => None,
            })
            .collect()
    });
    let rel = pointer - grid.origin;
    let col = (rel.x / grid.cell.x).max(0.0) as usize + 1;
    let row = (rel.y / grid.cell.y).max(0.0) as usize + 1;
    for delta_lines in events {
        let lines = whole_lines(state, delta_lines);
        if lines == 0 {
            continue;
        }
        if mode.intersects(TermMode::MOUSE_MODE) {
            session.write(wheel_report(mode, lines, col, row));
        } else {
            // Full-screen programs (less, vim) without mouse support expect arrow keys.
            let lines = state.take_arrows(lines, Instant::now());
            let arrow: &[u8] = if lines > 0 { b"\x1bOA" } else { b"\x1bOB" };
            if lines != 0 {
                session.write(arrow.repeat(lines.unsigned_abs() as usize));
            }
        }
    }
}

/// Accumulates fractional lines and returns the whole lines to forward to the program.
fn whole_lines(state: &mut ScrollState, delta_lines: f32) -> i32 {
    state.carry += delta_lines;
    let lines = state.carry.trunc() as i32;
    state.carry -= lines as f32;
    lines
}

/// Wheel events as the mouse reports a program asked for, one per line, at 1-based `col`/`row`.
fn wheel_report(mode: TermMode, lines: i32, col: usize, row: usize) -> String {
    let button = if lines > 0 { 64 } else { 65 };
    let report = if mode.contains(TermMode::SGR_MOUSE) {
        format!("\x1b[<{button};{col};{row}M")
    } else {
        // Legacy X10 encoding; only cells within its 223-column limit are reportable.
        let encode = |v: usize| char::from_u32((v.min(223) + 32) as u32).unwrap_or(' ');
        format!("\x1b[M{}{}{}", encode(button), encode(col), encode(row))
    };
    report.repeat(lines.unsigned_abs() as usize)
}

/// Scrollback geometry in lines.
#[derive(Clone, Copy)]
struct Extent {
    offset: usize,
    history: usize,
    rows: usize,
}

/// Draws the scrollbar when there is history, draggable along the right edge, and a "jump to
/// bottom" pill when the view is scrolled up. Returns whether the pointer is over either.
pub fn overlay(
    ui: &mut Ui,
    session: &mut TerminalSession,
    state: &mut ScrollState,
    rect: Rect,
    id: Id,
) -> bool {
    let frame = &session.frame;
    let extent =
        Extent { offset: frame.display_offset, history: frame.history_size, rows: frame.screen_lines };
    if extent.history == 0 {
        return false;
    }
    let mut over = scrollbar(ui, session, state, rect, id, extent);
    if extent.offset > 0 {
        over |= jump_to_bottom(ui, session, state, rect, id);
    }
    over
}

fn scrollbar(
    ui: &mut Ui,
    session: &TerminalSession,
    state: &mut ScrollState,
    rect: Rect,
    id: Id,
    extent: Extent,
) -> bool {
    let Extent { offset, history, rows } = extent;
    let track = Rect::from_min_max(
        pos2(rect.right() - SCROLLBAR_MARGIN - SCROLLBAR_WIDTH, rect.top() + SCROLLBAR_MARGIN),
        pos2(rect.right() - SCROLLBAR_MARGIN, rect.bottom() - SCROLLBAR_MARGIN),
    );
    let response = ui.interact(track, id.with("scrollbar"), Sense::click_and_drag());
    let total = (history + rows) as f32;
    let thumb_len = (track.height() * rows as f32 / total).max(MIN_THUMB);
    let travel = track.height() - thumb_len;
    let top = track.top() + travel * (history - offset) as f32 / history as f32;
    let thumb = Rect::from_min_size(pos2(track.left(), top), vec2(SCROLLBAR_WIDTH, thumb_len));

    if response.dragged() && travel > 0.0 {
        // One pixel of thumb travel covers this many lines of history.
        let lines_per_px = history as f32 / travel;
        state.scroll_lines(&mut session.term.lock(), -response.drag_delta().y * lines_per_px);
    }
    let pointer_near = ui.rect_contains_pointer(rect.with_min_x(track.left() - 24.0));
    let shown = pointer_near || response.dragged() || offset > 0;
    if shown {
        let alpha = if response.hovered() || response.dragged() { 160 } else { 90 };
        let color = ui.visuals().text_color().gamma_multiply(alpha as f32 / 255.0);
        ui.painter().rect_filled(thumb, CornerRadius::same(3), color);
    }
    response.hovered() || response.dragged()
}

fn jump_to_bottom(
    ui: &mut Ui,
    session: &TerminalSession,
    state: &mut ScrollState,
    rect: Rect,
    id: Id,
) -> bool {
    let label = "↓ Jump to bottom";
    let font = FontId::proportional(12.0);
    let galley = ui.painter().layout_no_wrap(label.into(), font, Color32::WHITE);
    let size = galley.size() + vec2(20.0, 10.0);
    let pill = Rect::from_min_size(pos2(rect.right() - size.x - 18.0, rect.bottom() - size.y - 12.0), size);
    let response = ui.interact(pill, id.with("jump-to-bottom"), Sense::click());
    let fill = if response.hovered() { accent(ui) } else { accent(ui).gamma_multiply(0.85) };
    ui.painter().rect(pill, CornerRadius::same(99), fill, Stroke::NONE, StrokeKind::Inside);
    ui.painter().text(
        pill.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(12.0),
        Color32::WHITE,
    );
    if response.clicked() {
        state.snap_to_bottom(&mut session.term.lock());
    }
    response.hovered()
}

#[cfg(test)]
#[path = "scroll_tests.rs"]
mod tests;
