use eframe::egui::{Rect, RichText, Sense, Ui, Vec2, vec2};

use super::Action;
use super::agent_status;
use super::chevron;
use super::design::{self, TEXT_CAPTION};
use super::sidebar::{tab_busy, tab_status};
use super::spinner::spinner;
use super::widgets::{GLYPH_BUTTON_SIZE, accent, ago, muted, tag};
use crate::git_status::GitStatus;
use crate::model::{LaunchPreset, Repo};
use crate::pr_status::{CheckState, PullRequest};
use crate::session_pool::SessionPool;

/// What the sidebar rows of one repo show.
pub struct RepoRow<'a> {
    pub repo: &'a Repo,
    pub status: Option<&'a GitStatus>,
    pub pr: Option<&'a PullRequest>,
    pub pool: &'a SessionPool,
    pub now: u64,
    /// Offered in the repo's new-tab menu.
    pub presets: &'a [LaunchPreset],
}

/// A repo's activity, right to left: recency, pull request, spinner, sync state, merged, attention.
/// Call inside a right-to-left layout.
pub fn indicators(ui: &mut Ui, row: &RepoRow, actions: &mut Vec<Action>) {
    let RepoRow { repo, status, pr, pool, now, .. } = *row;
    ui.spacing_mut().item_spacing.x = 8.0;
    // The spinner already says "now", and the row is short of room for the repo's name.
    let busy = repo.tabs.iter().any(|t| tab_busy(t, pool));
    if !busy {
        ui.label(RichText::new(ago(repo.last_active(), now)).color(muted(ui)).size(TEXT_CAPTION));
    }
    if let Some(pr) = pr {
        pr_badge(ui, pr, actions);
    }
    if busy {
        busy_mark(ui);
    }
    if let Some(status) = status {
        if status.ahead > 0 {
            ui.label(RichText::new("⬆").color(accent(ui)).size(TEXT_CAPTION))
                .on_hover_text(format!("{} commits to push", status.ahead));
        }
        if status.behind > 0 {
            ui.label(RichText::new("⬇").color(muted(ui)).size(TEXT_CAPTION))
                .on_hover_text(format!("{} commits to pull", status.behind));
        }
        if status.merged {
            tag(ui, "merged", muted(ui));
        }
    }
    let attention = repo.tabs.iter().filter_map(|t| tab_status(t, pool)).max();
    if let Some(attention) = attention.filter(|s| s.needs_attention()) {
        agent_status::pill(ui, attention);
    }
    ui.add_space(4.0);
}

/// Work-in-progress mark: Herdr's turning arc on every repo or tab with an agent at work.
pub fn busy_mark(ui: &mut Ui) {
    spinner(ui, 11.0);
}

/// `›`/`⌄` that shows or hides the repo's worktree rows.
pub fn tabs_chevron(ui: &mut Ui, repo: &Repo, actions: &mut Vec<Action>) {
    let (rect, _) = ui.allocate_exact_size(vec2(chevron::SIZE, chevron::SIZE), Sense::hover());
    // The mark is small; the target around it is not.
    let target = Rect::from_center_size(rect.center(), Vec2::splat(GLYPH_BUTTON_SIZE));
    let response = ui.interact(target, ui.auto_id_with("tabs-chevron"), Sense::click());
    let color = if response.hovered() { ui.visuals().text_color() } else { muted(ui) };
    chevron::paint(ui, rect, !repo.tabs_collapsed, color);
    if response.clicked() {
        actions.push(Action::ToggleRepoTabs(repo.id));
    }
}

/// `#123` tinted by CI state; hovering lists the checks, clicking opens the pull request.
fn pr_badge(ui: &mut Ui, pr: &PullRequest, actions: &mut Vec<Action>) {
    let color = match pr.ci() {
        Some(CheckState::Failing) => design::danger(ui),
        Some(CheckState::Pending) => ui.visuals().warn_fg_color,
        Some(CheckState::Passing) => design::success(ui),
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
