use eframe::egui::{ComboBox, RichText, Ui};

use super::controls::{card, number_field, section_label};
use crate::font_catalog::{FontCatalog, SYSTEM_UI_FONT};
use crate::model::Settings;

const CONTROL_WIDTH: f32 = 260.0;

pub fn show(ui: &mut Ui, settings: &mut Settings, fonts: &FontCatalog) {
    section_label(ui, "Terminal font");
    card(ui, |card| {
        card.row("Font family", "Used by terminals and the editor. Monospace fonts only.", |ui| {
            font_picker(ui, "terminal-font", &mut settings.font_family, "SF Mono (system)", fonts, true);
        });
        card.row("Font size", "Size of terminal and editor text.", |ui| {
            number_field(ui, &mut settings.font_size, 8.0..=32.0, 0.5, " pt");
        });
    });

    section_label(ui, "Interface font");
    card(ui, |card| {
        card.row("Font family", "Used by the sidebar, tabs, menus and settings.", |ui| {
            font_picker(
                ui,
                "ui-font",
                &mut settings.ui_font_family,
                &format!("{SYSTEM_UI_FONT} (system)"),
                fonts,
                false,
            );
        });
        card.row("Font size", "Size of interface text; the terminal keeps its own size.", |ui| {
            number_field(ui, &mut settings.ui_font_size, 9.0..=24.0, 0.5, " pt");
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

/// Lists installed families; `monospace_only` hides proportional ones, which the terminal can't use.
fn font_picker(
    ui: &mut Ui,
    id: &str,
    family: &mut String,
    default_label: &str,
    catalog: &FontCatalog,
    monospace_only: bool,
) {
    let selected = if family.is_empty() { default_label.to_string() } else { family.clone() };
    ComboBox::from_id_salt(id).selected_text(selected).width(CONTROL_WIDTH).show_ui(ui, |ui| {
        ui.selectable_value(family, String::new(), default_label);
        match catalog.fonts() {
            None => {
                ui.label(RichText::new("Scanning installed fonts…").weak());
            }
            Some(fonts) => {
                for font in fonts.iter().filter(|f| f.monospace || !monospace_only) {
                    ui.selectable_value(family, font.family.clone(), &font.family);
                }
            }
        }
    });
}
