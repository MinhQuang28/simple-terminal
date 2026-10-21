use std::path::Path;

use serde_json::{Value, json};
use uuid::Uuid;

use crate::claude_status_line::{self, StatusFile, StatusLine};
use crate::event_log::EventLog;

/// Environment variable naming the file Claude Code hooks append their events to.
pub const EVENTS_VAR: &str = "REPODECK_AGENT_EVENTS";
/// Environment variable holding the inline settings JSON that registers those hooks.
pub const HOOKS_VAR: &str = "REPODECK_AGENT_HOOKS";
const HOOKED_EVENTS: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "PermissionRequest",
    "Notification",
    "Stop",
    "StopFailure",
    "SessionEnd",
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum AgentStatus {
    #[default]
    Idle,
    Working,
    Done,
    NeedsInput,
    NeedsApproval,
    Error,
}

impl AgentStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "",
            Self::Working => "Working",
            Self::Done => "Done",
            Self::NeedsInput => "Your turn",
            Self::NeedsApproval => "Approve?",
            Self::Error => "Error",
        }
    }

    /// Whether the agent stopped and is waiting on the user.
    pub fn needs_attention(self) -> bool {
        matches!(self, Self::Done | Self::NeedsInput | Self::NeedsApproval | Self::Error)
    }
}

/// Settings JSON for `claude --settings` that makes every hooked event append its JSON payload,
/// newline-terminated, to the file named by `EVENTS_VAR`, and saves the status line JSON of an
/// agent started in `cwd`.
pub fn hooks_settings(cwd: &Path) -> String {
    let hook =
        json!([{ "hooks": [{ "type": "command", "command": format!("(cat; echo) >> \"${EVENTS_VAR}\"") }] }]);
    let hooks: serde_json::Map<String, Value> =
        HOOKED_EVENTS.iter().map(|event| (event.to_string(), hook.clone())).collect();
    json!({ "hooks": hooks, "statusLine": claude_status_line::settings(cwd) }).to_string()
}

/// Rewrites each `claude` invocation in a shell command line to load the hook settings.
pub fn with_hooks(command: &str) -> String {
    let mut out = Vec::new();
    let mut command_start = true;
    for token in command.split_whitespace() {
        if command_start && Path::new(token).file_name().is_some_and(|name| name == "claude") {
            out.push(format!("{token} --settings \"${HOOKS_VAR}\""));
        } else {
            out.push(token.to_string());
        }
        command_start = matches!(token, "||" | "&&" | ";" | "|") || token.ends_with(';');
    }
    out.join(" ")
}

/// Tracks what the agent in a pane is doing from the events its hooks append.
pub struct AgentEvents {
    log: EventLog,
    status_line: StatusFile,
    status: AgentStatus,
    session_id: Option<String>,
    /// What the agent is asking permission for, while it waits.
    request: Option<String>,
}

impl AgentEvents {
    pub fn create(pane: Uuid) -> Option<Self> {
        Some(Self {
            log: EventLog::create("agent-events", pane)?,
            status_line: StatusFile::create(pane)?,
            status: AgentStatus::Idle,
            session_id: None,
            request: None,
        })
    }

    pub fn path(&self) -> &Path {
        self.log.path()
    }

    /// Where the status line command saves Claude Code's status JSON.
    pub fn status_line_path(&self) -> &Path {
        self.status_line.path()
    }

    /// Subscription limits and context use, once Claude Code has reported them.
    pub fn status_line(&self) -> Option<StatusLine> {
        self.status_line.latest()
    }

    pub fn status(&self) -> AgentStatus {
        self.status
    }

    /// The Claude Code conversation the pane is in, once a hook has reported it.
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// The pending permission request, e.g. "Bash: npm test".
    pub fn request(&self) -> Option<&str> {
        self.request.as_deref().filter(|_| self.status == AgentStatus::NeedsApproval)
    }

    /// Applies newly appended events; returns the status if it changed.
    pub fn poll(&mut self) -> Option<AgentStatus> {
        let before = self.status;
        self.status_line.poll();
        for line in self.log.poll_lines() {
            let Ok(event) = serde_json::from_str::<Value>(&line) else { continue };
            if let Some(id) = event["session_id"].as_str() {
                self.session_id = Some(id.to_string());
            }
            if let Some(request) = request_from_event(&event) {
                self.request = Some(request);
            }
            if let Some(status) = status_from_event(&event) {
                self.status = status;
            }
        }
        (self.status != before).then_some(self.status)
    }

    /// The user has seen the finished result, so "Done" stops asking for attention.
    pub fn acknowledge_done(&mut self) {
        if self.status == AgentStatus::Done {
            self.status = AgentStatus::Idle;
        }
    }

    /// The user answered a prompt or typed into the pane, so it no longer needs attention.
    pub fn note_user_input(&mut self) {
        self.status = match self.status {
            AgentStatus::NeedsInput | AgentStatus::NeedsApproval => AgentStatus::Working,
            AgentStatus::Done | AgentStatus::Error => AgentStatus::Idle,
            other => other,
        };
    }
}

/// Summarises a `PermissionRequest` event as "tool: what", using the tool's most telling input.
fn request_from_event(event: &Value) -> Option<String> {
    if event["hook_event_name"].as_str()? != "PermissionRequest" {
        return None;
    }
    let tool = event["tool_name"].as_str().unwrap_or("Tool");
    let input = &event["tool_input"];
    let detail = ["command", "file_path", "pattern", "url", "prompt", "description"]
        .iter()
        .find_map(|key| input[key].as_str())
        .or_else(|| input.as_object()?.values().find_map(Value::as_str))
        .unwrap_or_default();
    let detail: String = detail.lines().next().unwrap_or_default().chars().take(120).collect();
    Some(if detail.is_empty() { tool.to_string() } else { format!("{tool}: {detail}") })
}

fn status_from_event(event: &Value) -> Option<AgentStatus> {
    Some(match event["hook_event_name"].as_str()? {
        "SessionStart" | "SessionEnd" => AgentStatus::Idle,
        "UserPromptSubmit" => AgentStatus::Working,
        "PermissionRequest" => AgentStatus::NeedsApproval,
        "Notification" => match event["notification_type"].as_str() {
            Some("permission_prompt") => AgentStatus::NeedsApproval,
            Some("idle_prompt" | "elicitation_dialog") => AgentStatus::NeedsInput,
            _ => return None,
        },
        "Stop" => AgentStatus::Done,
        "StopFailure" => AgentStatus::Error,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hooks_only_the_first_word_of_each_command() {
        let out = with_hooks("claude --continue || claude");
        assert_eq!(
            out,
            "claude --settings \"$REPODECK_AGENT_HOOKS\" --continue || claude --settings \"$REPODECK_AGENT_HOOKS\""
        );
        assert_eq!(with_hooks("echo claude"), "echo claude");
        assert_eq!(
            with_hooks("/opt/bin/claude --model opus"),
            "/opt/bin/claude --settings \"$REPODECK_AGENT_HOOKS\" --model opus"
        );
        assert_eq!(with_hooks("claude-foo"), "claude-foo");
    }

    #[test]
    fn settings_register_every_hooked_event() {
        let settings: Value = serde_json::from_str(&hooks_settings(Path::new("/"))).unwrap();
        for event in HOOKED_EVENTS {
            let command = settings["hooks"][event][0]["hooks"][0]["command"].as_str().unwrap();
            assert!(command.contains(EVENTS_VAR), "{event}");
        }
        assert!(settings["statusLine"]["command"].as_str().unwrap().contains(claude_status_line::STATUS_VAR));
    }

    #[test]
    fn summarises_permission_requests() {
        let event: Value = serde_json::from_str(
            r#"{"hook_event_name":"PermissionRequest","tool_name":"Bash","tool_input":{"command":"npm test\n","description":"Run tests"}}"#,
        )
        .unwrap();
        assert_eq!(request_from_event(&event).as_deref(), Some("Bash: npm test"));
        let plain: Value = serde_json::from_str(
            r#"{"hook_event_name":"PermissionRequest","tool_name":"WebSearch","tool_input":{}}"#,
        )
        .unwrap();
        assert_eq!(request_from_event(&plain).as_deref(), Some("WebSearch"));
    }

    #[test]
    fn maps_hook_events_to_statuses() {
        let status = |json: &str| serde_json::from_str(json).ok().and_then(|e| status_from_event(&e));
        assert_eq!(status(r#"{"hook_event_name":"Stop"}"#), Some(AgentStatus::Done));
        assert_eq!(status(r#"{"hook_event_name":"UserPromptSubmit"}"#), Some(AgentStatus::Working));
        assert_eq!(
            status(r#"{"hook_event_name":"Notification","notification_type":"permission_prompt"}"#),
            Some(AgentStatus::NeedsApproval)
        );
        assert_eq!(status(r#"{"hook_event_name":"Notification","notification_type":"auth_success"}"#), None);
        assert_eq!(status("not json"), None);
    }
}
