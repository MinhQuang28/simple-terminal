use eframe::egui::{Painter, Rect, Sense, Shape, Stroke, Ui, Vec2, vec2};

use super::widgets::AGENT;

/// Size of the icon next to tab titles.
pub const ICON_SIZE: f32 = 12.0;

/// Claude's starburst: twelve hand-cut rays as `[angle in degrees clockwise from up, length]`,
/// lengths relative to the icon radius. The uneven spacing is part of the mark.
const RAYS: &[[f32; 2]] = &[
    [6.0, 1.0],
    [40.0, 0.93],
    [76.0, 0.97],
    [98.0, 0.98],
    [128.0, 0.9],
    [152.0, 0.88],
    [176.0, 1.0],
    [206.0, 0.92],
    [232.0, 0.85],
    [268.0, 0.98],
    [302.0, 0.9],
    [330.0, 0.97],
];
const RAY_BASE_WIDTH: f32 = 0.16;
const RAY_TIP_WIDTH: f32 = 0.1;

/// Width of the icon drawn `height` points tall.
pub fn width(height: f32) -> f32 {
    height
}

/// Paints the starburst centered in `rect`, as vector shapes so it stays crisp at any size.
pub fn paint(painter: &Painter, rect: Rect) {
    let radius = rect.width().min(rect.height()) / 2.0;
    let center = rect.center();
    for &[angle, length] in RAYS {
        let (sin, cos) = angle.to_radians().sin_cos();
        let dir = vec2(sin, -cos);
        let normal = vec2(cos, sin);
        let tip = center + dir * radius * length;
        let base = RAY_BASE_WIDTH * radius / 2.0;
        let head = RAY_TIP_WIDTH * radius / 2.0;
        let quad =
            vec![center - normal * base, center + normal * base, tip + normal * head, tip - normal * head];
        painter.add(Shape::convex_polygon(quad, AGENT, Stroke::NONE));
    }
    painter.circle_filled(center, RAY_BASE_WIDTH * radius / 2.0, AGENT);
}

/// Allocates and paints the icon inline, `size` points tall.
pub fn icon(ui: &mut Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    paint(ui.painter(), rect);
}
