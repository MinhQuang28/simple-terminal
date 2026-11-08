//! The Code sidebar's Source Control (the repo's changes) and Library views.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui::{Color32, Label, RichText, ScrollArea, Ui};

use super::Action;
use super::commit_box;
use super::file_icon;
use super::library_panel;
use super::widgets::{muted, right_aligned, row};
use crate::git_status::{ChangedFile, GitStatus};
use crate::library_cache::LibraryCache;
use crate::model::{AppState, Repo, SidePanel};

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
            Some(status) if !status.files.is_empty() => {
                commit_box::show(ui, repo, status.files.len(), actions);
                for file in &status.files {
                    change_row(ui, repo, file, open_file.as_deref(), actions);
                }
            }
            Some(_) => {
                ui.label(RichText::new("Working tree clean").color(muted(ui)));
            }
            None => {
                ui.label(RichText::new("Not a git repository").color(muted(ui)));
            }
        },
    });
}

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
    let (response, ()) = row(ui, open_file == Some(path.as_path()), 0.0, |ui| {
        file_icon::show(ui, 15.0, name, false, false);
        ui.add(Label::new(RichText::new(name).size(13.0)).truncate().selectable(false));
        ui.add(Label::new(RichText::new(parent).size(11.5).color(muted(ui))).truncate().selectable(false));
        right_aligned(ui, |ui| {
            ui.label(RichText::new(file.status.to_string()).monospace().color(status_color(file.status)));
        });
    });
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

fn status_color(status: char) -> Color32 {
    match status {
        'A' | 'U' => Color32::from_rgb(0x8f, 0xc9, 0x7a),
        'D' => Color32::from_rgb(0xe0, 0x6c, 0x75),
        'R' | 'C' => Color32::from_rgb(0x61, 0xaf, 0xef),
        '!' => Color32::from_rgb(0xd1, 0x9a, 0x66),
        _ => Color32::from_rgb(0xe5, 0xc0, 0x7b),
    }
}
