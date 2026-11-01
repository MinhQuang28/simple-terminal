//! The top bar's branch chip: shows the checked-out branch and, clicked, lists the repo's local
//! branches to switch to, with a field to start a new one.

use std::sync::Arc;

use eframe::egui::epaint::CubicBezierShape;
use eframe::egui::{
    Align, Button, Color32, CornerRadius, FontId, Key, Layout, Popup, PopupCloseBehavior, Rect, RectAlign,
    Response, RichText, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, Ui, pos2, vec2,
};

use super::Action;
use super::design::{SURFACE_PADDING, TEXT_BODY, TEXT_SECONDARY, empty_state, section_label, surface_frame};
use super::sidebar::tab_status;
use super::widgets::muted;
use crate::agent_events::AgentStatus;
use crate::git_branch;
use crate::model::Repo;
use crate::session_pool::SessionPool;

const POPUP_WIDTH: f32 = 260.0;
const MAX_HEIGHT: f32 = 320.0;

type Branches = Arc<Result<Vec<String>, String>>;

/// The branch chip with its switch menu.
pub fn chip(ui: &mut Ui, repo: &Repo, branch: &str, pool: &SessionPool, actions: &mut Vec<Action>) {
    let response = chip_button(ui, branch).on_hover_text("Switch branch");
    let popup_id = Popup::default_response_id(&response);
    let cache = popup_id.with("branches");
    let new_name = popup_id.with("new");
    // Read afresh each time the menu opens.
    if !Popup::is_id_open(ui.ctx(), popup_id) {
        ui.data_mut(|d| {
            d.remove::<Branches>(cache);
            d.remove::<String>(new_name);
        });
    }
    Popup::from_toggle_button_response(&response)
        .align(RectAlign::BOTTOM_START)
        .gap(6.0)
        .layout(Layout::top_down_justified(Align::Min))
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .frame(surface_frame(ui.visuals(), SURFACE_PADDING))
        .width(POPUP_WIDTH)
        .show(|ui| {
            section_label(ui, "Switch branch");
            let working = working_agents(repo, pool);
            if working > 0 {
                let who = if working == 1 { "An agent is" } else { "Agents are" };
                let text = format!("{who} working here; switching changes the files under it.");
                let warn = ui.visuals().warn_fg_color;
                ui.label(RichText::new(text).size(TEXT_SECONDARY).color(warn));
            }
            let branches: Branches = ui.data_mut(|d| {
                d.get_temp_mut_or_insert_with(cache, || Arc::new(git_branch::local(&repo.path))).clone()
            });
            match branches.as_ref() {
                Err(err) => empty_state(ui, err),
                Ok(list) if list.is_empty() => empty_state(ui, "No local branches"),
                Ok(list) => {
                    ScrollArea::vertical().max_height(MAX_HEIGHT).show(ui, |ui| {
                        for name in list {
                            let current = name == branch;
                            let text = RichText::new(name).size(TEXT_BODY);
                            if ui.add(Button::selectable(current, text)).clicked() && !current {
                                let branch = name.clone();
                                actions.push(Action::SwitchBranch { repo: repo.id, branch, create: false });
                                ui.close();
                            }
                        }
                    });
                }
            }
            ui.separator();
            let mut name = ui.data_mut(|d| d.get_temp::<String>(new_name)).unwrap_or_default();
            let field = ui.add(TextEdit::singleline(&mut name).hint_text("New branch from here…"));
            let entered = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            if entered && !name.trim().is_empty() {
                let branch = name.trim().to_string();
                actions.push(Action::SwitchBranch { repo: repo.id, branch, create: true });
                name.clear();
                ui.close();
            }
            ui.data_mut(|d| d.insert_temp(new_name, name));
        });
}

/// Agent tabs in `repo` mid-turn: working, or stopped on a permission prompt.
fn working_agents(repo: &Repo, pool: &SessionPool) -> usize {
    let mid_turn = |s| matches!(s, AgentStatus::Working | AgentStatus::NeedsApproval);
    repo.tabs.iter().filter(|t| tab_status(t, pool).is_some_and(mid_turn)).count()
}

/// A quiet outlined pill, lit on hover: it labels the repo first and opens a menu second.
fn chip_button(ui: &mut Ui, branch: &str) -> Response {
    let color = ui.visuals().text_color().gamma_multiply(0.85);
    let galley = ui.painter().layout_no_wrap(branch.to_string(), FontId::proportional(12.5), color);
    let (pad, icon, gap) = (8.0, 12.0, 5.0);
    let size = vec2(pad + icon + gap + galley.size().x + pad, 22.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();
    if response.hovered() || Popup::is_id_open(ui.ctx(), Popup::default_response_id(&response)) {
        painter.rect_filled(rect, CornerRadius::same(6), ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let outline = Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color);
    painter.rect_stroke(rect, CornerRadius::same(6), outline, StrokeKind::Inside);
    let glyph = Rect::from_min_size(pos2(rect.left() + pad, rect.center().y - icon / 2.0), vec2(icon, icon));
    branch_glyph(ui, glyph, muted(ui));
    painter.galley(pos2(glyph.right() + gap, rect.center().y - galley.size().y / 2.0), galley, color);
    response
}

/// Lucide's git-branch in a 12-point square: a trunk, two nodes, and a branch curving off it.
pub fn branch_glyph(ui: &Ui, r: Rect, color: Color32) {
    let stroke = Stroke::new(1.2, color);
    let p = |x: f32, y: f32| pos2(r.left() + x, r.top() + y);
    let painter = ui.painter();
    painter.line_segment([p(3.0, 1.0), p(3.0, 7.5)], stroke);
    painter.circle_stroke(p(9.0, 3.0), 1.6, stroke);
    painter.circle_stroke(p(3.0, 9.0), 1.6, stroke);
    let curve = [p(9.0, 4.6), p(9.0, 7.0), p(7.0, 9.0), p(4.6, 9.0)];
    painter.add(CubicBezierShape::from_points_stroke(curve, false, Color32::TRANSPARENT, stroke));
}

/// The glyph inline at 12 points, for a row of labels.
pub fn inline_glyph(ui: &mut Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
    branch_glyph(ui, rect, color);
}
