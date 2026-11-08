use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};

use crate::app::RepodeckApp;
use crate::editor::EditorCommand;
use crate::model::{PaneKind, SplitDir};
use crate::pane_layout::Nav;
use crate::ui::Action;
use crate::ui::palette::Palette;

/// Global commands whose keys the Code view uses for its editor (⌘D, ⌘⌥↑, ⌘⇧P…); ⌘W and the
/// rest still work there.
const CODE_TAKES: &[&str] = &["pane.split_", "pane.focus_", "pane.swap_", "palette.prompts"];

impl RepodeckApp {
    pub(crate) fn handle_shortcuts(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        // The peek overlay covers the workspace; shortcuts would act on the tab beneath it.
        if self.dialog.is_some() || self.peek.is_some() {
            return;
        }
        let repo = self.state.selected_repo;

        // ⌘K without Shift clears the focused terminal (handled by the pane).
        let keys = crate::keymap_hint::active();
        let code = self.code_view();
        // The Code view's editor keys go first, so ⌘⇧K deletes a line and ⌘D selects the next match
        // there instead of opening the palette or splitting a pane.
        if code {
            self.code_shortcuts(ctx, actions);
        }
        if keys.pressed(ctx, "palette.toggle") {
            self.palette = if self.palette.is_none() { Some(Palette::default()) } else { None };
        }
        if self.palette.is_some() {
            return;
        }
        // Pane keys have no panes to act on in the Code view, and editor keys none in the terminals.
        let skip: Vec<&str> = crate::keymap::DEFAULTS
            .iter()
            .map(|(command, _)| *command)
            .filter(|command| {
                *command == "palette.toggle"
                    || command.starts_with("editor.")
                    || (code && CODE_TAKES.iter().any(|taken| command.starts_with(taken)))
            })
            .collect();
        let fired = keys.fired(ctx, &skip);
        let pressed = |command: &str| fired.contains(&command);
        if pressed("tab.move_left") {
            actions.extend(self.active_tab_move(-1));
        }
        if pressed("tab.move_right") {
            actions.extend(self.active_tab_move(1));
        }
        let navs = [("left", Nav::Left), ("right", Nav::Right), ("up", Nav::Up), ("down", Nav::Down)];
        for (side, nav) in navs {
            if pressed(&format!("pane.swap_{side}")) {
                actions.push(Action::SwapNeighbor(nav));
            }
        }
        for (side, nav) in navs {
            if pressed(&format!("pane.focus_{side}")) {
                actions.push(Action::FocusNeighbor(nav));
            }
        }
        if pressed("tab.reopen") {
            actions.push(Action::ReopenClosedTab);
        }
        if pressed("pane.zoom") {
            actions.push(Action::ToggleZoom);
        }
        if pressed("tab.previous") {
            actions.push(Action::SelectAdjacentTab(-1));
        }
        if pressed("tab.next") {
            actions.push(Action::SelectAdjacentTab(1));
        }
        if pressed("tab.new_agent") {
            actions.extend(repo.map(|repo| Action::NewTab { repo, kind: PaneKind::Agent }));
        }
        if pressed("pane.split_down") {
            actions.push(Action::Split { dir: SplitDir::Down, kind: PaneKind::Shell });
        }
        if pressed("palette.prompts") {
            self.palette = Some(Palette::with_query("Prompt: "));
        }
        if pressed("tab.close") {
            actions.push(Action::CloseActiveTab);
        }
        if pressed("attention.next") {
            actions.push(Action::FocusNextAttention);
        }
        if pressed("tab.new") {
            actions.extend(repo.map(|repo| Action::NewTab { repo, kind: PaneKind::Shell }));
        }
        if pressed("palette.files") && repo.is_some() {
            self.palette = Some(Palette::files());
        }
        if pressed("pane.split_right") {
            actions.push(Action::Split { dir: SplitDir::Right, kind: PaneKind::Shell });
        }
        if pressed("pane.close") {
            let active_file_tab = repo
                .and_then(|id| self.state.repo(id))
                .and_then(|r| r.tabs.iter().find(|t| Some(t.id) == r.active_tab && t.is_document()));
            actions.push(match (repo, active_file_tab) {
                (Some(repo), Some(tab)) => Action::CloseTab { repo, tab: tab.id },
                _ => Action::CloseActivePane,
            });
        }
        if pressed("file.save") {
            actions.push(Action::SaveActiveFile);
        }
        if pressed("panel.toggle") {
            actions.push(Action::ToggleSidePanel);
        }
        if pressed("search.files") {
            actions.push(Action::OpenSearch);
        }
        if pressed("view.terminals") {
            actions.push(Action::ShowCode(false));
        }
        if pressed("view.code") {
            actions.push(Action::ShowCode(true));
        }
        if pressed("settings.open") {
            actions.push(Action::OpenSettings);
        }
        let font_delta = if pressed("font.bigger") {
            1.0
        } else if pressed("font.smaller") {
            -1.0
        } else {
            0.0
        };
        if font_delta != 0.0 {
            let size = &mut self.state.settings.font_size;
            *size = (*size + font_delta).clamp(8.0, 32.0);
            self.dirty = true;
        }
        let cmd = Modifiers::COMMAND;
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
            if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(cmd, key)))
                && let Some(repo) = repo.and_then(|id| self.state.repo(id))
                && let Some(tab) = repo.view_tabs().nth(i)
            {
                actions.push(Action::SelectTab { repo: repo.id, tab: tab.id });
            }
        }
    }

    /// The Code view's keys: editor commands for the editor, and the view's own: format, split,
    /// go to line, the command palette.
    fn code_shortcuts(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        let keys = crate::keymap_hint::active();
        if self.palette.is_some() {
            if keys.pressed(ctx, "editor.command_palette") {
                self.palette = None;
            }
            return;
        }
        let skip: Vec<&str> = crate::keymap::DEFAULTS
            .iter()
            .map(|(command, _)| *command)
            .filter(|command| !command.starts_with("editor."))
            .collect();
        for command in keys.fired(ctx, &skip) {
            match command {
                "editor.format" => actions.push(Action::FormatActiveFile),
                "editor.go_to_line" => actions.push(Action::OpenGoToLine),
                "editor.split" => actions.push(Action::ToggleSplitEditor),
                "editor.command_palette" => self.palette = Some(Palette::default()),
                other => self.editor_commands.extend(EditorCommand::from_name(other)),
            }
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
