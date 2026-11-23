//! The status bar's model label for the active Claude Code pane, and the popup it opens to
//! switch that session to another model.

use eframe::egui::{
    Align, Align2, FontId, Label, Layout, Popup, PopupCloseBehavior, RectAlign, RichText, ScrollArea, Sense,
    Ui, vec2,
};

use super::Action;
use super::claude_icon;
use super::design::{RADIUS_ROW, TEXT_BODY, TEXT_SECONDARY, surface_frame};
use super::widgets::{muted, text_button};
use crate::claude_models::ModelOption;

const WIDTH: f32 = 280.0;
const ROW_HEIGHT: f32 = 30.0;

/// Draws `current`, the model answering in the active pane, as a button that lists `models`.
pub fn show(ui: &mut Ui, current: &str, models: &[ModelOption], actions: &mut Vec<Action>) {
    let button = text_button(ui, &format!("✦ {current}"), TEXT_SECONDARY).on_hover_text("Switch model");
    Popup::from_toggle_button_response(&button)
        .align(RectAlign::TOP_START)
        .gap(6.0)
        .layout(Layout::top_down(Align::Min))
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .frame(surface_frame(ui.visuals(), 6))
        .width(WIDTH)
        .show(|ui| {
            ui.set_width(WIDTH - 12.0);
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.horizontal(|ui| {
                ui.set_height(28.0);
                ui.add_space(10.0);
                claude_icon::icon(ui, 14.0);
                let title = RichText::new("Claude models").size(TEXT_SECONDARY).strong().color(muted(ui));
                ui.add(Label::new(title).selectable(false));
            });
            if models.is_empty() {
                let waiting = RichText::new("Asking Claude for its models…").size(TEXT_SECONDARY);
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    ui.label(waiting.color(muted(ui)));
                });
            }
            ScrollArea::vertical().max_height(ROW_HEIGHT * 9.5).show(ui, |ui| {
                for model in models {
                    if row(ui, &model.name, model.name == current) {
                        actions.push(Action::SetAgentModel(model.value.clone()));
                        ui.close();
                    }
                }
            });
        });
}

/// A model's row, with a check mark on the one in use; returns whether it was clicked.
fn row(ui: &mut Ui, name: &str, selected: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, RADIUS_ROW, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let color = if selected { ui.visuals().strong_text_color() } else { ui.visuals().text_color() };
    let font = FontId::proportional(TEXT_BODY);
    ui.painter().text(rect.left_center() + vec2(12.0, 0.0), Align2::LEFT_CENTER, name, font.clone(), color);
    if selected {
        ui.painter().text(rect.right_center() - vec2(12.0, 0.0), Align2::RIGHT_CENTER, "✓", font, color);
    }
    response.clicked()
}
