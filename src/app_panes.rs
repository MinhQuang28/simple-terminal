use eframe::egui;
use uuid::Uuid;

use crate::app::RepodeckApp;
use crate::model::{PaneKind, Tab};
use crate::ui::Action;
use crate::ui::dialogs::Dialog;

impl RepodeckApp {
    /// Handles pane layout and tab arrangement actions; returns whether state changed.
    pub(crate) fn apply_pane_action(&mut self, action: Action, ctx: &egui::Context) -> bool {
        match action {
            Action::ToggleZoom => {
                let Some(tab) = self.selected_repo_mut().and_then(|r| r.active_tab_mut()) else {
                    return false;
                };
                tab.zoomed = match (tab.zoomed, tab.active_pane) {
                    (None, Some(pane)) if tab.panes.len() > 1 => Some(pane),
                    _ => None,
                };
            }
            Action::FocusNeighbor(nav) => {
                let Some(tab) = self.selected_repo_mut().and_then(|r| r.active_tab_mut()) else {
                    return false;
                };
                let Some(next) = tab.active_pane.and_then(|from| tab.layout().neighbor(from, nav)) else {
                    return false;
                };
                tab.active_pane = Some(next);
                tab.zoomed = tab.zoomed.map(|_| next);
                self.pending_focus = Some(next);
                ctx.request_repaint();
                return false;
            }
            Action::SwapNeighbor(nav) => {
                let Some(tab) = self.selected_repo_mut().and_then(|r| r.active_tab_mut()) else {
                    return false;
                };
                // Behind a zoomed pane the swap couldn't be seen.
                if tab.zoomed.is_some() {
                    return false;
                }
                let mut layout = tab.layout();
                let Some(active) = tab.active_pane else { return false };
                let Some(other) = layout.neighbor(active, nav) else { return false };
                // The active pane moves and keeps focus, as dragging it there would.
                layout.swap(active, other);
                tab.layout = Some(layout);
                self.pending_focus = Some(active);
                ctx.request_repaint();
            }
            Action::ResizeSplit { tab, path, index, delta } => {
                let Some(tab) = self.tab_mut(tab) else { return false };
                let mut layout = tab.layout();
                layout.resize(&path, index, delta);
                tab.layout = Some(layout);
            }
            Action::OpenRenameTab { repo, tab } => {
                let Some(t) = self.state.repo(repo).and_then(|r| r.tabs.iter().find(|t| t.id == tab)) else {
                    return false;
                };
                let name = t.name.clone().unwrap_or_else(|| t.title.clone());
                self.dialog = Some(Dialog::RenameTab { repo, tab, name });
                return false;
            }
            Action::MoveTab { repo, tab, to } => {
                let Some(repo) = self.state.repo_mut(repo) else { return false };
                repo.move_in_view(tab, to);
            }
            Action::MoveTabToRepo { from, tab, to } => {
                let Some(repo) = self.state.repo_mut(from) else { return false };
                let Some(moved) = repo.remove_tab(tab) else { return false };
                let Some(target) = self.state.repo_mut(to) else { return false };
                target.tabs.push(moved);
                target.activate(tab);
                self.state.selected_repo = Some(to);
                self.focus_active_pane();
            }
            Action::SelectAdjacentTab(offset) => {
                let Some(repo) = self.selected_repo_mut() else { return false };
                let Some(next) = repo.adjacent_tab(offset) else { return false };
                repo.active_tab = Some(next);
                // Stepping onto a settled tab brings it back, as selecting it does.
                repo.tabs.iter_mut().filter(|t| t.id == next).for_each(|t| t.settled = false);
                self.focus_active_pane();
            }
            _ => return false,
        }
        true
    }

    /// Opens a tab in `repo` running launch preset `index`: as an agent pane with its own command,
    /// or a terminal that runs it.
    pub(crate) fn run_preset(&mut self, repo: Uuid, index: usize) {
        let Some(preset) = self.state.settings.presets.get(index).cloned() else { return };
        let command = preset.command.trim().to_string();
        if command.is_empty() {
            return;
        }
        let kind = if preset.agent { PaneKind::Agent } else { PaneKind::Shell };
        if let Some(pane) = self.new_tab(repo, kind, None) {
            self.set_pane_command(pane, command);
        }
    }

    pub(crate) fn close_pane(&mut self, tab_id: Uuid, pane_id: Uuid) {
        for repo in self.state.projects.iter_mut().flat_map(|p| &mut p.repos) {
            let Some(tab) = repo.tabs.iter_mut().find(|t| t.id == tab_id) else { continue };
            tab.panes.retain(|p| p.id != pane_id);
            let mut layout = tab.layout();
            layout.remove(pane_id);
            tab.layout = Some(layout);
            if tab.zoomed == Some(pane_id) {
                tab.zoomed = None;
            }
            if tab.panes.is_empty() {
                repo.remove_tab(tab_id);
            } else if tab.active_pane == Some(pane_id) {
                tab.active_pane = tab.panes.last().map(|p| p.id);
            }
            break;
        }
        self.after_structure_change();
        self.focus_active_pane();
    }

    fn tab_mut(&mut self, tab: Uuid) -> Option<&mut Tab> {
        self.state
            .projects
            .iter_mut()
            .flat_map(|p| &mut p.repos)
            .flat_map(|r| &mut r.tabs)
            .find(|t| t.id == tab)
    }
}
