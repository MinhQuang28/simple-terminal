use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui::{Color32, CornerRadius, FontId, Label, RichText, ScrollArea, Sense, StrokeKind, Ui, vec2};

use super::Action;
use super::library_panel;
use super::widgets::{file_icon, muted, right_aligned, row};
use crate::file_tree::FileTree;
use crate::git_status::{ChangedFile, GitStatus};
use crate::library_cache::LibraryCache;
use crate::model::{AppState, Repo, SidePanel};

const INDENT: f32 = 14.0;

pub fn show(
    ui: &mut Ui,
    state: &AppState,
    status: Option<Arc<GitStatus>>,
    tree: &mut FileTree,
    library: &LibraryCache,
    actions: &mut Vec<Action>,
) {
    let changes = status.as_ref().map_or(0, |s| s.files.len());
    ui.horizontal(|ui| {
        segment(ui, state.side_panel == SidePanel::Files, "Files", SidePanel::Files, actions);
        segment(
            ui,
            state.side_panel == SidePanel::Changes,
            &format!("Changes {changes}"),
            SidePanel::Changes,
            actions,
        );
        segment(ui, state.side_panel == SidePanel::Library, "Library", SidePanel::Library, actions);
    });
    ui.add_space(6.0);

    let Some(repo) = state.selected_repo.and_then(|id| state.repo(id)) else {
        ui.label(RichText::new("Select a repository").color(muted(ui)));
        return;
    };
    let open_file =
        repo.active_tab.and_then(|id| repo.tabs.iter().find(|t| t.id == id)).and_then(|t| t.file.clone());
    ScrollArea::vertical().auto_shrink(false).show(ui, |ui| match state.side_panel {
        SidePanel::Files => directory(ui, tree, repo, &repo.path, 0, open_file.as_deref(), actions),
        SidePanel::Library => library_panel::show(ui, repo, &state.settings, library, actions),
        SidePanel::Changes => match status {
            Some(status) if !status.files.is_empty() => {
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

fn segment(ui: &mut Ui, selected: bool, label: &str, panel: SidePanel, actions: &mut Vec<Action>) {
    let color = if selected { ui.visuals().text_color() } else { muted(ui) };
    let galley = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(13.0), color);
    let (rect, response) = ui.allocate_exact_size(galley.size() + vec2(20.0, 12.0), Sense::click());
    if selected {
        let visuals = &ui.visuals().widgets;
        ui.painter().rect_filled(rect, CornerRadius::same(8), visuals.inactive.weak_bg_fill);
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(8),
            visuals.noninteractive.bg_stroke,
            StrokeKind::Inside,
        );
    }
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
    if response.clicked() {
        actions.push(Action::SetSidePanel(panel));
    }
}

fn directory(
    ui: &mut Ui,
    tree: &mut FileTree,
    repo: &Repo,
    dir: &Path,
    depth: usize,
    open_file: Option<&Path>,
    actions: &mut Vec<Action>,
) {
    let entries = tree.children(dir);
    for entry in entries.iter() {
        let expanded = entry.is_dir && tree.is_expanded(&entry.path);
        let selected = open_file == Some(entry.path.as_path());
        let (response, ()) = row(ui, selected, depth as f32 * INDENT, |ui| {
            let chevron = match (entry.is_dir, expanded) {
                (true, true) => "⌄",
                (true, false) => "›",
                (false, _) => " ",
            };
            ui.label(RichText::new(chevron).color(muted(ui)).monospace());
            file_icon(ui, entry.is_dir);
            ui.add(Label::new(RichText::new(&entry.name).size(13.0)).truncate().selectable(false));
        });
        if response.clicked() {
            if entry.is_dir {
                tree.toggle(&entry.path);
            } else {
                actions.push(Action::OpenFile { repo: repo.id, path: entry.path.clone() });
            }
        }
        if expanded {
            directory(ui, tree, repo, &entry.path, depth + 1, open_file, actions);
        }
    }
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
