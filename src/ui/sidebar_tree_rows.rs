//! The rows under a repo in the sidebar: its terminals, then those of its linked worktrees, which
//! look the same apart from a branch glyph and the branch name in front of the title.

use eframe::egui::{Align, Label, Layout, RichText, Stroke, Ui, pos2};

use super::Action;
use super::agent_status;
use super::branch_menu::inline_glyph;
use super::claude_icon::ICON_SIZE;
use super::program_icon;
use super::repo_indicators::busy_mark;
use super::sidebar::{tab_agent, tab_busy, tab_menu, tab_program, tab_status, tab_title};
use super::widgets::{glyph_button, muted, right_aligned, row_sized};
use crate::git_watcher::GitWatcher;
use crate::model::{AppState, PaneKind, Repo, Tab};
use crate::programs;
use crate::session_pool::SessionPool;

/// The branch row and the tree guides line up with the repo name, past its avatar.
pub const TEXT_INDENT: f32 = 6.0 + 24.0 + 10.0;
const TAB_INDENT: f32 = TEXT_INDENT + 22.0;
const TAB_ROW_HEIGHT: f32 = 26.0;

/// One row under a repo: a tab of the repo or of one of its worktrees (`branch` set), or a
/// worktree with no tabs open (`tab` empty).
pub struct TreeRow<'a> {
    pub repo: &'a Repo,
    pub tab: Option<&'a Tab>,
    pub branch: Option<String>,
}

/// The repo's own terminal and agent tabs, then each worktree's; settled tabs stay out unless
/// they are the one on screen.
pub fn tree_rows<'a>(
    repo: &'a Repo,
    worktrees: &[&'a Repo],
    state: &AppState,
    git: &GitWatcher,
) -> Vec<TreeRow<'a>> {
    let mut rows: Vec<TreeRow> = visible_tabs(repo, state)
        .into_iter()
        .map(|tab| TreeRow { repo, tab: Some(tab), branch: None })
        .collect();
    for &worktree in worktrees {
        let branch = git.status(&worktree.path).map_or_else(|| worktree.name.clone(), |s| s.branch.clone());
        let tabs = visible_tabs(worktree, state);
        if tabs.is_empty() {
            rows.push(TreeRow { repo: worktree, tab: None, branch: Some(branch) });
            continue;
        }
        for tab in tabs {
            rows.push(TreeRow { repo: worktree, tab: Some(tab), branch: Some(branch.clone()) });
        }
    }
    rows
}

pub fn visible_tabs<'a>(repo: &'a Repo, state: &AppState) -> Vec<&'a Tab> {
    let active = (state.selected_repo == Some(repo.id)).then_some(repo.active_tab).flatten();
    repo.tabs.iter().filter(|t| !t.is_document() && (!t.settled || Some(t.id) == active)).collect()
}

pub fn tree_row(
    ui: &mut Ui,
    state: &AppState,
    row: &TreeRow,
    last: bool,
    pool: &SessionPool,
    actions: &mut Vec<Action>,
) {
    let shown = state.selected_repo == Some(row.repo.id);
    match row.tab {
        Some(tab) => {
            let selected = shown && row.repo.active_tab == Some(tab.id);
            tab_row(ui, state, row, tab, selected, last, pool, actions);
        }
        None => worktree_row(ui, row, shown, last, actions),
    }
}

#[allow(clippy::too_many_arguments)]
fn tab_row(
    ui: &mut Ui,
    state: &AppState,
    row: &TreeRow,
    tab: &Tab,
    selected: bool,
    last: bool,
    pool: &SessionPool,
    actions: &mut Vec<Action>,
) {
    let repo = row.repo;
    // The branch row above already names the branch, so idle tabs show what runs in them.
    let agent = tab_agent(tab, pool);
    let program = tab_program(tab, pool);
    let fallback = match tab.kind() {
        PaneKind::Agent => agent.label(),
        // A terminal names what it is running, e.g. "Neovim".
        PaneKind::Shell if !std::ptr::eq(program, programs::shell()) => program.name.as_str(),
        PaneKind::Shell => "Terminal",
    };
    let title = match &row.branch {
        Some(branch) => format!("{branch} · {}", tab_title(tab, pool, fallback)),
        None => tab_title(tab, pool, fallback),
    };
    let busy = tab_busy(tab, pool);
    let (response, close) = row_sized(ui, selected, TAB_INDENT, TAB_ROW_HEIGHT, |ui| {
        // Controls go first so the title truncates to the space they leave.
        right_aligned(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let close = glyph_button(ui, "×").on_hover_text("Close tab").clicked();
            if busy {
                busy_mark(ui);
            }
            if let Some(status) = tab_status(tab, pool).filter(|s| s.needs_attention()) {
                agent_status::pill(ui, status);
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                match row.branch {
                    Some(_) => inline_glyph(ui, muted(ui)),
                    None => program_icon::icon(ui, ICON_SIZE, program),
                }
                let color = if selected {
                    ui.visuals().text_color()
                } else {
                    ui.visuals().text_color().gamma_multiply(0.8)
                };
                ui.add(Label::new(RichText::new(title).size(12.5).color(color)).truncate().selectable(false));
            });
            close
        })
    });
    tree_connector(ui, response.rect.left() + TEXT_INDENT + 6.0, response.rect, last);
    if close {
        actions.push(Action::CloseTab { repo: repo.id, tab: tab.id });
    } else if response.clicked() {
        actions.push(Action::SelectTab { repo: repo.id, tab: tab.id });
    }
    let response =
        if row.branch.is_some() { response.on_hover_text(repo.path.display().to_string()) } else { response };
    response.context_menu(|ui| {
        tab_menu(ui, state, repo.id, tab.id, actions);
        if row.branch.is_some() {
            worktree_menu(ui, repo, actions);
        }
    });
}

/// A worktree whose tabs are all closed, so it stays reachable from its repo.
fn worktree_row(ui: &mut Ui, row: &TreeRow, selected: bool, last: bool, actions: &mut Vec<Action>) {
    let branch = row.branch.as_deref().unwrap_or(&row.repo.name);
    let (response, ()) = row_sized(ui, selected, TAB_INDENT, TAB_ROW_HEIGHT, |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        inline_glyph(ui, muted(ui));
        ui.add(Label::new(RichText::new(branch).size(12.5).color(muted(ui))).truncate().selectable(false));
    });
    tree_connector(ui, response.rect.left() + TEXT_INDENT + 6.0, response.rect, last);
    if response.clicked() {
        actions.push(Action::SelectRepo(row.repo.id));
    }
    response.on_hover_text(row.repo.path.display().to_string()).context_menu(|ui| {
        if ui.button("New Claude session").clicked() {
            actions.push(Action::NewTab { repo: row.repo.id, kind: PaneKind::Agent });
        }
        if ui.button("New terminal").clicked() {
            actions.push(Action::NewTab { repo: row.repo.id, kind: PaneKind::Shell });
        }
        worktree_menu(ui, row.repo, actions);
    });
}

fn worktree_menu(ui: &mut Ui, repo: &Repo, actions: &mut Vec<Action>) {
    ui.separator();
    if ui.button("Reveal worktree in Finder").clicked() {
        actions.push(Action::RevealRepo(repo.id));
    }
    if ui.button("Remove worktree…").clicked() {
        actions.push(Action::OpenRemoveWorktree(repo.id));
    }
}

/// The `├`/`└` guide joining a tab to its branch; `last` stops the vertical line at this row.
fn tree_connector(ui: &Ui, x: f32, row: eframe::egui::Rect, last: bool) {
    let stroke = Stroke::new(1.0, muted(ui).gamma_multiply(0.5));
    let bottom = if last { row.center().y } else { row.bottom() };
    ui.painter().line_segment([pos2(x, row.top()), pos2(x, bottom)], stroke);
    ui.painter().line_segment([pos2(x, row.center().y), pos2(x + 8.0, row.center().y)], stroke);
}
