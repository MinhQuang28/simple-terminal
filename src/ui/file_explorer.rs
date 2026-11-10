//! The Code view's folder tree: `.gitignore`-aware, lazily listed, with vscode-icons icons.

use std::path::Path;

use eframe::egui::{Label, RichText, Ui};

use super::Action;
use super::chevron;
use super::file_icon;
use super::widgets::{muted, row_sized};
use crate::file_tree::{Entry, FileTree};
use crate::model::Repo;

const INDENT: f32 = 12.0;
const ROW_HEIGHT: f32 = 24.0;
const ICON_SIZE: f32 = 16.0;

/// Rows for `dir`'s entries, and recursively for its expanded folders.
pub fn directory(
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
        let (response, ()) = row_sized(ui, selected, depth as f32 * INDENT, ROW_HEIGHT, |ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            if entry.is_dir {
                chevron::show(ui, expanded);
            } else {
                ui.add_space(chevron::SIZE);
            }
            file_icon::show(ui, ICON_SIZE, &entry.name, entry.is_dir, expanded);
            let mut name = RichText::new(&entry.name).size(13.0);
            if entry.ignored {
                name = name.color(muted(ui));
            }
            ui.add(Label::new(name).truncate().selectable(false));
        });
        if response.clicked() {
            if entry.is_dir {
                tree.toggle(&entry.path);
            } else {
                actions.push(Action::OpenFile { repo: repo.id, path: entry.path.clone() });
            }
        }
        response.context_menu(|ui| entry_menu(ui, repo, entry, actions));
        if expanded {
            directory(ui, tree, repo, &entry.path, depth + 1, open_file, actions);
        }
    }
}

fn entry_menu(ui: &mut Ui, repo: &Repo, entry: &Entry, actions: &mut Vec<Action>) {
    if !entry.is_dir && ui.button("Open file").clicked() {
        actions.push(Action::OpenFile { repo: repo.id, path: entry.path.clone() });
    }
    if ui.button("Reveal in Finder").clicked() {
        actions.push(Action::RevealPath(entry.path.clone()));
    }
    ui.separator();
    if ui.button("Move to Trash…").clicked() {
        actions.push(Action::TrashPath { repo: repo.id, path: entry.path.clone() });
    }
}
