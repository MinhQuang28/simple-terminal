use std::path::Path;

use eframe::egui;
use uuid::Uuid;

use crate::app::RepodeckApp;
use crate::app_actions::close_tab;
use crate::model::{Pane, PaneKind, SplitDir, Tab, now_secs};
use crate::pane_layout::PaneLayout;
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
                let Some(from) = repo.tabs.iter().position(|t| t.id == tab) else { return false };
                let moved = repo.tabs.remove(from);
                repo.tabs.insert(to.min(repo.tabs.len()), moved);
            }
            Action::MoveTabToRepo { from, tab, to } => {
                let Some(repo) = self.state.repo_mut(from) else { return false };
                let Some(index) = repo.tabs.iter().position(|t| t.id == tab) else { return false };
                let moved = repo.tabs.remove(index);
                if repo.active_tab == Some(tab) {
                    repo.active_tab =
                        repo.tabs.get(index.min(repo.tabs.len().saturating_sub(1))).map(|t| t.id);
                }
                let Some(target) = self.state.repo_mut(to) else { return false };
                target.active_tab = Some(moved.id);
                target.tabs.push(moved);
                self.state.selected_repo = Some(to);
                self.focus_active_pane();
            }
            Action::SelectAdjacentTab(offset) => {
                let Some(repo) = self.selected_repo_mut() else { return false };
                let Some(index) = repo.tabs.iter().position(|t| Some(t.id) == repo.active_tab) else {
                    return false;
                };
                let next = (index as i32 + offset).rem_euclid(repo.tabs.len() as i32) as usize;
                repo.active_tab = Some(repo.tabs[next].id);
                self.focus_active_pane();
            }
            _ => return false,
        }
        true
    }

    pub(crate) fn split(&mut self, dir: SplitDir, kind: PaneKind) {
        let repo_path = self.selected_repo_mut().map(|r| r.path.clone());
        // The new pane starts where the active one currently is.
        let active = self
            .selected_repo_mut()
            .and_then(|r| r.active_tab_mut())
            .and_then(|t| t.active_pane.and_then(|id| t.panes.iter().find(|p| p.id == id)))
            .map(|p| (p.id, p.cwd.clone()));
        let cwd = active
            .and_then(|(id, saved)| self.pool.get(id).and_then(|s| s.cwd().map(Path::to_path_buf)).or(saved))
            .filter(|c| Some(c) != repo_path.as_ref());
        let Some(tab) = self.selected_repo_mut().and_then(|r| r.active_tab_mut()) else { return };
        let pane = Pane { id: Uuid::new_v4(), kind, cwd, session: None };
        let id = pane.id;
        let target = tab.active_pane.or_else(|| tab.panes.first().map(|p| p.id));
        let mut layout = tab.layout();
        match target {
            Some(target) if layout.split(target, dir, id) => {}
            _ => layout = PaneLayout::from_flat(&[id], dir),
        }
        tab.layout = Some(layout);
        tab.zoomed = None;
        // New panes open next to the one being split rather than at the far end.
        let at = tab
            .active_pane
            .and_then(|active| tab.panes.iter().position(|p| p.id == active))
            .map_or(tab.panes.len(), |i| i + 1);
        tab.panes.insert(at, pane);
        tab.active_pane = Some(id);
        tab.last_active = now_secs();
        self.pool.mark_fresh(id);
        self.pending_focus = Some(id);
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
                close_tab(repo, tab_id);
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
