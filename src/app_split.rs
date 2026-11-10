//! Splitting panes: beside the active one for the user, or beside any pane for a script.

use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::app::RepodeckApp;
use crate::model::{Pane, PaneKind, SplitDir, now_secs};
use crate::pane_layout::PaneLayout;

impl RepodeckApp {
    /// The folder `pane` is in now (or was saved in), when that isn't its repo's root, so panes
    /// and tabs opened from it start where the user is working.
    pub(crate) fn pane_cwd(&self, pane: Uuid) -> Option<PathBuf> {
        let (repo, saved) = self.state.projects.iter().flat_map(|p| &p.repos).find_map(|r| {
            let p = r.tabs.iter().flat_map(|t| &t.panes).find(|p| p.id == pane)?;
            Some((r, p.cwd.clone()))
        })?;
        let live = self.pool.get(pane).and_then(|s| s.cwd().map(Path::to_path_buf));
        live.or(saved).filter(|c| *c != repo.path)
    }

    /// [`Self::pane_cwd`] of the selected repo's active pane.
    pub(crate) fn active_pane_cwd(&mut self) -> Option<PathBuf> {
        let tab = self.selected_repo_mut()?.active_tab_mut()?;
        let pane = tab.active_pane.or_else(|| tab.panes.first().map(|p| p.id))?;
        self.pane_cwd(pane)
    }

    /// Splits the active pane and focuses the new one; returns it.
    pub(crate) fn split(&mut self, dir: SplitDir, kind: PaneKind) -> Option<Uuid> {
        let tab = self.selected_repo_mut()?.active_tab_mut()?;
        let target = tab.active_pane.or_else(|| tab.panes.first().map(|p| p.id))?;
        tab.zoomed = None;
        let id = self.split_pane(target, dir, kind)?;
        let tab = self.selected_repo_mut()?.active_tab_mut()?;
        tab.active_pane = Some(id);
        self.pending_focus = Some(id);
        Some(id)
    }

    /// Records what `pane` runs (see `Pane::command`).
    pub(crate) fn set_pane_command(&mut self, pane: Uuid, command: String) {
        let panes = self.state.projects.iter_mut().flat_map(|p| &mut p.repos).flat_map(|r| &mut r.tabs);
        if let Some(p) = panes.flat_map(|t| &mut t.panes).find(|p| p.id == pane) {
            p.command = Some(command);
            self.dirty = true;
        }
    }

    /// Records that `child`'s agent was started by the one in `parent`, when both panes exist.
    pub(crate) fn set_pane_parent(&mut self, child: Uuid, parent: Option<Uuid>) {
        let Some(parent) = parent.filter(|p| *p != child && self.pane_exists(*p)) else { return };
        let panes = self.state.projects.iter_mut().flat_map(|p| &mut p.repos).flat_map(|r| &mut r.tabs);
        if let Some(p) = panes.flat_map(|t| &mut t.panes).find(|p| p.id == child) {
            p.parent = Some(parent);
            self.dirty = true;
        }
    }

    fn pane_exists(&self, pane: Uuid) -> bool {
        let tabs = self.state.projects.iter().flat_map(|p| &p.repos).flat_map(|r| &r.tabs);
        tabs.flat_map(|t| &t.panes).any(|p| p.id == pane)
    }

    /// The pane `pane`'s agent reports to: the one that started it, while an agent still runs
    /// there to read the result.
    pub(crate) fn reports_to(&self, pane: Uuid) -> Option<Uuid> {
        let tabs = self.state.projects.iter().flat_map(|p| &p.repos).flat_map(|r| &r.tabs);
        let parent = tabs.flat_map(|t| &t.panes).find(|p| p.id == pane)?.parent?;
        self.pool.get(parent).is_some_and(|s| s.is_agent()).then_some(parent)
    }

    /// Puts a new pane beside `target`, in whatever tab holds it, without moving the user's
    /// selection or focus; returns it.
    pub(crate) fn split_pane(&mut self, target: Uuid, dir: SplitDir, kind: PaneKind) -> Option<Uuid> {
        let cwd = self.pane_cwd(target);
        let tab = self
            .state
            .projects
            .iter_mut()
            .flat_map(|p| &mut p.repos)
            .flat_map(|r| &mut r.tabs)
            .find(|t| t.panes.iter().any(|p| p.id == target))?;
        let pane = Pane { id: Uuid::new_v4(), kind, cwd, session: None, command: None, parent: None };
        let id = pane.id;
        let mut layout = tab.layout();
        if !layout.split(target, dir, id) {
            layout = PaneLayout::from_flat(&[target, id], dir);
        }
        tab.layout = Some(layout);
        // New panes open next to the one being split rather than at the far end.
        let at = tab.panes.iter().position(|p| p.id == target).map_or(tab.panes.len(), |i| i + 1);
        tab.panes.insert(at, pane);
        tab.last_active = now_secs();
        self.pool.mark_fresh(id);
        self.dirty = true;
        Some(id)
    }
}
