use eframe::egui::{
    Align, Align2, CornerRadius, FontId, Layout, Rect, RectAlign, Response, RichText, Sense, Stroke,
    StrokeKind, Ui, ViewportCommand, pos2, vec2,
};

use super::Action;
use super::branch_menu;
use super::context_ring;
use super::design::{RADIUS_ROW, TEXT_BODY, TEXT_CAPTION, TEXT_SECONDARY, focus_ring};
use super::inbox_panel;
use super::model_menu;
use super::open_in_menu;
use super::usage_panel;
use super::widgets::{icon_button_frame, muted, right_aligned, text_button, tinted_text_button};
use crate::agent_launch::Agent;
use crate::claude_models::ModelOption;
use crate::claude_status_line::StatusLine;
use crate::font_catalog::SYSTEM_FONT;
use crate::git_status::GitStatus;
use crate::inbox::Inbox;
use crate::model::{AppState, Repo, SidePanel};
use crate::session_pool::SessionPool;

/// Height of the view switch's pills, which fit the status bar.
const VIEW_SWITCH_HEIGHT: f32 = 22.0;

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
    (agent, limits, context): (Agent, Option<&StatusLine>, Option<f64>),
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
                branch_menu::chip(ui, repo, &status.branch, pool, actions);
            }
        }
        right_aligned(ui, |ui| {
            ui.add_space(4.0);
            // The sidebar and gitignored-files toggles belong to the Code view.
            let code = state.selected_repo.and_then(|id| state.repo(id)).is_some_and(|r| r.code_view);
            if code
                && side_panel_icon(ui, state.show_code_panel)
                    .on_hover_text(crate::keymap_hint::with_hint("Toggle sidebar", "panel.toggle"))
                    .clicked()
            {
                actions.push(Action::ToggleSidePanel);
            }
            if code {
                let hint =
                    if state.show_ignored_files { "Hide gitignored files" } else { "Show gitignored files" };
                if ignored_icon(ui, state.show_ignored_files).on_hover_text(hint).clicked() {
                    actions.push(Action::ToggleIgnoredFiles);
                }
            }
            inbox_panel::bell(ui, state, inbox, pool, actions);
            usage_panel::gauge(ui, state, agent, limits, context, actions);
            if let Some(repo) = state.selected_repo.and_then(|id| state.repo(id)) {
                open_in_menu::show(ui, repo, actions);
            }
            if search_icon(ui).on_hover_text("Find in terminal (⌘F)").clicked() {
                actions.push(Action::OpenFind);
            }
        });
    });
}

/// Pills at the status bar's right end that switch the repo between its terminals and its code,
/// each with its key.
fn view_switch(ui: &mut Ui, repo: &Repo, actions: &mut Vec<Action>) {
    ui.spacing_mut().item_spacing.x = 6.0;
    // Laid out from the right, so the last one listed comes first.
    let views = [("Code", "view.code", true), ("Terminals", "view.terminals", false)];
    for (label, command, code) in views {
        let selected = repo.code_view == code;
        let color = if selected { ui.visuals().strong_text_color() } else { muted(ui) };
        let name = ui.painter().layout_no_wrap(label.into(), FontId::proportional(TEXT_BODY), color);
        let keys = crate::keymap_hint::hint(command);
        let key = ui.painter().layout_no_wrap(keys, FontId::proportional(TEXT_CAPTION), muted(ui));
        let key_width = if key.size().x > 0.0 { key.size().x + 7.0 } else { 0.0 };
        let size = vec2(12.0 + name.size().x + key_width + 12.0, VIEW_SWITCH_HEIGHT);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        let visuals = &ui.visuals().widgets;
        let radius = CornerRadius::same((VIEW_SWITCH_HEIGHT / 2.0) as u8);
        if selected {
            ui.painter().rect_filled(rect, radius, visuals.inactive.weak_bg_fill);
        } else if response.hovered() {
            ui.painter().rect_filled(rect, radius, visuals.hovered.weak_bg_fill);
        }
        ui.painter().rect_stroke(rect, radius, visuals.noninteractive.bg_stroke, StrokeKind::Inside);
        focus_ring(ui, &response, RADIUS_ROW);
        let y = rect.center().y;
        let name_width = name.size().x;
        ui.painter().galley(pos2(rect.left() + 12.0, y - name.size().y / 2.0), name, color);
        let key_pos = pos2(rect.left() + 12.0 + name_width + 7.0, y - key.size().y / 2.0);
        ui.painter().galley(key_pos, key, color);
        if response.clicked() && !selected {
            actions.push(Action::ShowCode(code));
        }
    }
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

/// Circled slash, lit while gitignored files are listed in the files panel.
fn ignored_icon(ui: &mut Ui, active: bool) -> Response {
    let (rect, response, color) = icon_button_frame(ui, active);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, "⊘", FontId::proportional(17.0), color);
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

/// Each part leads to where it is changed: the branch to the switch menu, the change count to
/// Source Control, the theme and font to Settings. `model` is the one answering in the active
/// Claude pane, if any.
/// What the status bar shows of the Claude Code session in the active pane.
pub struct AgentInfo<'a> {
    /// The model answering, once Claude Code has reported it.
    pub model: Option<&'a str>,
    /// Share of the context window in use, 0 to 100.
    pub context: Option<f64>,
    /// Models the session can be switched to.
    pub models: &'a [ModelOption],
    /// Whether the model can be switched from here, which only Claude Code offers.
    pub switchable: bool,
}

pub fn status_bar(
    ui: &mut Ui,
    state: &AppState,
    status: Option<&GitStatus>,
    agent: &AgentInfo<'_>,
    pool: &SessionPool,
    actions: &mut Vec<Action>,
) {
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        right_aligned(ui, |ui| {
            if let Some(repo) = state.selected_repo.and_then(|id| state.repo(id)) {
                ui.add_space(6.0);
                ui.scope(|ui| view_switch(ui, repo, actions));
                ui.add_space(8.0);
            }
            let font =
                if state.settings.font_family.is_empty() { SYSTEM_FONT } else { &state.settings.font_family };
            let info = format!("{} · {} {}pt", state.settings.theme.name, font, state.settings.font_size);
            let settings = crate::keymap_hint::with_hint("Settings", "settings.open");
            if text_button(ui, &info, TEXT_CAPTION).on_hover_text(settings).clicked() {
                actions.push(Action::OpenSettings);
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                let repo = state.selected_repo.and_then(|id| state.repo(id));
                let (Some(status), Some(repo)) = (status, repo) else { return };
                ui.add_space(4.0);
                branch_menu::inline_glyph(ui, muted(ui));
                let branch = text_button(ui, &status.branch, TEXT_SECONDARY).on_hover_text("Switch branch");
                branch_menu::switch_menu(
                    ui,
                    &branch,
                    RectAlign::TOP_START,
                    repo,
                    &status.branch,
                    pool,
                    actions,
                );
                if status.ahead > 0 || status.behind > 0 {
                    let sync = format!("↑{} ↓{}", status.ahead, status.behind);
                    ui.label(RichText::new(sync).color(muted(ui)).size(TEXT_SECONDARY));
                    ui.add_space(4.0);
                }
                if status.changed > 0 {
                    let color = state.settings.theme.modified();
                    let changed = format!("● {} changed", status.changed);
                    let button = tinted_text_button(ui, &changed, TEXT_SECONDARY, color, color);
                    if button.on_hover_text("Open Source Control").clicked() {
                        actions.push(Action::SetSidePanel(SidePanel::Changes));
                    }
                }
                if let Some(model) = agent.model {
                    ui.add_space(4.0);
                    if agent.switchable {
                        model_menu::show(ui, model, agent.models, actions);
                    } else {
                        ui.label(RichText::new(format!("✦ {model}")).size(TEXT_SECONDARY).color(muted(ui)));
                    }
                }
                if let Some(context) = agent.context {
                    ui.add_space(6.0);
                    let hint = format!("Context window: {context:.0}% used");
                    context_ring::show(ui, context, VIEW_SWITCH_HEIGHT, &state.settings.theme)
                        .on_hover_text(hint);
                }
            });
        });
    });
}
