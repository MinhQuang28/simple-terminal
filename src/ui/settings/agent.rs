use eframe::egui::{TextEdit, Ui};

use super::controls::{card, section_label, segmented, toggle};
use super::presets;
use crate::model::{AgentRenderer, PaneKind, Settings};

const COMMAND_WIDTH: f32 = 300.0;
/// Agents offered as the provider: (program, new session command, restored session command).
const PROVIDERS: &[(&str, &str, &str)] = &[
    ("claude", "claude", "claude --continue || claude"),
    ("codex", "codex", "codex resume --last || codex"),
    ("gemini", "gemini", "gemini"),
];
const PANE_KINDS: [(PaneKind, &str); 2] = [(PaneKind::Shell, "Terminal"), (PaneKind::Agent, "Claude Code")];

pub fn show(ui: &mut Ui, settings: &mut Settings) {
    section_label(ui, "Agent");
    card(ui, |card| {
        card.row(
            "Provider",
            "The agent a new session starts. Choosing one fills the two commands below, which stay \
             editable. Gemini runs without status in the sidebar.",
            |ui| {
                let first = settings.agent_command.split_whitespace().next().unwrap_or_default();
                let current = PROVIDERS.iter().position(|(name, _, _)| *name == first);
                let mut picked = current.unwrap_or(PROVIDERS.len());
                let options = [(0, "Claude"), (1, "Codex"), (2, "Gemini"), (PROVIDERS.len(), "Custom")];
                if segmented(ui, &mut picked, &options)
                    && let Some((_, start, resume)) = PROVIDERS.get(picked)
                {
                    settings.agent_command = (*start).into();
                    settings.agent_resume_command = (*resume).into();
                }
            },
        );
        card.row(
            "New session command",
            "Runs in tabs opened with “New Claude session”. Claude Code, Codex and OpenCode are \
             recognised: their status, prompts and replies reach the sidebar and inbox.",
            |ui| {
                ui.add(
                    TextEdit::singleline(&mut settings.agent_command)
                        .code_editor()
                        .desired_width(COMMAND_WIDTH),
                );
            },
        );
        card.row(
            "Restored session command",
            "Runs for Claude tabs restored after restarting Repodeck, to resume their conversation.",
            |ui| {
                ui.add(
                    TextEdit::singleline(&mut settings.agent_resume_command)
                        .code_editor()
                        .desired_width(COMMAND_WIDTH),
                );
            },
        );
        card.row(
            "New tab opens",
            "What the new-tab key starts: a terminal or Claude Code. The other stays in the tab \
             strip's + menu.",
            |ui| {
                segmented(ui, &mut settings.new_tab, &PANE_KINDS);
            },
        );
        card.row(
            "Opening a repository starts",
            "What starts in a repository shown with nothing running in it: the first click on it \
             in the sidebar, or right after adding it.",
            |ui| {
                segmented(ui, &mut settings.repo_opens, &PANE_KINDS);
            },
        );
        card.row(
            "Renderer",
            "Classic: Claude writes into the pane's scrollback, which scrolls smoothly, is \
             searchable, and stays after Claude exits. Fullscreen: Claude scrolls its own \
             transcript, a line per wheel step. Applies to sessions started from now on.",
            |ui| {
                segmented(
                    ui,
                    &mut settings.agent_renderer,
                    &[(AgentRenderer::Classic, "Classic"), (AgentRenderer::Fullscreen, "Fullscreen")],
                );
            },
        );
        card.row(
            "Prompt at the bottom",
            "Draws the pane's content against its bottom edge, so with the Classic renderer \
             Claude's prompt box stays put while the conversation above it grows.",
            |ui| {
                toggle(ui, &mut settings.agent_prompt_at_bottom);
            },
        );
        card.row(
            "Use subscription",
            "Hides ANTHROPIC_API_KEY and ANTHROPIC_AUTH_TOKEN from Claude so it signs in with your \
             Claude subscription instead of billing an API key per token. Turn off to keep using \
             a key or a gateway from your shell. Applies to sessions started from now on.",
            |ui| {
                toggle(ui, &mut settings.agent_hide_api_keys);
            },
        );
    });
    presets::show(ui, settings);
}
