//! Subscription usage: a pill in the top bar with the share left, which opens a card with a dial
//! for the tightest limit and one bar per limit window.

use eframe::egui::{
    Align2, Color32, CornerRadius, FontId, Pos2, Response, RichText, Sense, Shape, Stroke, StrokeKind, Ui,
    pos2, vec2,
};

use super::Action;
use super::claude_icon;
use super::design::{self, TEXT_BODY, TEXT_SECONDARY, TEXT_TITLE, hairline};
use super::widgets::{ago, glyph_button, icon_button_frame, muted, right_aligned};
use crate::claude_status_line::{StatusLine, Window, duration, now};
use crate::model::AppState;

const WIDTH: f32 = 320.0;
const BAR_HEIGHT: f32 = 4.0;
const DIAL_RADIUS: f32 = 76.0;
const DIAL_STROKE: f32 = 9.0;
/// The dial opens downwards: it runs from lower left, over the top, to lower right.
const DIAL_START: f32 = 150.0;
const DIAL_SWEEP: f32 = 240.0;
/// Usage from which a window shows yellow, then red.
const WARN_AT: f64 = 60.0;
const CRITICAL_AT: f64 = 85.0;

/// `limits` are the subscription's account-wide usage; `context` is the active agent pane's.
pub fn gauge(
    ui: &mut Ui,
    state: &AppState,
    limits: Option<&StatusLine>,
    context: Option<f64>,
    actions: &mut Vec<Action>,
) {
    let response = match limits {
        Some(limits) => pill(ui, state, limits.peak_used()),
        None => gauge_icon(ui),
    };
    let response = response.on_hover_text("Claude usage");
    design::popup(ui, &response, WIDTH, |ui| contents(ui, state, limits, context, actions));
}

fn left(used: f64) -> f64 {
    (100.0 - used).clamp(0.0, 100.0)
}

fn contents(
    ui: &mut Ui,
    state: &AppState,
    limits: Option<&StatusLine>,
    context: Option<f64>,
    actions: &mut Vec<Action>,
) {
    ui.spacing_mut().item_spacing.y = 6.0;
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        claude_icon::icon(ui, 16.0);
        ui.label(RichText::new("Claude").size(TEXT_TITLE).strong());
        right_aligned(ui, |ui| {
            if glyph_button(ui, "↻").on_hover_text("Ask Claude again").clicked() {
                actions.push(Action::RefreshUsage);
            }
        });
    });
    let now = now();
    match limits {
        Some(limits) => {
            let windows = limits.windows();
            let tightest =
                windows.iter().max_by(|a, b| a.limit.used_at(now).total_cmp(&b.limit.used_at(now)));
            if let Some(window) = tightest {
                dial(ui, state, window, now);
            }
            for window in &windows {
                let used = window.limit.used_at(now);
                let resets = format!("Resets in {}", duration(window.limit.resets_at.saturating_sub(now)));
                bar_row(ui, state, &window.label(), &format!("{:.0}% left", left(used)), &resets, used);
            }
        }
        None => {
            ui.add_space(8.0);
            let text = "Asking Claude for your limits. They show for Pro and Max plans.";
            ui.label(RichText::new(text).color(muted(ui)).size(TEXT_SECONDARY));
        }
    }
    if let Some(context) = context {
        let amount = format!("{:.0}% left", left(context));
        bar_row(ui, state, "Context window", &amount, "This pane", context);
    }
    if let Some(limits) = limits {
        design::divider(ui);
        let updated = format!("Updated {} ago", ago(limits.reported_at, now)).replace("now ago", "just now");
        ui.label(RichText::new(updated).color(muted(ui)).size(TEXT_SECONDARY));
    }
}

/// Points along the dial from its start to `share` (0 to 1) of its sweep.
fn arc(center: Pos2, share: f32) -> Vec<Pos2> {
    let steps = (48.0 * share).ceil().max(1.0) as usize;
    (0..=steps)
        .map(|i| {
            let angle = (DIAL_START + DIAL_SWEEP * share * i as f32 / steps as f32).to_radians();
            center + DIAL_RADIUS * vec2(angle.cos(), angle.sin())
        })
        .collect()
}

/// The share of `window` left, as an arc with the number in its middle.
fn dial(ui: &mut Ui, state: &AppState, window: &Window, now: u64) {
    let used = window.limit.used_at(now);
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), DIAL_RADIUS * 1.5 + 34.0), Sense::hover());
    let center = pos2(rect.center().x, rect.top() + DIAL_RADIUS + 10.0);
    let painter = ui.painter();
    // A line's ends are square, so a dot at each end rounds them.
    let stroke = |points: Vec<Pos2>, color: Color32| {
        for end in [points.first(), points.last()].into_iter().flatten() {
            painter.circle_filled(*end, DIAL_STROKE / 2.0, color);
        }
        painter.add(Shape::line(points, Stroke::new(DIAL_STROKE, color)));
    };
    // Opaque, or the dots at the ends would show darker through the line.
    let track = ui.visuals().window_fill.lerp_to_gamma(ui.visuals().text_color(), 0.14);
    stroke(arc(center, 1.0), track);
    let share = (left(used) / 100.0) as f32;
    if share > 0.0 {
        stroke(arc(center, share), level_color(state, used));
    }
    let number = format!("{:.0}%", left(used));
    let ink = ui.visuals().strong_text_color();
    painter.text(center - vec2(0.0, 4.0), Align2::CENTER_CENTER, number, FontId::proportional(36.0), ink);
    let caption = format!("{} left", window.label());
    let below = pos2(center.x, rect.bottom() - 10.0);
    painter.text(below, Align2::CENTER_CENTER, caption, FontId::proportional(TEXT_SECONDARY), muted(ui));
}

/// A limit: "Label      85% left" over a track filled to what is left, with `note` under it.
fn bar_row(ui: &mut Ui, state: &AppState, label: &str, amount: &str, note: &str, used: f64) {
    let color = level_color(state, used);
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(TEXT_BODY));
        right_aligned(ui, |ui| ui.label(RichText::new(amount).size(TEXT_BODY).strong()));
    });
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), BAR_HEIGHT), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(2), ui.visuals().text_color().gamma_multiply(0.12));
    let fill = rect.with_max_x(rect.left() + rect.width() * (left(used) / 100.0) as f32);
    if fill.width() > 0.0 {
        painter.rect_filled(fill, CornerRadius::same(2), color);
    }
    ui.label(RichText::new(note).color(muted(ui)).size(TEXT_SECONDARY));
}

fn level_color(state: &AppState, used: f64) -> Color32 {
    if used >= CRITICAL_AT {
        state.settings.theme.danger()
    } else if used >= WARN_AT {
        state.settings.theme.modified()
    } else {
        state.settings.theme.success()
    }
}

/// The share left of the most used limit, in an outlined pill coloured by how much that is.
fn pill(ui: &mut Ui, state: &AppState, used: f64) -> Response {
    let color = level_color(state, used);
    let text = format!("{:.0}%", left(used));
    let galley = ui.painter().layout_no_wrap(text, FontId::proportional(TEXT_SECONDARY), color);
    let (rect, response) = ui.allocate_exact_size(vec2(galley.size().x + 20.0, 24.0), Sense::click());
    let radius = CornerRadius::same(12);
    if response.hovered() {
        ui.painter().rect_filled(rect, radius, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    ui.painter().rect_stroke(rect, radius, Stroke::new(1.0, hairline(ui)), StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
    response
}

/// A grey speedometer, shown until Claude has reported the limits.
fn gauge_icon(ui: &mut Ui) -> Response {
    let (rect, response, color) = icon_button_frame(ui, false);
    let center = rect.center() + vec2(0.0, 2.5);
    let radius = 6.5;
    let at = |share: f32, radius: f32| {
        let angle = std::f32::consts::PI * (1.0 + share);
        pos2(center.x + radius * angle.cos(), center.y + radius * angle.sin())
    };
    let painter = ui.painter();
    painter
        .add(Shape::line((0..=16).map(|i| at(i as f32 / 16.0, radius)).collect(), Stroke::new(1.6, color)));
    painter.line_segment([center, at(0.15, radius - 1.5)], Stroke::new(1.6, color));
    painter.circle_filled(center, 1.4, color);
    response
}
