//! The `›`/`⌄` fold marker, painted so it sits on the row's middle: the font glyphs sit on the
//! baseline, below the middle of the text beside them.

use eframe::egui::epaint::PathShape;
use eframe::egui::{Color32, Rect, Response, Sense, Stroke, Ui, pos2, vec2};

use super::widgets::muted;

pub const SIZE: f32 = 14.0;

/// Paints the marker centred in `rect`: pointing down when `open`, right otherwise.
pub fn paint(ui: &Ui, rect: Rect, open: bool, color: Color32) {
    let (c, r) = (rect.center(), 3.0);
    let points = if open {
        vec![pos2(c.x - r, c.y - r / 2.0), pos2(c.x, c.y + r / 2.0), pos2(c.x + r, c.y - r / 2.0)]
    } else {
        vec![pos2(c.x - r / 2.0, c.y - r), pos2(c.x + r / 2.0, c.y), pos2(c.x - r / 2.0, c.y + r)]
    };
    ui.painter().add(PathShape::line(points, Stroke::new(1.3, color)));
}

/// A muted marker in a square of its own, e.g. in front of a folder's name.
pub fn show(ui: &mut Ui, open: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(SIZE, SIZE), Sense::hover());
    paint(ui, rect, open, muted(ui));
    response
}
