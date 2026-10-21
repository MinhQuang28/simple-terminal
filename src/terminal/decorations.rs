use alacritty_terminal::term::cell::Flags;
use eframe::egui::{Color32, Painter, Rect, Shape, Stroke, pos2};

/// Underlines and strikethrough for one cell; undercurl is drawn as a short-period zigzag.
pub fn paint(painter: &Painter, rect: Rect, flags: Flags, color: Color32) {
    let stroke = Stroke::new(1.0, color);
    if flags.contains(Flags::UNDERCURL) {
        let step = 2.0;
        let top = rect.bottom() - 2.5;
        let count = (rect.width() / step).ceil().max(1.0) as usize;
        let points = (0..=count)
            .map(|i| {
                let x = (rect.left() + i as f32 * step).min(rect.right());
                let y = if i % 2 == 0 { top } else { top + 1.5 };
                pos2(x, y)
            })
            .collect();
        painter.add(Shape::line(points, stroke));
    } else if flags.intersects(Flags::ALL_UNDERLINES) {
        painter.hline(rect.x_range(), rect.bottom() - 1.0, stroke);
        if flags.contains(Flags::DOUBLE_UNDERLINE) {
            painter.hline(rect.x_range(), rect.bottom() - 3.0, stroke);
        }
    }
    if flags.contains(Flags::STRIKEOUT) {
        painter.hline(rect.x_range(), rect.center().y, stroke);
    }
}
