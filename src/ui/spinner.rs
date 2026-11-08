//! Agent state marks, after Herdr's: a turning arc while working (or grey while loading), a filled
//! dot once done, and a warm red for anything that waits on the user.

use std::f32::consts::{FRAC_PI_2, TAU};
use std::time::Duration;

use eframe::egui::{Color32, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2, vec2};

/// Spinner frame interval. Each frame repaints the whole window (~1 ms); 30 Hz keeps a 0.9 s turn
/// smooth (12° a step) at half a 60 Hz cost, while it is often the only thing waking the window.
const SPINNER_FRAME: Duration = Duration::from_micros(1_000_000 / 30);
/// One full turn of the arc, in seconds, as Herdr spins it.
const TURN_PERIOD: f64 = 0.9;

/// Herdr's status colors (working, blocked, done) for dark and light backgrounds.
const DARK: [Color32; 3] = [
    Color32::from_rgb(0xe5, 0xc0, 0x7b),
    Color32::from_rgb(0xf0, 0x70, 0x5f),
    Color32::from_rgb(0x76, 0xa8, 0xff),
];
const LIGHT: [Color32; 3] = [
    Color32::from_rgb(0x9a, 0x67, 0x00),
    Color32::from_rgb(0xc9, 0x3c, 0x2c),
    Color32::from_rgb(0x2f, 0x6f, 0xe4),
];

fn palette(ui: &Ui) -> &'static [Color32; 3] {
    if ui.visuals().dark_mode { &DARK } else { &LIGHT }
}

/// An agent at work.
pub fn working_color(ui: &Ui) -> Color32 {
    palette(ui)[0]
}

/// An agent waiting on the user: input, approval, or an error to look at.
pub fn blocked_color(ui: &Ui) -> Color32 {
    palette(ui)[1]
}

/// An agent that finished while the user looked elsewhere.
pub fn done_color(ui: &Ui) -> Color32 {
    palette(ui)[2]
}

/// The working spinner: a faint ring with a quarter arc turning around it.
pub fn spinner_at(ui: &Ui, rect: Rect) {
    arc_spinner(ui, rect, working_color(ui));
}

pub fn spinner(ui: &mut Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    spinner_at(ui, rect);
}

/// The same spinner in grey for plain loading, so color keeps meaning "an agent is working".
pub fn loading_spinner_at(ui: &Ui, rect: Rect) {
    arc_spinner(ui, rect, ui.visuals().text_color().gamma_multiply(0.6));
}

pub fn loading_spinner(ui: &mut Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    loading_spinner_at(ui, rect);
}

/// A filled status dot centred in `rect`, `diameter` across.
pub fn dot_at(ui: &Ui, center: Pos2, diameter: f32, color: Color32) {
    ui.painter().circle_filled(center, diameter / 2.0, color);
}

/// Allocates a `size` square holding a 7 pt dot, as Herdr marks a finished agent.
pub fn dot(ui: &mut Ui, size: f32, color: Color32) -> eframe::egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    dot_at(ui, rect.center(), 7.0, color);
    response
}

/// Herdr's spinner.svg drawn at `rect`: viewBox 24, ring r 9.4 at 24% opacity, stroke 3.2.
fn arc_spinner(ui: &Ui, rect: Rect, color: Color32) {
    let Some(time) = tick(ui, rect) else { return };
    let scale = rect.width().min(rect.height()) / 24.0;
    let (center, radius) = (rect.center(), 9.4 * scale);
    let width = (3.2 * scale).max(1.2);
    let painter = ui.painter();
    painter.circle_stroke(center, radius, Stroke::new(width, color.gamma_multiply(0.24)));
    let start = (time / TURN_PERIOD).fract() as f32 * TAU - FRAC_PI_2;
    const SEGMENTS: usize = 12;
    let points: Vec<Pos2> = (0..=SEGMENTS)
        .map(|i| center + radius * Vec2::angled(start + FRAC_PI_2 * i as f32 / SEGMENTS as f32))
        .collect();
    // Round caps, as the SVG's stroke-linecap.
    painter.circle_filled(points[0], width / 2.0, color);
    painter.circle_filled(points[SEGMENTS], width / 2.0, color);
    painter.add(Shape::line(points, Stroke::new(width, color)));
}

/// The time now, scheduling the next shared frame; `None` when `rect` is off screen, which then
/// costs nothing.
fn tick(ui: &Ui, rect: Rect) -> Option<f64> {
    if !ui.is_rect_visible(rect) {
        return None;
    }
    let time = ui.input(|i| i.time);
    // Snapped to a shared tick so spinners and other timed repaints land on the same frames.
    let frame = SPINNER_FRAME.as_secs_f64();
    let wait = frame - time.rem_euclid(frame);
    // A tick that is all but due now would cost an extra frame; wait for the next one.
    let wait = if wait < 0.002 { wait + frame } else { wait };
    ui.ctx().request_repaint_after(Duration::from_secs_f64(wait));
    Some(time)
}
