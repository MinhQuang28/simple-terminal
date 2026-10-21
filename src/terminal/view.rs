use std::path::Path;
use std::time::Duration;

use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::TermMode;
use eframe::egui::{
    self, Event, EventFilter, Id, ImeEvent, Pos2, Response, Sense, Stroke, StrokeKind, Ui, vec2,
};

use super::find::{self, FindState};
use super::input::{key_to_bytes, paste_bytes};
use super::links::Link;
use super::mouse::{self, MouseReportState};
use super::render::{self, CursorState, Grid};
use super::scroll::{self, ScrollState};
use super::session::TerminalSession;
use super::style::TerminalStyle;
use super::view_chrome::{cursor_visible, exit_badge, hover_link, report_focus, starting_indicator};

const BELL_FLASH: Duration = Duration::from_millis(150);
/// How soon a pane showing a stale frame tries again to capture the terminal.
const REFRESH_RETRY: Duration = Duration::from_millis(8);

/// Per-view UI state kept in egui memory between frames.
#[derive(Clone, Default)]
struct ViewState {
    scroll: ScrollState,
    mouse: MouseReportState,
    find: FindState,
    preedit: String,
    /// Focus state last sent to the program, when it asked to be told.
    reported_focus: Option<bool>,
    /// egui time of the last keystroke; the cursor stays solid right after typing.
    last_input: f64,
}

/// What a frame of the terminal view produced for its caller.
pub struct Shown {
    pub response: Response,
    /// A file the user ⌘-clicked, to open in an editor.
    pub open_file: Option<Link>,
}

/// Draws `session` filling the available space and routes keyboard/mouse input to it. Paths in
/// the output resolve against `cwd`.
/// Asks the next focused terminal pane to open its find bar.
pub fn request_find(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(find_request_id(), true));
}

fn find_request_id() -> Id {
    Id::unique("terminal-find-request")
}

pub fn show(
    ui: &mut Ui,
    session: &mut TerminalSession,
    id: Id,
    style: &TerminalStyle<'_>,
    cwd: &Path,
) -> Shown {
    let rect = ui.available_rect_before_wrap();
    let response = ui.interact(rect, id, Sense::click_and_drag());
    ui.advance_cursor_after_rect(rect);
    if response.clicked() || response.drag_started() {
        response.request_focus();
    }
    session.set_visible(true);
    session.sync_theme(style.theme);

    let painter = ui.painter_at(rect);
    let (cell, text_offset) = render::metrics(&painter, style);
    let inner = rect.shrink(style.padding);
    let cols = (inner.width() / cell.x).floor().max(1.0) as u16;
    let rows = (inner.height() / cell.y).floor().max(1.0) as u16;
    session.resize(cols, rows);

    if let Some(text) = session.process_events() {
        ui.ctx().copy_text(text);
    }

    let mut state: ViewState = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    let focused = response.has_focus();
    state.find.shortcut(ui, focused);
    if focused && ui.data_mut(|d| d.remove_temp::<bool>(find_request_id())).is_some() {
        state.find.open = true;
    }
    if focused {
        ui.memory_mut(|m| {
            let filter =
                EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true };
            m.set_focus_lock_filter(id, filter);
        });
        if handle_keyboard(ui, session, &mut state, style) {
            state.last_input = ui.input(|i| i.time);
            session.note_user_input();
        }
    }
    // The previous frame's mode; reading the live one would wait on the PTY thread.
    let mode = session.frame.mode;
    report_focus(ui, session, mode, focused, &mut state.reported_focus);
    let grid = Grid { origin: inner.min, cell, text_offset };
    match response.hover_pos() {
        Some(pointer) => scroll::handle_wheel(ui, session, &mut state.scroll, grid, pointer),
        None => scroll::scroll_scrollback(ui, session, &mut state.scroll, 0.0),
    }
    if mode.contains(TermMode::ALT_SCREEN) {
        state.scroll.clear_pixel_offset();
    }
    let grid = Grid { origin: inner.min + vec2(0.0, state.scroll.pixel_offset(cell.y)), ..grid };

    painter.rect_filled(rect, 0.0, style.theme.background.color());
    let cursor_state = CursorState { focused, visible: cursor_visible(ui, style, focused, state.last_input) };
    if !session.frame.refresh(&session.term, style.theme) {
        ui.ctx().request_repaint_after(REFRESH_RETRY);
    }
    let preedit = focused.then_some(state.preedit.as_str());
    let cursor_rect = render::paint(&painter, grid, style, &session.frame, cursor_state, preedit);
    if !scroll::overlay(ui, session, &mut state.scroll, rect, id)
        && !mouse::report(ui, &response, session, mode, grid, (cols, rows), &mut state.mouse)
    {
        handle_selection(&response, session, grid, (cols, rows));
        if style.copy_on_select
            && (response.drag_stopped() || response.double_clicked() || response.triple_clicked())
            && let Some(text) = session.term.lock().selection_to_string()
        {
            ui.ctx().copy_text(text);
        }
    }
    exit_badge(ui, session, rect, style);
    starting_indicator(ui, session, rect);
    let open_file = hover_link(ui, &response, session, grid, (cols, rows), cwd);
    if state.find.open {
        find::overlay(ui, session, &mut state.find, rect, grid, &response, style);
    }
    if let Some(at) = session.bell_at.filter(|at| at.elapsed() < BELL_FLASH) {
        painter.rect_stroke(rect, 0.0, Stroke::new(2.0, style.theme.accent()), StrokeKind::Inside);
        ui.ctx().request_repaint_after(BELL_FLASH - at.elapsed());
    }

    if focused {
        ui.output_mut(|o| {
            o.ime = Some(egui::output::IMEOutput {
                purpose: egui::IMEPurpose::Terminal,
                rect,
                cursor_rect,
                should_interrupt_composition: false,
            });
        });
    }
    ui.data_mut(|d| d.insert_temp(id, state));
    Shown { response, open_file }
}

/// Returns whether anything was sent to the PTY.
fn handle_keyboard(
    ui: &Ui,
    session: &mut TerminalSession,
    state: &mut ViewState,
    style: &TerminalStyle<'_>,
) -> bool {
    let events = ui.input(|i| i.events.clone());
    let mode = session.frame.mode;
    let mut typed = false;
    // Option+key with Meta enabled sends Esc+key; the composed character egui also reports for
    // the same press (e.g. ∂ for ⌥D) is dropped.
    let mut skip_text = false;
    for event in events {
        let bytes = match event {
            Event::Text(_) if std::mem::take(&mut skip_text) => None,
            Event::Text(text) => Some(text.into_bytes()),
            Event::Key { key, pressed: true, modifiers, .. }
                if modifiers.command
                    && !modifiers.ctrl
                    && matches!(key, egui::Key::ArrowUp | egui::Key::ArrowDown) =>
            {
                session.scroll_to_prompt(if key == egui::Key::ArrowUp { -1 } else { 1 });
                None
            }
            Event::Key { key, pressed: true, modifiers, .. }
                if style.option_as_meta
                    && modifiers.alt
                    && !modifiers.ctrl
                    && !modifiers.command
                    && key.name().len() == 1 =>
            {
                skip_text = true;
                let name = key.name();
                let c = if modifiers.shift { name.to_uppercase() } else { name.to_lowercase() };
                Some(format!("\x1b{c}").into_bytes())
            }
            Event::Key { key, pressed: true, modifiers, .. } => key_to_bytes(key, modifiers, mode),
            Event::Paste(text) => Some(paste_bytes(&text, mode)),
            // An image with no text alongside it (screenshots, "Copy Image"), pasted as a path.
            Event::PasteImage(image) => {
                session.paste_image(image, ui.ctx());
                typed = true;
                None
            }
            Event::Copy | Event::Cut => {
                if let Some(text) = session.term.lock().selection_to_string() {
                    ui.ctx().copy_text(text);
                }
                None
            }
            Event::Ime(ImeEvent::Preedit { text, .. }) => {
                state.preedit = text;
                None
            }
            Event::Ime(ImeEvent::Commit(text)) => {
                state.preedit.clear();
                Some(text.into_bytes())
            }
            _ => None,
        };
        if let Some(bytes) = bytes.filter(|b| !b.is_empty()) {
            session.send_input(bytes);
            typed = true;
        }
    }
    // Skips the lock, which may wait on the PTY thread mid-parse, when there's nothing to undo.
    let frame = &session.frame;
    if typed && (frame.has_selection || frame.display_offset > 0 || state.scroll.remainder != 0.0) {
        let mut term = session.term.lock();
        term.selection = None;
        state.scroll.snap_to_bottom(&mut term);
    }
    typed
}

fn handle_selection(response: &Response, session: &mut TerminalSession, grid: Grid, size: (u16, u16)) {
    let Some(pos) = response.interact_pointer_pos() else { return };
    let mut term = session.term.lock();
    let offset = term.grid().display_offset() as i32;
    let (point, side) = grid_point(grid, pos, size, offset);

    if response.double_clicked() {
        term.selection = Some(Selection::new(SelectionType::Semantic, point, side));
    } else if response.triple_clicked() {
        term.selection = Some(Selection::new(SelectionType::Lines, point, side));
    } else if response.drag_started() {
        term.selection = Some(Selection::new(SelectionType::Simple, point, side));
    } else if response.dragged() {
        if let Some(selection) = term.selection.as_mut() {
            selection.update(point, side);
        }
    } else if response.clicked() {
        term.selection = None;
    }
    // The frame was captured before this change; typing checks it to decide whether to clear.
    session.frame.has_selection = term.selection.is_some();
}

fn grid_point(grid: Grid, pos: Pos2, (cols, rows): (u16, u16), display_offset: i32) -> (Point, Side) {
    let rel = pos - grid.origin;
    let col_f = (rel.x / grid.cell.x).clamp(0.0, cols as f32 - 0.01);
    let row = ((rel.y / grid.cell.y).floor() as i32).clamp(0, rows as i32 - 1);
    let side = if col_f.fract() < 0.5 { Side::Left } else { Side::Right };
    (Point::new(Line(row - display_offset), Column(col_f as usize)), side)
}
