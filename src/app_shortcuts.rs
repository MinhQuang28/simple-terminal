use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};

use crate::app::RepodeckApp;
use crate::model::{PaneKind, SplitDir};
use crate::pane_layout::Nav;
use crate::ui::Action;
use crate::ui::palette::Palette;

impl RepodeckApp {
    pub(crate) fn handle_shortcuts(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        if self.dialog.is_some() {
            return;
        }
        let cmd = Modifiers::COMMAND;
        let cmd_shift = Modifiers::COMMAND | Modifiers::SHIFT;
        let cmd_alt = Modifiers::COMMAND | Modifiers::ALT;
        let cmd_alt_shift = Modifiers::COMMAND | Modifiers::ALT | Modifiers::SHIFT;
        let pressed = |mods, key| ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(mods, key)));
        let repo = self.state.selected_repo;

        if pressed(cmd, Key::K) {
            self.palette = if self.palette.is_none() { Some(Palette::default()) } else { None };
        }
        if self.palette.is_some() {
            return;
        }
        if pressed(cmd_alt_shift, Key::ArrowLeft) {
            actions.extend(self.active_tab_move(-1));
        }
        if pressed(cmd_alt_shift, Key::ArrowRight) {
            actions.extend(self.active_tab_move(1));
        }
        for (key, nav) in [
            (Key::ArrowLeft, Nav::Left),
            (Key::ArrowRight, Nav::Right),
            (Key::ArrowUp, Nav::Up),
            (Key::ArrowDown, Nav::Down),
        ] {
            if pressed(cmd_alt, key) {
                actions.push(Action::FocusNeighbor(nav));
            }
        }
        if pressed(cmd_alt, Key::T) {
            actions.push(Action::ReopenClosedTab);
        }
        if pressed(cmd_shift, Key::Enter) {
            actions.push(Action::ToggleZoom);
        }
        if pressed(cmd_shift, Key::OpenBracket) {
            actions.push(Action::SelectAdjacentTab(-1));
        }
        if pressed(cmd_shift, Key::CloseBracket) {
            actions.push(Action::SelectAdjacentTab(1));
        }

        // Shift variants first: consume_shortcut treats a missing Shift as "don't care".
        if pressed(cmd_shift, Key::T) {
            actions.extend(repo.map(|repo| Action::NewTab { repo, kind: PaneKind::Agent }));
        }
        if pressed(cmd_shift, Key::D) {
            actions.push(Action::Split { dir: SplitDir::Down, kind: PaneKind::Shell });
        }
        if pressed(cmd_shift, Key::P) {
            self.palette = Some(Palette::with_query("Prompt: "));
        }
        if pressed(cmd_shift, Key::W) {
            actions.push(Action::CloseActiveTab);
        }
        if pressed(Modifiers::COMMAND | Modifiers::CTRL, Key::N) {
            actions.push(Action::FocusNextAttention);
        }
        if pressed(cmd, Key::T) {
            actions.extend(repo.map(|repo| Action::NewTab { repo, kind: PaneKind::Shell }));
        }
        if pressed(cmd, Key::P) && repo.is_some() {
            self.palette = Some(Palette::files());
        }
        if pressed(cmd, Key::D) {
            actions.push(Action::Split { dir: SplitDir::Right, kind: PaneKind::Shell });
        }
        if pressed(cmd, Key::W) {
            let active_file_tab = repo
                .and_then(|id| self.state.repo(id))
                .and_then(|r| r.tabs.iter().find(|t| Some(t.id) == r.active_tab && t.is_document()));
            actions.push(match (repo, active_file_tab) {
                (Some(repo), Some(tab)) => Action::CloseTab { repo, tab: tab.id },
                _ => Action::CloseActivePane,
            });
        }
        if pressed(cmd, Key::S) {
            actions.push(Action::SaveActiveFile);
        }
        if pressed(cmd, Key::B) {
            actions.push(Action::ToggleSidePanel);
        }
        if pressed(cmd, Key::Comma) {
            actions.push(Action::OpenSettings);
        }
        let font_delta = if pressed(cmd, Key::Equals) || pressed(cmd, Key::Plus) {
            1.0
        } else if pressed(cmd, Key::Minus) {
            -1.0
        } else {
            0.0
        };
        if font_delta != 0.0 {
            let size = &mut self.state.settings.font_size;
            *size = (*size + font_delta).clamp(8.0, 32.0);
            self.dirty = true;
        }
        let digits = [
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
            Key::Num8,
            Key::Num9,
        ];
        for (i, key) in digits.into_iter().enumerate() {
            if pressed(cmd, key)
                && let Some(repo) = repo.and_then(|id| self.state.repo(id))
                && let Some(tab) = repo.tabs.get(i)
            {
                actions.push(Action::SelectTab { repo: repo.id, tab: tab.id });
            }
        }
    }

    fn active_tab_move(&self, offset: i32) -> Option<Action> {
        let repo = self.state.repo(self.state.selected_repo?)?;
        let index = repo.tabs.iter().position(|t| Some(t.id) == repo.active_tab)?;
        let to = (index as i32 + offset).clamp(0, repo.tabs.len() as i32 - 1) as usize;
        Some(Action::MoveTab { repo: repo.id, tab: repo.tabs[index].id, to })
    }
}
