//! Peek: another tab's pane shown live over the current one, to read what an agent did or is
//! asking without losing your place. `Esc` or a click outside closes it; "Go to" switches there.

use eframe::egui::{Align, Context, Id, Label, Layout, Modal, RichText, Sense, Ui};
use uuid::Uuid;

use super::Action;
use super::design;
use super::dialog_kit::{Tone, button};
use super::sidebar::tab_title;
use super::widgets::muted;
use crate::model::{AppState, Pane, Repo, Tab};
use crate::session_pool::SessionPool;
use crate::terminal::{TerminalStyle, peek_view};

/// Share of the window the overlay covers.
const COVER: f32 = 0.8;

pub fn show(ctx: &Context, state: &AppState, pool: &mut SessionPool, pane: Uuid, actions: &mut Vec<Action>) {
    let Some((repo, tab, pane)) = find(state, pane) else {
        actions.push(Action::ClosePeek);
        return;
    };
    let frame = design::surface_frame(&ctx.global_style().visuals, 12);
    let size = ctx.content_rect().size() * COVER;
    let modal = Modal::new(Id::unique("peek")).frame(frame).show(ctx, |ui| {
        ui.set_min_size(size);
        ui.set_max_size(size);
        header(ui, repo, tab, pool, pane.id, actions);
        ui.add_space(8.0);
        let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
        let cwd = pane.cwd.as_deref().unwrap_or(&repo.path);
        match pool.get_or_spawn(pane, cwd, &state.settings, ui.ctx()) {
            Ok(session) => peek_view::show(ui, session, &TerminalStyle::new(&state.settings), rect),
            Err(err) => {
                ui.put(rect, Label::new(RichText::new(err).color(muted(ui))));
            }
        }
    });
    if modal.should_close() {
        actions.push(Action::ClosePeek);
    }
}

fn header(ui: &mut Ui, repo: &Repo, tab: &Tab, pool: &SessionPool, pane: Uuid, actions: &mut Vec<Action>) {
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            if button(ui, "Close", Tone::Secondary).clicked() {
                actions.push(Action::ClosePeek);
            }
            if button(ui, "Go to", Tone::Primary).clicked() {
                actions.push(Action::GoToPane(pane));
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.add_space(4.0);
                let title = tab_title(tab, pool, "Terminal");
                ui.add(Label::new(RichText::new(title).strong()).truncate().selectable(false));
                ui.add_space(6.0);
                ui.label(RichText::new(&repo.name).color(muted(ui)));
            });
        });
    });
}

fn find(state: &AppState, pane: Uuid) -> Option<(&Repo, &Tab, &Pane)> {
    state
        .projects
        .iter()
        .flat_map(|p| &p.repos)
        .find_map(|r| r.tabs.iter().find_map(|t| t.panes.iter().find(|p| p.id == pane).map(|p| (r, t, p))))
}
