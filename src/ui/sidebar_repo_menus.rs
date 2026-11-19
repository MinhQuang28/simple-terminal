//! Menus of a repo in the sidebar: the "⋯" button's and the right-click one.

use eframe::egui::{Button, Ui};

use super::Action;
use super::repo_indicators::RepoRow;
use crate::model::{PaneKind, Repo};
use crate::pr_status::CheckState;
use crate::worktree;

/// The "⋯" menu: what is done to the repo itself. New tabs start from the tab bar and the
/// right-click menu.
pub fn repo_actions_menu(ui: &mut Ui, repo: &Repo, actions: &mut Vec<Action>) {
    if ui.button("Rename").clicked() {
        actions.push(Action::OpenRenameRepo(repo.id));
    }
    if ui.button("New worktree").clicked() {
        actions.push(Action::OpenNewWorktree(repo.id));
    }
    if ui.button("Open worktree").clicked() {
        actions.push(Action::OpenWorktreeList(repo.id));
    }
    ui.separator();
    if ui.button("Close").clicked() {
        actions.push(Action::RemoveRepo(repo.id));
    }
}

fn new_tab_menu(ui: &mut Ui, row: &RepoRow, actions: &mut Vec<Action>) {
    let repo = row.repo;
    if ui.add(Button::new("New terminal").shortcut_text(crate::keymap_hint::hint("tab.new"))).clicked() {
        actions.push(Action::NewTab { repo: repo.id, kind: PaneKind::Shell });
    }
    if ui
        .add(Button::new("New Claude session").shortcut_text(crate::keymap_hint::hint("tab.new_agent")))
        .clicked()
    {
        actions.push(Action::NewTab { repo: repo.id, kind: PaneKind::Agent });
    }
    let worktree = Button::new("New Claude session in worktree")
        .shortcut_text(crate::keymap_hint::hint("tab.new_agent_worktree"));
    if ui.add(worktree).clicked() {
        actions.push(Action::NewWorktreeAgent(repo.id));
    }
    let presets = row.presets.iter().enumerate().filter(|(_, p)| !p.command.trim().is_empty());
    for (i, (preset, p)) in presets.enumerate() {
        if i == 0 {
            ui.separator();
        }
        let name = if p.name.trim().is_empty() { p.command.trim() } else { p.name.trim() };
        if ui.button(name).on_hover_text(&p.command).clicked() {
            actions.push(Action::RunPreset { repo: repo.id, preset });
        }
    }
}

pub fn repo_menu(ui: &mut Ui, row: &RepoRow, actions: &mut Vec<Action>) {
    let (repo, pr) = (row.repo, row.pr);
    new_tab_menu(ui, row, actions);
    ui.separator();
    match pr {
        Some(pr) => {
            if ui.button(format!("Open pull request #{}", pr.number)).clicked() {
                actions.push(Action::OpenUrl(pr.url.clone()));
            }
            if pr.ci() == Some(CheckState::Failing) && ui.button("Fix failing CI with Claude").clicked() {
                actions.push(Action::AutoFixCi(repo.id));
            }
        }
        None => {
            if ui.button("Create pull request…").clicked() {
                actions.push(Action::CreatePullRequest(repo.id));
            }
        }
    }
    if ui.button("New worktree on a branch…").clicked() {
        actions.push(Action::OpenNewWorktree(repo.id));
    }
    if ui.button("Open worktree…").clicked() {
        actions.push(Action::OpenWorktreeList(repo.id));
    }
    if ui.button("Worktree setup…").clicked() {
        actions.push(Action::OpenRepoSetup(repo.id));
    }
    if worktree::is_worktree(&repo.path) && ui.button("Remove worktree…").clicked() {
        actions.push(Action::OpenRemoveWorktree(repo.id));
    }
    if ui.button("Reveal in Finder").clicked() {
        actions.push(Action::RevealRepo(repo.id));
    }
    ui.separator();
    if ui.button("Remove from project").clicked() {
        actions.push(Action::RemoveRepo(repo.id));
    }
}
