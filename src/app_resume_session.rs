use uuid::Uuid;

use crate::app::RepodeckApp;
use crate::model::PaneKind;

impl RepodeckApp {
    /// Selects the tab already showing `session`, or opens a Claude tab that resumes it.
    pub(crate) fn resume_session(&mut self, repo_id: Uuid, session: String, title: String) {
        let Some(repo) = self.state.repo_mut(repo_id) else { return };
        let existing = repo
            .tabs
            .iter()
            .find(|t| t.panes.iter().any(|p| p.session.as_deref() == Some(session.as_str())))
            .map(|t| t.id);
        if let Some(tab) = existing {
            repo.active_tab = Some(tab);
            self.state.selected_repo = Some(repo_id);
            self.focus_active_pane();
            return;
        }
        self.new_tab(repo_id, PaneKind::Agent, None);
        if let Some(tab) = self.state.repo_mut(repo_id).and_then(|r| r.active_tab_mut()) {
            tab.title = title;
            tab.panes[0].session = Some(session);
        }
        self.dirty = true;
    }
}
