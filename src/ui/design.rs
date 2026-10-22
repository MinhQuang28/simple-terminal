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
/// Inner padding of floating surfaces.
pub const SURFACE_PADDING: i8 = 8;
/// Height of a single-line list row.
pub const ROW_HEIGHT: f32 = 32.0;

/// Type scale, in points.
pub const TEXT_TITLE: f32 = 15.5;
pub const TEXT_BODY: f32 = 13.5;
pub const TEXT_SECONDARY: f32 = 12.5;
pub const TEXT_CAPTION: f32 = 11.5;

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
    let color = ui.visuals().text_color().gamma_multiply(0.55);
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
        let color = ui.visuals().text_color().gamma_multiply(0.6);
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
        CornerRadius::same(4),
        ui.visuals().text_color().gamma_multiply(0.06),
        Stroke::new(1.0, hairline(ui)),
        StrokeKind::Inside,
    );
    ui.painter().galley(rect.min + vec2(5.0, 2.0), galley, color);
}
