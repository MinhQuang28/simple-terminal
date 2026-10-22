use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};

use super::box_arms::{Weight, arms};

/// Whether `c` is drawn as shapes by [`paint`].
pub fn handles(c: char) -> bool {
    matches!(c, '\u{2500}'..='\u{257F}' | '\u{2580}'..='\u{259F}' | '\u{E0B0}'..='\u{E0B3}')
}

/// Paints box-drawing, block, shade and powerline glyphs as shapes so they join seamlessly
/// across cells at any font size. Returns false for characters it doesn't cover.
pub fn paint(painter: &Painter, rect: Rect, c: char, color: Color32) -> bool {
    match c {
        '\u{2500}'..='\u{257F}' => box_line(painter, rect, c, color),
        '\u{2580}'..='\u{259F}' => {
            block(painter, rect, c, color);
            true
        }
        '\u{E0B0}'..='\u{E0B3}' => {
            powerline(painter, rect, c, color);
            true
        }
        _ => false,
    }
}

fn light_width(rect: Rect) -> f32 {
    (rect.height() / 14.0).round().max(1.0)
}

fn box_line(painter: &Painter, rect: Rect, c: char, color: Color32) -> bool {
    let light = light_width(rect);
    let center = rect.center();
    match c {
        '╭' | '╮' | '╯' | '╰' => rounded_corner(painter, rect, c, Stroke::new(light, color)),
        '╱' => {
            painter.line_segment([rect.left_bottom(), rect.right_top()], Stroke::new(light, color));
        }
        '╲' => {
            painter.line_segment([rect.left_top(), rect.right_bottom()], Stroke::new(light, color));
        }
        '╳' => {
            let stroke = Stroke::new(light, color);
            painter.line_segment([rect.left_bottom(), rect.right_top()], stroke);
            painter.line_segment([rect.left_top(), rect.right_bottom()], stroke);
        }
        _ => {
            let Some(arms) = arms(c) else { return false };
            for (i, weight) in arms.into_iter().enumerate() {
                arm(painter, rect, center, i, weight, light, color);
            }
        }
    }
    true
}

/// Draws one arm from the cell center to its edge; `side` is 0 up, 1 down, 2 left, 3 right.
fn arm(painter: &Painter, rect: Rect, center: Pos2, side: usize, weight: Weight, light: f32, color: Color32) {
    let (thick, offsets): (f32, &[f32]) = match weight {
        Weight::None => return,
        Weight::Light => (light, &[0.0]),
        Weight::Heavy => (light * 2.5, &[0.0]),
        Weight::Double => (light, &[-light * 1.5, light * 1.5]),
    };
    let half = thick / 2.0;
    for offset in offsets {
        let r = match side {
            0 => Rect::from_min_max(
                pos2(center.x - half + offset, rect.top()),
                pos2(center.x + half + offset, center.y + half),
            ),
            1 => Rect::from_min_max(
                pos2(center.x - half + offset, center.y - half),
                pos2(center.x + half + offset, rect.bottom()),
            ),
            2 => Rect::from_min_max(
                pos2(rect.left(), center.y - half + offset),
                pos2(center.x + half, center.y + half + offset),
            ),
            _ => Rect::from_min_max(
                pos2(center.x - half, center.y - half + offset),
                pos2(rect.right(), center.y + half + offset),
            ),
        };
        painter.rect_filled(r, 0.0, color);
    }
}

fn rounded_corner(painter: &Painter, rect: Rect, c: char, stroke: Stroke) {
    let center = rect.center();
    let radius = rect.width().min(rect.height()) / 2.0;
    // Each corner joins one vertical and one horizontal arm through a quarter circle.
    let (vertical_end, horizontal_end, arc_center, start_angle) = match c {
        '╭' => (
            pos2(center.x, rect.bottom()),
            pos2(rect.right(), center.y),
            center + vec2(radius, radius),
            180.0,
        ),
        '╮' => (
            pos2(center.x, rect.bottom()),
            pos2(rect.left(), center.y),
            center + vec2(-radius, radius),
            270.0,
        ),
        '╯' => {
            (pos2(center.x, rect.top()), pos2(rect.left(), center.y), center + vec2(-radius, -radius), 0.0)
        }
        _ => (pos2(center.x, rect.top()), pos2(rect.right(), center.y), center + vec2(radius, -radius), 90.0),
    };
    let mut points = Vec::with_capacity(12);
    points.push(vertical_end);
    for i in 0..=8 {
        let angle = (start_angle + 90.0 * i as f32 / 8.0).to_radians();
        points.push(arc_center + radius * vec2(angle.cos(), angle.sin()));
    }
    points.push(horizontal_end);
    if c == '╯' || c == '╰' {
        points.reverse();
    }
    painter.add(Shape::line(points, stroke));
}

fn block(painter: &Painter, rect: Rect, c: char, color: Color32) {
    let (w, h) = (rect.width(), rect.height());
    let fill = |r: Rect| {
        painter.rect_filled(r, 0.0, color);
    };
    let lower = |eighths: f32| fill(rect.with_min_y(rect.bottom() - h * eighths / 8.0));
    let left = |eighths: f32| fill(rect.with_max_x(rect.left() + w * eighths / 8.0));
    let quadrant = |ul: bool, ur: bool, ll: bool, lr: bool| {
        let (cx, cy) = (rect.center().x, rect.center().y);
        if ul {
            fill(Rect::from_min_max(rect.min, pos2(cx, cy)));
        }
        if ur {
            fill(Rect::from_min_max(pos2(cx, rect.top()), pos2(rect.right(), cy)));
        }
        if ll {
            fill(Rect::from_min_max(pos2(rect.left(), cy), pos2(cx, rect.bottom())));
        }
        if lr {
            fill(Rect::from_min_max(pos2(cx, cy), rect.max));
        }
    };
    match c {
        '▀' => fill(rect.with_max_y(rect.center().y)),
        '▁'..='█' => lower((c as u32 - 0x2580) as f32),
        '▉'..='▏' => left((0x2590 - c as u32) as f32),
        '▐' => fill(rect.with_min_x(rect.center().x)),
        '░' => fill_shade(painter, rect, color, 0.25),
        '▒' => fill_shade(painter, rect, color, 0.5),
        '▓' => fill_shade(painter, rect, color, 0.75),
        '▔' => fill(rect.with_max_y(rect.top() + h / 8.0)),
        '▕' => fill(rect.with_min_x(rect.right() - w / 8.0)),
        '▖' => quadrant(false, false, true, false),
        '▗' => quadrant(false, false, false, true),
        '▘' => quadrant(true, false, false, false),
        '▙' => quadrant(true, false, true, true),
        '▚' => quadrant(true, false, false, true),
        '▛' => quadrant(true, true, true, false),
        '▜' => quadrant(true, true, false, true),
        '▝' => quadrant(false, true, false, false),
        '▞' => quadrant(false, true, true, false),
        _ => quadrant(false, true, true, true),
    }
}

fn fill_shade(painter: &Painter, rect: Rect, color: Color32, opacity: f32) {
    painter.rect_filled(rect, 0.0, color.gamma_multiply(opacity));
}

fn powerline(painter: &Painter, rect: Rect, c: char, color: Color32) {
    let (l, r, t, b, cy) = (rect.left(), rect.right(), rect.top(), rect.bottom(), rect.center().y);
    let stroke = Stroke::new(light_width(rect), color);
    match c {
        '\u{E0B0}' => {
            painter.add(Shape::convex_polygon(vec![pos2(l, t), pos2(r, cy), pos2(l, b)], color, Stroke::NONE))
        }
        '\u{E0B1}' => painter.add(Shape::line(vec![pos2(l, t), pos2(r, cy), pos2(l, b)], stroke)),
        '\u{E0B2}' => {
            painter.add(Shape::convex_polygon(vec![pos2(r, t), pos2(l, cy), pos2(r, b)], color, Stroke::NONE))
        }
        _ => painter.add(Shape::line(vec![pos2(r, t), pos2(l, cy), pos2(r, b)], stroke)),
    };
}
