use std::path::Path;

use eframe::egui::{
    Align, Button, Color32, CursorIcon, Id, Layout, Rect, RichText, Sense, Stroke, StrokeKind, Ui, UiBuilder,
    vec2,
};
use uuid::Uuid;

use super::Action;
use super::diff_view;
use super::pane_chrome::{exited_overlay, pane_menu};
use super::tab_bar;
use super::widgets::{accent, muted};
use crate::claude_history::ClaudeHistory;
use crate::diff::DiffCache;
use crate::editor::{self, Buffers};
use crate::file_drag::{self, FileDrag};
use crate::git_watcher::GitWatcher;
use crate::model::{AppState, Repo, SplitDir, Tab};
use crate::pane_layout::{Divider, PaneLayout};
use crate::session_pool::SessionPool;
use crate::terminal::links::Link;
use crate::terminal::{TerminalStyle, view};

const PANE_GAP: f32 = 4.0;

pub fn pane_widget_id(pane: Uuid) -> Id {
    Id::unique(("terminal", pane))
}

/// Mutable app state the content card needs while laying out a frame.
pub struct Workspace<'a> {
    pub state: &'a AppState,
    pub git: &'a GitWatcher,
    pub pool: &'a mut SessionPool,
    pub buffers: &'a mut Buffers,
    pub diffs: &'a mut DiffCache,
    pub history: &'a ClaudeHistory,
    pub drag: &'a FileDrag,
    pub pending_focus: &'a mut Option<Uuid>,
    /// Line and column to place the editor cursor at once the file tab shows.
    pub goto: &'a mut Option<(usize, usize)>,
    pub actions: &'a mut Vec<Action>,
}

/// Contents of the rounded card: the tab strip plus the active terminal panes or file editor.
pub fn show(ui: &mut Ui, ws: &mut Workspace<'_>) {
    let state = ws.state;
    let card = ui.max_rect();
    let Some(repo) = state.selected_repo.and_then(|id| state.repo(id)) else {
        empty_state(ui, state, ws.actions);
        folder_drop(ui, ws.drag, card, ws.actions);
        return;
    };
    // Dropping a folder on the tab strip opens a shell there, like the Finder service.
    folder_drop(ui, ws.drag, card.with_max_y(card.top() + tab_bar::TAB_HEIGHT), ws.actions);
    let branch = ws.git.status(&repo.path).map(|s| s.branch.clone());
    tab_bar::show(ui, repo, branch.as_deref(), ws.pool, ws.buffers, ws.history, ws.actions);
    let separator = ui.available_rect_before_wrap();
    ui.painter().hline(separator.x_range(), separator.top(), Stroke::new(1.0, state.settings.theme.border()));
    ui.add_space(1.0);
    let Some(tab) = repo.active_tab.and_then(|id| repo.tabs.iter().find(|t| t.id == id)) else {
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new("No terminals open — ⌘T for a shell, ⇧⌘T for Claude").color(muted(ui)));
        });
        return;
    };
    match (&tab.file, &tab.diff) {
        (Some(path), _) => file_editor(ui, ws, repo, path),
        (None, Some(path)) => {
            let status = ws.git.status(&repo.path);
            let text = ws.diffs.get(&repo.path, path, status.as_ref());
            let (galley, comments) = ws.diffs.view_slots(path);
            let settings = &state.settings;
            diff_view::show(
                ui,
                repo.id,
                &repo.path,
                path,
                text.as_ref(),
                galley,
                comments,
                settings,
                ws.actions,
            );
        }
        (None, None) => panes(ui, ws, repo, tab),
    }
}

fn file_editor(ui: &mut Ui, ws: &mut Workspace<'_>, repo: &Repo, path: &Path) {
    let id = Id::unique(("editor", path));
    if ws.pending_focus.take().is_some() {
        ui.memory_mut(|m| m.request_focus(id));
    }
    let buffer = ws.buffers.get(path);
    if editor::show(ui, buffer, path, &repo.path, &ws.state.settings, id, ws.goto.take()) {
        buffer.save(path);
    }
}

fn folder_drop(ui: &Ui, drag: &FileDrag, rect: Rect, actions: &mut Vec<Action>) {
    if drag.hovering(rect) {
        FileDrag::paint_target(ui, rect);
    }
    if let Some(paths) = drag.dropped_in(rect) {
        actions.extend(paths.iter().filter(|p| p.is_dir()).map(|p| Action::OpenFolderTab(p.clone())));
    }
}

fn empty_state(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() / 3.0);
        ui.heading("Repodeck");
        ui.add_space(8.0);
        let hint = if state.projects.is_empty() {
            "Create a project, then add the repositories that belong to it."
        } else {
            "Select a repository in the sidebar."
        };
        ui.label(RichText::new(hint).color(muted(ui)));
        ui.add_space(12.0);
        if state.projects.is_empty() && ui.button("New project").clicked() {
            actions.push(Action::OpenNewProject);
        }
    });
}

fn panes(ui: &mut Ui, ws: &mut Workspace<'_>, repo: &Repo, tab: &Tab) {
    let Workspace { state, pool, pending_focus, actions, drag, .. } = ws;
    // Inset so square terminal backgrounds stay inside the card's rounded corners.
    let area =
        ui.available_rect_before_wrap().shrink2(vec2(4.0, 0.0)).with_max_y(ui.max_rect().bottom() - 6.0);
    let layout = tab.layout();
    let zoomed = tab.zoomed.filter(|z| tab.panes.iter().any(|p| p.id == *z));
    let rects: Vec<(Uuid, Rect)> = match zoomed {
        Some(id) => vec![(id, area)],
        None => layout.rects(area, PANE_GAP),
    };
    let multiple = tab.panes.len() > 1;
    for (pane_id, rect) in rects {
        let Some(pane) = tab.panes.iter().find(|p| p.id == pane_id) else { continue };
        let mut child = ui.new_child(UiBuilder::new().max_rect(rect).layout(Layout::top_down(Align::Min)));
        let id = pane_widget_id(pane.id);
        if **pending_focus == Some(pane.id) {
            child.memory_mut(|m| m.request_focus(id));
            **pending_focus = None;
        }
        let cwd = pane.cwd.as_deref().unwrap_or(&repo.path);
        match pool.get_or_spawn(pane, cwd, &state.settings, ui.ctx()) {
            Ok(session) => {
                let cwd = session.cwd().map(Path::to_path_buf).unwrap_or_else(|| cwd.to_path_buf());
                let shown = view::show(&mut child, session, id, &TerminalStyle::new(&state.settings), &cwd);
                let response = shown.response;
                if response.gained_focus() || response.clicked() || response.secondary_clicked() {
                    actions.push(Action::FocusPane { tab: tab.id, pane: pane.id });
                }
                if let Some(Link::File { path, line, col }) = shown.open_file {
                    actions.push(Action::OpenFileAt { repo: repo.id, path, line, col });
                }
                response.context_menu(|ui| pane_menu(ui, session, tab.id, pane.id, actions));
                if drag.hovering(rect) {
                    FileDrag::paint_target(ui, rect);
                }
                if let Some(paths) = drag.dropped_in(rect) {
                    session.paste(&file_drag::shell_paths(paths));
                    actions.push(Action::FocusPane { tab: tab.id, pane: pane.id });
                }
                if session.exited {
                    exited_overlay(&mut child, rect, tab.id, pane.id, actions);
                }
            }
            Err(err) => {
                child.painter().rect_filled(rect, 0.0, state.settings.theme.background.color());
                child.label(RichText::new(err).color(Color32::LIGHT_RED));
                if child.button("Retry").clicked() {
                    actions.push(Action::RestartPane(pane.id));
                }
            }
        }
        let active = tab.active_pane == Some(pane.id);
        if multiple && active && zoomed.is_none() {
            let stroke = Stroke::new(1.0, accent(ui).gamma_multiply(0.6));
            ui.painter().rect_stroke(rect, 2.0, stroke, StrokeKind::Outside);
        }
        if multiple && (active || ui.rect_contains_pointer(rect)) {
            let button_rect = Rect::from_min_size(rect.right_top() + vec2(-28.0, 6.0), vec2(22.0, 22.0));
            let label = if zoomed.is_some() { "⤡" } else { "×" };
            let hint = if zoomed.is_some() { "Unzoom (⇧⌘↩)" } else { "Close pane (⌘W)" };
            let button = Button::new(RichText::new(label).size(15.0)).corner_radius(4.0);
            if ui.put(button_rect, button).on_hover_text(hint).clicked() {
                actions.push(if zoomed.is_some() {
                    Action::ToggleZoom
                } else {
                    Action::ClosePane { tab: tab.id, pane: pane.id }
                });
            }
        }
    }
    if zoomed.is_none() {
        dividers(ui, &layout, area, tab.id, actions);
    }
    ui.advance_cursor_after_rect(area);
}

/// Draggable gaps between sibling panes.
fn dividers(ui: &mut Ui, layout: &PaneLayout, area: Rect, tab: Uuid, actions: &mut Vec<Action>) {
    for (i, divider) in layout.dividers(area, PANE_GAP).into_iter().enumerate() {
        let id = Id::unique(("divider", tab, i));
        let response = ui.interact(divider.rect, id, Sense::drag());
        let (icon, extent) = match divider.dir {
            SplitDir::Right => (CursorIcon::ResizeHorizontal, area.width()),
            SplitDir::Down => (CursorIcon::ResizeVertical, area.height()),
        };
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(icon);
            ui.painter().rect_filled(divider.rect.shrink(1.0), 1.0, accent(ui).gamma_multiply(0.5));
        }
        let delta = match divider.dir {
            SplitDir::Right => response.drag_delta().x,
            SplitDir::Down => response.drag_delta().y,
        };
        if delta != 0.0 {
            let Divider { path, index, .. } = divider;
            actions.push(Action::ResizeSplit { tab, path, index, delta: delta / extent.max(1.0) });
        }
    }
}
