use eframe::egui::{self, CentralPanel, CornerRadius, Frame, Margin, Panel, Stroke};

use crate::app::RepodeckApp;
use crate::model::PaneKind;
use crate::ui::workspace::Workspace;
use crate::ui::{Action, chrome, files_panel, sidebar, workspace};

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

        Panel::left("sidebar")
            .resizable(true)
            .show_separator_line(false)
            .default_size(270.0)
            .size_range(200.0..=480.0)
            .frame(Frame::new().fill(window_bg).inner_margin(Margin {
                left: 10,
                right: 6,
                top: 0,
                bottom: 8,
            }))
            .show(ui, |ui| {
                sidebar::show(ui, &self.state, &self.git, &self.prs, &self.pool, &self.drag, actions)
            });

        if self.state.show_side_panel {
            Panel::right("files")
                .resizable(true)
                .show_separator_line(false)
                .default_size(280.0)
                .size_range(200.0..=520.0)
                .frame(Frame::new().fill(window_bg).inner_margin(Margin {
                    left: 8,
                    right: 10,
                    top: 44,
                    bottom: 8,
                }))
                .show(ui, |ui| {
                    files_panel::show(
                        ui,
                        &self.state,
                        status.clone(),
                        &mut self.file_tree,
                        &self.library,
                        actions,
                    )
                });
        }

        Panel::top("topbar")
            .exact_size(44.0)
            .show_separator_line(false)
            .frame(Frame::new().fill(window_bg).inner_margin(Margin::symmetric(6, 0)))
            .show(ui, |ui| {
                chrome::top_bar(ui, &self.state, status.as_deref(), &self.inbox, &self.pool, actions)
            });

        Panel::bottom("statusbar")
            .exact_size(28.0)
            .show_separator_line(false)
            .frame(Frame::new().fill(window_bg).inner_margin(Margin::symmetric(6, 0)))
            .show(ui, |ui| {
                chrome::status_bar(
                    ui,
                    &self.state,
                    status.as_deref(),
                    self.pool.latest_limits(),
                    self.active_context(),
                )
            });

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
                    actions,
                };
                workspace::show(ui, &mut ws);
            });
        });
    }

    /// Share of the context window the conversation in the active Claude pane fills.
    fn active_context(&self) -> Option<f64> {
        let repo = self.state.selected_repo.and_then(|id| self.state.repo(id))?;
        let tab = repo.tabs.iter().find(|t| Some(t.id) == repo.active_tab)?;
        let pane = tab.panes.iter().find(|p| Some(p.id) == tab.active_pane).or(tab.panes.first())?;
        if pane.kind != PaneKind::Agent {
            return None;
        }
        self.pool.get(pane.id)?.agent_status_line()?.context_percent
    }
}
