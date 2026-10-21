use std::time::Duration;

use eframe::egui::containers::menu::MenuButton;
use eframe::egui::{
    Align, Align2, Button, Color32, CornerRadius, FontId, Layout, Rect, Response, RichText, Sense, Shape,
    Stroke, Ui, UiBuilder, vec2,
};

pub const AGENT: Color32 = Color32::from_rgb(0xe0, 0x8a, 0x5f);

/// Hit size of the square icon buttons in the sidebar footer and tab bar.
pub const ICON_BUTTON_SIZE: f32 = 28.0;
const ICON_GLYPH_SIZE: f32 = 19.0;

/// Secondary text color of the active theme.
pub fn muted(ui: &Ui) -> Color32 {
    ui.visuals().text_color().gamma_multiply(0.6)
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
        ui.painter().rect_filled(rect, CornerRadius::same(7), ui.visuals().widgets.hovered.weak_bg_fill);
    }
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

/// Small rounded label, e.g. an agent state or "merged".
pub fn tag(ui: &mut Ui, text: &str, color: Color32) -> Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(10.5), color);
    let (rect, response) = ui.allocate_exact_size(galley.size() + vec2(10.0, 4.0), Sense::click());
    let fill = if response.hovered() { 0.3 } else { 0.18 };
    ui.painter().rect_filled(rect, 6.0, color.gamma_multiply(fill));
    ui.painter().galley(rect.min + vec2(5.0, 2.0), galley, color);
    response
}

/// Side of the small glyph buttons in sidebar rows.
pub const GLYPH_BUTTON_SIZE: f32 = 18.0;

/// A square button holding one glyph, such as the row "+" and "×"; it gains a filled background
/// on hover and is flat otherwise.
pub fn glyph_button(ui: &mut Ui, glyph: &str) -> Response {
    glyph_button_in(ui, glyph, |ui, button| ui.add(button))
}

/// [`glyph_button`] that opens a menu.
pub fn menu_icon<R>(ui: &mut Ui, glyph: &str, add_contents: impl FnOnce(&mut Ui) -> R) -> Response {
    glyph_button_in(ui, glyph, |ui, button| MenuButton::from_button(button).ui(ui, add_contents).0)
}

fn glyph_button_in<R>(ui: &mut Ui, glyph: &str, add: impl FnOnce(&mut Ui, Button<'_>) -> R) -> R {
    ui.scope(|ui| {
        let hover_fill = ui.visuals().text_color().gamma_multiply(0.14);
        let widgets = &mut ui.visuals_mut().widgets;
        widgets.hovered.weak_bg_fill = hover_fill;
        widgets.hovered.bg_stroke = Stroke::NONE;
        widgets.hovered.expansion = 0.0;
        widgets.active.weak_bg_fill = hover_fill.gamma_multiply(1.6);
        widgets.active.bg_stroke = Stroke::NONE;
        widgets.active.expansion = 0.0;
        let button = Button::new(RichText::new(glyph).color(muted(ui)).size(13.0))
            .corner_radius(CornerRadius::same(5))
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
        ui.painter().rect_filled(rect, CornerRadius::same(6), fill);
    }
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

/// Small vector folder/file icon; emoji fonts lack consistent glyphs for these.
pub fn file_icon(ui: &mut Ui, is_dir: bool) {
    let (rect, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
    let color = muted(ui);
    let stroke = eframe::egui::Stroke::new(1.2, color);
    let painter = ui.painter();
    if is_dir {
        let body = eframe::egui::Rect::from_min_max(rect.min + vec2(1.0, 4.0), rect.max - vec2(1.0, 2.0));
        let tab = eframe::egui::Rect::from_min_size(rect.min + vec2(1.0, 2.0), vec2(5.0, 3.0));
        painter.rect_filled(tab, CornerRadius::same(1), color);
        painter.rect_stroke(body, CornerRadius::same(2), stroke, eframe::egui::StrokeKind::Inside);
    } else {
        let page = eframe::egui::Rect::from_min_max(rect.min + vec2(2.5, 1.0), rect.max - vec2(2.5, 1.0));
        painter.rect_stroke(page, CornerRadius::same(2), stroke, eframe::egui::StrokeKind::Inside);
        for i in 0..3 {
            let y = page.top() + 4.0 + i as f32 * 2.5;
            painter.hline(
                page.left() + 2.0..=page.right() - 2.0,
                y,
                eframe::egui::Stroke::new(1.0, color.gamma_multiply(0.7)),
            );
        }
    }
}

/// Spinner animation frame rate; egui's own `Spinner` repaints every frame, which keeps the whole
/// window at full refresh rate for as long as any tab is busy.
const SPINNER_FRAME: Duration = Duration::from_millis(150);

/// Rotating arc like egui's `Spinner`, animated at a modest rate.
pub fn spinner_at(ui: &Ui, rect: Rect, color: Color32) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    ui.ctx().request_repaint_after(SPINNER_FRAME);
    let radius = rect.height().min(rect.width()) * 0.5 - 1.5;
    let time = ui.input(|i| i.time);
    let start = time * std::f64::consts::TAU;
    let end = start + 240f64.to_radians() * time.sin();
    let points: Vec<_> = (0..16)
        .map(|i| {
            let angle = start + (end - start) * i as f64 / 16.0;
            let (sin, cos) = angle.sin_cos();
            rect.center() + radius * vec2(cos as f32, sin as f32)
        })
        .collect();
    ui.painter().add(Shape::line(points, Stroke::new(2.0, color)));
}

pub fn spinner(ui: &mut Ui, size: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    spinner_at(ui, rect, color);
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
