//! What the command palette offers: commands with their shortcuts, prompts, repos and tabs.

use super::Action;
use super::sidebar::tab_title;
use crate::keymap_hint;
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
    pub hint: String,
    pub kind: Kind,
    pub action: Action,
}

pub fn items(state: &AppState, pool: &SessionPool) -> Vec<Item> {
    let mut items = Vec::new();
    // `hint` is a keymap command whose key is shown, a fixed key such as "⌘F", or the kind of a
    // non-command item.
    let mut push = |label: &str, hint: &str, action: Action| {
        let (kind, hint) = match hint {
            "repo" => (Kind::Repo, String::new()),
            "tab" => (Kind::Tab, String::new()),
            "prompt" => (Kind::Prompt, String::new()),
            command if command.contains('.') => (Kind::Command, keymap_hint::hint(command)),
            keys => (Kind::Command, keys.to_string()),
        };
        items.push(Item { label: label.to_string(), hint, kind, action });
    };
    let selected = state.selected_repo.and_then(|id| state.repo(id));
    let active_tab = selected.and_then(|r| r.tabs.iter().find(|t| Some(t.id) == r.active_tab));
    if let Some(repo) = selected {
        push("New terminal", "tab.new", Action::NewTab { repo: repo.id, kind: PaneKind::Shell });
        push("New Claude session", "tab.new_agent", Action::NewTab { repo: repo.id, kind: PaneKind::Agent });
        push("Reveal repository in Finder", "", Action::RevealRepo(repo.id));
        push("New Claude session in worktree", "tab.new_agent_worktree", Action::NewWorktreeAgent(repo.id));
        push("New git worktree on a branch…", "", Action::OpenNewWorktree(repo.id));
        push("Worktree setup command…", "", Action::OpenRepoSetup(repo.id));
        if worktree::is_worktree(&repo.path) {
            push("Remove this worktree…", "", Action::OpenRemoveWorktree(repo.id));
        }
        match active_tab {
            Some(tab) if tab.file.is_some() => {
                push("Find in file", "⌘F", Action::OpenFind);
                push("Go to line…", "⌘L", Action::OpenGoToLine);
            }
            Some(tab) if !tab.is_document() => push("Find in terminal", "⌘F", Action::OpenFind),
            _ => {}
        }
        push("Open file…", "palette.files", Action::OpenQuickOpen);
        push("Next tab", "tab.next", Action::SelectAdjacentTab(1));
        push("Previous tab", "tab.previous", Action::SelectAdjacentTab(-1));
    }
    if let (Some(repo), Some(tab)) = (selected, active_tab) {
        push("Rename tab…", "", Action::OpenRenameTab { repo: repo.id, tab: tab.id });
        push("Close tab", "tab.close", Action::CloseActiveTab);
        let index = repo.tabs.iter().position(|t| t.id == tab.id).unwrap_or(0);
        if index > 0 {
            push(
                "Move tab left",
                "tab.move_left",
                Action::MoveTab { repo: repo.id, tab: tab.id, to: index - 1 },
            );
        }
        if index + 1 < repo.tabs.len() {
            push(
                "Move tab right",
                "tab.move_right",
                Action::MoveTab { repo: repo.id, tab: tab.id, to: index + 1 },
            );
        }
        if let Some(pane) = tab.active_pane {
            push("Restart pane", "", Action::RestartPane(pane));
        }
    }
    push(
        "Split pane right",
        "pane.split_right",
        Action::Split { dir: SplitDir::Right, kind: PaneKind::Shell },
    );
    push("Split pane down", "pane.split_down", Action::Split { dir: SplitDir::Down, kind: PaneKind::Shell });
    push("Split pane right with Claude", "", Action::Split { dir: SplitDir::Right, kind: PaneKind::Agent });
    push("Split pane down with Claude", "", Action::Split { dir: SplitDir::Down, kind: PaneKind::Agent });
    push("Close pane", "pane.close", Action::CloseActivePane);
    push("Zoom pane", "pane.zoom", Action::ToggleZoom);
    for (side, nav) in [("left", Nav::Left), ("right", Nav::Right), ("up", Nav::Up), ("down", Nav::Down)] {
        push(&format!("Focus pane {side}"), &format!("pane.focus_{side}"), Action::FocusNeighbor(nav));
        push(&format!("Swap pane {side}"), &format!("pane.swap_{side}"), Action::SwapNeighbor(nav));
    }
    push("Reopen closed tab", "tab.reopen", Action::ReopenClosedTab);
    push("Next pane needing attention", "attention.next", Action::FocusNextAttention);
    push("Mark all notifications read", "", Action::MarkNoticesRead);
    push("Toggle sidebar", "panel.toggle", Action::ToggleSidePanel);
    push("Show terminals", "view.terminals", Action::ShowCode(false));
    push("Show code", "view.code", Action::ShowCode(true));
    push("Search in files", "search.files", Action::OpenSearch);
    push("Format document", "editor.format", Action::FormatActiveFile);
    push("Split editor", "editor.split", Action::ToggleSplitEditor);
    push("Show changes", "", Action::SetSidePanel(SidePanel::Changes));
    push("Settings", "settings.open", Action::OpenSettings);
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
