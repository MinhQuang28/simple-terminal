//! Subscription usage: a gauge in the top bar that opens a card with one bar per limit window,
//! like "Session · 85% left · Resets in 1h 25m".

use eframe::egui::{
    Align, Color32, CornerRadius, Layout, Popup, PopupCloseBehavior, Response, RichText, Sense, Shape,
    Stroke, Ui, pos2, vec2,
};

use super::claude_icon;
use super::widgets::{GOOD, ago, icon_button_frame, muted, right_aligned};
use crate::claude_status_line::{StatusLine, Window, duration, now};
use crate::model::AppState;

const WIDTH: f32 = 320.0;
const BAR_HEIGHT: f32 = 6.0;
/// Usage from which a window shows yellow, then red.
const WARN_AT: f64 = 60.0;
const CRITICAL_AT: f64 = 85.0;

/// `limits` are the subscription's account-wide usage; `context` is the active agent pane's.
pub fn gauge(ui: &mut Ui, state: &AppState, limits: Option<&StatusLine>, context: Option<f64>) {
    let peak = limits.map_or(0.0, StatusLine::peak_used);
    let response = gauge_icon(ui, state, limits.map(|_| peak)).on_hover_text("Claude usage");
    Popup::from_toggle_button_response(&response)
        .layout(Layout::top_down(Align::Min))
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .width(WIDTH)
        .show(|ui| contents(ui, state, limits, context));
}

fn contents(ui: &mut Ui, state: &AppState, limits: Option<&StatusLine>, context: Option<f64>) {
    ui.spacing_mut().item_spacing.y = 6.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        claude_icon::icon(ui, 16.0);
        ui.label(RichText::new("Claude Code").size(15.0).strong());
    });
    ui.add_space(6.0);
    let now = now();
    match limits {
        Some(limits) => {
            for window in limits.windows() {
                limit_row(ui, state, &window, now);
            }
        }
        None => {
            let text = "Usage shows after the first reply in a Claude pane (Pro and Max plans).";
            ui.label(RichText::new(text).color(muted(ui)).size(12.5));
        }
    }
    if let Some(context) = context {
        bar_row(ui, state, "Context window", &format!("{:.0}% used", context), "this pane", context);
    }
    if let Some(limits) = limits {
        ui.add_space(4.0);
        ui.separator();
        let updated = format!("Updated {} ago", ago(limits.reported_at, now)).replace("now ago", "just now");
        ui.label(RichText::new(updated).color(muted(ui)).size(12.0));
    }
}

fn limit_row(ui: &mut Ui, state: &AppState, window: &Window, now: u64) {
    let used = window.limit.used_at(now);
    let left = format!("{:.0}% left", (100.0 - used).max(0.0));
    let resets = format!("Resets in {}", duration(window.limit.resets_at.saturating_sub(now)));
    bar_row(ui, state, &window.label(), &left, &resets, used);
}

/// A labelled bar: "Label      85% left  Resets in 1h 25m" over a track filled to `used` percent.
fn bar_row(ui: &mut Ui, state: &AppState, label: &str, amount: &str, note: &str, used: f64) {
    let color = level_color(state, used);
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(13.5));
        right_aligned(ui, |ui| {
            ui.label(RichText::new(note).color(muted(ui)).size(12.5));
            ui.label(RichText::new(amount).color(color).size(13.0).strong());
        });
    });
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), BAR_HEIGHT), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(3), ui.visuals().text_color().gamma_multiply(0.12));
    let fill = rect.with_max_x(rect.left() + rect.width() * (used.clamp(0.0, 100.0) / 100.0) as f32);
    if fill.width() > 0.0 {
        painter.rect_filled(fill, CornerRadius::same(3), color);
    }
    ui.add_space(6.0);
}

fn level_color(state: &AppState, used: f64) -> Color32 {
    if used >= CRITICAL_AT {
        Color32::from_rgb(0xe0, 0x5a, 0x4f)
    } else if used >= WARN_AT {
        state.settings.theme.modified()
    } else {
        GOOD
    }
}

/// A speedometer arc with a needle at `used` percent; grey when nothing has been reported.
fn gauge_icon(ui: &mut Ui, state: &AppState, used: Option<f64>) -> Response {
    let (rect, response, grey) = icon_button_frame(ui, false);
    let color = used.map_or(grey, |used| level_color(state, used));
    let center = rect.center() + vec2(0.0, 2.5);
    let radius = 6.5;
    let arc: Vec<_> = (0..=16)
        .map(|i| {
            let angle = std::f32::consts::PI * (1.0 + i as f32 / 16.0);
            pos2(center.x + radius * angle.cos(), center.y + radius * angle.sin())
        })
        .collect();
    let painter = ui.painter();
    painter.add(Shape::line(arc, Stroke::new(1.6, color)));
    let angle = std::f32::consts::PI * (1.0 + used.unwrap_or(15.0) as f32 / 100.0);
    let tip = pos2(center.x + (radius - 1.5) * angle.cos(), center.y + (radius - 1.5) * angle.sin());
    painter.line_segment([center, tip], Stroke::new(1.6, color));
    painter.circle_filled(center, 1.4, color);
    response
}
