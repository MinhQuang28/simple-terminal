//! The tab strip's "+": a popup of what a new tab can run, one row each with its icon and keys.

use eframe::egui::{
    Align, Button, Label, Layout, Popup, PopupCloseBehavior, RectAlign, RichText, Ui, UiBuilder,
};

use super::Action;
use super::design::{TEXT_SECONDARY, divider, surface_frame};
use super::program_icon;
use super::widgets::{muted, right_aligned, row_sized};
use crate::agent_launch::Agent;
use crate::model::{AppState, PaneKind, Repo};
use crate::programs::{self, Program};

const WIDTH: f32 = 290.0;
const ROW_HEIGHT: f32 = 34.0;
const ICON_SIZE: f32 = 16.0;
const TEXT: f32 = 14.0;

/// The "+" button and, while open, its popup.
pub fn show(ui: &mut Ui, state: &AppState, repo: &Repo, actions: &mut Vec<Action>) {
    let button = ui.add(Button::new(RichText::new("+").size(19.0)).frame_when_inactive(false));
    Popup::from_toggle_button_response(&button)
        .align(RectAlign::BOTTOM_START)
        .gap(6.0)
        .layout(Layout::top_down(Align::Min))
        .close_behavior(PopupCloseBehavior::CloseOnClick)
        .frame(surface_frame(ui.visuals(), 6))
        .width(WIDTH)
        .show(|ui| {
            ui.set_width(WIDTH - 12.0);
            ui.spacing_mut().item_spacing.y = 2.0;
            let claude = programs::agent(Agent::Claude);
            let new_tab = |kind| Action::NewTab { repo: repo.id, kind };
            // The kind the new-tab key opens shows that key; Claude Code otherwise has its own.
            let keys = |kind, own| if state.settings.new_tab == kind { "tab.new" } else { own };
            if row(ui, claude, "Claude Code", keys(PaneKind::Agent, "tab.new_agent")) {
                actions.push(new_tab(PaneKind::Agent));
            }
            if row(ui, claude, "Claude Code in a worktree", "tab.new_agent_worktree") {
                actions.push(Action::NewWorktreeAgent(repo.id));
            }
            let presets = state.settings.presets.iter().enumerate();
            for (i, (preset, p)) in presets.filter(|(_, p)| !p.command.trim().is_empty()).enumerate() {
                if i == 0 {
                    divider(ui);
                }
                let name = if p.name.trim().is_empty() { p.command.trim() } else { p.name.trim() };
                let program = programs::for_command(&p.command).unwrap_or(programs::shell());
                if row(ui, program, name, "") {
                    actions.push(Action::RunPreset { repo: repo.id, preset });
                }
            }
            divider(ui);
            if row(ui, programs::shell(), "Terminal", keys(PaneKind::Shell, "")) {
                actions.push(new_tab(PaneKind::Shell));
            }
        });
}

/// A row with its icon, label and the keys of `command`; returns whether it was clicked.
fn row(ui: &mut Ui, program: &Program, label: &str, command: &str) -> bool {
    let (response, ()) = row_sized(ui, false, 6.0, ROW_HEIGHT, |ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        program_icon::icon(ui, ICON_SIZE, program);
        ui.add(Label::new(RichText::new(label).size(TEXT)).selectable(false).truncate());
        let keys = crate::keymap_hint::hint(command);
        let rest = ui.available_rect_before_wrap();
        ui.scope_builder(UiBuilder::new().max_rect(rest), |ui| {
            right_aligned(ui, |ui| {
                ui.add_space(6.0);
                ui.add(
                    Label::new(RichText::new(keys).size(TEXT_SECONDARY).color(muted(ui))).selectable(false),
                );
            });
        });
    });
    response.clicked()
}
