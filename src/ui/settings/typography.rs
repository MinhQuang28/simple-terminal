use eframe::egui::{ComboBox, RichText, Ui};

use super::controls::{card, number_field, section_label};
use crate::font_catalog::FontCatalog;
use crate::model::Settings;

const SYSTEM_FONT_LABEL: &str = "SF Mono (system)";
const CONTROL_WIDTH: f32 = 260.0;

pub fn show(ui: &mut Ui, settings: &mut Settings, fonts: &FontCatalog) {
    section_label(ui, "Font");
    card(ui, |card| {
        card.row("Font family", "Used by terminals, the editor and the app interface.", |ui| {
            font_picker(ui, &mut settings.font_family, fonts);
        });
        card.row("Font size", "Size of terminal and editor text.", |ui| {
            number_field(ui, &mut settings.font_size, 8.0..=32.0, 0.5, " pt");
        });
    });

    section_label(ui, "Spacing");
    card(ui, |card| {
        card.row("Line height", "Vertical space per terminal line, relative to the font size.", |ui| {
            number_field(ui, &mut settings.line_height, 1.0..=2.0, 0.05, "×");
        });
        card.row("Letter spacing", "Extra horizontal space between characters.", |ui| {
            number_field(ui, &mut settings.letter_spacing, -1.0..=4.0, 0.25, " pt");
        });
        card.row("Padding", "Space between the terminal text and the pane edges.", |ui| {
            number_field(ui, &mut settings.padding, 0.0..=32.0, 1.0, " pt");
        });
    });
}

fn font_picker(ui: &mut Ui, family: &mut String, catalog: &FontCatalog) {
    let selected = if family.is_empty() { SYSTEM_FONT_LABEL.to_string() } else { family.clone() };
    ComboBox::from_id_salt("font-family").selected_text(selected).width(CONTROL_WIDTH).show_ui(ui, |ui| {
        ui.selectable_value(family, String::new(), SYSTEM_FONT_LABEL);
        match catalog.fonts() {
            None => {
                ui.label(RichText::new("Scanning installed fonts…").weak());
            }
            Some(fonts) => {
                for font in fonts {
                    ui.selectable_value(family, font.family.clone(), &font.family);
                }
            }
        }
    });
}
