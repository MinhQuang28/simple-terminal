use eframe::egui::{Button, Color32, RichText, Ui, vec2};

use super::Action;
use super::agent_status;
use super::sidebar::{tab_busy, tab_status};
use super::widgets::{accent, ago, muted, spinner, tag};
use crate::git_status::GitStatus;
use crate::model::Repo;
use crate::pr_status::{CheckState, PullRequest};
use crate::session_pool::SessionPool;

/// What the sidebar rows of one repo show.
pub struct RepoRow<'a> {
    pub repo: &'a Repo,
    pub status: Option<&'a GitStatus>,
    pub pr: Option<&'a PullRequest>,
    pub pool: &'a SessionPool,
    pub now: u64,
}

/// A repo's activity, right to left: recency, pull request, spinner, sync state, merged, attention.
/// Call inside a right-to-left layout.
pub fn indicators(ui: &mut Ui, row: &RepoRow, actions: &mut Vec<Action>) {
    let RepoRow { repo, status, pr, pool, now } = *row;
    ui.spacing_mut().item_spacing.x = 8.0;
    ui.label(RichText::new(ago(repo.last_active(), now)).color(muted(ui)).size(11.5));
    if let Some(pr) = pr {
        pr_badge(ui, pr, actions);
    }
    if repo.tabs.iter().any(|t| tab_busy(t, pool)) {
        busy_mark(ui);
    }
    if let Some(status) = status {
        if status.ahead > 0 {
            ui.label(RichText::new("⬆").color(accent(ui)).size(11.5))
                .on_hover_text(format!("{} commits to push", status.ahead));
        }
        if status.behind > 0 {
            ui.label(RichText::new("⬇").color(muted(ui)).size(11.5))
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

/// Work-in-progress mark: a pulsing dot on every repo or tab with an agent at work.
pub fn busy_mark(ui: &mut Ui) {
    spinner(ui, 11.0);
}

/// `›`/`⌄` that shows or hides the repo's terminal rows.
pub fn tabs_chevron(ui: &mut Ui, repo: &Repo, actions: &mut Vec<Action>) {
    let glyph = if repo.tabs_collapsed { "›" } else { "⌄" };
    let chevron =
        Button::new(RichText::new(glyph).color(muted(ui)).size(13.0)).frame(false).min_size(vec2(14.0, 14.0));
    if ui.add(chevron).clicked() {
        actions.push(Action::ToggleRepoTabs(repo.id));
    }
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
