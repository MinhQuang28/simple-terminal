//! OSC 7501, the Program Status Protocol: programs (Claude Code since 2.1.295, Crush, Pi,
//! Terraform…) report whether they are working, done, or blocked on the user, as
//! `ESC ] 7501 ; state=blocked:kind=permission:app=claude-code:msg=<base64> ST`. Claude Code only
//! reports once the terminal has answered its `ESC ] 7501 ; ? ST` query at startup.
//!
//! This gives status to agents Repodeck didn't start itself, e.g. `claude` typed in a terminal
//! tab, without hooks. Only the root record (no `id`) is kept: sub-records describe parts of a
//! program's work that the sidebar has no place for.

use crate::agent_events::AgentStatus;
use crate::agent_launch::Agent;

/// Reply to the feature query: the states and blocked kinds we understand.
pub const QUERY_REPLY: &[u8] =
    b"\x1b]7501;?:states=idle,working,done,blocked,error:kinds=permission,question,auth\x1b\\";

/// One report as the program sent it.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    /// `None` for `clear`.
    pub status: Option<AgentStatus>,
    pub app: String,
    pub message: String,
    /// Reports with an id address a sub-record.
    pub root: bool,
}

/// Parses the payload after `7501;`; `None` for the query, a sub-record, or a malformed report.
pub fn parse(body: &str) -> Option<Report> {
    let mut state = None;
    let (mut kind, mut app, mut message, mut root) = ("", "", String::new(), true);
    for pair in body.split(':') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        match key {
            "state" => state = Some(value),
            "kind" => kind = value,
            "app" => app = value,
            "msg" => message = decode_base64(value).unwrap_or_default(),
            "id" => root = value.is_empty(),
            // Unknown keys are ignored, as the protocol asks.
            _ => {}
        }
    }
    let status = match state? {
        "idle" => Some(AgentStatus::Idle),
        "working" => Some(AgentStatus::Working),
        "done" => Some(AgentStatus::Done),
        "error" => Some(AgentStatus::Error),
        "blocked" if kind == "permission" => Some(AgentStatus::NeedsApproval),
        "blocked" => Some(AgentStatus::NeedsInput),
        "clear" => None,
        // An unrecognised state voids the whole report.
        _ => return None,
    };
    // Decoded text must not carry control characters.
    let message = message.chars().filter(|c| !c.is_control()).collect();
    Some(Report { status, app: app.chars().take(32).collect(), message, root })
}

/// The pane's root record.
#[derive(Default)]
pub struct ProgramStatus {
    status: Option<AgentStatus>,
    app: String,
    /// The status line that came with the current state.
    message: String,
}

impl ProgramStatus {
    /// Applies a report; returns the new status and its message when the status changed.
    pub fn apply(&mut self, report: Report) -> Option<(AgentStatus, String)> {
        if !report.root {
            return None;
        }
        let before = self.status;
        self.status = report.status;
        self.message = report.message.clone();
        if report.status.is_some() {
            self.app = report.app;
        }
        (self.status != before).then(|| (self.status.unwrap_or_default(), report.message))
    }

    pub fn status(&self) -> Option<AgentStatus> {
        self.status
    }

    /// What the program asks permission for, while it waits on it.
    pub fn request(&self) -> Option<&str> {
        (self.status == Some(AgentStatus::NeedsApproval)).then_some(self.message.as_str())
    }

    /// The agent behind the reports, by the `app` it gave.
    pub fn agent(&self) -> Option<Agent> {
        self.status?;
        match self.app.as_str() {
            "claude-code" | "claude" => Some(Agent::Claude),
            "codex" | "codex-cli" => Some(Agent::Codex),
            "opencode" => Some(Agent::OpenCode),
            _ => None,
        }
    }

    /// A new shell prompt means the program is gone: working and blocked records lapse, as the
    /// protocol says; done and error stay until seen.
    pub fn prompt_started(&mut self) {
        if matches!(
            self.status,
            Some(
                AgentStatus::Working
                    | AgentStatus::NeedsApproval
                    | AgentStatus::NeedsInput
                    | AgentStatus::Idle
            )
        ) {
            self.status = None;
        }
    }

    /// "Done" turns to idle once the user has seen it.
    pub fn acknowledge_done(&mut self) {
        if self.status == Some(AgentStatus::Done) {
            self.status = Some(AgentStatus::Idle);
        }
    }
}

/// Standard base64 with optional padding; `None` if malformed or not UTF-8.
fn decode_base64(text: &str) -> Option<String> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for c in text.trim_end_matches('=').bytes() {
        acc = (acc << 6) | u32::from(value(c)?);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_states_kinds_and_messages() {
        let r = parse("state=blocked:kind=permission:app=claude-code:msg=QXBwbHk/").unwrap();
        assert_eq!(
            (r.status, r.app.as_str(), r.message.as_str()),
            (Some(AgentStatus::NeedsApproval), "claude-code", "Apply?")
        );
        assert_eq!(parse("state=blocked:kind=question").unwrap().status, Some(AgentStatus::NeedsInput));
        assert_eq!(parse("state=working:progress=40:future=1").unwrap().status, Some(AgentStatus::Working));
        assert_eq!(parse("state=clear").unwrap().status, None);
        assert!(!parse("state=working:id=a/b").unwrap().root);
        assert_eq!(parse("state=bogus"), None);
        assert_eq!(parse("?"), None);
        assert_eq!(decode_base64("aGk").as_deref(), Some("hi"));
    }

    #[test]
    fn tracks_the_root_record() {
        let mut status = ProgramStatus::default();
        let working = parse("state=working:app=claude-code").unwrap();
        assert_eq!(status.apply(working.clone()), Some((AgentStatus::Working, String::new())));
        assert_eq!(status.apply(working), None, "no change, no report");
        assert_eq!(status.agent(), Some(Agent::Claude));
        assert_eq!(status.apply(parse("state=done:id=sub").unwrap()), None, "sub-records are ignored");
        status.apply(parse("state=done:msg=RmluaXNoZWQ=").unwrap());
        status.acknowledge_done();
        assert_eq!(status.status(), Some(AgentStatus::Idle));
        status.prompt_started();
        assert_eq!(status.status(), None);
    }
}
