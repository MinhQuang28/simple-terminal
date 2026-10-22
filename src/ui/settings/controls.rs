use eframe::egui::{
    Align, Color32, CornerRadius, Frame, Layout, Margin, Response, RichText, Sense, Stroke, Ui, vec2,
};

use super::fields::steady_hover;
use crate::ui::widgets::muted;

/// Space above and below each row's content.
const ROW_PADDING_Y: f32 = 14.0;
/// Height of the band a row's title shares with its control, so a switch, a field or a segmented
/// group all sit level with the title's line whatever the description below does.
const CONTROL_BAND: f32 = 32.0;
const SEGMENT_WIDTH: f32 = 64.0;
const SEGMENT_HEIGHT: f32 = 26.0;
const CARD_PADDING_X: i8 = 20;

/// Uppercase caption above a card, e.g. "THEME".
pub fn section_label(ui: &mut Ui, text: &str) {
    ui.add_space(22.0);
    ui.label(RichText::new(text.to_uppercase()).size(11.5).strong().color(muted(ui)));
    ui.add_space(8.0);
}

/// A rounded group of settings rows separated by hairlines.
pub struct Card<'a> {
    ui: &'a mut Ui,
    rows: usize,
}

impl Card<'_> {
    /// A row with a title and description on the left and `control` on the right.
    pub fn row(&mut self, title: &str, description: &str, control: impl FnOnce(&mut Ui)) {
        self.separator();
        let ui = &mut *self.ui;
        ui.add_space(ROW_PADDING_Y);
        ui.horizontal_top(|ui| {
            let text_width = (ui.available_width() * 0.55).max(220.0);
            ui.allocate_ui_with_layout(vec2(text_width, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.allocate_ui_with_layout(
                    vec2(text_width, CONTROL_BAND),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.label(RichText::new(title).size(14.0).color(ui.visuals().strong_text_color()));
                    },
                );
                if !description.is_empty() {
                    ui.label(RichText::new(description).size(12.5).color(muted(ui)));
                }
            });
            let control_width = ui.available_width();
            ui.allocate_ui_with_layout(
                vec2(control_width, CONTROL_BAND),
                Layout::right_to_left(Align::Center),
                control,
            );
        });
        ui.add_space(ROW_PADDING_Y);
    }

    /// Free-form content spanning the card's width.
    pub fn block(&mut self, add_contents: impl FnOnce(&mut Ui)) {
        self.separator();
        self.ui.add_space(14.0);
        add_contents(self.ui);
        self.ui.add_space(14.0);
    }

    fn separator(&mut self) {
        if self.rows > 0 {
            let full = self.ui.max_rect().expand2(vec2(f32::from(CARD_PADDING_X), 0.0));
            let y = self.ui.cursor().top();
            let stroke = Stroke::new(1.0, border(self.ui));
            self.ui.painter().hline(full.x_range(), y, stroke);
        }
        self.rows += 1;
    }
}

pub fn card(ui: &mut Ui, add_rows: impl FnOnce(&mut Card<'_>)) {
    let visuals = ui.visuals();
    let fill = visuals.extreme_bg_color.lerp_to_gamma(visuals.text_color(), 0.035);
    let inner_width = ui.available_width() - 2.0 * f32::from(CARD_PADDING_X) - 2.0;
    Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, border(ui)))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::symmetric(CARD_PADDING_X, 0))
        .show(ui, |ui| {
            ui.set_width(inner_width);
            add_rows(&mut Card { ui, rows: 0 });
        });
}

fn border(ui: &Ui) -> Color32 {
    ui.visuals().text_color().gamma_multiply(0.12)
}

/// Joined buttons where exactly one option is selected, like "Block | Bar | Underline".
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str)]) -> bool {
    let mut changed = false;
    Frame::new()
        .stroke(Stroke::new(1.0, border(ui)))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(3))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            steady_hover(ui);
            // Rows lay controls out right-to-left; keep the options in reading order and stop the
            // group from growing to the row's full size.
            let width = options.len() as f32 * (SEGMENT_WIDTH + 2.0) - 2.0;
            ui.allocate_ui_with_layout(
                vec2(width, SEGMENT_HEIGHT),
                Layout::left_to_right(Align::Center),
                |ui| {
                    for &(option, label) in options {
                        let selected = *value == option;
                        let text = if selected {
                            RichText::new(label).color(ui.visuals().strong_text_color())
                        } else {
                            RichText::new(label).color(muted(ui))
                        };
                        let fill =
                            if selected { ui.visuals().selection.bg_fill } else { Color32::TRANSPARENT };
                        let button = eframe::egui::Button::new(text)
                            .fill(fill)
                            .stroke(Stroke::NONE)
                            .corner_radius(CornerRadius::same(6))
                            .min_size(vec2(SEGMENT_WIDTH, SEGMENT_HEIGHT));
                        if ui.add(button).clicked() && !selected {
                            *value = option;
                            changed = true;
                        }
                    }
                },
            );
        });
    changed
}

/// An iOS-style on/off switch.
pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let size = vec2(36.0, 20.0);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(response.id, *on);
    let off_fill = ui.visuals().text_color().gamma_multiply(0.18);
    let fill = off_fill.lerp_to_gamma(ui.visuals().hyperlink_color, t);
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(10), fill);
    let x = eframe::egui::lerp(rect.left() + 10.0..=rect.right() - 10.0, t);
    painter.circle_filled(eframe::egui::pos2(x, rect.center().y), 8.0, Color32::WHITE);
    response
}

#[cfg(test)]
#[path = "controls_tests.rs"]
mod tests;
