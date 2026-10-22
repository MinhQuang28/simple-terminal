use std::ops::RangeInclusive;

use eframe::egui::emath::Numeric;
use eframe::egui::{
    Align, Button, Color32, CornerRadius, Direction, DragValue, Frame, Layout, Margin, Response, RichText,
    Stroke, Ui, vec2,
};

const STEPPER_SIZE: f32 = 26.0;
const NUMBER_FIELD_WIDTH: f32 = 72.0;

/// A numeric input box with −/+ steppers; click the value to type, or drag it.
pub fn number_field<T: Numeric>(
    ui: &mut Ui,
    value: &mut T,
    range: RangeInclusive<T>,
    step: f64,
    suffix: &str,
) -> Response {
    let stepper = |ui: &mut Ui, label: &str| {
        let button = Button::new(RichText::new(label).size(15.0))
            .fill(ui.visuals().text_color().gamma_multiply(0.12))
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(6))
            .min_size(vec2(STEPPER_SIZE, STEPPER_SIZE));
        ui.add(button).clicked()
    };
    let bump = |value: &mut T, delta: f64| {
        let next = (value.to_f64() + delta).clamp(range.start().to_f64(), range.end().to_f64());
        *value = T::from_f64((next / step).round() * step);
    };
    // Rows lay controls out right-to-left; keep "− value +" in reading order.
    let gap = 6.0;
    let width = 2.0 * STEPPER_SIZE + NUMBER_FIELD_WIDTH + 2.0 * (gap + 8.0 + 1.0);
    ui.allocate_ui_with_layout(vec2(width, STEPPER_SIZE), Layout::left_to_right(Align::Center), |ui| {
        // The steppers and the value take exactly the sizes above, whatever padding the app's
        // style gives buttons; anything wider would spill past the row and widen the card.
        ui.spacing_mut().button_padding = vec2(0.0, 0.0);
        ui.spacing_mut().item_spacing.x = gap;
        steady_hover(ui);
        if stepper(ui, "−") {
            bump(value, -step);
        }
        let field = Frame::new()
            .fill(ui.visuals().extreme_bg_color)
            .stroke(field_stroke(ui))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(Margin::symmetric(8, 0))
            .show(ui, |ui| {
                let widgets = &mut ui.visuals_mut().widgets;
                for state in [&mut widgets.inactive, &mut widgets.hovered, &mut widgets.active] {
                    state.weak_bg_fill = Color32::TRANSPARENT;
                    state.bg_fill = Color32::TRANSPARENT;
                    state.bg_stroke = Stroke::NONE;
                }
                let decimals = if step.fract() == 0.0 { 0 } else { 2 };
                ui.allocate_ui_with_layout(
                    vec2(NUMBER_FIELD_WIDTH, STEPPER_SIZE - 2.0),
                    Layout::centered_and_justified(Direction::LeftToRight),
                    |ui| {
                        ui.add(
                            DragValue::new(value)
                                .range(range.clone())
                                .speed(step)
                                .suffix(suffix)
                                .min_decimals(decimals)
                                .max_decimals(decimals),
                        )
                    },
                )
                .inner
            })
            .inner;
        if stepper(ui, "+") {
            bump(value, step);
        }
        field
    })
    .inner
}

/// Outline of a field or picker, strong enough to read as a control on the card.
fn field_stroke(ui: &Ui) -> Stroke {
    Stroke::new(1.0, ui.visuals().text_color().gamma_multiply(0.35))
}

/// Stops buttons growing a pixel when hovered or pressed, which reads as a flicker on a stepper.
pub(super) fn steady_hover(ui: &mut Ui) {
    let widgets = &mut ui.visuals_mut().widgets;
    widgets.hovered.expansion = 0.0;
    widgets.active.expansion = 0.0;
}

/// Makes a combo box in `ui` look like the number fields: dark well, visible outline.
pub fn picker_visuals(ui: &mut Ui) {
    let fill = ui.visuals().extreme_bg_color;
    let hovered = fill.lerp_to_gamma(ui.visuals().text_color(), 0.04);
    let stroke = field_stroke(ui);
    steady_hover(ui);
    let widgets = &mut ui.visuals_mut().widgets;
    for (state, fill) in [
        (&mut widgets.inactive, fill),
        (&mut widgets.hovered, hovered),
        (&mut widgets.active, hovered),
        (&mut widgets.open, hovered),
    ] {
        state.weak_bg_fill = fill;
        state.bg_fill = fill;
        state.bg_stroke = stroke;
    }
}
