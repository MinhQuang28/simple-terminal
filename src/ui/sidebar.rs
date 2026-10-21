use eframe::egui::{Align, Button, Frame, Label, Layout, Panel, Rect, RichText, ScrollArea, Ui};
use uuid::Uuid;

use super::Action;
use super::chrome::window_drag_area;
use super::sidebar_rows::repo_block;
use super::widgets::{avatar, glyph_button, icon_button, menu_icon, muted, right_aligned, row_sized};
use crate::agent_events::AgentStatus;
use crate::file_drag::FileDrag;
use crate::git_watcher::GitWatcher;
use crate::model::{AppState, Project, Tab, now_secs};
use crate::pr_status::PrWatcher;
use crate::session_pool::SessionPool;
use crate::tab_title;

const TRAFFIC_LIGHTS_HEIGHT: f32 = 34.0;
const PROJECT_GAP: f32 = 14.0;

pub fn show(
    ui: &mut Ui,
    state: &AppState,
    git: &GitWatcher,
    prs: &PrWatcher,
    pool: &SessionPool,
    drag: &FileDrag,
    actions: &mut Vec<Action>,
) {
    // Room for the macOS traffic lights, which float over the hidden titlebar.
    let strip = ui.max_rect().with_max_y(ui.max_rect().top() + TRAFFIC_LIGHTS_HEIGHT);
    window_drag_area(ui, strip);
    ui.add_space(TRAFFIC_LIGHTS_HEIGHT);
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        ui.label(RichText::new("Projects").color(muted(ui)).size(12.5));
        right_aligned(ui, |ui| {
            // Match the inner shrink of the rows below so the buttons line up.
            ui.add_space(6.0);
            if glyph_button(ui, "+").on_hover_text("New project").clicked() {
                actions.push(Action::OpenNewProject);
            }
        });
    });
    ui.add_space(8.0);

    Panel::bottom("sidebar-footer").show_separator_line(false).frame(Frame::new()).show(ui, |ui| {
        ui.horizontal(|ui| {
            if icon_button(ui, "⚙").on_hover_text("Settings (⌘,)").clicked() {
                actions.push(Action::OpenSettings);
            }
        });
    });

    let mut spans: Vec<(Uuid, Rect)> = Vec::new();
    ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let now = now_secs();
        for project in &state.projects {
            let top = ui.cursor().top();
            project_row(ui, project, actions);
            if !project.collapsed {
                ui.add_space(4.0);
                for repo in &project.repos {
                    repo_block(ui, repo, state, git, prs, pool, now, actions);
                }
                if project.repos.is_empty() {
                    let (response, ()) = row_sized(ui, false, 12.0, 28.0, |ui| {
                        ui.label(RichText::new("Add repositories…").color(muted(ui)).size(13.0));
                    });
                    if response.clicked() {
                        actions.push(Action::AddRepos(project.id));
                    }
                }
            }
            let span = ui.max_rect().x_range();
            spans.push((project.id, Rect::from_x_y_ranges(span, top..=ui.cursor().top())));
            ui.add_space(PROJECT_GAP);
        }
        if state.projects.is_empty() {
            ui.add_space(12.0);
            ui.label(RichText::new("No projects yet.").color(muted(ui)));
            if ui.button("New project").clicked() {
                actions.push(Action::OpenNewProject);
            }
        }
    });
    drop_target(ui, drag, &spans, actions);
}

/// Dropping folders on a project adds them there; elsewhere in the sidebar, to the current one.
fn drop_target(ui: &Ui, drag: &FileDrag, spans: &[(Uuid, Rect)], actions: &mut Vec<Action>) {
    let panel = ui.max_rect();
    let Some(pointer) = drag.pointer() else { return };
    let project = spans.iter().find(|(_, rect)| rect.contains(pointer));
    if drag.hovering(panel) {
        FileDrag::paint_target(ui, project.map_or(panel, |(_, rect)| rect.expand(2.0)));
    }
    if let Some(paths) = drag.dropped_in(panel) {
        actions.push(Action::AddDroppedRepos { project: project.map(|(id, _)| *id), paths: paths.to_vec() });
    }
}

fn project_row(ui: &mut Ui, project: &Project, actions: &mut Vec<Action>) {
    let (response, ()) = row_sized(ui, false, 0.0, 32.0, |ui| {
        right_aligned(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            menu_icon(ui, "⋯", |ui| {
                if ui.button("Add repositories…").clicked() {
                    actions.push(Action::AddRepos(project.id));
                }
                if ui.button("Rename…").clicked() {
                    actions.push(Action::OpenRenameProject(project.id));
                }
                ui.separator();
                if ui.button("Remove project").clicked() {
                    actions.push(Action::RemoveProject(project.id));
                }
            });
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                avatar(ui, &project.name, 22.0);
                ui.add(
                    Label::new(RichText::new(&project.name).strong().size(15.0)).truncate().selectable(false),
                );
                let chevron = if project.collapsed { "›" } else { "⌄" };
                ui.label(RichText::new(chevron).color(muted(ui)).size(12.0));
            });
        });
    });
    if response.clicked() {
        actions.push(Action::ToggleProject(project.id));
    }
}

pub fn tab_menu(ui: &mut Ui, repo: Uuid, tab: Uuid, actions: &mut Vec<Action>) {
    if ui.button("Rename tab…").clicked() {
        actions.push(Action::OpenRenameTab { repo, tab });
    }
    if ui.add(Button::new("Close tab").shortcut_text("⇧⌘W")).clicked() {
        actions.push(Action::CloseTab { repo, tab });
    }
}

/// Tab label: the running task's summary, else `fallback`.
pub fn tab_title(tab: &Tab, pool: &SessionPool, fallback: &str) -> String {
    let pane = tab.active_pane.or_else(|| tab.panes.first().map(|p| p.id));
    let live = pane.and_then(|id| pool.get(id)).and_then(|s| s.title.as_deref());
    tab_title::display(tab, live, fallback)
}

/// Whether an agent in the tab reports it is working, or a pane is producing output the user
/// didn't type.
pub fn tab_busy(tab: &Tab, pool: &SessionPool) -> bool {
    tab.panes.iter().filter_map(|p| pool.get(p.id)).any(|s| {
        s.agent_status() == Some(AgentStatus::Working)
            || s.title.as_deref().is_some_and(tab_title::is_working)
            || s.is_busy()
    })
}

/// The most pressing agent status among the tab's panes.
pub fn tab_status(tab: &Tab, pool: &SessionPool) -> Option<AgentStatus> {
    tab.panes.iter().filter_map(|p| pool.agent_status(p.id)).max()
}
