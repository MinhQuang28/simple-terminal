use std::path::PathBuf;
use std::time::{Duration, Instant};

use uuid::Uuid;

use crate::app::{PendingPaste, RepodeckApp};
use crate::app_actions::close_tab;
use crate::model::{PaneKind, Tab};

/// Pasting into a freshly opened Claude tab waits for Claude to take over the terminal.
const AGENT_START_DELAY: Duration = Duration::from_millis(2500);

impl RepodeckApp {
    pub(crate) fn close_tab(&mut self, repo_id: Uuid, tab_id: Uuid) {
        let Some(repo) = self.state.repo_mut(repo_id) else { return };
        let file = repo.tabs.iter().find(|t| t.id == tab_id).and_then(|t| t.file.clone());
        if let Some(tab) = close_tab(repo, tab_id) {
            self.closed_tabs.push((repo_id, tab));
            self.closed_tabs.truncate(20);
        }
        if let Some(path) = file {
            let still_open = self
                .state
                .projects
                .iter()
                .flat_map(|p| &p.repos)
                .flat_map(|r| &r.tabs)
                .any(|t| t.file.as_ref() == Some(&path));
            if !still_open {
                self.buffers.close(&path);
            }
        }
        self.after_structure_change();
        self.focus_active_pane();
    }

    pub(crate) fn open_file(&mut self, repo_id: Uuid, path: PathBuf) {
        let Some(repo) = self.state.repo_mut(repo_id) else { return };
        let tab_id = match repo.tabs.iter().find(|t| t.file.as_ref() == Some(&path)) {
            Some(tab) => tab.id,
            None => {
                let tab = Tab::for_file(path);
                let id = tab.id;
                repo.tabs.push(tab);
                id
            }
        };
        repo.active_tab = Some(tab_id);
        self.state.selected_repo = Some(repo_id);
        self.pending_focus = Some(tab_id);
    }

    pub(crate) fn open_diff(&mut self, repo_id: Uuid, path: PathBuf) {
        let Some(repo) = self.state.repo_mut(repo_id) else { return };
        let tab_id = match repo.tabs.iter().find(|t| t.diff.as_ref() == Some(&path)) {
            Some(tab) => tab.id,
            None => {
                let tab = Tab::for_diff(path.clone());
                let id = tab.id;
                repo.tabs.push(tab);
                id
            }
        };
        self.diffs.forget(&path);
        repo.active_tab = Some(tab_id);
        self.state.selected_repo = Some(repo_id);
        self.pending_focus = Some(tab_id);
    }

    /// Pastes into the active tab's Claude pane, else any Claude tab of the repo, else a new one
    /// once it has started.
    pub(crate) fn send_to_agent(&mut self, repo_id: Uuid, text: String) {
        let Some(repo) = self.state.repo(repo_id) else { return };
        let agent_pane = |tab: &Tab| tab.panes.iter().find(|p| p.kind == PaneKind::Agent).map(|p| p.id);
        let active = repo.tabs.iter().find(|t| Some(t.id) == repo.active_tab).and_then(agent_pane);
        let any = repo.tabs.iter().find_map(agent_pane);
        let (tab, pane) = match active.or(any) {
            Some(pane) => {
                (repo.tabs.iter().find(|t| t.panes.iter().any(|p| p.id == pane)).map(|t| t.id), pane)
            }
            None => {
                self.new_tab(repo_id, PaneKind::Agent, None);
                let tab = self.state.repo(repo_id).and_then(|r| r.active_tab);
                let pane = tab.and_then(|t| {
                    self.state.repo(repo_id)?.tabs.iter().find(|x| x.id == t)?.panes.first().map(|p| p.id)
                });
                (tab, pane.unwrap_or_default())
            }
        };
        if let Some(tab) = tab {
            self.state.selected_repo = Some(repo_id);
            if let Some(r) = self.state.repo_mut(repo_id) {
                r.active_tab = Some(tab);
            }
            self.pending_focus = Some(pane);
        }
        self.pending_paste =
            Some(PendingPaste { pane, text, not_before: Instant::now() + AGENT_START_DELAY });
    }

    /// Delivers a queued paste once its pane is running and has had time to start its program.
    pub(crate) fn flush_pending_paste(&mut self) {
        let Some(paste) = &self.pending_paste else { return };
        if Instant::now() < paste.not_before {
            return;
        }
        if let Some(session) = self.pool.get(paste.pane) {
            session.paste(&paste.text);
            self.pending_paste = None;
        }
    }

    pub(crate) fn active_file(&self) -> Option<PathBuf> {
        let repo = self.state.repo(self.state.selected_repo?)?;
        repo.tabs.iter().find(|t| Some(t.id) == repo.active_tab)?.file.clone()
    }
}
