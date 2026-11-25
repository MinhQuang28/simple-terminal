//! One look for every modal dialog: a title, an optional description, fields, and a footer with
//! the actions right-aligned, the default one filled in the accent color.

use eframe::egui::{
    Align, Color32, Context, CornerRadius, FontId, Id, Key, Layout, Modal, ModalResponse, Response, RichText,
    Sense, Stroke, StrokeKind, TextEdit, Ui, vec2,
};

use super::design::{self, RADIUS_ROW, TEXT_BODY, TEXT_SECONDARY, TEXT_TITLE};
use super::widgets::{accent, muted};

const WIDTH: f32 = 400.0;
const BUTTON_HEIGHT: f32 = 30.0;
const DESTRUCTIVE: Color32 = Color32::from_rgb(0xd9, 0x4b, 0x45);

/// How an action button is filled.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// The accent-filled default action.
    Primary,
    /// Flat, for alternatives such as "Don't save".
    Secondary,
    /// Red-filled, for actions that delete something.
    Destructive,
}

/// What the footer's buttons reported this frame.
#[derive(Default)]
pub struct Footer {
    /// Index into the actions passed to [`footer`].
    pub action: Option<usize>,
    pub cancel: bool,
}

/// Shows a centered modal with the dialog frame; `Esc` and clicks outside ask to close it.
pub fn modal<R>(ctx: &Context, add_contents: impl FnOnce(&mut Ui) -> R) -> ModalResponse<R> {
    let frame = design::surface_frame(&ctx.global_style().visuals, 20);
    Modal::new(Id::unique("repodeck-dialog")).frame(frame).show(ctx, |ui| {
        ui.set_width(WIDTH);
        ui.spacing_mut().item_spacing.y = 8.0;
        add_contents(ui)
    })
}

pub fn title(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(TEXT_TITLE).strong());
}

pub fn description(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(TEXT_SECONDARY).color(muted(ui)));
}

/// A full-width single-line field; `code` sets a monospace font.
pub fn field(ui: &mut Ui, value: &mut String, hint: &str, code: bool) -> Response {
    let font = if code { FontId::monospace(13.0) } else { FontId::proportional(TEXT_BODY) };
    let edit = TextEdit::singleline(value)
        .hint_text(hint)
        .font(font)
        .margin(vec2(10.0, 7.0))
        .desired_width(f32::INFINITY);
    ui.add(edit)
}

/// Puts the caret in `field` once the dialog has taken focus from the pane behind it. Asking
/// every frame would reset egui's key filter and cancel IME composition on each keystroke.
pub fn focus_initially(ui: &Ui, field: &Response) {
    if ui.memory(|m| m.focused().is_none()) {
        field.request_focus();
    }
}

/// A full-width multi-line field.
pub fn text_area(ui: &mut Ui, value: &mut String, hint: &str, rows: usize) -> Response {
    let edit = TextEdit::multiline(value)
        .hint_text(hint)
        .font(FontId::proportional(TEXT_BODY))
        .margin(vec2(10.0, 7.0))
        .desired_rows(rows)
        .desired_width(f32::INFINITY);
    ui.add(edit)
}

/// Right-aligned buttons: "Cancel" at the left, then `actions` with the first (the default) at
/// the right edge. When `enter_submits`, the Enter key counts as clicking the default.
pub fn footer(ui: &mut Ui, actions: &[(&str, Tone)], enter_submits: bool) -> Footer {
    ui.add_space(6.0);
    let mut footer = Footer::default();
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        for (index, (label, tone)) in actions.iter().enumerate() {
            if button(ui, label, *tone).clicked() {
                footer.action = Some(index);
            }
        }
        footer.cancel = button(ui, "Cancel", Tone::Secondary).clicked();
    });
    if enter_submits && footer.action.is_none() && ui.input(|i| i.key_pressed(Key::Enter)) {
        footer.action = Some(0);
    }
    footer
}

/// A pill button filled by `tone`, brightening on hover.
pub fn button(ui: &mut Ui, label: &str, tone: Tone) -> Response {
    let visuals = ui.visuals().clone();
    let (fill, text, stroke) = match tone {
        Tone::Primary => (accent(ui), design::on_color(accent(ui)), Stroke::NONE),
        Tone::Destructive => (DESTRUCTIVE, Color32::WHITE, Stroke::NONE),
        Tone::Secondary => (
            visuals.widgets.inactive.weak_bg_fill,
            visuals.text_color(),
            Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color),
        ),
    };
    let galley = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(TEXT_BODY), text);
    let size = vec2(galley.size().x + 28.0, BUTTON_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let fill = if response.is_pointer_button_down_on() {
        fill.lerp_to_gamma(Color32::BLACK, 0.12)
    } else if response.hovered() {
        fill.lerp_to_gamma(Color32::WHITE, 0.08)
    } else {
        fill
    };
    ui.painter().rect(rect, CornerRadius::same(RADIUS_ROW), fill, stroke, StrokeKind::Inside);
    design::focus_ring(ui, &response, RADIUS_ROW);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, text);
    response.on_hover_cursor(eframe::egui::CursorIcon::PointingHand)
}
