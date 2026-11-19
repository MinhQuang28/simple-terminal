//! The design language every surface shares: one radius per role, one type scale, one popup
//! frame, one way to show a shortcut or a section caption.

use eframe::egui::{
    Align, Color32, CornerRadius, FontId, Frame, Layout, Margin, Popup, PopupCloseBehavior, RectAlign,
    Response, RichText, Sense, Shadow, Stroke, StrokeKind, Ui, Visuals, vec2,
};

/// Corner radius of floating surfaces: popups, dialogs, cards.
pub const RADIUS_SURFACE: u8 = 12;
/// Corner radius of selectable rows and controls.
pub const RADIUS_ROW: u8 = 6;
/// Corner radius of small marks inside a row: shortcut chips, checkboxes.
pub const RADIUS_CHIP: u8 = 4;
/// Inner padding of floating surfaces.
pub const SURFACE_PADDING: i8 = 8;
/// Height of a single-line list row.
pub const ROW_HEIGHT: f32 = 32.0;

/// Type scale, in points.
pub const TEXT_TITLE: f32 = 15.5;
pub const TEXT_BODY: f32 = 13.5;
pub const TEXT_SECONDARY: f32 = 12.5;
pub const TEXT_CAPTION: f32 = 11.5;
/// Tags and counts beside a label.
pub const TEXT_MICRO: f32 = 10.5;
/// Digits inside a round badge on an icon.
pub const TEXT_BADGE: f32 = 9.0;

/// Failures and anything about to be lost, in the theme's red.
pub fn danger(ui: &Ui) -> Color32 {
    ui.visuals().error_fg_color
}

/// Healthy states, in the theme's green.
pub fn success(ui: &Ui) -> Color32 {
    crate::theme::success(ui.ctx())
}

/// Black or white, whichever reads better on `fill`.
pub fn on_color(fill: Color32) -> Color32 {
    let [r, g, b, _] = fill.to_array().map(f32::from);
    let luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255.0;
    if luminance > 0.6 { Color32::from_rgb(0x15, 0x15, 0x15) } else { Color32::WHITE }
}

/// Outlines a hand-painted control while it holds keyboard focus, as egui does for its own.
pub fn focus_ring(ui: &Ui, response: &Response, radius: u8) {
    if response.has_focus() {
        let stroke = Stroke::new(1.5, ui.visuals().hyperlink_color);
        ui.painter().rect_stroke(response.rect, CornerRadius::same(radius), stroke, StrokeKind::Inside);
    }
}

/// Hairline for borders and separators.
pub fn hairline(ui: &Ui) -> Color32 {
    ui.visuals().text_color().gamma_multiply(0.12)
}

/// Background of the keyboard-selected row: a wash of the accent.
pub fn selection_fill(ui: &Ui) -> Color32 {
    ui.visuals().hyperlink_color.gamma_multiply(0.22)
}

/// The frame of every floating surface, over the window fill with a hairline and soft shadow.
pub fn surface_frame(visuals: &Visuals, padding: i8) -> Frame {
    Frame::new()
        .fill(visuals.window_fill)
        .stroke(Stroke::new(1.0, visuals.text_color().gamma_multiply(0.12)))
        .corner_radius(CornerRadius::same(RADIUS_SURFACE))
        .inner_margin(Margin::same(padding))
        .shadow(Shadow { offset: [0, 6], blur: 24, spread: 0, color: Color32::from_black_alpha(90) })
}

/// A popup anchored below `button`, `width` wide, in the shared surface frame. The popup lays
/// its contents top-down whatever layout the button sits in, and closes on clicks outside.
pub fn popup<R>(
    ui: &Ui,
    button: &Response,
    width: f32,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    Popup::from_toggle_button_response(button)
        .align(RectAlign::BOTTOM_END)
        .gap(6.0)
        .layout(Layout::top_down(Align::Min))
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .frame(surface_frame(ui.visuals(), SURFACE_PADDING))
        .width(width)
        .show(|ui| {
            ui.set_width(width - 2.0 * f32::from(SURFACE_PADDING));
            ui.spacing_mut().item_spacing.y = 4.0;
            add_contents(ui)
        })
        .map(|response| response.inner)
}

/// Uppercase caption introducing a group of rows.
pub fn section_label(ui: &mut Ui, text: &str) {
    let color = ui.visuals().weak_text_color();
    ui.add_space(6.0);
    ui.label(RichText::new(text.to_uppercase()).size(TEXT_CAPTION).strong().color(color));
    ui.add_space(2.0);
}

/// Hairline across the available width with breathing room around it.
pub fn divider(ui: &mut Ui) {
    ui.add_space(4.0);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(rect.x_range(), rect.center().y, Stroke::new(1.0, hairline(ui)));
    ui.add_space(4.0);
}

/// Centered muted text for a list with nothing in it.
pub fn empty_state(ui: &mut Ui, text: &str) {
    ui.add_space(14.0);
    ui.vertical_centered(|ui| {
        let color = ui.visuals().weak_text_color();
        ui.label(RichText::new(text).size(TEXT_SECONDARY).color(color));
    });
    ui.add_space(14.0);
}

/// A keyboard shortcut as a small outlined chip, e.g. "⇧⌘T".
pub fn kbd(ui: &mut Ui, keys: &str) {
    let color = ui.visuals().text_color().gamma_multiply(0.7);
    let galley = ui.painter().layout_no_wrap(keys.to_string(), FontId::proportional(TEXT_CAPTION), color);
    let (rect, _) = ui.allocate_exact_size(galley.size() + vec2(10.0, 4.0), Sense::hover());
    ui.painter().rect(
        rect,
        CornerRadius::same(RADIUS_CHIP),
        ui.visuals().text_color().gamma_multiply(0.06),
        Stroke::new(1.0, hairline(ui)),
        StrokeKind::Inside,
    );
    ui.painter().galley(rect.min + vec2(5.0, 2.0), galley, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_on_light_accents_is_dark_and_on_dark_accents_is_white() {
        assert_eq!(on_color(Color32::from_rgb(0xf5, 0xc5, 0x42)), Color32::from_rgb(0x15, 0x15, 0x15));
        assert_eq!(on_color(Color32::from_rgb(0x2b, 0x5c, 0xd6)), Color32::WHITE);
    }
}
