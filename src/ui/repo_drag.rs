use eframe::egui::{
    DragAndDrop, FontId, Id, LayerId, Order, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, vec2,
};
use uuid::Uuid;

use super::Action;
use crate::file_drag::FileDrag;
use crate::model::AppState;

/// The repo carried while its sidebar row is dragged.
struct RepoDrag(Uuid);

/// Where a repo block sits in the sidebar, for working out where a dragged repo would land.
pub struct RepoSpan {
    pub project: Uuid,
    pub repo: Uuid,
    pub rect: Rect,
}

/// Lets the repo row be picked up and dragged to another place or project; clicks still select it.
pub fn drag_source(row: Response, repo: Uuid) -> Response {
    let row = row.interact(Sense::click_and_drag());
    row.dnd_set_drag_payload(RepoDrag(repo));
    row
}

/// Shows where a dragged repo would land and moves it there on release: between the repos of the
/// project under the pointer, or at the end of a collapsed or empty one.
/// `viewport` is the visible part of the project list, `projects` each project's area in it.
pub fn drop_target(
    ui: &Ui,
    state: &AppState,
    viewport: Rect,
    projects: &[(Uuid, Rect)],
    repos: &[RepoSpan],
    actions: &mut Vec<Action>,
) {
    let Some(drag) = DragAndDrop::payload::<RepoDrag>(ui.ctx()) else { return };
    let Some(pointer) = ui.ctx().pointer_latest_pos() else { return };
    let Some(repo) = state.repo(drag.0) else { return };
    ghost(ui, &repo.name, pointer);
    if !viewport.contains(pointer) {
        return;
    }
    let Some(&(project, area)) = projects.iter().find(|(_, rect)| rect.contains(pointer)) else { return };
    let home = state.projects.iter().find(|p| p.repos.iter().any(|r| r.id == repo.id));
    // Positions count the other repos only, as the dragged one leaves its place first.
    let others: Vec<Rect> =
        repos.iter().filter(|s| s.project == project && s.repo != repo.id).map(|s| s.rect).collect();
    let index = others.iter().filter(|r| r.center().y < pointer.y).count();
    if let Some(home) = home.filter(|p| p.id == project) {
        let current = home.repos.iter().position(|r| r.id == repo.id);
        if others.is_empty() || current == Some(index) {
            return;
        }
    } else {
        FileDrag::paint_target(ui, area.intersect(viewport).expand(2.0));
    }
    if let Some(y) = insertion_y(&others, index) {
        let line = ui.painter().with_clip_rect(viewport);
        line.hline(area.x_range().shrink(8.0), y, Stroke::new(2.0, ui.visuals().hyperlink_color));
    }
    if ui.input(|i| i.pointer.any_released()) {
        DragAndDrop::clear_payload(ui.ctx());
        let index = if others.is_empty() { usize::MAX } else { index };
        actions.push(Action::MoveRepo { repo: repo.id, project, index });
    }
}

/// The gap above the `index`th repo, or below the last one.
fn insertion_y(others: &[Rect], index: usize) -> Option<f32> {
    match others.get(index) {
        Some(rect) => Some(rect.top()),
        None => others.last().map(|rect| rect.bottom() - 3.0),
    }
}

/// The repo's name following the pointer, above everything else.
fn ghost(ui: &Ui, name: &str, pointer: Pos2) {
    let painter = ui.ctx().layer_painter(LayerId::new(Order::Tooltip, Id::unique("repo-drag-ghost")));
    let color = ui.visuals().text_color();
    let galley = painter.layout_no_wrap(name.to_owned(), FontId::proportional(13.0), color);
    let text = pointer + vec2(16.0, 6.0);
    let rect = Rect::from_min_size(text, galley.size()).expand2(vec2(8.0, 4.0));
    painter.rect_filled(rect, 6.0, ui.visuals().window_fill);
    painter.rect_stroke(rect, 6.0, ui.visuals().window_stroke, StrokeKind::Outside);
    painter.galley(text, galley, color);
}
