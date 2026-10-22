use eframe::egui::{TextEdit, Ui};

use super::controls::{card, section_label, segmented, toggle};
use crate::model::{AgentRenderer, Settings};

const COMMAND_WIDTH: f32 = 300.0;

pub fn show(ui: &mut Ui, settings: &mut Settings) {
    section_label(ui, "Agent");
    card(ui, |card| {
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
}
