use eframe::egui::{CentralPanel, CornerRadius, Frame, Margin, Panel, Stroke};

use crate::app::RepodeckApp;
use crate::claude_status_line::StatusLine;
use crate::ui::workspace::Workspace;
use crate::ui::{Action, chrome, code_sidebar, sidebar, workspace};

impl RepodeckApp {
    /// Sidebar, files panel, top/status bars, and the rounded content card in the middle.
    pub(crate) fn layout(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let theme = &self.state.settings.theme;
        let window_bg = theme.window_background();
        let card = Frame::new()
            .fill(theme.background.color())
            .stroke(Stroke::new(1.0, theme.border()))
            .corner_radius(CornerRadius::same(12));
        let status = self
            .state
            .selected_repo
            .and_then(|id| self.state.repo(id))
            .and_then(|r| self.git.status(&r.path));

        // In the Code view a VS Code-style sidebar (files, search, changes, library) takes the
        // project sidebar's place; the terminals have the project sidebar alone.
        let code_repo = self.state.selected_repo.and_then(|id| self.state.repo(id)).filter(|r| r.code_view);
        let left_panel = match code_repo {
            Some(_) => self.state.show_code_panel,
            None => true,
        };
        let sidebar_frame =
            Frame::new().fill(window_bg).inner_margin(Margin { left: 10, right: 6, top: 0, bottom: 8 });
        let mut handle = None;
        match code_repo {
            Some(repo) if self.state.show_code_panel => {
                let panel = Panel::left("code-sidebar")
                    .resizable(true)
                    .show_separator_line(false)
                    .default_size(330.0)
                    .size_range(270.0..=900.0)
                    .frame(sidebar_frame)
                    .show(ui, |ui| {
                        let sources = code_sidebar::Sources {
                            status: status.clone(),
                            tree: &mut self.file_tree,
                            search: &mut self.search,
                            library: &self.library,
                        };
                        code_sidebar::show(ui, &self.state, repo, sources, actions)
                    });
                handle = Some((panel.response.rect, "code-sidebar"));
            }
            Some(_) => {}
            None => {
                let panel = Panel::left("sidebar")
                    .resizable(true)
                    .show_separator_line(false)
                    .default_size(270.0)
                    .size_range(200.0..=600.0)
                    .frame(sidebar_frame)
                    .show(ui, |ui| {
                        sidebar::show(ui, &self.state, &self.git, &self.prs, &self.pool, &self.drag, actions)
                    });
                handle = Some((panel.response.rect, "sidebar"));
            }
        }
        let agent_status = self.active_status_line();
        let model = agent_status.as_ref().and_then(|s| s.model.clone());
        // With no sidebar on the left, the top bar starts past the traffic lights.
        let top_left = if left_panel { 6 } else { 80 };

        Panel::top("topbar")
            .exact_size(44.0)
            .show_separator_line(false)
            .frame(Frame::new().fill(window_bg).inner_margin(Margin {
                left: top_left,
                right: 6,
                top: 0,
                bottom: 0,
            }))
            .show(ui, |ui| {
                let context = agent_status.as_ref().and_then(|s| s.context_percent);
                chrome::top_bar(ui, &self.state, status.as_deref(), &self.inbox, &self.pool, context, actions)
            });

        Panel::bottom("statusbar")
            .exact_size(28.0)
            .show_separator_line(false)
            .frame(Frame::new().fill(window_bg).inner_margin(Margin::symmetric(6, 0)))
            .show(ui, |ui| chrome::status_bar(ui, &self.state, status.as_deref(), model.as_deref()));

        CentralPanel::default().frame(Frame::new().fill(window_bg)).show(ui, |ui| {
            card.show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                let mut ws = Workspace {
                    state: &self.state,
                    git: &self.git,
                    pool: &mut self.pool,
                    buffers: &mut self.buffers,
                    diffs: &mut self.diffs,
                    history: &self.claude_history,
                    drag: &self.drag,
                    pending_focus: &mut self.pending_focus,
                    goto: &mut self.pending_goto,
                    editor_commands: &mut self.editor_commands,
                    actions,
                };
                workspace::show(ui, &mut ws);
            });
        });
        if let Some((rect, name)) = handle {
            resize_handle(ui, rect, name, window_bg);
        }
    }

    /// What Claude Code last reported in the active pane, while an agent runs there.
    fn active_status_line(&self) -> Option<StatusLine> {
        let repo = self.state.selected_repo.and_then(|id| self.state.repo(id))?;
        let tab = repo.tabs.iter().find(|t| Some(t.id) == repo.active_tab)?;
        let pane = tab.panes.iter().find(|p| Some(p.id) == tab.active_pane).or(tab.panes.first())?;
        let session = self.pool.get(pane.id)?;
        session.is_agent().then(|| session.agent_status_line()).flatten()
    }
}

/// Lets the right edge of the side panel `name`, filled with `fill`, be dragged from either side
/// of it, with only the cursor to show for it. Call it after the panels beside it: egui's own
/// handle sits under them, so their content took every drag that started past the edge.
fn resize_handle(ui: &egui::Ui, panel: egui::Rect, name: &str, fill: egui::Color32) {
    let radius = ui.style().interaction.resize_grab_radius_side;
    let x = panel.right();
    let rect = egui::Rect::from_x_y_ranges(x - radius..=x + radius, panel.y_range());
    // The id egui's `Panel` gives its resize handle, which is what it reads the drag from.
    let id = ui.scope_id().with(name).with("__resize");
    let on_top = egui::InteractOptions { move_to_top: true };
    let response = ui.interact_opt(rect, id, egui::Sense::click_and_drag(), on_top);
    if !response.hovered() && !response.dragged() {
        return;
    }
    ui.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    // The edge stays plain: paint over the line egui draws just inside it.
    let widgets = &ui.visuals().widgets;
    let width = widgets.hovered.fg_stroke.width.max(widgets.active.fg_stroke.width) + 1.0;
    ui.painter().rect_filled(panel.with_min_x(x - width), 0.0, fill);
}

#[cfg(test)]
#[path = "app_layout_tests.rs"]
mod tests;
