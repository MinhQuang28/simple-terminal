use eframe::egui::{Align2, CornerRadius, FontId, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use super::controls::{card, section_label};
use crate::model::Settings;
use crate::theme::{Hex, Theme, presets};

const CARD_SIZE: [f32; 2] = [164.0, 92.0];
const CUSTOM_NAME: &str = "Custom";

pub fn show(ui: &mut Ui, settings: &mut Settings) {
    let theme = &mut settings.theme;
    section_label(ui, "Theme");
    card(ui, |card| {
        card.block(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(12.0, 12.0);
                for preset in presets() {
                    let selected = theme.name == preset.name;
                    if theme_card(ui, &preset, selected).clicked() {
                        *theme = preset;
                    }
                }
            });
        });
    });

    section_label(ui, "Colors");
    let mut edited = false;
    card(ui, |card| {
        for (title, description, color) in [
            ("Background", "Terminal and editor background.", &mut theme.background),
            ("Foreground", "Default text color.", &mut theme.foreground),
            ("Cursor", "Color of the terminal cursor.", &mut theme.cursor),
            ("Selection", "Highlight behind selected text.", &mut theme.selection),
        ] {
            card.row(title, description, |ui| edited |= color_button(ui, color));
        }
        let ansi = &mut theme.ansi;
        card.row("ANSI palette", "The 16 colors programs use. Top row normal, bottom row bright.", |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().interact_size = vec2(24.0, 18.0);
                ui.add_space(8.0);
                for row in [0, 8] {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        for color in &mut ansi[row..row + 8] {
                            edited |= color_button(ui, color);
                        }
                    });
                }
                ui.add_space(8.0);
            });
        });
    });
    if edited {
        theme.name = CUSTOM_NAME.into();
    }

    section_label(ui, "Reset");
    card(ui, |card| {
        let description = "Reset theme, fonts, spacing and cursor. Shell and agent commands are kept.";
        card.row("Restore defaults", description, |ui| {
            if ui.button("Restore defaults").clicked() {
                *settings = Settings {
                    shell: std::mem::take(&mut settings.shell),
                    agent_command: std::mem::take(&mut settings.agent_command),
                    agent_resume_command: std::mem::take(&mut settings.agent_resume_command),
                    ..Settings::default()
                };
            }
        });
    });
}

fn color_button(ui: &mut Ui, color: &mut Hex) -> bool {
    ui.color_edit_button_srgb(&mut color.0).changed()
}

/// A miniature terminal preview of `theme`, like Warp's theme picker.
fn theme_card(ui: &mut Ui, theme: &Theme, selected: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(CARD_SIZE.into(), Sense::click());
    let painter = ui.painter_at(rect);
    let radius = CornerRadius::same(10);
    painter.rect_filled(rect, radius, theme.background.color());

    let mono = FontId::monospace(11.0);
    let left = rect.left() + 10.0;
    painter.text(
        rect.min + vec2(10.0, 10.0),
        Align2::LEFT_TOP,
        "~/repo",
        mono.clone(),
        theme.ansi[4].color(),
    );
    painter.text(rect.min + vec2(58.0, 10.0), Align2::LEFT_TOP, "main", mono.clone(), theme.ansi[2].color());
    painter.text(rect.min + vec2(10.0, 26.0), Align2::LEFT_TOP, "❯ ls -la", mono, theme.foreground.color());
    for i in 0..8 {
        let center = pos2(left + 6.0 + i as f32 * 15.0, rect.top() + 52.0);
        painter.circle_filled(center, 5.0, theme.ansi[i + 8].color());
    }
    painter.text(
        pos2(left, rect.bottom() - 10.0),
        Align2::LEFT_BOTTOM,
        &theme.name,
        FontId::proportional(12.0),
        theme.foreground.color(),
    );

    let stroke = if selected {
        Stroke::new(2.0, ui.visuals().hyperlink_color)
    } else if response.hovered() {
        Stroke::new(1.0, ui.visuals().text_color().gamma_multiply(0.5))
    } else {
        Stroke::new(1.0, ui.visuals().text_color().gamma_multiply(0.12))
    };
    painter.rect_stroke(rect, radius, stroke, StrokeKind::Inside);
    response.on_hover_text(&theme.name)
}
