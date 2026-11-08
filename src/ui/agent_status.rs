use eframe::egui::{Color32, Ui};

use super::spinner::{blocked_color, done_color, dot, working_color};
use super::widgets::tag;
use crate::agent_events::AgentStatus;

/// Herdr's status colors: yellow working, blue done, warm red for anything waiting on the user.
pub fn color(ui: &Ui, status: AgentStatus) -> Color32 {
    match status {
        AgentStatus::Idle => ui.visuals().weak_text_color(),
        AgentStatus::Working => working_color(ui),
        AgentStatus::Done => done_color(ui),
        AgentStatus::NeedsInput | AgentStatus::NeedsApproval | AgentStatus::Error => blocked_color(ui),
    }
}

/// The status mark beside a row: a dot once done, as Herdr marks it, and a small rounded label
/// naming what the agent waits for otherwise.
pub fn pill(ui: &mut Ui, status: AgentStatus) {
    if status == AgentStatus::Done {
        dot(ui, 11.0, color(ui, status)).on_hover_text(status.label());
    } else {
        tag(ui, status.label(), color(ui, status));
    }
}
