use eframe::egui::{Color32, Ui};

use super::widgets::{AGENT, accent, tag};
use crate::agent_events::AgentStatus;

pub fn color(ui: &Ui, status: AgentStatus) -> Color32 {
    match status {
        AgentStatus::Idle | AgentStatus::Working => ui.visuals().weak_text_color(),
        AgentStatus::Done => accent(ui),
        AgentStatus::NeedsInput | AgentStatus::NeedsApproval => AGENT,
        AgentStatus::Error => Color32::from_rgb(0xe5, 0x53, 0x53),
    }
}

/// Small rounded label naming what the agent is waiting for.
pub fn pill(ui: &mut Ui, status: AgentStatus) {
    tag(ui, status.label(), color(ui, status));
}
