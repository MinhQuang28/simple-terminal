use eframe::egui::{
    Align, Align2, Button, Color32, CornerRadius, FontId, Label, Layout, RichText, Sense, Stroke, Ui, pos2,
    vec2,
};

use super::Action;
use super::agent_status;
use super::claude_icon::{self, ICON_SIZE};
use super::sidebar::{tab_busy, tab_menu, tab_status, tab_title};
use super::widgets::{
    accent, ago, avatar, glyph_button, menu_icon, muted, right_aligned, row_sized, spinner, tag,
};
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
/// File and diff tabs only appear in the tab bar.
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
    repo_row(ui, repo, pr.as_deref(), actions);
    let expandable = !tabs.is_empty();
    branch_row(ui, repo, selected && !tab_active, expandable, git, pr.as_deref(), pool, now, actions);
    if !repo.tabs_collapsed {
        for (index, tab) in tabs.iter().enumerate() {
            tab_row(ui, repo, tab, Some(tab.id) == active_tab, index + 1 == tabs.len(), pool, actions);
        }
    }
    ui.add_space(6.0);
}

fn tab_row(
    ui: &mut Ui,
    repo: &Repo,
    tab: &Tab,
    selected: bool,
    last: bool,
    pool: &SessionPool,
    actions: &mut Vec<Action>,
) {
    // The branch row above already names the branch, so idle tabs show what runs in them.
    let fallback = match tab.kind() {
        PaneKind::Agent => "Claude",
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
                spinner(ui, 11.0, muted(ui));
            }
            if let Some(status) = tab_status(tab, pool).filter(|s| s.needs_attention()) {
                agent_status::pill(ui, status);
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                match tab.kind() {
                    PaneKind::Agent => claude_icon::icon(ui, ICON_SIZE),
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
    response.context_menu(|ui| tab_menu(ui, repo.id, tab.id, actions));
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

fn repo_row(ui: &mut Ui, repo: &Repo, pr: Option<&PullRequest>, actions: &mut Vec<Action>) {
    let (response, ()) = row_sized(ui, false, REPO_INDENT, 32.0, |ui| {
        right_aligned(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            menu_icon(ui, "+", |ui| new_tab_menu(ui, repo, actions)).on_hover_text("New tab");
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                avatar(ui, &repo.name, AVATAR);
                ui.add(Label::new(RichText::new(&repo.name).size(14.0)).truncate().selectable(false));
            });
        });
    });
    if response.clicked() {
        actions.push(Action::SelectRepo(repo.id));
    }
    response
        .on_hover_text(repo.path.display().to_string())
        .context_menu(|ui| repo_menu(ui, repo, pr, actions));
}

/// Branch name with sync state and recency; selecting it shows the repo without a terminal.
#[allow(clippy::too_many_arguments)]
fn branch_row(
    ui: &mut Ui,
    repo: &Repo,
    selected: bool,
    expandable: bool,
    git: &GitWatcher,
    pr: Option<&PullRequest>,
    pool: &SessionPool,
    now: u64,
    actions: &mut Vec<Action>,
) {
    let status = git.status(&repo.path);
    let busy = repo.tabs.iter().any(|t| tab_busy(t, pool));
    let indent = if expandable { TEXT_INDENT - 20.0 } else { TEXT_INDENT };
    let (response, ()) = row_sized(ui, selected, indent, 28.0, |ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let name = status.as_ref().map_or("local", |s| s.branch.as_str());
        let color =
            if selected { ui.visuals().text_color() } else { ui.visuals().text_color().gamma_multiply(0.85) };
        let attention = repo.tabs.iter().filter_map(|t| tab_status(t, pool)).max();
        // Indicators go first so the branch name truncates to the space they leave.
        right_aligned(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.label(RichText::new(ago(repo.last_active(), now)).color(muted(ui)).size(11.5));
            if let Some(pr) = pr {
                pr_badge(ui, pr, actions);
            }
            if busy {
                spinner(ui, 11.0, muted(ui));
            }
            if let Some(status) = &status {
                if status.ahead > 0 {
                    ui.label(RichText::new("⬆").color(accent(ui)).size(11.5))
                        .on_hover_text(format!("{} commits to push", status.ahead));
                }
                if status.behind > 0 {
                    ui.label(RichText::new("⬇").color(muted(ui)).size(11.5))
                        .on_hover_text(format!("{} commits to pull", status.behind));
                }
            }
            if status.as_ref().is_some_and(|s| s.merged) {
                tag(ui, "merged", muted(ui));
            }
            if let Some(attention) = attention.filter(|s| s.needs_attention()) {
                agent_status::pill(ui, attention);
            }
            ui.add_space(4.0);
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if expandable {
                    let glyph = if repo.tabs_collapsed { "›" } else { "⌄" };
                    let chevron = Button::new(RichText::new(glyph).color(muted(ui)).size(13.0))
                        .frame(false)
                        .min_size(vec2(14.0, 14.0));
                    if ui.add(chevron).clicked() {
                        actions.push(Action::ToggleRepoTabs(repo.id));
                    }
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
    response.context_menu(|ui| repo_menu(ui, repo, pr, actions));
}

/// `#123` tinted by CI state; hovering lists the checks, clicking opens the pull request.
fn pr_badge(ui: &mut Ui, pr: &PullRequest, actions: &mut Vec<Action>) {
    let color = match pr.ci() {
        Some(CheckState::Failing) => Color32::from_rgb(0xe5, 0x53, 0x53),
        Some(CheckState::Pending) => ui.visuals().warn_fg_color,
        Some(CheckState::Passing) => Color32::from_rgb(0x7f, 0xd8, 0x8f),
        None => muted(ui),
    };
    let mut hover = format!("#{} {}", pr.number, pr.title);
    if pr.draft {
        hover.push_str(" (draft)");
    }
    for check in &pr.checks {
        let mark = match check.state {
            CheckState::Passing => "✓",
            CheckState::Failing => "✗",
            CheckState::Pending => "…",
        };
        hover.push_str(&format!("\n{mark} {}", check.name));
    }
    if tag(ui, &format!("#{}", pr.number), color).on_hover_text(hover).clicked() {
        actions.push(Action::OpenUrl(pr.url.clone()));
    }
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
