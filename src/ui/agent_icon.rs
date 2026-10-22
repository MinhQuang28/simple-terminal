//! The icon for whichever agent a pane runs.

use eframe::egui::{Align2, FontId, Painter, Rect, Sense, Ui, Vec2};

use super::{claude_icon, codex_icon};
use crate::agent_launch::Agent;

/// Width of the icon drawn `height` points tall.
pub fn width(height: f32, _agent: Agent) -> f32 {
    height
}

/// Paints the agent's mark centered in `rect`; `text` is the surrounding text colour.
pub fn paint(painter: &Painter, rect: Rect, agent: Agent, text: eframe::egui::Color32) {
    match agent {
        Agent::Claude => claude_icon::paint(painter, rect),
        Agent::Codex => codex_icon::paint(painter, rect, text),
        Agent::OpenCode => {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                "◇",
                FontId::proportional(rect.height()),
                text,
            );
        }
    }
}

/// Allocates and paints the icon inline, `size` points tall.
pub fn icon(ui: &mut Ui, size: f32, agent: Agent) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    paint(ui.painter(), rect, agent, ui.visuals().text_color());
}
