use eframe::egui::{TextEdit, Ui};

use super::controls::{card, section_label};
use crate::model::Settings;

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
    });
}
