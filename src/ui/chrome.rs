use eframe::egui::{
    CornerRadius, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, ViewportCommand, pos2, vec2,
};

use super::Action;
use super::inbox_panel;
use super::usage_panel;
use super::widgets::{icon_button_frame, muted, right_aligned};
use crate::font_catalog::SYSTEM_FONT;
use crate::git_status::GitStatus;
use crate::inbox::Inbox;
use crate::model::AppState;
use crate::session_pool::SessionPool;

/// Makes `rect` drag the window, since the native titlebar is hidden.
/// Must run before widgets in that area so they stay clickable on top.
pub fn window_drag_area(ui: &mut Ui, rect: Rect) {
    let response = ui.interact(rect, ui.scope_id().with("window-drag"), Sense::click_and_drag());
    if response.drag_started() {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
    if response.double_clicked() {
        let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
        ui.ctx().send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    }
}

pub fn top_bar(
    ui: &mut Ui,
    state: &AppState,
    status: Option<&GitStatus>,
    inbox: &Inbox,
    pool: &SessionPool,
    context: Option<f64>,
    actions: &mut Vec<Action>,
) {
    window_drag_area(ui, ui.max_rect());
    ui.horizontal_centered(|ui| {
        let project = state
            .selected_repo
            .and_then(|id| state.projects.iter().find(|p| p.repos.iter().any(|r| r.id == id)));
        let repo = state.selected_repo.and_then(|id| state.repo(id));
        if let (Some(project), Some(repo)) = (project, repo) {
            ui.label(RichText::new(&project.name).color(muted(ui)));
            ui.label(RichText::new("›").color(muted(ui)));
            ui.label(RichText::new(&repo.name).strong());
            if let Some(status) = status {
                ui.add_space(6.0);
                ui.label(RichText::new(format!("⎇ {}", status.branch)).color(muted(ui)).size(12.5));
            }
        }
        right_aligned(ui, |ui| {
            ui.add_space(4.0);
            if side_panel_icon(ui, state.show_side_panel).on_hover_text("Toggle files panel (⌘B)").clicked()
            {
                actions.push(Action::ToggleSidePanel);
            }
            inbox_panel::bell(ui, state, inbox, pool, actions);
            usage_panel::gauge(ui, state, pool.latest_limits().as_ref(), context);
            if search_icon(ui).on_hover_text("Find in terminal (⌘F)").clicked() {
                actions.push(Action::OpenFind);
            }
        });
    });
}

/// Outlined window with a divider marking the right-hand panel.
fn side_panel_icon(ui: &mut Ui, active: bool) -> Response {
    let (rect, response, color) = icon_button_frame(ui, active);
    let stroke = Stroke::new(1.3, color);
    let frame = Rect::from_center_size(rect.center(), vec2(15.0, 12.0));
    let painter = ui.painter();
    painter.rect_stroke(frame, CornerRadius::same(3), stroke, StrokeKind::Inside);
    let x = frame.right() - 5.0;
    painter.line_segment([pos2(x, frame.top()), pos2(x, frame.bottom())], stroke);
    response
}

/// Magnifying glass.
fn search_icon(ui: &mut Ui) -> Response {
    let (rect, response, color) = icon_button_frame(ui, false);
    let stroke = Stroke::new(1.5, color);
    let center = rect.center() + vec2(-1.5, -1.5);
    let painter = ui.painter();
    painter.circle_stroke(center, 5.0, stroke);
    let start = center + vec2(3.6, 3.6);
    painter.line_segment([start, start + vec2(3.5, 3.5)], stroke);
    response
}

pub fn status_bar(ui: &mut Ui, state: &AppState, status: Option<&GitStatus>) {
    ui.horizontal_centered(|ui| {
        ui.add_space(4.0);
        let Some(status) = status else { return };
        ui.label(RichText::new(format!("⎇ {}", status.branch)).color(muted(ui)).size(12.0));
        if status.ahead > 0 || status.behind > 0 {
            ui.label(
                RichText::new(format!("↑{} ↓{}", status.ahead, status.behind)).color(muted(ui)).size(12.0),
            );
        }
        if status.changed > 0 {
            let color = state.settings.theme.modified();
            ui.label(RichText::new(format!("● {} changed", status.changed)).color(color).size(12.0));
        }
        right_aligned(ui, |ui| {
            ui.add_space(8.0);
            let font =
                if state.settings.font_family.is_empty() { SYSTEM_FONT } else { &state.settings.font_family };
            let info = format!("{} · {} {}pt", state.settings.theme.name, font, state.settings.font_size);
            ui.label(RichText::new(info).color(muted(ui)).size(11.5));
        });
    });
}
