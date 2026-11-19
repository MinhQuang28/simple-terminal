use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};

use crate::app::RepodeckApp;
use crate::editor::EditorCommand;
use crate::ui::Action;
use crate::ui::palette::Palette;

/// Global commands whose keys the Code view uses for its editor (⌘D, ⌘⌥↑, ⌘⇧P…); ⌘W and the
/// rest still work there.
pub(crate) const CODE_TAKES: &[&str] = &["pane.split_", "pane.focus_", "pane.swap_", "palette.prompts"];

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
        for command in fired {
            actions.extend(self.command_actions(command).unwrap_or_default());
        }
        // ⌘1 and ⌘2 pick the view, so the tabs take Control.
        let ctrl = Modifiers::CTRL;
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
            if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(ctrl, key)))
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
}
