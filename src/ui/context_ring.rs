//! How full an agent's context window is: a ring that closes as it fills, with the share beside it.

use eframe::egui::{FontId, Response, Sense, Shape, Stroke, Ui, pos2, vec2};

use super::design::TEXT_SECONDARY;
use super::widgets::muted;
use crate::theme::Theme;

const RING_SIZE: f32 = 12.0;
const GAP: f32 = 5.0;
/// Share used from which the ring shows as a warning, then as nearly full.
const WARN_AT: f64 = 60.0;
const CRITICAL_AT: f64 = 85.0;

/// Draws the ring for `used`, a percentage, `height` tall.
pub fn show(ui: &mut Ui, used: f64, height: f32, theme: &Theme) -> Response {
    let used = used.clamp(0.0, 100.0);
    let color = match used {
        u if u >= CRITICAL_AT => theme.danger(),
        u if u >= WARN_AT => theme.modified(),
        _ => muted(ui),
    };
    let text = format!("{used:.0}%");
    let text = ui.painter().layout_no_wrap(text, FontId::proportional(TEXT_SECONDARY), color);
    let size = vec2(RING_SIZE + GAP + text.size().x, height);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let center = pos2(rect.left() + RING_SIZE / 2.0, rect.center().y);
    let radius = RING_SIZE / 2.0 - 1.0;
    let painter = ui.painter();
    painter.circle_stroke(center, radius, Stroke::new(1.5, muted(ui).gamma_multiply(0.35)));
    let share = (used / 100.0) as f32;
    if share > 0.0 {
        // From twelve o'clock, clockwise.
        let steps = (32.0 * share).ceil().max(1.0) as usize;
        let point = |i: usize| {
            let turn = std::f32::consts::TAU * share * i as f32 / steps as f32;
            let angle = turn - std::f32::consts::FRAC_PI_2;
            center + radius * vec2(angle.cos(), angle.sin())
        };
        painter.add(Shape::line((0..=steps).map(point).collect(), Stroke::new(1.5, color)));
    }
    let x = rect.left() + RING_SIZE + GAP;
    painter.galley(pos2(x, rect.center().y - text.size().y / 2.0), text, color);
    response
}
