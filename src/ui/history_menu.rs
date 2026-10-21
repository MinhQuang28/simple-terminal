use eframe::egui::{
    Align, CornerRadius, Label, Layout, Popup, PopupCloseBehavior, RectAlign, Response, RichText, ScrollArea,
    Sense, Stroke, Ui, pos2, vec2,
};

use super::Action;
use super::widgets::{ago, icon_button_frame, muted, spinner};
use crate::claude_history::ClaudeHistory;
use crate::model::{Repo, now_secs};

const POPUP_WIDTH: f32 = 360.0;

/// Clock button that lists the repo's past Claude conversations; picking one resumes it in a tab.
pub fn show(ui: &mut Ui, repo: &Repo, history: &ClaudeHistory, actions: &mut Vec<Action>) {
    let button = clock_button(ui).on_hover_text("Claude conversation history");
    Popup::from_toggle_button_response(&button)
        .align(RectAlign::BOTTOM_END)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .width(POPUP_WIDTH)
        .show(|ui| {
            ui.set_width(POPUP_WIDTH);
            match history.sessions(&repo.path) {
                None => {
                    ui.horizontal(|ui| {
                        spinner(ui, 12.0, muted(ui));
                        ui.label(RichText::new("Loading conversations…").color(muted(ui)));
                    });
                }
                Some(sessions) if sessions.is_empty() => {
                    ui.add_space(10.0);
                    ui.label(RichText::new("No conversation history").color(muted(ui)));
                    ui.add_space(10.0);
                }
                Some(sessions) => {
                    let now = now_secs();
                    ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                        for session in sessions.iter() {
                            if session_row(ui, &session.title, &ago(session.modified, now)).clicked() {
                                actions.push(Action::ResumeSession {
                                    repo: repo.id,
                                    session: session.id.clone(),
                                    title: session.title.clone(),
                                });
                                ui.close();
                            }
                        }
                    });
                }
            }
        });
}

fn session_row(ui: &mut Ui, title: &str, when: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(6), ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let mut child = ui.new_child(
        eframe::egui::UiBuilder::new()
            .max_rect(rect.shrink2(vec2(8.0, 0.0)))
            .layout(Layout::right_to_left(Align::Center)),
    );
    child.label(RichText::new(when).size(11.5).color(muted(&child)));
    child.add(Label::new(RichText::new(title).size(13.0)).truncate().selectable(false));
    response
}

fn clock_button(ui: &mut Ui) -> Response {
    // The popup is keyed by the button's response id, which is the next auto id.
    let popup_id = ui.next_auto_id().with("popup");
    let open = Popup::is_id_open(ui.ctx(), popup_id);
    let (rect, response, color) = icon_button_frame(ui, open);
    debug_assert_eq!(Popup::default_response_id(&response), popup_id);
    let stroke = Stroke::new(1.4, color);
    let center = rect.center();
    let painter = ui.painter();
    painter.circle_stroke(center, 7.0, stroke);
    painter.line_segment([center, pos2(center.x, center.y - 4.0)], stroke);
    painter.line_segment([center, pos2(center.x + 3.0, center.y + 1.5)], stroke);
    response
}
