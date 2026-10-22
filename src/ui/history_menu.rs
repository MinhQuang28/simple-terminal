use eframe::egui::{
    Align, CornerRadius, Label, Layout, Popup, PopupCloseBehavior, RectAlign, Response, RichText, ScrollArea,
    Sense, Stroke, Ui, UiBuilder, pos2, vec2,
};

use super::Action;
use super::claude_icon;
use super::widgets::{AGENT, ago, icon_button_frame, muted, spinner};
use crate::claude_history::{ClaudeHistory, Session};
use crate::model::{Repo, now_secs};

const POPUP_WIDTH: f32 = 400.0;
const ROW_HEIGHT: f32 = 40.0;
/// Transcript sizes at which one more activity dot lights up.
const DOT_STEPS: [u64; 7] = [16 << 10, 64 << 10, 256 << 10, 1 << 20, 4 << 20, 16 << 20, 64 << 20];

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
                            if session_row(ui, session, now).clicked() {
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

/// Claude mark in a ring, the title, when it was last active, and dots for how long it ran.
fn session_row(ui: &mut Ui, session: &Session, now: u64) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(6), ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let ring = pos2(rect.left() + 20.0, rect.center().y);
    ui.painter().circle_stroke(ring, 11.0, Stroke::new(1.2, AGENT));
    claude_icon::paint(ui.painter(), eframe::egui::Rect::from_center_size(ring, vec2(11.0, 11.0)));

    let text_rect = rect.with_min_x(rect.left() + 40.0).with_max_x(rect.right() - 10.0);
    let mut child =
        ui.new_child(UiBuilder::new().max_rect(text_rect).layout(Layout::right_to_left(Align::Center)));
    child.spacing_mut().item_spacing.x = 10.0;
    activity_dots(&mut child, session.size);
    let when = match ago(session.modified, now).as_str() {
        "" => String::new(),
        "now" => "just now".into(),
        when => format!("{when} ago"),
    };
    child.add(Label::new(RichText::new(when).size(12.0).color(muted(&child))).selectable(false));
    child.add(Label::new(RichText::new(&session.title).size(13.5)).truncate().selectable(false));
    response
}

/// Eight dots, lit from the left by the transcript's size on a log scale.
fn activity_dots(ui: &mut Ui, size: u64) {
    let lit = 1 + DOT_STEPS.iter().filter(|&&step| size >= step).count();
    let (rect, response) = ui.allocate_exact_size(vec2(8.0 * 5.0, 10.0), Sense::hover());
    let text = ui.visuals().text_color();
    for i in 0..8 {
        let center = pos2(rect.left() + 2.5 + i as f32 * 5.0, rect.center().y);
        let color = if i < lit { text.gamma_multiply(0.7) } else { text.gamma_multiply(0.18) };
        ui.painter().circle_filled(center, 1.5, color);
    }
    response.on_hover_text(format!("{} transcript", human_size(size)));
}

fn human_size(bytes: u64) -> String {
    match bytes {
        0..1_024 => format!("{bytes} B"),
        1_024..1_048_576 => format!("{:.0} KB", bytes as f64 / 1_024.0),
        _ => format!("{:.1} MB", bytes as f64 / 1_048_576.0),
    }
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
