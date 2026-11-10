//! The Code sidebar's Source Control (the repo's changes) and Library views.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui::{
    Align, Color32, CornerRadius, FontId, Id, Label, Layout, RichText, ScrollArea, Sense, Ui, vec2,
};

use super::Action;
use super::chevron;
use super::commit_box;
use super::file_icon;
use super::library_panel;
use super::widgets::{muted, right_aligned, row_sized};
use crate::git_status::{ChangedFile, GitStatus};
use crate::library_cache::LibraryCache;
use crate::model::{AppState, Repo, SidePanel};

const ROW_HEIGHT: f32 = 24.0;
/// Files sit under the section header's title, past its chevron.
const FILE_INDENT: f32 = 12.0;

/// The changes list with its commit box, or the library.
pub(super) fn body(
    ui: &mut Ui,
    state: &AppState,
    repo: &Repo,
    panel: SidePanel,
    status: Option<Arc<GitStatus>>,
    library: &LibraryCache,
    actions: &mut Vec<Action>,
) {
    let open_file =
        repo.active_tab.and_then(|id| repo.tabs.iter().find(|t| t.id == id)).and_then(|t| t.file.clone());
    ScrollArea::vertical().id_salt(("side-panel", panel)).auto_shrink(false).show(ui, |ui| match panel {
        SidePanel::Library => library_panel::show(ui, repo, &state.settings, library, actions),
        _ => match status {
            Some(status) => {
                commit_box::show(ui, repo, &status, actions);
                if status.files.is_empty() {
                    ui.add_space(6.0);
                    ui.vertical_centered(|ui| {
                        let text = format!("No changes on {}", status.branch);
                        ui.label(RichText::new(text).size(12.5).color(muted(ui)));
                    });
                } else if section_header(ui, repo, "Changes", status.files.len()) {
                    for file in &status.files {
                        change_row(ui, repo, file, open_file.as_deref(), actions);
                    }
                }
            }
            None => {
                ui.label(RichText::new("Not a git repository").color(muted(ui)));
            }
        },
    });
}

/// `⌄ Changes  27`; clicking folds the list. Returns whether it is open.
fn section_header(ui: &mut Ui, repo: &Repo, title: &str, count: usize) -> bool {
    let id = Id::unique(("changes-folded", repo.id, title));
    let folded = ui.data(|d| d.get_temp(id).unwrap_or(false));
    let (response, ()) = row_sized(ui, false, 0.0, ROW_HEIGHT, |ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        chevron::show(ui, !folded);
        ui.label(RichText::new(title).size(12.5).strong());
        right_aligned(ui, |ui| count_badge(ui, count));
    });
    if response.clicked() {
        ui.data_mut(|d| d.insert_temp(id, !folded));
    }
    !folded
}

/// A rounded count, like the number beside VS Code's Changes.
fn count_badge(ui: &mut Ui, count: usize) {
    let text =
        ui.painter().layout_no_wrap(count.to_string(), FontId::proportional(10.5), ui.visuals().text_color());
    let size = vec2((text.size().x + 10.0).max(18.0), 16.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(8), ui.visuals().widgets.hovered.bg_fill);
    ui.painter().galley(rect.center() - text.size() / 2.0, text, ui.visuals().text_color());
}

/// `[icon] name  folder   M`, the status letter coloured as in VS Code.
fn change_row(
    ui: &mut Ui,
    repo: &Repo,
    file: &ChangedFile,
    open_file: Option<&Path>,
    actions: &mut Vec<Action>,
) {
    let path: PathBuf = repo.path.join(&file.path);
    let (name, parent) = match file.path.rsplit_once('/') {
        Some((parent, name)) => (name, parent),
        None => (file.path.as_str(), ""),
    };
    let selected = open_file == Some(path.as_path());
    let (response, ()) = row_sized(ui, selected, FILE_INDENT, ROW_HEIGHT, |ui| {
        right_aligned(ui, |ui| {
            let letter = RichText::new(file.status.to_string()).size(12.0).strong();
            ui.label(letter.color(status_color(file.status)));
            ui.add_space(2.0);
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                file_icon::show(ui, 15.0, name, false, false);
                let mut label = RichText::new(name).size(13.0);
                if file.status == 'D' {
                    label = label.strikethrough().color(muted(ui));
                }
                ui.add(Label::new(label).truncate().selectable(false));
                let folder = RichText::new(parent).size(11.5).color(muted(ui));
                ui.add(Label::new(folder).truncate().selectable(false));
            });
        });
    });
    let response = response.on_hover_text(&file.path);
    if response.clicked() {
        actions.push(Action::OpenDiff { repo: repo.id, path: path.clone() });
    }
    response.context_menu(|ui| {
        if ui.button("Open file").clicked() && path.is_file() {
            actions.push(Action::OpenFile { repo: repo.id, path: path.clone() });
        }
        if ui.button("Show diff").clicked() {
            actions.push(Action::OpenDiff { repo: repo.id, path: path.clone() });
        }
    });
}

/// VS Code's git decoration colours.
fn status_color(status: char) -> Color32 {
    match status {
        'U' => Color32::from_rgb(0x73, 0xc9, 0x91),
        'A' => Color32::from_rgb(0x81, 0xb8, 0x8b),
        'D' => Color32::from_rgb(0xc7, 0x4e, 0x39),
        'R' | 'C' => Color32::from_rgb(0x73, 0xc9, 0x91),
        '!' => Color32::from_rgb(0xe4, 0x67, 0x6b),
        _ => Color32::from_rgb(0xe2, 0xc0, 0x8d),
    }
}
