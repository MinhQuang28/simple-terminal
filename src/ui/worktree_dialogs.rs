//! The dialogs for removing a worktree and for opening one of a repository's checkouts.

use std::path::{Path, PathBuf};

use eframe::egui::{Checkbox, Color32, Key, Label, RichText, ScrollArea, Ui};
use uuid::Uuid;

use super::dialog_kit::{self as kit, Tone};
use super::dialogs::DialogResult;
use super::widgets::{accent, muted, right_aligned, row_sized};
use crate::worktree::Checkout;

const WARNING: Color32 = Color32::from_rgb(0xe4, 0x67, 0x6b);
const ROW_HEIGHT: f32 = 42.0;

pub struct RemoveWorktree {
    pub repo: Uuid,
    pub name: String,
    /// Empty on a detached HEAD.
    pub branch: String,
    /// Uncommitted changes in the worktree when the dialog opened.
    pub changes: usize,
    /// Commits only a detached HEAD holds.
    pub stray_commits: u32,
    pub delete_branch: bool,
}

impl RemoveWorktree {
    /// Returns `(close, result)`. What is at risk is spelled out; only then is git told to force.
    pub fn show(&mut self, ui: &mut Ui) -> (bool, Option<DialogResult>) {
        kit::title(ui, "Remove worktree");
        let kept =
            if self.branch.is_empty() { String::new() } else { format!(" Branch {} stays.", self.branch) };
        kit::description(ui, &format!("Deletes the folder {} and closes its tabs.{kept}", self.name));
        let plural =
            |n: usize, one: &str, many: &str| if n == 1 { one.to_string() } else { format!("{n} {many}") };
        if self.changes > 0 {
            let what = plural(self.changes, "1 uncommitted change", "uncommitted changes");
            ui.label(RichText::new(format!("{what} will be lost.")).color(WARNING));
        }
        if self.stray_commits > 0 {
            let what = plural(self.stray_commits as usize, "1 commit is", "commits are");
            ui.label(RichText::new(format!("{what} on no branch and will be lost.")).color(WARNING));
        }
        if !self.risky() {
            kit::description(ui, "Nothing uncommitted here: no work is lost.");
        }
        if !self.branch.is_empty() {
            let label = format!("Also delete branch {} (only if merged)", self.branch);
            ui.add(Checkbox::new(&mut self.delete_branch, label));
        }
        let ok = if self.risky() { "Remove anyway" } else { "Remove" };
        // Losing work takes a click: Enter only confirms a removal that loses nothing.
        let footer = kit::footer(ui, &[(ok, Tone::Destructive)], !self.risky());
        let result = footer.action.map(|_| DialogResult::RemoveWorktree {
            repo: self.repo,
            delete_branch: self.delete_branch,
            force: self.risky(),
        });
        (result.is_some() || footer.cancel, result)
    }

    fn risky(&self) -> bool {
        self.changes > 0 || self.stray_commits > 0
    }
}

pub struct OpenWorktree {
    pub repo: Uuid,
    pub repo_name: String,
    pub checkouts: Vec<Checkout>,
    /// Folders already in the sidebar.
    pub open: Vec<PathBuf>,
    pub filter: String,
    pub selected: usize,
}

impl OpenWorktree {
    /// Returns `(close, result)`: a filter field over the list, ↑↓ to choose, Enter to open.
    pub fn show(&mut self, ui: &mut Ui) -> (bool, Option<DialogResult>) {
        kit::title(ui, &format!("Open worktree of {}", self.repo_name));
        let field = kit::field(ui, &mut self.filter, "Filter by branch or folder", false);
        kit::focus_initially(ui, &field);
        if field.changed() {
            self.selected = 0;
        }
        let needle = self.filter.trim().to_lowercase();
        let shown: Vec<&Checkout> = self
            .checkouts
            .iter()
            .filter(|c| {
                let text = format!("{} {}", c.branch.as_deref().unwrap_or(""), c.path.display());
                text.to_lowercase().contains(&needle)
            })
            .collect();
        let step =
            ui.input(|i| i.key_pressed(Key::ArrowDown) as usize + (i.key_pressed(Key::ArrowUp) as usize) * 2);
        if !shown.is_empty() {
            self.selected = match step {
                1 => (self.selected + 1) % shown.len(),
                2 => (self.selected + shown.len() - 1) % shown.len(),
                _ => self.selected.min(shown.len() - 1),
            };
        }
        let mut clicked = None;
        ScrollArea::vertical().max_height(ROW_HEIGHT * 6.5).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            for (index, checkout) in shown.iter().enumerate() {
                let open = self.open.contains(&checkout.path);
                let response = checkout_row(ui, checkout, open, index == self.selected);
                if step != 0 && index == self.selected {
                    response.scroll_to_me(None);
                }
                if response.clicked() {
                    clicked = Some(index);
                }
            }
            if shown.is_empty() {
                kit::description(ui, "No matching worktrees.");
            }
        });
        let footer = kit::footer(ui, &[("Open", Tone::Primary)], true);
        let chosen = clicked.or(footer.action.map(|_| self.selected)).and_then(|i| shown.get(i));
        let result = chosen.map(|c| DialogResult::OpenWorktree { repo: self.repo, path: c.path.clone() });
        (result.is_some() || footer.cancel, result)
    }
}

/// Branch over folder, with "main checkout" or "open" at the right.
fn checkout_row(ui: &mut Ui, checkout: &Checkout, open: bool, selected: bool) -> eframe::egui::Response {
    let (response, ()) = row_sized(ui, selected, 4.0, ROW_HEIGHT, |ui| {
        right_aligned(ui, |ui| {
            ui.add_space(4.0);
            if open {
                ui.label(RichText::new("open").size(11.0).color(accent(ui)));
            }
            if checkout.main {
                ui.label(RichText::new("main checkout").size(11.0).color(muted(ui)));
            }
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.add_space(5.0);
                let branch = checkout.branch.as_deref().unwrap_or("detached");
                ui.add(Label::new(RichText::new(branch).size(13.0)).truncate().selectable(false));
                let folder = RichText::new(home_relative(&checkout.path)).size(11.5).color(muted(ui));
                ui.add(Label::new(folder).truncate().selectable(false));
            });
        });
    });
    response
}

/// The path with the home folder as `~`.
fn home_relative(path: &Path) -> String {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match home.and_then(|home| path.strip_prefix(home).ok().map(Path::to_path_buf)) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}
