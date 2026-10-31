use eframe::egui::{
    Align, Align2, Button, CornerRadius, FontId, Label, Layout, RichText, Sense, Stroke, Ui, pos2, vec2,
};

use super::Action;
use super::agent_icon;
use super::agent_status;
use super::claude_icon::ICON_SIZE;
use super::repo_drag;
use super::repo_indicators::{RepoRow, busy_mark, indicators, tabs_chevron};
use super::sidebar::{tab_agent, tab_busy, tab_menu, tab_status, tab_title};
use super::widgets::{avatar, glyph_button, menu_icon, muted, right_aligned, row_sized};
use crate::git_watcher::GitWatcher;
use crate::model::{AppState, PaneKind, Repo, Tab};
use crate::pr_status::{CheckState, PrWatcher, PullRequest};
use crate::session_pool::SessionPool;
use crate::worktree;

const REPO_INDENT: f32 = 6.0;
const AVATAR: f32 = 24.0;
/// The branch row lines up with the repo name, past the avatar.
const TEXT_INDENT: f32 = REPO_INDENT + AVATAR + 10.0;
const TAB_INDENT: f32 = TEXT_INDENT + 22.0;
const TAB_ROW_HEIGHT: f32 = 26.0;

/// A repo as a block: its name, its branch line with activity, and the terminals open in it.
/// Without branches the activity moves onto the name row. File and diff tabs only appear in the
/// tab bar.
#[allow(clippy::too_many_arguments)]
pub fn repo_block(
    ui: &mut Ui,
    repo: &Repo,
    state: &AppState,
    git: &GitWatcher,
    prs: &PrWatcher,
    pool: &SessionPool,
    now: u64,
    actions: &mut Vec<Action>,
) {
    let selected = state.selected_repo == Some(repo.id);
    let tabs: Vec<&Tab> = repo.tabs.iter().filter(|t| !t.is_document()).collect();
    let active_tab = selected.then_some(repo.active_tab).flatten();
    let tab_active = tabs.iter().any(|t| Some(t.id) == active_tab);

    let pr = prs.pr(&repo.path).flatten();
    let status = git.status(&repo.path);
    let row = RepoRow { repo, status: status.as_deref(), pr: pr.as_deref(), pool, now };
    let expandable = !tabs.is_empty();
    if state.settings.sidebar_branches {
        repo_row(ui, &row, None, actions);
        branch_row(ui, &row, selected && !tab_active, expandable, actions);
    } else {
        repo_row(ui, &row, Some((selected && !tab_active, expandable)), actions);
    }
    if !repo.tabs_collapsed {
        for (index, tab) in tabs.iter().enumerate() {
            let (selected, last) = (Some(tab.id) == active_tab, index + 1 == tabs.len());
            tab_row(ui, state, repo, tab, selected, last, pool, actions);
        }
    }
    ui.add_space(6.0);
}

#[allow(clippy::too_many_arguments)]
fn tab_row(
    ui: &mut Ui,
    state: &AppState,
    repo: &Repo,
    tab: &Tab,
    selected: bool,
    last: bool,
    pool: &SessionPool,
    actions: &mut Vec<Action>,
) {
    // The branch row above already names the branch, so idle tabs show what runs in them.
    let agent = tab_agent(tab, pool);
    let fallback = match tab.kind() {
        PaneKind::Agent => agent.label(),
        PaneKind::Shell => "Terminal",
    };
    let title = tab_title(tab, pool, fallback);
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
                match tab.kind() {
                    PaneKind::Agent => agent_icon::icon(ui, ICON_SIZE, agent),
                    PaneKind::Shell => terminal_icon(ui),
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
    response.context_menu(|ui| tab_menu(ui, state, repo.id, tab.id, actions));
}

/// The `├`/`└` guide joining a tab to its branch; `last` stops the vertical line at this row.
fn tree_connector(ui: &Ui, x: f32, row: eframe::egui::Rect, last: bool) {
    let stroke = Stroke::new(1.0, muted(ui).gamma_multiply(0.5));
    let bottom = if last { row.center().y } else { row.bottom() };
    ui.painter().line_segment([pos2(x, row.top()), pos2(x, bottom)], stroke);
    ui.painter().line_segment([pos2(x, row.center().y), pos2(x + 8.0, row.center().y)], stroke);
}

/// Small rounded square with a prompt glyph.
fn terminal_icon(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ICON_SIZE, ICON_SIZE), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(3), muted(ui).gamma_multiply(0.5));
    painter.text(
        rect.center() + vec2(0.0, -0.5),
        Align2::CENTER_CENTER,
        ">_",
        FontId::proportional(ICON_SIZE * 0.7),
        ui.visuals().extreme_bg_color,
    );
}

/// The repo's avatar and name. `compact` (selected, expandable) also puts the activity and the tab
/// chevron here, for when there's no branch row below.
fn repo_row(ui: &mut Ui, row: &RepoRow, compact: Option<(bool, bool)>, actions: &mut Vec<Action>) {
    let repo = row.repo;
    let selected = compact.is_some_and(|(selected, _)| selected);
    let (response, ()) = row_sized(ui, selected, REPO_INDENT, 32.0, |ui| {
        right_aligned(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            menu_icon(ui, "+", |ui| new_tab_menu(ui, repo, actions)).on_hover_text("New tab");
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
    response
        .on_hover_text(repo.path.display().to_string())
        .context_menu(|ui| repo_menu(ui, repo, row.pr, actions));
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
    response.context_menu(|ui| repo_menu(ui, repo, row.pr, actions));
}

fn new_tab_menu(ui: &mut Ui, repo: &Repo, actions: &mut Vec<Action>) {
    if ui.add(Button::new("New terminal").shortcut_text("⌘T")).clicked() {
        actions.push(Action::NewTab { repo: repo.id, kind: PaneKind::Shell });
    }
    if ui.add(Button::new("New Claude session").shortcut_text("⇧⌘T")).clicked() {
        actions.push(Action::NewTab { repo: repo.id, kind: PaneKind::Agent });
    }
}

fn repo_menu(ui: &mut Ui, repo: &Repo, pr: Option<&PullRequest>, actions: &mut Vec<Action>) {
    new_tab_menu(ui, repo, actions);
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
    if ui.button("New worktree…").clicked() {
        actions.push(Action::OpenNewWorktree(repo.id));
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
