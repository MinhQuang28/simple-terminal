use eframe::egui::{
    Align, Align2, Button, Color32, FontId, Id, Label, Layout, Popup, PopupCloseBehavior, Response, RichText,
    ScrollArea, Sense, Shape, Stroke, Ui, UiBuilder, pos2, vec2,
};

use super::Action;
use super::agent_status;
use super::widgets::{AGENT, accent, ago, icon_button_frame, muted, right_aligned};
use crate::inbox::{Inbox, Notice, Source};
use crate::model::AppState;
use crate::model::now_secs;
use crate::session_pool::SessionPool;

const WIDTH: f32 = 340.0;
const MAX_HEIGHT: f32 = 380.0;
const ROW_HEIGHT: f32 = 44.0;

/// Bell with the unread count; clicking it opens the list of notices.
pub fn bell(ui: &mut Ui, state: &AppState, inbox: &Inbox, pool: &SessionPool, actions: &mut Vec<Action>) {
    let approvals = pending_approvals(state, pool);
    let response =
        bell_icon(ui, inbox.unread() + approvals.len(), !approvals.is_empty()).on_hover_text("Notifications");
    let filter = Id::unique("inbox-unread-only");
    Popup::from_toggle_button_response(&response)
        .layout(Layout::top_down(Align::Min))
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .width(WIDTH)
        .show(|ui| {
            approvals_section(ui, &approvals, actions);
            contents(ui, inbox, filter, actions);
        });
}

/// Agents currently waiting for permission, with where they are and what they ask.
struct Approval {
    source: Source,
    title: String,
    request: String,
}

fn pending_approvals(state: &AppState, pool: &SessionPool) -> Vec<Approval> {
    let mut out = Vec::new();
    for repo in state.projects.iter().flat_map(|p| &p.repos) {
        for tab in &repo.tabs {
            for pane in &tab.panes {
                let Some(request) = pool.get(pane.id).and_then(|s| s.approval_request()) else { continue };
                out.push(Approval {
                    source: Source { repo: repo.id, tab: tab.id, pane: pane.id },
                    title: format!("{} · {}", repo.name, tab.title),
                    request: request.to_string(),
                });
            }
        }
    }
    out
}

/// One row per waiting agent: allow or deny without switching to its pane.
fn approvals_section(ui: &mut Ui, approvals: &[Approval], actions: &mut Vec<Action>) {
    if approvals.is_empty() {
        return;
    }
    ui.label(RichText::new("Approvals").strong());
    ui.add_space(4.0);
    for approval in approvals {
        let Source { repo, tab, pane } = approval.source;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            right_aligned(ui, |ui| {
                if ui.small_button("Deny").clicked() {
                    actions.push(Action::AnswerApproval { pane, allow: false });
                }
                if ui.small_button("Allow").clicked() {
                    actions.push(Action::AnswerApproval { pane, allow: true });
                }
                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    let title = Label::new(RichText::new(&approval.title).color(muted(ui)).size(11.5));
                    if ui.add(title.truncate().selectable(false).sense(Sense::click())).clicked() {
                        actions.push(Action::SelectTab { repo, tab });
                        ui.close();
                    }
                    let request = RichText::new(&approval.request).color(AGENT).size(12.5);
                    ui.add(Label::new(request).truncate().selectable(false));
                });
            });
        });
        ui.add_space(4.0);
    }
    ui.separator();
}

fn contents(ui: &mut Ui, inbox: &Inbox, filter: Id, actions: &mut Vec<Action>) {
    let mut unread_only = ui.data(|d| d.get_temp(filter)).unwrap_or(false);
    // Title and filter only: the UI font may be a wide monospace one, so bulk actions sit in a
    // footer instead of crowding this row.
    ui.horizontal(|ui| {
        ui.label(RichText::new("Notifications").strong().size(13.0));
        right_aligned(ui, |ui| {
            if ui.selectable_label(unread_only, RichText::new("Unread").size(12.0)).clicked() {
                unread_only = !unread_only;
                ui.data_mut(|d| d.insert_temp(filter, unread_only));
            }
        });
    });
    ui.separator();
    let notices: Vec<&Notice> = inbox.notices().iter().rev().filter(|n| !unread_only || !n.read).collect();
    if notices.is_empty() {
        let text = if unread_only { "No unread notifications" } else { "Nothing yet" };
        ui.add_space(14.0);
        ui.vertical_centered(|ui| ui.label(RichText::new(text).color(muted(ui)).size(12.5)));
        ui.add_space(14.0);
        return;
    }
    let now = now_secs();
    ScrollArea::vertical().max_height(MAX_HEIGHT).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for notice in notices {
            if notice_row(ui, notice, now).clicked() {
                actions.push(Action::OpenNotice(notice.id));
                ui.close();
            }
        }
    });
    ui.separator();
    ui.horizontal(|ui| {
        let link = |ui: &mut Ui, text: &str| {
            ui.add(Button::new(RichText::new(text).color(muted(ui)).size(12.0)).frame(false))
        };
        if link(ui, "Mark all read").clicked() {
            actions.push(Action::MarkNoticesRead);
        }
        right_aligned(ui, |ui| {
            if link(ui, "Clear").clicked() {
                actions.push(Action::ClearNotices);
            }
        });
    });
}

fn notice_row(ui: &mut Ui, notice: &Notice, now: u64) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 6.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let color = agent_status::color(ui, notice.status);
    let dot = if notice.read { color.gamma_multiply(0.4) } else { color };
    ui.painter().circle_filled(pos2(rect.left() + 10.0, rect.center().y), 4.0, dot);
    let text_rect =
        rect.shrink2(vec2(0.0, 5.0)).with_min_x(rect.left() + 22.0).with_max_x(rect.right() - 6.0);
    let mut child = ui.new_child(UiBuilder::new().max_rect(text_rect).layout(Layout::top_down(Align::Min)));
    child.spacing_mut().item_spacing.y = 2.0;
    let primary = if notice.read { ui.visuals().text_color().gamma_multiply(0.75) } else { color };
    child.horizontal(|ui| {
        ui.add(Label::new(RichText::new(ago(notice.at, now)).color(muted(ui)).size(11.0)).selectable(false));
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.add(
                Label::new(RichText::new(&notice.label).strong().color(primary).size(12.5)).selectable(false),
            );
            ui.add(
                Label::new(RichText::new(&notice.title).color(muted(ui)).size(12.0))
                    .truncate()
                    .selectable(false),
            );
        });
    });
    if !notice.detail.is_empty() {
        let detail = ui.visuals().text_color().gamma_multiply(if notice.read { 0.6 } else { 0.85 });
        child.add(
            Label::new(RichText::new(&notice.detail).color(detail).size(12.0)).truncate().selectable(false),
        );
    }
    response
}

/// Outlined bell, with a badge showing `unread` when non-zero; orange while an approval waits.
fn bell_icon(ui: &mut Ui, unread: usize, urgent: bool) -> Response {
    let (rect, response, color) = icon_button_frame(ui, false);
    let stroke = Stroke::new(1.3, color);
    let c = rect.center();
    // Dome on straight shoulders that flare into the rim; the clapper hangs below.
    let (radius, dome_y, shoulder_y, rim_y) = (4.2, c.y - 1.5, c.y + 3.0, c.y + 4.5);
    let mut outline = vec![pos2(c.x - 6.0, rim_y), pos2(c.x - 4.6, shoulder_y)];
    outline.extend((0..=10).map(|i| {
        let angle = std::f32::consts::PI * (1.0 + i as f32 / 10.0);
        pos2(c.x + radius * angle.cos(), dome_y + radius * angle.sin())
    }));
    outline.extend([pos2(c.x + 4.6, shoulder_y), pos2(c.x + 6.0, rim_y)]);
    let painter = ui.painter();
    painter.add(Shape::closed_line(outline, stroke));
    painter.line_segment([pos2(c.x - 1.5, rim_y + 2.2), pos2(c.x + 1.5, rim_y + 2.2)], stroke);
    if unread > 0 {
        let badge = rect.right_top() + vec2(-7.0, 7.0);
        painter.circle_filled(badge, 5.5, if urgent { AGENT } else { accent(ui) });
        let count = if unread > 9 { "9+".to_string() } else { unread.to_string() };
        painter.text(badge, Align2::CENTER_CENTER, count, FontId::proportional(8.5), Color32::WHITE);
    }
    response
}

#[cfg(test)]
mod tests {
    use eframe::egui::{CentralPanel, Context, RawInput};
    use uuid::Uuid;

    use super::*;
    use crate::agent_events::AgentStatus;
    use crate::inbox::Source;

    #[test]
    fn lists_notices_and_honours_the_unread_filter() {
        let mut inbox = Inbox::default();
        let source = Source { repo: Uuid::nil(), tab: Uuid::nil(), pane: Uuid::nil() };
        inbox.push(source, AgentStatus::Done, "Done", "repo · tab".into(), String::new());
        inbox.push(
            source,
            AgentStatus::Error,
            "Command failed in 20s",
            "repo · tab".into(),
            "cargo test".into(),
        );
        inbox.mark_read(1);
        let ctx = Context::default();
        let filter = Id::unique("test-unread-only");
        let rows = |unread_only: bool| {
            ctx.data_mut(|d| d.insert_temp(filter, unread_only));
            let mut actions = Vec::new();
            let mut output = ctx.run_ui(RawInput::default(), |ui| {
                CentralPanel::default().show(ui, |ui| contents(ui, &inbox, filter, &mut actions));
            });
            output.textures_delta.clear();
            output.shapes.len()
        };
        assert!(rows(false) > rows(true));
    }
}
