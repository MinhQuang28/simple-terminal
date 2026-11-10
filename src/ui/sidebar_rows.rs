use eframe::egui::{Align, Button, Label, Layout, RichText, Ui};

use super::Action;
use super::repo_drag;
use super::repo_indicators::{RepoRow, indicators, tabs_chevron};
use super::sidebar_tree_rows::{TEXT_INDENT, tree_row, tree_rows};
use super::widgets::{avatar, menu_icon, muted, right_aligned, row_sized};
use crate::git_watcher::GitWatcher;
use crate::model::{AppState, PaneKind, Repo};
use crate::pr_status::{CheckState, PrWatcher};
use crate::session_pool::SessionPool;
use crate::worktree;

const REPO_INDENT: f32 = 6.0;
const AVATAR: f32 = 24.0;

/// A repo as a block: its name, its branch line with activity, and the terminals open in it and
/// in its `worktrees`. Without branches the activity moves onto the name row. File and diff tabs
/// only appear in the tab bar.
#[allow(clippy::too_many_arguments)]
pub fn repo_block(
    ui: &mut Ui,
    repo: &Repo,
    worktrees: &[&Repo],
    state: &AppState,
    git: &GitWatcher,
    prs: &PrWatcher,
    pool: &SessionPool,
    now: u64,
    actions: &mut Vec<Action>,
) {
    let selected = state.selected_repo == Some(repo.id);
    let rows = tree_rows(repo, worktrees, state, git);
    let tab_active =
        selected && rows.iter().any(|r| r.repo.id == repo.id && r.tab.map(|t| t.id) == repo.active_tab);

    let pr = prs.pr(&repo.path).flatten();
    let status = git.status(&repo.path);
    let row = RepoRow {
        repo,
        status: status.as_deref(),
        pr: pr.as_deref(),
        pool,
        now,
        presets: &state.settings.presets,
    };
    let expandable = !rows.is_empty();
    if state.settings.sidebar_branches {
        repo_row(ui, &row, None, actions);
        branch_row(ui, &row, selected && !tab_active, expandable, actions);
    } else {
        repo_row(ui, &row, Some((selected && !tab_active, expandable)), actions);
    }
    if !repo.tabs_collapsed {
        for (index, tree_row_data) in rows.iter().enumerate() {
            tree_row(ui, state, tree_row_data, index + 1 == rows.len(), pool, actions);
        }
    }
    ui.add_space(6.0);
}

/// The repo's avatar and name. `compact` (selected, expandable) also puts the activity and the tab
/// chevron here, for when there's no branch row below.
fn repo_row(ui: &mut Ui, row: &RepoRow, compact: Option<(bool, bool)>, actions: &mut Vec<Action>) {
    let repo = row.repo;
    let selected = compact.is_some_and(|(selected, _)| selected);
    let (response, ()) = row_sized(ui, selected, REPO_INDENT, 32.0, |ui| {
        right_aligned(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            menu_icon(ui, "+", |ui| new_tab_menu(ui, row, actions)).on_hover_text("New tab");
            if compact.is_some() {
                indicators(ui, row, actions);
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                avatar(ui, &repo.name, AVATAR);
                let expandable = compact.is_some_and(|(_, expandable)| expandable);
                // Leave the chevron its room so a long name truncates instead of pushing it out.
                let chevron_width = if expandable { 20.0 } else { 0.0 };
                ui.scope(|ui| {
                    ui.set_max_width((ui.available_width() - chevron_width).max(0.0));
                    ui.add(Label::new(RichText::new(&repo.name).size(14.0)).truncate().selectable(false));
                });
                if expandable {
                    tabs_chevron(ui, repo, actions);
                }
            });
        });
    });
    let response = repo_drag::drag_source(response, repo.id);
    if response.clicked() {
        actions.push(Action::SelectRepo(repo.id));
    }
    response.on_hover_text(repo.path.display().to_string()).context_menu(|ui| repo_menu(ui, row, actions));
}

/// Branch name with sync state and recency; selecting it shows the repo without a terminal.
fn branch_row(ui: &mut Ui, row: &RepoRow, selected: bool, expandable: bool, actions: &mut Vec<Action>) {
    let repo = row.repo;
    let indent = if expandable { TEXT_INDENT - 20.0 } else { TEXT_INDENT };
    let (response, ()) = row_sized(ui, selected, indent, 28.0, |ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let name = row.status.map_or("local", |s| s.branch.as_str());
        let color =
            if selected { ui.visuals().text_color() } else { ui.visuals().text_color().gamma_multiply(0.85) };
        // Indicators go first so the branch name truncates to the space they leave.
        right_aligned(ui, |ui| {
            indicators(ui, row, actions);
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if expandable {
                    tabs_chevron(ui, repo, actions);
                }
                // Leave the star its room so a long branch name truncates instead of pushing it out.
                let star_width = 16.0;
                ui.scope(|ui| {
                    ui.set_max_width((ui.available_width() - star_width).max(0.0));
                    let label = Label::new(RichText::new(name).size(13.0).color(color));
                    ui.add(label.truncate().selectable(false));
                });
                ui.label(RichText::new("★").color(muted(ui)).size(10.0));
            });
        });
    });
    if response.clicked() {
        actions.push(Action::SelectRepo(repo.id));
    }
    response.context_menu(|ui| repo_menu(ui, row, actions));
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

fn repo_menu(ui: &mut Ui, row: &RepoRow, actions: &mut Vec<Action>) {
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
