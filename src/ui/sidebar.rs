use eframe::egui::{
    Align, Button, Frame, Label, Layout, Panel, Rect, RichText, ScrollArea, Sense, Stroke, Ui, vec2,
};
use uuid::Uuid;

use super::Action;
use super::agent_status;
use super::agents_list;
use super::chrome::window_drag_area;
use super::design::{TEXT_CAPTION, hairline};
use super::repo_drag::{self, RepoSpan};
use super::repo_indicators::busy_mark;
use super::sidebar_rows::repo_block;
use super::widgets::{glyph_button, icon_button, menu_icon, muted, right_aligned, row_sized};
use crate::agent_events::AgentStatus;
use crate::agent_launch::Agent;
use crate::file_drag::FileDrag;
use crate::git_watcher::GitWatcher;
use crate::model::{AppState, PaneKind, Project, Repo, SidebarView, Tab, now_secs};
use crate::pr_status::PrWatcher;
use crate::programs::{self, Program};
use crate::session_pool::SessionPool;
use crate::tab_title;

pub(super) const TRAFFIC_LIGHTS_HEIGHT: f32 = 34.0;
/// Space on each side of the hairline that separates projects.
const PROJECT_GAP: f32 = 10.0;

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
    let view = state.sidebar_view;
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        ui.spacing_mut().item_spacing.x = 12.0;
        view_switch(ui, "Projects", view, SidebarView::Projects, actions);
        let agents = agents_list::count(state, pool);
        let label = if agents > 0 { format!("Agents {agents}") } else { "Agents".into() };
        view_switch(ui, &label, view, SidebarView::Agents, actions);
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
            if icon_button(ui, "⚙")
                .on_hover_text(crate::keymap_hint::with_hint("Settings", "settings.open"))
                .clicked()
            {
                actions.push(Action::OpenSettings);
            }
        });
    });

    if view == SidebarView::Agents {
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            agents_list::show(ui, state, pool, actions);
        });
        return;
    }

    let mut spans: Vec<(Uuid, Rect)> = Vec::new();
    let mut repo_spans: Vec<RepoSpan> = Vec::new();
    let list = ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let now = now_secs();
        for (index, project) in state.projects.iter().enumerate() {
            if index > 0 {
                project_divider(ui);
            }
            let top = ui.cursor().top();
            project_row(ui, project, pool, actions);
            if !project.collapsed {
                ui.add_space(4.0);
                for repo in &project.repos {
                    // A worktree shows inside the repo it was made from.
                    if repo.worktree_of.is_some_and(|main| project.repos.iter().any(|r| r.id == main)) {
                        continue;
                    }
                    let worktrees: Vec<&Repo> =
                        project.repos.iter().filter(|r| r.worktree_of == Some(repo.id)).collect();
                    let top = ui.cursor().top();
                    repo_block(ui, repo, &worktrees, state, git, prs, pool, now, actions);
                    let rect = Rect::from_x_y_ranges(ui.max_rect().x_range(), top..=ui.cursor().top());
                    repo_spans.push(RepoSpan { project: project.id, repo: repo.id, rect });
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
    repo_drag::drop_target(ui, state, list.inner_rect, &spans, &repo_spans, actions);
}

/// Hairline between projects so each group reads as its own block.
fn project_divider(ui: &mut Ui) {
    ui.add_space(PROJECT_GAP);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(rect.x_range().shrink(6.0), rect.center().y, Stroke::new(1.0, hairline(ui)));
    ui.add_space(PROJECT_GAP);
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

/// "Projects" / "Agents" header switch; the current view reads as a heading, the other muted.
fn view_switch(ui: &mut Ui, text: &str, current: SidebarView, view: SidebarView, actions: &mut Vec<Action>) {
    let color = if current == view { ui.visuals().text_color() } else { muted(ui) };
    let label =
        Label::new(RichText::new(text).color(color).size(12.5)).selectable(false).sense(Sense::click());
    if ui.add(label).clicked() && current != view {
        actions.push(Action::SetSidebarView(view));
    }
}

fn project_row(ui: &mut Ui, project: &Project, pool: &SessionPool, actions: &mut Vec<Action>) {
    let (response, ()) = row_sized(ui, false, 0.0, 26.0, |ui| {
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
            // A folded project still says when an agent in it works or waits on the user.
            if project.collapsed {
                let tabs = || project.repos.iter().flat_map(|r| &r.tabs);
                if tabs().any(|t| tab_busy(t, pool)) {
                    busy_mark(ui);
                }
                if let Some(status) =
                    tabs().filter_map(|t| tab_status(t, pool)).filter(|s| s.needs_attention()).max()
                {
                    agent_status::pill(ui, status);
                }
            }
            // A caption rather than a row, so it doesn't read as another repo.
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.add_space(6.0);
                let color = ui.visuals().text_color().gamma_multiply(0.55);
                let name =
                    RichText::new(project.name.to_uppercase()).strong().size(TEXT_CAPTION).color(color);
                ui.add(Label::new(name).truncate().selectable(false));
                let chevron = if project.collapsed { "›" } else { "⌄" };
                ui.label(RichText::new(chevron).color(muted(ui)).size(12.0));
            });
        });
    });
    if response.clicked() {
        actions.push(Action::ToggleProject(project.id));
    }
}

pub fn tab_menu(ui: &mut Ui, state: &AppState, repo: Uuid, tab: Uuid, actions: &mut Vec<Action>) {
    // A look at a tab that isn't on screen, without leaving the one that is.
    let shown =
        state.selected_repo == Some(repo) && state.repo(repo).is_some_and(|r| r.active_tab == Some(tab));
    let pane = state
        .repo(repo)
        .and_then(|r| r.tabs.iter().find(|t| t.id == tab))
        .and_then(|t| t.active_pane.or_else(|| t.panes.first().map(|p| p.id)));
    if let Some(pane) = pane.filter(|_| !shown)
        && ui.button("Peek").clicked()
    {
        actions.push(Action::Peek(pane));
    }
    if ui.button("Rename tab…").clicked() {
        actions.push(Action::OpenRenameTab { repo, tab });
    }
    let others: Vec<&Repo> = state.projects.iter().flat_map(|p| &p.repos).filter(|r| r.id != repo).collect();
    if !others.is_empty() {
        ui.menu_button("Move to", |ui| {
            ui.set_min_width(180.0);
            for other in others {
                if ui.button(&other.name).clicked() {
                    actions.push(Action::MoveTabToRepo { from: repo, tab, to: other.id });
                    ui.close();
                }
            }
        });
    }
    let settled =
        state.repo(repo).and_then(|r| r.tabs.iter().find(|t| t.id == tab)).is_some_and(|t| t.settled);
    let settle = if settled { "Bring back" } else { "Settle (hide until it needs you)" };
    if ui.button(settle).clicked() {
        actions.push(Action::ToggleSettleTab { repo, tab });
    }
    if ui.add(Button::new("Close tab").shortcut_text(crate::keymap_hint::hint("tab.close"))).clicked() {
        actions.push(Action::CloseTab { repo, tab });
    }
}

/// Tab label: the running task's summary, else `fallback`.
pub fn tab_title(tab: &Tab, pool: &SessionPool, fallback: &str) -> String {
    let pane = tab.active_pane.or_else(|| tab.panes.first().map(|p| p.id));
    let live = pane.and_then(|id| pool.get(id)).and_then(|s| s.title());
    tab_title::display(tab, live, fallback)
}

/// What the tab shows as its icon: the agent in it (one Repodeck started, or one reporting from
/// a terminal), else what its terminal is running, else the shell.
pub fn tab_program(tab: &Tab, pool: &SessionPool) -> &'static Program {
    let sessions = || tab.panes.iter().filter_map(|p| pool.get(p.id));
    if let Some(agent) = sessions().find_map(|s| s.agent_kind()) {
        return programs::agent(agent);
    }
    if tab.kind() == PaneKind::Agent {
        return programs::agent(tab_agent(tab, pool));
    }
    let pane = tab.active_pane.or_else(|| tab.panes.first().map(|p| p.id));
    let running = pane.and_then(|id| pool.get(id)).and_then(|s| s.running_command());
    running
        .map(|line| {
            programs::for_command(line).or_else(|| programs::get("program")).unwrap_or(programs::shell())
        })
        .unwrap_or(programs::shell())
}

/// Which agent the tab runs; Claude until its pane has started.
pub fn tab_agent(tab: &Tab, pool: &SessionPool) -> Agent {
    tab.panes.iter().filter_map(|p| pool.get(p.id)).find_map(|s| s.agent_kind()).unwrap_or(Agent::Claude)
}

/// Whether an agent in the tab reports it is working, or a pane is producing output the user
/// didn't type.
pub fn tab_busy(tab: &Tab, pool: &SessionPool) -> bool {
    tab.panes.iter().filter_map(|p| pool.get(p.id)).any(|s| {
        s.agent_status() == Some(AgentStatus::Working)
            || s.raw_title().is_some_and(tab_title::is_working)
            || s.is_busy()
    })
}

/// The most pressing agent status among the tab's panes.
pub fn tab_status(tab: &Tab, pool: &SessionPool) -> Option<AgentStatus> {
    tab.panes.iter().filter_map(|p| pool.agent_status(p.id)).max()
}
