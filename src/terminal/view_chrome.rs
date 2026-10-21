use std::path::Path;
use std::process::Command;
use std::time::Duration;

use alacritty_terminal::term::TermMode;
use eframe::egui::{self, CursorIcon, Response, Stroke, Ui, vec2};

use super::links::{self, Link};
use super::render::Grid;
use super::session::TerminalSession;
use super::style::TerminalStyle;
use crate::ui::widgets::{muted, spinner_at};

const BLINK_INTERVAL: f64 = 0.53;
const EXIT_BADGE: Duration = Duration::from_secs(4);
/// Longest an agent pane shows its starting indicator; past this a blank screen is the program's.
const STARTING_FOR: Duration = Duration::from_secs(30);

/// With ⌘ held, underlines the link under the pointer; a click opens URLs directly and returns
/// file links for the caller to open.
pub(super) fn hover_link(
    ui: &Ui,
    response: &Response,
    session: &TerminalSession,
    grid: Grid,
    size: (u16, u16),
    cwd: &Path,
) -> Option<Link> {
    let pos = response.hover_pos()?;
    if !ui.input(|i| i.modifiers.command) {
        return None;
    }
    let rel = pos - grid.origin;
    let col = ((rel.x / grid.cell.x).floor() as i64).clamp(0, size.0 as i64 - 1) as usize;
    let row = ((rel.y / grid.cell.y).floor() as i64).clamp(0, size.1 as i64 - 1) as i32;
    let span = links::at(&session.term.lock(), row, col, cwd)?;
    let underline = grid.cell_rect(span.row, span.cols.start, span.cols.len());
    let color = ui.visuals().hyperlink_color;
    ui.painter().hline(underline.x_range(), underline.bottom() - 1.0, Stroke::new(1.0, color));
    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    if !response.clicked() {
        return None;
    }
    match span.link {
        Link::Url(url) => {
            if let Err(err) = Command::new("open").arg(&url).spawn() {
                log::error!("Failed to open {url}: {err}");
            }
            None
        }
        file @ Link::File { .. } => Some(file),
    }
}

/// Shows the exit code of a command that just failed, bottom right, for a few seconds.
pub(super) fn exit_badge(ui: &Ui, session: &TerminalSession, rect: egui::Rect, style: &TerminalStyle<'_>) {
    let Some(finished) = session.last_finished().filter(|f| f.exit_code != 0 && f.at.elapsed() < EXIT_BADGE)
    else {
        return;
    };
    let text = format!("exit {}", finished.exit_code);
    let font = egui::FontId::proportional(11.0);
    let galley = ui.painter().layout_no_wrap(text, font, style.theme.background.color());
    let size = galley.size() + vec2(10.0, 4.0);
    let badge = egui::Rect::from_min_size(rect.right_bottom() - size - vec2(12.0, 8.0), size);
    ui.painter().rect_filled(badge, 5.0, style.theme.ansi[1].color());
    ui.painter().galley(badge.min + vec2(5.0, 2.0), galley, style.theme.background.color());
    ui.ctx().request_repaint_after(EXIT_BADGE - finished.at.elapsed());
}

/// A spinner over an agent pane that hasn't drawn anything yet; resuming a long conversation
/// can take seconds while Claude Code loads it, and an empty pane reads as a hang.
pub(super) fn starting_indicator(ui: &Ui, session: &TerminalSession, rect: egui::Rect) {
    let starting = session.agent_status().is_some()
        && session.frame.blank
        && !session.exited
        && session.spawned_at.elapsed() < STARTING_FOR;
    if !starting {
        return;
    }
    let color = muted(ui);
    let font = egui::FontId::proportional(13.0);
    let galley = ui.painter().layout_no_wrap("Starting Claude…".into(), font, color);
    let size = vec2(14.0 + 8.0 + galley.size().x, galley.size().y.max(14.0));
    let origin = rect.center() - size / 2.0;
    spinner_at(
        ui,
        egui::Rect::from_min_size(origin + vec2(0.0, (size.y - 14.0) / 2.0), vec2(14.0, 14.0)),
        color,
    );
    ui.painter().galley(origin + vec2(22.0, (size.y - galley.size().y) / 2.0), galley, color);
}

/// Tells programs that opted in (mode 1004) when keyboard focus enters or leaves the pane.
pub(super) fn report_focus(
    ui: &Ui,
    session: &TerminalSession,
    mode: TermMode,
    focused: bool,
    reported: &mut Option<bool>,
) {
    let has_focus = focused && ui.input(|i| i.focused);
    if (*reported) == Some(has_focus) {
        return;
    }
    (*reported) = Some(has_focus);
    if mode.contains(TermMode::FOCUS_IN_OUT) {
        session.write(if has_focus { b"\x1b[I".to_vec() } else { b"\x1b[O".to_vec() });
    }
}

/// Blink phase of a focused cursor; schedules the repaint for the next phase flip.
pub(super) fn cursor_visible(ui: &Ui, style: &TerminalStyle<'_>, focused: bool, last_input: f64) -> bool {
    if !style.cursor_blink || !focused || !ui.input(|i| i.focused) {
        return true;
    }
    let elapsed = (ui.input(|i| i.time) - last_input).max(0.0);
    let until_flip = BLINK_INTERVAL - elapsed % BLINK_INTERVAL;
    ui.ctx().request_repaint_after(Duration::from_secs_f64(until_flip));
    ((elapsed / BLINK_INTERVAL) as u64).is_multiple_of(2)
}
