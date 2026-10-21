use std::collections::HashMap;
use std::time::{Duration, Instant};

use uuid::Uuid;

use crate::agent_events::AgentStatus;
use crate::model::now_secs;

const MAX_NOTICES: usize = 200;
/// Minimum gap between system notifications from one pane, so a chatty script can't flood
/// Notification Center.
const SYSTEM_NOTICE_GAP: Duration = Duration::from_secs(3);

/// Where a notice came from, so opening it can show that pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Source {
    pub repo: Uuid,
    pub tab: Uuid,
    pub pane: Uuid,
}

/// Something a pane reported that the user may want to come back to.
#[derive(Clone, Debug)]
pub struct Notice {
    pub id: u64,
    /// Unix seconds.
    pub at: u64,
    pub source: Source,
    /// Colors the notice; `Done` for plain messages and successful commands.
    pub status: AgentStatus,
    pub label: String,
    /// "repo · tab".
    pub title: String,
    pub detail: String,
    pub read: bool,
}

/// Notices of this run, oldest first; nothing is persisted.
#[derive(Default)]
pub struct Inbox {
    notices: Vec<Notice>,
    next_id: u64,
    notified_at: HashMap<Uuid, Instant>,
}

impl Inbox {
    pub fn notices(&self) -> &[Notice] {
        &self.notices
    }

    pub fn get(&self, id: u64) -> Option<&Notice> {
        self.notices.iter().find(|n| n.id == id)
    }

    pub fn unread(&self) -> usize {
        self.notices.iter().filter(|n| !n.read).count()
    }

    /// Adds a notice unless the pane's latest unread one already says the same thing, which
    /// happens when an agent re-asks for the same approval.
    pub fn push(&mut self, source: Source, status: AgentStatus, label: &str, title: String, detail: String) {
        let repeat = self
            .notices
            .iter()
            .rev()
            .find(|n| n.source.pane == source.pane && !n.read)
            .is_some_and(|n| n.label == label && n.detail == detail);
        if repeat {
            return;
        }
        self.next_id += 1;
        let notice = Notice {
            id: self.next_id,
            at: now_secs(),
            source,
            status,
            label: label.to_string(),
            title,
            detail,
            read: false,
        };
        self.notices.push(notice);
        if self.notices.len() > MAX_NOTICES {
            self.notices.remove(0);
        }
    }

    /// Whether a system notification for `pane` may go out now; records it if so.
    pub fn allow_system_notice(&mut self, pane: Uuid) -> bool {
        let now = Instant::now();
        let recent = self.notified_at.get(&pane).is_some_and(|at| now - *at < SYSTEM_NOTICE_GAP);
        if !recent {
            self.notified_at.insert(pane, now);
        }
        !recent
    }

    pub fn mark_read(&mut self, id: u64) {
        if let Some(notice) = self.notices.iter_mut().find(|n| n.id == id) {
            notice.read = true;
        }
    }

    /// The user is looking at `pane`, so its notices are no longer news.
    pub fn mark_pane_read(&mut self, pane: Uuid) {
        for notice in self.notices.iter_mut().filter(|n| n.source.pane == pane) {
            notice.read = true;
        }
    }

    pub fn mark_all_read(&mut self) {
        for notice in &mut self.notices {
            notice.read = true;
        }
    }

    pub fn clear(&mut self) {
        self.notices.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> Source {
        Source { repo: Uuid::nil(), tab: Uuid::nil(), pane: Uuid::nil() }
    }

    #[test]
    fn counts_unread_and_marks_by_pane() {
        let mut inbox = Inbox::default();
        inbox.push(source(), AgentStatus::Done, "Done", "r · t".into(), "a".into());
        inbox.push(source(), AgentStatus::Error, "Error", "r · t".into(), "b".into());
        assert_eq!(inbox.unread(), 2);
        inbox.mark_pane_read(Uuid::nil());
        assert_eq!(inbox.unread(), 0);
    }

    #[test]
    fn skips_an_unread_repeat_but_not_after_it_was_read() {
        let mut inbox = Inbox::default();
        inbox.push(source(), AgentStatus::NeedsApproval, "Approve?", "r · t".into(), "".into());
        inbox.push(source(), AgentStatus::NeedsApproval, "Approve?", "r · t".into(), "".into());
        assert_eq!(inbox.notices().len(), 1);
        inbox.mark_all_read();
        inbox.push(source(), AgentStatus::NeedsApproval, "Approve?", "r · t".into(), "".into());
        assert_eq!(inbox.notices().len(), 2);
    }

    #[test]
    fn keeps_only_the_newest_notices() {
        let mut inbox = Inbox::default();
        for i in 0..MAX_NOTICES + 5 {
            inbox.push(source(), AgentStatus::Done, "Done", "r".into(), i.to_string());
        }
        assert_eq!(inbox.notices().len(), MAX_NOTICES);
        assert_eq!(inbox.notices()[0].detail, "5");
    }
}
