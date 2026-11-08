//! The Code view's card, laid out like VS Code's editor area: a strip of file and diff tabs over
//! the editor, and the file a second time beside it when split (⌘\). The folder tree and search
//! are in the sidebar (`code_sidebar`); terminals and agents stay in the other view.

use eframe::egui::{Align, Button, Layout, RichText, Stroke, Ui, UiBuilder};

use super::Action;
use super::code_tabs;
use super::file_icon;
use super::widgets::muted;
use super::workspace::{self, Workspace, editor_id};
use crate::model::{Repo, Tab};

const HEADER: f32 = 26.0;

pub fn show(ui: &mut Ui, ws: &mut Workspace<'_>, repo: &Repo) {
    code_tabs::show(ui, repo, ws.buffers, ws.actions);
    let border = Stroke::new(1.0, ws.state.settings.theme.border());
    let below = ui.available_rect_before_wrap();
    ui.painter().hline(below.x_range(), below.top(), border);
    ui.add_space(1.0);
    editor_groups(ui, ws, repo, border);
}

/// The active file, and the split one beside it. Editor commands go to whichever has focus.
fn editor_groups(ui: &mut Ui, ws: &mut Workspace<'_>, repo: &Repo, border: Stroke) {
    let active = repo.active_tab.and_then(|id| repo.view_tabs().find(|t| t.id == id));
    let split = repo.split_tab.and_then(|id| repo.view_tabs().find(|t| t.id == id));
    let Some(active) = active else {
        empty(ui);
        return;
    };
    let Some(split) = split else {
        workspace::document(ui, ws, repo, active, 0, true);
        return;
    };
    let area = ui.available_rect_before_wrap();
    let middle = area.center().x;
    ui.painter().vline(middle, area.y_range(), border);
    let right_focused = split.file.as_deref().is_some_and(|p| ui.memory(|m| m.has_focus(editor_id(p, 1))));
    let mut left =
        ui.new_child(UiBuilder::new().max_rect(area.with_max_x(middle)).layout(Layout::top_down(Align::Min)));
    workspace::document(&mut left, ws, repo, active, 0, !right_focused);
    let mut right = ui.new_child(
        UiBuilder::new().max_rect(area.with_min_x(middle + 1.0)).layout(Layout::top_down(Align::Min)),
    );
    split_header(&mut right, split, ws.actions);
    workspace::document(&mut right, ws, repo, split, 1, right_focused);
    ui.advance_cursor_after_rect(area);
}

/// The second editor names its file, with a × that closes the split.
fn split_header(ui: &mut Ui, tab: &Tab, actions: &mut Vec<Action>) {
    let path = tab.file.as_deref().or(tab.diff.as_deref());
    let name = path.and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    ui.horizontal(|ui| {
        ui.set_height(HEADER);
        ui.add_space(10.0);
        file_icon::show(ui, 14.0, &name, false, false);
        ui.label(RichText::new(&name).size(12.5).color(muted(ui)));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(6.0);
            let close = ui.add(Button::new(RichText::new("×").size(14.0).color(muted(ui))).frame(false));
            if close.on_hover_text(crate::keymap_hint::with_hint("Close split", "editor.split")).clicked() {
                actions.push(Action::ToggleSplitEditor);
            }
        });
    });
}

fn empty(ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() / 3.0);
        ui.label(RichText::new("No file open").size(15.0).color(muted(ui)));
        ui.add_space(6.0);
        let quick_open = crate::keymap_hint::hint("palette.files");
        let terminals = crate::keymap_hint::hint("view.terminals");
        let mut hints = vec!["Pick a file in the sidebar".to_string()];
        if !quick_open.is_empty() {
            hints.push(format!("{quick_open} to find one by name"));
        }
        if !terminals.is_empty() {
            hints.push(format!("{terminals} for the terminals"));
        }
        for hint in hints {
            ui.label(RichText::new(hint).size(12.5).color(muted(ui).gamma_multiply(0.8)));
        }
    });
}
