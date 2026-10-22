use eframe::egui::{TextEdit, Ui};

use super::controls::{card, section_label, segmented};
use crate::model::{AgentRenderer, Settings};

const COMMAND_WIDTH: f32 = 300.0;

pub fn show(ui: &mut Ui, settings: &mut Settings) {
    section_label(ui, "Claude");
    card(ui, |card| {
        card.row("New session command", "Runs in tabs opened with “New Claude session”.", |ui| {
            ui.add(
                TextEdit::singleline(&mut settings.agent_command).code_editor().desired_width(COMMAND_WIDTH),
            );
        });
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
            "Fullscreen: Claude scrolls its own transcript, a line per wheel step. Classic: Claude \
             writes into the pane's scrollback, which scrolls smoothly, is searchable, and stays \
             after Claude exits. Applies to sessions started from now on.",
            |ui| {
                segmented(
                    ui,
                    &mut settings.agent_renderer,
                    &[(AgentRenderer::Fullscreen, "Fullscreen"), (AgentRenderer::Classic, "Classic")],
                );
            },
        );
    });
}
