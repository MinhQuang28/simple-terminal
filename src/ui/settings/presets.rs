//! Launch presets: commands offered in each repo's new-tab menu, such as a dev server or a
//! second agent beside Claude.

use eframe::egui::{Button, RichText, TextEdit, Ui};

use super::controls::{card, section_label};
use crate::model::{LaunchPreset, Settings};
use crate::ui::widgets::muted;

const NAME_WIDTH: f32 = 130.0;
const COMMAND_WIDTH: f32 = 240.0;

pub fn show(ui: &mut Ui, settings: &mut Settings) {
    section_label(ui, "Launch presets");
    card(ui, |card| {
        card.block(|ui| {
            ui.label(
                RichText::new(
                    "Shown in each repo's + menu. A terminal preset runs its command in a new shell \
                     tab; an agent preset starts it as an agent (Codex, OpenCode, Claude with \
                     flags), with status in the sidebar.",
                )
                .color(muted(ui)),
            );
            ui.add_space(8.0);
            let mut remove = None;
            for (i, preset) in settings.presets.iter_mut().enumerate() {
                ui.push_id(i, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(
                            TextEdit::singleline(&mut preset.name)
                                .hint_text("Name")
                                .desired_width(NAME_WIDTH),
                        );
                        ui.add(
                            TextEdit::singleline(&mut preset.command)
                                .hint_text("npm run dev")
                                .code_editor()
                                .desired_width(COMMAND_WIDTH),
                        );
                        ui.checkbox(&mut preset.agent, "Agent");
                        if ui.add(Button::new("×").frame(false)).on_hover_text("Remove").clicked() {
                            remove = Some(i);
                        }
                    });
                });
            }
            if let Some(i) = remove {
                settings.presets.remove(i);
            }
            if ui.button("Add preset").clicked() {
                settings.presets.push(LaunchPreset::default());
            }
        });
    });
}
