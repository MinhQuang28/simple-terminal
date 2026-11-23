//! What each named app command does (the names in `keymap::DEFAULTS`), shared by the keyboard
//! shortcuts and `repodeck do <command>`. Commands act on the selected repo and its active tab,
//! as a key press would.

use crate::app::RepodeckApp;
use crate::model::{PaneKind, SplitDir};
use crate::pane_layout::Nav;
use crate::ui::Action;
use crate::ui::palette::Palette;

const NAVS: [(&str, Nav); 4] =
    [("left", Nav::Left), ("right", Nav::Right), ("up", Nav::Up), ("down", Nav::Down)];

impl RepodeckApp {
    /// The actions `command` stands for; `None` when it isn't an app command (the editor's own
    /// commands go to the editor instead). Commands that only change the palette or font size
    /// apply here and return no actions.
    pub(crate) fn command_actions(&mut self, command: &str) -> Option<Vec<Action>> {
        let repo = self.state.selected_repo;
        let actions = match command {
            "palette.toggle" => {
                self.palette = if self.palette.is_none() { Some(Palette::default()) } else { None };
                vec![]
            }
            "palette.prompts" => {
                self.palette = Some(Palette::with_query("Prompt: "));
                vec![]
            }
            "palette.files" => {
                if repo.is_some() {
                    self.palette = Some(Palette::files());
                }
                vec![]
            }
            "tab.move_left" => self.active_tab_move(-1).into_iter().collect(),
            "tab.move_right" => self.active_tab_move(1).into_iter().collect(),
            "tab.reopen" => vec![Action::ReopenClosedTab],
            "tab.previous" => vec![Action::SelectAdjacentTab(-1)],
            "tab.next" => vec![Action::SelectAdjacentTab(1)],
            "tab.new_agent_worktree" => repo.map(Action::NewWorktreeAgent).into_iter().collect(),
            "tab.new_agent" => {
                repo.map(|repo| Action::NewTab { repo, kind: PaneKind::Agent }).into_iter().collect()
            }
            "tab.new" => {
                let kind = self.state.settings.new_tab;
                repo.map(|repo| Action::NewTab { repo, kind }).into_iter().collect()
            }
            "tab.close" => vec![Action::CloseActiveTab],
            "pane.zoom" => vec![Action::ToggleZoom],
            "pane.split_down" => vec![Action::Split { dir: SplitDir::Down, kind: PaneKind::Shell }],
            "pane.split_right" => vec![Action::Split { dir: SplitDir::Right, kind: PaneKind::Shell }],
            "pane.close" => vec![self.close_action()],
            "attention.next" => vec![Action::FocusNextAttention],
            "file.save" => vec![Action::SaveActiveFile],
            "panel.toggle" => vec![Action::ToggleSidePanel],
            "search.files" => vec![Action::OpenSearch],
            "view.terminals" => vec![Action::ShowCode(false)],
            "view.code" => vec![Action::ShowCode(true)],
            "settings.open" => vec![Action::OpenSettings],
            "font.bigger" | "font.smaller" => {
                let delta = if command == "font.bigger" { 1.0 } else { -1.0 };
                let size = &mut self.state.settings.font_size;
                *size = (*size + delta).clamp(8.0, 32.0);
                self.dirty = true;
                vec![]
            }
            other => {
                let (verb, side) = other.rsplit_once('_')?;
                let nav = NAVS.iter().find(|(name, _)| *name == side)?.1;
                match verb {
                    "pane.swap" => vec![Action::SwapNeighbor(nav)],
                    "pane.focus" => vec![Action::FocusNeighbor(nav)],
                    _ => return None,
                }
            }
        };
        Some(actions)
    }

    /// ⌘W: closes the active file tab, or else the active pane.
    fn close_action(&self) -> Action {
        let repo = self.state.selected_repo;
        let active_file_tab = repo
            .and_then(|id| self.state.repo(id))
            .and_then(|r| r.tabs.iter().find(|t| Some(t.id) == r.active_tab && t.is_document()));
        match (repo, active_file_tab) {
            (Some(repo), Some(tab)) => Action::CloseTab { repo, tab: tab.id },
            _ => Action::CloseActivePane,
        }
    }

    fn active_tab_move(&self, offset: i32) -> Option<Action> {
        let repo = self.state.repo(self.state.selected_repo?)?;
        let tab = repo.active_tab?;
        let tabs = repo.view_tabs().count() as i32;
        let index = repo.view_tabs().position(|t| t.id == tab)? as i32;
        let to = (index + offset).clamp(0, tabs - 1) as usize;
        Some(Action::MoveTab { repo: repo.id, tab, to })
    }
}

/// App commands `repodeck do` accepts: every bindable command except the editor's.
pub fn names() -> impl Iterator<Item = &'static str> {
    crate::keymap::DEFAULTS.iter().map(|(name, _)| *name).filter(|name| !name.starts_with("editor."))
}
