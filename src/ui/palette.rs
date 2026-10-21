use eframe::egui::{Align, Context, Id, Key, Layout, Modal, Modifiers, RichText, ScrollArea, TextEdit, Ui};

use super::Action;
use super::sidebar::tab_title;
use super::widgets::muted;
use crate::model::{AppState, PaneKind, SidePanel, SplitDir};
use crate::pane_layout::Nav;
use crate::session_pool::SessionPool;
use crate::worktree;

const MAX_RESULTS: usize = 12;

/// Command palette (⌘K): fuzzy-filters commands and tab switches, runs the chosen one.
#[derive(Default)]
pub struct Palette {
    query: String,
    selected: usize,
}

struct Item {
    label: String,
    hint: &'static str,
    action: Action,
}

impl Palette {
    /// A palette already filtered to `query`, e.g. "Prompt: ".
    pub fn with_query(query: &str) -> Self {
        Self { query: query.to_string(), selected: 0 }
    }

    /// Shows the palette; returns true when it should close.
    pub fn show(
        &mut self,
        ctx: &Context,
        state: &AppState,
        pool: &SessionPool,
        actions: &mut Vec<Action>,
    ) -> bool {
        let items = items(state, pool);
        let mut ranked: Vec<(i64, &Item)> =
            items.iter().filter_map(|i| score(&self.query, &i.label).map(|s| (s, i))).collect();
        ranked.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
        ranked.truncate(MAX_RESULTS);
        self.selected = self.selected.min(ranked.len().saturating_sub(1));

        let mut close = false;
        let (up, down, enter) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.key_pressed(Key::Enter),
            )
        });
        if up {
            self.selected = self.selected.saturating_sub(1);
        }
        if down && self.selected + 1 < ranked.len() {
            self.selected += 1;
        }
        let modal = Modal::new(Id::unique("palette")).show(ctx, |ui| {
            ui.set_width(520.0);
            let edit = ui.add(
                TextEdit::singleline(&mut self.query)
                    .hint_text("Type a command or tab name…")
                    .desired_width(f32::INFINITY)
                    .font(eframe::egui::FontId::proportional(16.0)),
            );
            edit.request_focus();
            if edit.changed() {
                self.selected = 0;
            }
            ui.add_space(6.0);
            ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                for (index, (_, item)) in ranked.iter().enumerate() {
                    if row(ui, item, index == self.selected) {
                        actions.push(item.action.clone());
                        close = true;
                    }
                }
                if ranked.is_empty() {
                    ui.label(RichText::new("No matches").color(muted(ui)));
                }
            });
        });
        if enter && let Some((_, item)) = ranked.get(self.selected) {
            actions.push(item.action.clone());
            close = true;
        }
        close || modal.should_close()
    }
}

fn row(ui: &mut Ui, item: &Item, selected: bool) -> bool {
    let response = ui
        .scope(|ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(&item.label).strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(item.hint).color(muted(ui)).size(11.5));
                });
            });
        })
        .response
        .interact(eframe::egui::Sense::click());
    if selected {
        ui.painter().rect_filled(response.rect, 4.0, ui.visuals().selection.bg_fill.gamma_multiply(0.4));
    }
    response.clicked()
}

fn items(state: &AppState, pool: &SessionPool) -> Vec<Item> {
    let mut items = Vec::new();
    let mut push = |label: &str, hint: &'static str, action: Action| {
        items.push(Item { label: label.to_string(), hint, action });
    };
    let selected = state.selected_repo.and_then(|id| state.repo(id));
    let active_tab = selected.and_then(|r| r.tabs.iter().find(|t| Some(t.id) == r.active_tab));
    if let Some(repo) = selected {
        push("New terminal", "⌘T", Action::NewTab { repo: repo.id, kind: PaneKind::Shell });
        push("New Claude session", "⇧⌘T", Action::NewTab { repo: repo.id, kind: PaneKind::Agent });
        push("Reveal repository in Finder", "", Action::RevealRepo(repo.id));
        push("New git worktree…", "", Action::OpenNewWorktree(repo.id));
        push("Worktree setup command…", "", Action::OpenRepoSetup(repo.id));
        if worktree::is_worktree(&repo.path) {
            push("Remove this worktree…", "", Action::OpenRemoveWorktree(repo.id));
        }
        push("Find in terminal", "⌘F", Action::OpenFind);
        push("Next tab", "⇧⌘]", Action::SelectAdjacentTab(1));
        push("Previous tab", "⇧⌘[", Action::SelectAdjacentTab(-1));
    }
    if let (Some(repo), Some(tab)) = (selected, active_tab) {
        push("Rename tab…", "", Action::OpenRenameTab { repo: repo.id, tab: tab.id });
        push("Close tab", "⇧⌘W", Action::CloseActiveTab);
        let index = repo.tabs.iter().position(|t| t.id == tab.id).unwrap_or(0);
        if index > 0 {
            push("Move tab left", "⌥⇧⌘←", Action::MoveTab { repo: repo.id, tab: tab.id, to: index - 1 });
        }
        if index + 1 < repo.tabs.len() {
            push("Move tab right", "⌥⇧⌘→", Action::MoveTab { repo: repo.id, tab: tab.id, to: index + 1 });
        }
        if let Some(pane) = tab.active_pane {
            push("Restart pane", "", Action::RestartPane(pane));
        }
    }
    push("Split pane right", "⌘D", Action::Split { dir: SplitDir::Right, kind: PaneKind::Shell });
    push("Split pane down", "⇧⌘D", Action::Split { dir: SplitDir::Down, kind: PaneKind::Shell });
    push("Split pane right with Claude", "", Action::Split { dir: SplitDir::Right, kind: PaneKind::Agent });
    push("Split pane down with Claude", "", Action::Split { dir: SplitDir::Down, kind: PaneKind::Agent });
    push("Close pane", "⌘W", Action::CloseActivePane);
    push("Zoom pane", "⇧⌘↩", Action::ToggleZoom);
    for (label, hint, nav) in [
        ("Focus pane left", "⌥⌘←", Nav::Left),
        ("Focus pane right", "⌥⌘→", Nav::Right),
        ("Focus pane up", "⌥⌘↑", Nav::Up),
        ("Focus pane down", "⌥⌘↓", Nav::Down),
    ] {
        push(label, hint, Action::FocusNeighbor(nav));
    }
    push("Reopen closed tab", "⌥⌘T", Action::ReopenClosedTab);
    push("Next pane needing attention", "⌃⌘N", Action::FocusNextAttention);
    push("Mark all notifications read", "", Action::MarkNoticesRead);
    push("Toggle files panel", "⌘B", Action::ToggleSidePanel);
    push("Show files", "", Action::SetSidePanel(SidePanel::Files));
    push("Show changes", "", Action::SetSidePanel(SidePanel::Changes));
    push("Settings", "⌘,", Action::OpenSettings);
    push("Show library", "", Action::SetSidePanel(SidePanel::Library));
    push("New prompt…", "", Action::OpenEditPrompt(None));
    for prompt in &state.settings.prompts {
        push(&format!("Prompt: {}", prompt.name), "prompt", Action::InsertPrompt(prompt.text.clone()));
    }
    push("New project", "", Action::OpenNewProject);
    for repo in state.projects.iter().flat_map(|p| &p.repos) {
        push(&format!("Go to {}", repo.name), "repo", Action::SelectRepo(repo.id));
        for tab in &repo.tabs {
            let title =
                tab_title(tab, pool, if tab.kind() == PaneKind::Agent { "Claude" } else { "Terminal" });
            push(
                &format!("{} › {title}", repo.name),
                "tab",
                Action::SelectTab { repo: repo.id, tab: tab.id },
            );
        }
    }
    items
}

/// Subsequence match score: higher for matches at word starts and consecutive characters;
/// `None` when `query` is not a subsequence of `text`. Empty queries match everything.
fn score(query: &str, text: &str) -> Option<i64> {
    let text: Vec<char> = text.to_lowercase().chars().collect();
    let mut score = 0;
    let mut last = None;
    for q in query.to_lowercase().chars().filter(|c| !c.is_whitespace()) {
        let from = last.map_or(0, |i| i + 1);
        let index = (from..text.len()).find(|i| text[*i] == q)?;
        score += 1;
        if index > 0 && last == Some(index - 1) {
            score += 3;
        }
        if index == 0 || !text[index - 1].is_alphanumeric() {
            score += 2;
        }
        last = Some(index);
    }
    Some(score - last.unwrap_or(0) as i64 / 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_subsequences_and_rejects_others() {
        assert!(score("nt", "New terminal") > score("nt", "Settings for tnt"));
        assert_eq!(score("xyz", "New terminal"), None);
        assert!(score("", "anything").is_some());
    }
}
