use eframe::egui::containers::menu::MenuButton;
use eframe::egui::{
    Align, Align2, Button, Color32, CornerRadius, CursorIcon, FontId, Layout, Rect, Response, RichText,
    Sense, Stroke, Ui, UiBuilder, vec2,
};

use super::design::{self, RADIUS_ROW, TEXT_BODY, TEXT_MICRO};

pub const AGENT: Color32 = Color32::from_rgb(0xe0, 0x8a, 0x5f);

/// Hit size of the square icon buttons in the sidebar footer and tab bar.
pub const ICON_BUTTON_SIZE: f32 = 28.0;
const ICON_GLYPH_SIZE: f32 = 19.0;

/// Secondary text color of the active theme, kept readable on its background.
pub fn muted(ui: &Ui) -> Color32 {
    ui.visuals().weak_text_color()
}

pub fn accent(ui: &Ui) -> Color32 {
    ui.visuals().hyperlink_color
}

/// Reserves a square icon button, painting its hover background. Returns the response and the
/// glyph color to draw it with.
pub fn icon_button_frame(ui: &mut Ui, active: bool) -> (Rect, Response, Color32) {
    let (rect, response) = ui.allocate_exact_size(vec2(ICON_BUTTON_SIZE, ICON_BUTTON_SIZE), Sense::click());
    let lit = active || response.hovered();
    if lit {
        let fill = ui.visuals().widgets.hovered.weak_bg_fill;
        ui.painter().rect_filled(rect, CornerRadius::same(RADIUS_ROW), fill);
    }
    design::focus_ring(ui, &response, RADIUS_ROW);
    let color = if lit { ui.visuals().strong_text_color() } else { muted(ui) };
    (rect, response, color)
}

/// A square button showing a single text glyph.
pub fn icon_button(ui: &mut Ui, glyph: &str) -> Response {
    let (rect, response, color) = icon_button_frame(ui, false);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        glyph,
        FontId::proportional(ICON_GLYPH_SIZE),
        color,
    );
    response
}

/// Muted caption text that gains a rounded background and brighter text on hover, so it reads
/// as clickable; used for footer links such as "Mark all read".
pub fn text_button(ui: &mut Ui, text: &str, size: f32) -> Response {
    tinted_text_button(ui, text, size, muted(ui), ui.visuals().strong_text_color())
}

/// [`text_button`] in a color of its own, `idle` at rest and `lit` under the pointer.
pub fn tinted_text_button(ui: &mut Ui, text: &str, size: f32, idle: Color32, lit: Color32) -> Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(size), idle);
    let (rect, response) = ui.allocate_exact_size(galley.size() + vec2(16.0, 8.0), Sense::click());
    let color = if response.hovered() {
        let fill = ui.visuals().widgets.hovered.weak_bg_fill;
        ui.painter().rect_filled(rect, CornerRadius::same(RADIUS_ROW), fill);
        lit
    } else {
        idle
    };
    design::focus_ring(ui, &response, RADIUS_ROW);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// Small rounded label, e.g. an agent state or "merged".
pub fn tag(ui: &mut Ui, text: &str, color: Color32) -> Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(TEXT_MICRO), color);
    let (rect, response) = ui.allocate_exact_size(galley.size() + vec2(10.0, 4.0), Sense::click());
    let fill = if response.hovered() { 0.3 } else { 0.18 };
    ui.painter().rect_filled(rect, 6.0, color.gamma_multiply(fill));
    ui.painter().galley(rect.min + vec2(5.0, 2.0), galley, color);
    response
}

/// Side of the small glyph buttons in sidebar rows: the smallest target that is easy to hit.
pub const GLYPH_BUTTON_SIZE: f32 = 24.0;

/// A square button holding one glyph, such as the row "+" and "×"; it gains a filled background
/// on hover and is flat otherwise.
pub fn glyph_button(ui: &mut Ui, glyph: &str) -> Response {
    glyph_button_in(ui, glyph, muted(ui), |ui, button| ui.add(button))
}

/// [`glyph_button`] that opens a menu. Its glyph shows only while `shown` (the row is hovered or
/// current) or its menu is open, so a list of rows isn't a column of buttons; the button keeps
/// its place either way.
pub fn menu_icon<R>(
    ui: &mut Ui,
    glyph: &str,
    shown: bool,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> Response {
    let open_id = ui.auto_id_with("menu-icon-open");
    let open = ui.data(|d| d.get_temp::<bool>(open_id)).unwrap_or(false);
    let color = if shown || open { muted(ui) } else { Color32::TRANSPARENT };
    let (response, menu) =
        glyph_button_in(ui, glyph, color, |ui, button| MenuButton::from_button(button).ui(ui, add_contents));
    ui.data_mut(|d| d.insert_temp(open_id, menu.is_some()));
    response
}

fn glyph_button_in<R>(
    ui: &mut Ui,
    glyph: &str,
    color: Color32,
    add: impl FnOnce(&mut Ui, Button<'_>) -> R,
) -> R {
    ui.scope(|ui| {
        let hover_fill = ui.visuals().text_color().gamma_multiply(0.14);
        let widgets = &mut ui.visuals_mut().widgets;
        widgets.hovered.weak_bg_fill = hover_fill;
        widgets.hovered.bg_stroke = Stroke::NONE;
        widgets.hovered.expansion = 0.0;
        widgets.active.weak_bg_fill = hover_fill.gamma_multiply(1.6);
        widgets.active.bg_stroke = Stroke::NONE;
        widgets.active.expansion = 0.0;
        let button = Button::new(RichText::new(glyph).color(color).size(TEXT_BODY))
            .corner_radius(CornerRadius::same(RADIUS_ROW))
            .min_size(vec2(GLYPH_BUTTON_SIZE, GLYPH_BUTTON_SIZE))
            .frame_when_inactive(false);
        add(ui, button)
    })
    .inner
}

/// A full-width clickable row whose contents are laid out left-to-right.
/// Widgets added inside take click priority over the row itself.
pub fn row<R>(
    ui: &mut Ui,
    selected: bool,
    indent: f32,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> (Response, R) {
    row_sized(ui, selected, indent, 28.0, add_contents)
}

/// Whether the pointer is over the row being filled; ask first thing inside its contents.
pub fn row_hovered(ui: &Ui) -> bool {
    ui.rect_contains_pointer(ui.max_rect())
}

/// [`row`] with an explicit height.
pub fn row_sized<R>(
    ui: &mut Ui,
    selected: bool,
    indent: f32,
    height: f32,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> (Response, R) {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    let fill = if selected {
        Some(ui.visuals().widgets.active.weak_bg_fill)
    } else if response.hovered() {
        Some(ui.visuals().widgets.hovered.weak_bg_fill)
    } else {
        None
    };
    if let Some(fill) = fill {
        ui.painter().rect_filled(rect, CornerRadius::same(RADIUS_ROW), fill);
    }
    design::focus_ring(ui, &response, RADIUS_ROW);
    let inner = rect.shrink2(vec2(6.0, 0.0));
    let inner = inner.with_min_x(inner.min.x + indent);
    let mut child =
        ui.new_child(UiBuilder::new().max_rect(inner).layout(Layout::left_to_right(Align::Center)));
    let result = add_contents(&mut child);
    (response, result)
}

/// Rounded square with the name's initial, tinted by a color derived from the name.
pub fn avatar(ui: &mut Ui, name: &str, size: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let hue = name.bytes().fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32)) % 360;
    let color = eframe::egui::ecolor::Hsva::new(hue as f32 / 360.0, 0.45, 0.42, 1.0);
    ui.painter().rect_filled(rect, CornerRadius::same((size * 0.28) as u8), Color32::from(color));
    let initial = name.chars().next().unwrap_or('?').to_uppercase().to_string();
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        initial,
        FontId::proportional(size * 0.55),
        Color32::WHITE,
    );
}

/// Compact relative time like "now", "5m", "3h", "2d".
pub fn ago(timestamp: u64, now: u64) -> String {
    if timestamp == 0 {
        return String::new();
    }
    let secs = now.saturating_sub(timestamp);
    match secs {
        0..60 => "now".into(),
        60..3_600 => format!("{}m", secs / 60),
        3_600..86_400 => format!("{}h", secs / 3_600),
        _ => format!("{}d", secs / 86_400),
    }
}

/// Lays out `add_contents` from the right edge of the remaining row space.
pub fn right_aligned<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    ui.with_layout(Layout::right_to_left(Align::Center), add_contents).inner
}

#[cfg(test)]
mod tests {
    use super::ago;

    #[test]
    fn formats_relative_times() {
        assert_eq!(ago(0, 100), "");
        assert_eq!(ago(100, 130), "now");
        assert_eq!(ago(100, 100 + 5 * 60), "5m");
        assert_eq!(ago(100, 100 + 3 * 3_600), "3h");
        assert_eq!(ago(100, 100 + 2 * 86_400), "2d");
    }
}
