//! What the command palette offers: commands with their shortcuts, prompts, repos and tabs.

use super::Action;
use super::sidebar::tab_title;
use crate::model::{AppState, PaneKind, SidePanel, SplitDir};
use crate::pane_layout::Nav;
use crate::session_pool::SessionPool;
use crate::worktree;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Command,
    Tab,
    Repo,
    Prompt,
    File,
}

#[derive(Clone)]
pub struct Item {
    pub label: String,
    /// The shortcut, for commands that have one.
    pub hint: &'static str,
    pub kind: Kind,
    pub action: Action,
}

pub fn items(state: &AppState, pool: &SessionPool) -> Vec<Item> {
    let mut items = Vec::new();
    let mut push = |label: &str, hint: &'static str, action: Action| {
        let (kind, hint) = match hint {
            "repo" => (Kind::Repo, ""),
            "tab" => (Kind::Tab, ""),
            "prompt" => (Kind::Prompt, ""),
            keys => (Kind::Command, keys),
        };
        items.push(Item { label: label.to_string(), hint, kind, action });
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
        push("Open file…", "⌘P", Action::OpenQuickOpen);
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
