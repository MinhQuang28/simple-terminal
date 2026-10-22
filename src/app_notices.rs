use std::collections::HashSet;

use uuid::Uuid;

use crate::agent_events::AgentStatus;
use crate::app::RepodeckApp;
use crate::attention;
use crate::inbox::Source;
use crate::shell_events::Finished;

/// Something a pane reported during a frame.
pub(crate) enum Report {
    /// An agent stopped and waits on the user, saying `message` (possibly nothing).
    Status { status: AgentStatus, message: String },
    /// A long command finished.
    Command(Finished),
    /// A program sent an OSC notification.
    Message(String),
}

pub(crate) struct PaneReport {
    pub source: Source,
    pub repo_name: String,
    pub tab_title: String,
    pub report: Report,
}

impl RepodeckApp {
    /// Files reports in the inbox, posts system notifications for panes the user isn't watching,
    /// and marks notices of the panes on screen as read.
    pub(crate) fn deliver(
        &mut self,
        reports: Vec<PaneReport>,
        window_focused: bool,
        visible: &HashSet<Uuid>,
    ) {
        for PaneReport { source, repo_name, tab_title, report } in reports {
            let watching = window_focused && visible.contains(&source.pane);
            log::debug!("report for pane {} (watching={watching})", source.pane);
            let (status, label, detail) = match report {
                Report::Status { status, message } => (status, status.label().to_string(), message),
                Report::Command(finished) => {
                    let ok = finished.exit_code == 0;
                    let outcome = if ok { "finished" } else { "failed" };
                    let label = format!("Command {outcome} in {}s", finished.duration.as_secs());
                    let status = if ok { AgentStatus::Done } else { AgentStatus::Error };
                    (status, label, finished.command)
                }
                Report::Message(text) => (AgentStatus::NeedsInput, "Notification".to_string(), text),
            };
            let settings = &self.state.settings;
            if !watching && settings.notifications && self.inbox.allow_system_notice(source.pane) {
                let body = if detail.is_empty() { &tab_title } else { &detail };
                attention::notify(&format!("{repo_name} · {label}"), body, settings.notification_sound);
            }
            self.inbox.push(source, status, &label, format!("{repo_name} · {tab_title}"), detail);
        }
        if window_focused {
            for pane in visible {
                self.inbox.mark_pane_read(*pane);
            }
        }
    }

    /// Shows the next pane (after the active one) whose agent is waiting; false when none is.
    pub(crate) fn focus_next_attention(&mut self) -> bool {
        let current = self
            .state
            .selected_repo
            .and_then(|id| self.state.repo(id))
            .and_then(|r| r.tabs.iter().find(|t| Some(t.id) == r.active_tab))
            .and_then(|t| t.active_pane);
        let all: Vec<Source> = self
            .state
            .projects
            .iter()
            .flat_map(|p| &p.repos)
            .flat_map(|r| r.tabs.iter().map(move |t| (r.id, t)))
            .flat_map(|(repo, t)| t.panes.iter().map(move |p| Source { repo, tab: t.id, pane: p.id }))
            .collect();
        let waiting = |s: &Source| self.pool.agent_status(s.pane).is_some_and(AgentStatus::needs_attention);
        let start = current.and_then(|c| all.iter().position(|s| s.pane == c)).map_or(0, |i| i + 1);
        let Some(next) = all.iter().cycle().skip(start).take(all.len()).find(|s| waiting(s)).copied() else {
            return false;
        };
        let Source { repo, tab, pane } = next;
        self.state.selected_repo = Some(repo);
        if let Some(t) = self.state.repo_mut(repo).and_then(|r| {
            r.active_tab = Some(tab);
            r.tabs.iter_mut().find(|t| t.id == tab)
        }) {
            t.active_pane = Some(pane);
        }
        self.settings_open = false;
        self.focus_active_pane();
        true
    }

    /// Shows `pane`; false when no tab holds it.
    pub(crate) fn focus_pane(&mut self, pane: Uuid) -> bool {
        let found = self
            .state
            .projects
            .iter()
            .flat_map(|p| &p.repos)
            .flat_map(|r| r.tabs.iter().map(move |t| (r.id, t)))
            .find(|(_, t)| t.panes.iter().any(|p| p.id == pane))
            .map(|(repo, t)| Source { repo, tab: t.id, pane });
        let Some(Source { repo, tab, pane }) = found else { return false };
        self.state.selected_repo = Some(repo);
        if let Some(t) = self.state.repo_mut(repo).and_then(|r| {
            r.active_tab = Some(tab);
            r.tabs.iter_mut().find(|t| t.id == tab)
        }) {
            t.active_pane = Some(pane);
        }
        self.settings_open = false;
        self.focus_active_pane();
        true
    }

    /// A message from a script, filed against its pane when it said which one it runs in.
    pub(crate) fn notify_from_cli(&mut self, pane: Option<Uuid>, text: String) {
        let located = pane.and_then(|pane| {
            self.state.projects.iter().flat_map(|p| &p.repos).find_map(|r| {
                let tab = r.tabs.iter().find(|t| t.panes.iter().any(|p| p.id == pane))?;
                Some(PaneReport {
                    source: Source { repo: r.id, tab: tab.id, pane },
                    repo_name: r.name.clone(),
                    tab_title: tab.title.clone(),
                    report: Report::Message(text.clone()),
                })
            })
        });
        match located {
            Some(report) => self.deliver(vec![report], false, &HashSet::new()),
            None => {
                let nowhere = Source { repo: Uuid::nil(), tab: Uuid::nil(), pane: Uuid::nil() };
                if self.state.settings.notifications {
                    attention::notify("Repodeck", &text, self.state.settings.notification_sound);
                }
                self.inbox.push(nowhere, AgentStatus::NeedsInput, "Notification", String::new(), text);
            }
        }
    }

    pub(crate) fn open_notice(&mut self, id: u64) {
        let Some(notice) = self.inbox.get(id) else { return };
        let Source { repo, pane, .. } = notice.source;
        self.inbox.mark_read(id);
        if !self.focus_pane(pane) && self.state.repo(repo).is_some() {
            self.state.selected_repo = Some(repo);
            self.focus_active_pane();
        }
    }
}
