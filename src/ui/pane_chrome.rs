use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::vte::ansi::{ClearMode, Handler};
use eframe::egui::{Align, Button, Color32, Layout, Rect, RichText, Ui, UiBuilder, vec2};
use uuid::Uuid;

use super::Action;
use super::widgets::muted;
use crate::model::{PaneKind, SplitDir};
use crate::terminal::TerminalSession;

/// Right-click menu of a terminal pane.
pub fn pane_menu(ui: &mut Ui, session: &TerminalSession, tab: Uuid, pane: Uuid, actions: &mut Vec<Action>) {
    if ui.add(Button::new("Split right").shortcut_text("⌘D")).clicked() {
        actions.push(Action::Split { dir: SplitDir::Right, kind: PaneKind::Shell });
    }
    if ui.add(Button::new("Split down").shortcut_text("⇧⌘D")).clicked() {
        actions.push(Action::Split { dir: SplitDir::Down, kind: PaneKind::Shell });
    }
    if ui.button("Split right with Claude").clicked() {
        actions.push(Action::Split { dir: SplitDir::Right, kind: PaneKind::Agent });
    }
    if ui.button("Split down with Claude").clicked() {
        actions.push(Action::Split { dir: SplitDir::Down, kind: PaneKind::Agent });
    }
    if ui.add(Button::new("Zoom pane").shortcut_text("⇧⌘↩")).clicked() {
        actions.push(Action::ToggleZoom);
    }
    ui.separator();
    let selection = session.term.lock().selection_to_string();
    if ui.add_enabled(selection.is_some(), Button::new("Copy").shortcut_text("⌘C")).clicked()
        && let Some(text) = selection
    {
        ui.ctx().copy_text(text);
    }
    let output = session.last_output();
    if ui.add_enabled(output.is_some(), Button::new("Copy last output")).clicked()
        && let Some(text) = output
    {
        ui.ctx().copy_text(text);
    }
    if ui.button("Paste").clicked() {
        ui.ctx().send_viewport_cmd(eframe::egui::ViewportCommand::RequestPaste);
    }
    if ui.button("Select all").clicked() {
        let mut term = session.term.lock();
        let end = Point::new(Line(term.screen_lines() as i32 - 1), Column(term.columns() - 1));
        let start = Point::new(Line(-(term.grid().history_size() as i32)), Column(0));
        let mut selection = Selection::new(SelectionType::Simple, start, Side::Left);
        selection.update(end, Side::Right);
        term.selection = Some(selection);
    }
    if ui.button("Clear scrollback").clicked() {
        session.term.lock().clear_screen(ClearMode::Saved);
    }
    if ui.button("Restart").clicked() {
        actions.push(Action::RestartPane(pane));
    }
    ui.separator();
    if ui.add(Button::new("Close pane").shortcut_text("⌘W")).clicked() {
        actions.push(Action::ClosePane { tab, pane });
    }
}

pub fn exited_overlay(ui: &mut Ui, rect: Rect, tab: Uuid, pane: Uuid, actions: &mut Vec<Action>) {
    let bar = Rect::from_min_max(rect.left_bottom() - vec2(0.0, 34.0), rect.right_bottom());
    ui.painter().rect_filled(bar, 0.0, Color32::from_black_alpha(200));
    let mut bar_ui =
        ui.new_child(UiBuilder::new().max_rect(bar.shrink(6.0)).layout(Layout::left_to_right(Align::Center)));
    bar_ui.label(RichText::new("Process exited").color(muted(ui)));
    if bar_ui.button("Restart").clicked() {
        actions.push(Action::RestartPane(pane));
    }
    if bar_ui.button("Close").clicked() {
        actions.push(Action::ClosePane { tab, pane });
    }
}
