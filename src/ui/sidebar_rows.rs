use eframe::egui::{Align, Label, Layout, RichText, Ui};

use super::Action;
use super::design::{TEXT_BODY, TEXT_CAPTION, TEXT_MICRO};
use super::repo_drag;
use super::repo_indicators::{RepoRow, indicators, tabs_chevron};
use super::sidebar_repo_menus::{repo_actions_menu, repo_menu};
use super::sidebar_worktree_rows::{TEXT_INDENT, worktree_row};
use super::widgets::{GLYPH_BUTTON_SIZE, avatar, menu_icon, muted, right_aligned, row_hovered, row_sized};
use crate::git_watcher::GitWatcher;
use crate::model::{AppState, PaneKind, Repo, RepoLayout};
use crate::pr_status::PrWatcher;
use crate::session_pool::SessionPool;

const REPO_INDENT: f32 = 6.0;
const AVATAR: f32 = 24.0;
const REPO_ROW_HEIGHT: f32 = 40.0;

/// A repo as a block: a row with its name, branch and activity, and a line per worktree with
/// that worktree's branch and activity. Tabs only appear in the tab bar and the Agents strip.
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
    let presets = &state.settings.presets;
    // A folded repo has no worktree row to carry the highlight, so it stays on the repo.
    let selected = state.selected_repo == Some(repo.id)
        || (repo.tabs_collapsed && worktrees.iter().any(|w| state.selected_repo == Some(w.id)));
    let pr = prs.pr(&repo.path).flatten();
    let status = git.status(&repo.path);
    let row = RepoRow { repo, status: status.as_deref(), pr: pr.as_deref(), pool, now, presets };
    let expandable = !worktrees.is_empty();
    match state.settings.repo_layout {
        RepoLayout::Compact => repo_row(ui, &row, selected, expandable, actions),
        RepoLayout::Stacked => {
            name_row(ui, &row, selected, actions);
            branch_row(ui, &row, selected, expandable, actions);
        }
    }
    if !repo.tabs_collapsed {
        for (index, &worktree) in worktrees.iter().enumerate() {
            let pr = prs.pr(&worktree.path).flatten();
            let status = git.status(&worktree.path);
            let row =
                RepoRow { repo: worktree, status: status.as_deref(), pr: pr.as_deref(), pool, now, presets };
            let shown = state.selected_repo == Some(worktree.id);
            worktree_row(ui, &row, shown, index + 1 == worktrees.len(), actions);
        }
    }
    ui.add_space(2.0);
}

/// The two-row layout's first row: the repo's avatar and name. The "⋯" shows on the `current`
/// repo and on the one under the pointer.
fn name_row(ui: &mut Ui, row: &RepoRow, current: bool, actions: &mut Vec<Action>) {
    let repo = row.repo;
    let (response, ()) = row_sized(ui, false, REPO_INDENT, 32.0, |ui| {
        let shown = current || row_hovered(ui);
        right_aligned(ui, |ui| {
            menu_icon(ui, "⋯", shown, |ui| repo_actions_menu(ui, repo, actions));
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                avatar(ui, &repo.name, AVATAR);
                ui.add(Label::new(RichText::new(&repo.name).size(TEXT_BODY)).truncate().selectable(false));
            });
        });
    });
    let response = repo_drag::drag_source(response, repo.id);
    if response.clicked() {
        actions.push(Action::SelectRepo(repo.id));
    }
    response.on_hover_text(repo.path.display().to_string()).context_menu(|ui| repo_menu(ui, row, actions));
}

/// The two-row layout's second row: branch name with sync state, agent status and recency. A
/// click shows the repo's terminals; `expandable` adds the chevron that folds the worktree rows.
fn branch_row(ui: &mut Ui, row: &RepoRow, selected: bool, expandable: bool, actions: &mut Vec<Action>) {
    let repo = row.repo;
    let indent = if expandable { TEXT_INDENT - 20.0 } else { TEXT_INDENT };
    let (response, ()) = row_sized(ui, selected, indent, 28.0, |ui| {
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
                ui.scope(|ui| {
                    ui.set_max_width((ui.available_width() - 16.0).max(0.0));
                    let label = Label::new(RichText::new(name).size(TEXT_BODY).color(color));
                    ui.add(label.truncate().selectable(false));
                });
                ui.label(RichText::new("★").color(muted(ui)).size(TEXT_MICRO));
            });
        });
    });
    if response.clicked() {
        select_with_terminal(repo, actions);
    }
    response.context_menu(|ui| repo_menu(ui, row, actions));
}

/// The compact layout: avatar, name over its branch, activity on the right. A click
/// shows the repo's terminals; `expandable` adds the chevron that folds the worktree rows. The
/// "⋯" shows on the selected repo and on the one under the pointer.
fn repo_row(ui: &mut Ui, row: &RepoRow, selected: bool, expandable: bool, actions: &mut Vec<Action>) {
    let repo = row.repo;
    let (response, ()) = row_sized(ui, selected, REPO_INDENT, REPO_ROW_HEIGHT, |ui| {
        let shown = selected || row_hovered(ui);
        // Controls go first so the name and branch truncate to the space they leave.
        right_aligned(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            menu_icon(ui, "⋯", shown, |ui| repo_actions_menu(ui, repo, actions));
            indicators(ui, row, actions);
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                avatar(ui, &repo.name, AVATAR);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        // Leave the chevron its room so a long name truncates instead of pushing it out.
                        let chevron_width = if expandable { GLYPH_BUTTON_SIZE } else { 0.0 };
                        ui.scope(|ui| {
                            ui.set_max_width((ui.available_width() - chevron_width).max(0.0));
                            let name = RichText::new(&repo.name).size(TEXT_BODY);
                            ui.add(Label::new(name).truncate().selectable(false));
                        });
                        if expandable {
                            tabs_chevron(ui, repo, actions);
                        }
                    });
                    let branch = row.status.map_or("local", |s| s.branch.as_str());
                    let branch = RichText::new(branch).size(TEXT_CAPTION).color(muted(ui));
                    ui.add(Label::new(branch).truncate().selectable(false));
                });
            });
        });
    });
    let response = repo_drag::drag_source(response, repo.id);
    if response.clicked() {
        select_with_terminal(repo, actions);
    }
    response.on_hover_text(repo.path.display().to_string()).context_menu(|ui| repo_menu(ui, row, actions));
}

/// Shows `repo`, starting a terminal in it when it has none open, so a click on a branch lands
/// on a prompt instead of an empty view.
pub fn select_with_terminal(repo: &Repo, actions: &mut Vec<Action>) {
    actions.push(Action::SelectRepo(repo.id));
    if repo.tabs.iter().all(|t| t.is_document()) {
        actions.push(Action::NewTab { repo: repo.id, kind: PaneKind::Shell });
    }
}
