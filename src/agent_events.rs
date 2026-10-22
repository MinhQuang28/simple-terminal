use std::path::Path;

use serde_json::{Value, json};
use uuid::Uuid;

use crate::claude_status_line::{self, StatusFile, StatusLine};
use crate::event_log::EventLog;

/// Environment variable naming the file Claude Code hooks append their events to.
pub const EVENTS_VAR: &str = "REPODECK_AGENT_EVENTS";
/// Environment variable holding the inline settings JSON that registers those hooks.
pub const HOOKS_VAR: &str = "REPODECK_AGENT_HOOKS";
/// Variables that make Claude Code bill an API key instead of the user's subscription.
pub const API_KEY_VARS: &[&str] = &["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN"];
const MESSAGE_CHARS: usize = 200;
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
/// agent started in `cwd`. It also turns off Claude Code's wheel acceleration: it ramps the step
/// up with every consecutive wheel event, which turns a trackpad's stream of small events into
/// ever larger jumps the longer a flick lasts. With `hide_api_keys`, the settings also blank the
/// API key variables, in case the user's shell start-up files export them.
pub fn hooks_settings(cwd: &Path, hide_api_keys: bool) -> String {
    let hook =
        json!([{ "hooks": [{ "type": "command", "command": format!("(cat; echo) >> \"${EVENTS_VAR}\"") }] }]);
    let hooks: serde_json::Map<String, Value> =
        HOOKED_EVENTS.iter().map(|event| (event.to_string(), hook.clone())).collect();
    let mut settings = json!({
        "hooks": hooks,
        "statusLine": claude_status_line::settings(cwd),
        "wheelScrollAccelerationEnabled": false,
    });
    if hide_api_keys {
        let blank: serde_json::Map<String, Value> =
            API_KEY_VARS.iter().map(|var| (var.to_string(), Value::String(String::new()))).collect();
        settings["env"] = Value::Object(blank);
    }
    settings.to_string()
}

/// Tracks what the agent in a pane is doing from the events its hooks append.
pub struct AgentEvents {
    log: EventLog,
    status_line: StatusFile,
    status: AgentStatus,
    session_id: Option<String>,
    /// What the agent is asking permission for, while it waits.
    request: Option<String>,
    /// What the agent said with its latest status: its last reply, a notification, or an error.
    message: Option<String>,
    /// The first line of the prompt the current turn answers.
    prompt: Option<String>,
    /// The agent reported its session over and hasn't started another since.
    ended: bool,
}

impl AgentEvents {
    pub fn create(pane: Uuid) -> Option<Self> {
        Some(Self {
            log: EventLog::create("agent-events", pane)?,
            status_line: StatusFile::create(pane)?,
            status: AgentStatus::Idle,
            session_id: None,
            request: None,
            message: None,
            prompt: None,
            ended: false,
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

    /// Whether the agent's session has ended, leaving the pane to whatever runs after it.
    pub fn ended(&self) -> bool {
        self.ended
    }

    /// The shell printed a prompt again: whatever agent ran from it is gone, whether or not it
    /// got to report so (a crash, a kill). Before any hook arrived the prompt is the one the agent
    /// was started from.
    pub fn note_shell_prompt(&mut self) {
        if self.session_id.is_some() {
            self.ended = true;
        }
    }

    /// The Claude Code conversation the pane is in, once a hook has reported it.
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// The pending permission request, e.g. "Bash: npm test".
    pub fn request(&self) -> Option<&str> {
        self.request.as_deref().filter(|_| self.status == AgentStatus::NeedsApproval)
    }

    /// The text that came with the current status, e.g. Claude's last reply once it stopped.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// The prompt that started the current turn, once a hook has reported it.
    pub fn prompt(&self) -> Option<&str> {
        self.prompt.as_deref()
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
            if let Some(prompt) = event["prompt"].as_str().and_then(first_line) {
                self.prompt = Some(prompt);
            }
            if let Some(status) = status_from_event(&event) {
                self.status = status;
                self.message = message_from_event(&event);
            }
            // Any hook but the last one means an agent is alive in the pane again.
            if event["hook_event_name"].is_string() {
                self.ended = ends_session(&event);
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

/// The first line of the text a status-changing event carries, trimmed to fit a notice.
fn message_from_event(event: &Value) -> Option<String> {
    let key = match event["hook_event_name"].as_str()? {
        "Stop" => "last_assistant_message",
        "Notification" => "message",
        "StopFailure" => "error",
        _ => return None,
    };
    first_line(event[key].as_str()?)
}

/// The first non-blank line of `text`, trimmed to fit a notice.
fn first_line(text: &str) -> Option<String> {
    let line = text.lines().map(str::trim).find(|line| !line.is_empty())?;
    Some(line.chars().take(MESSAGE_CHARS).collect())
}

/// Whether the event leaves the pane to whatever runs after the agent: `/clear` and an in-session
/// `/resume` also end a session, but start another in the same process right away.
fn ends_session(event: &Value) -> bool {
    event["hook_event_name"] == "SessionEnd" && !matches!(event["reason"].as_str(), Some("clear" | "resume"))
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
        // Codex asks questions through a tool; its hook is registered for that tool only.
        "PreToolUse" if event["tool_name"] == "request_user_input" => AgentStatus::NeedsInput,
        "Stop" => AgentStatus::Done,
        "StopFailure" => AgentStatus::Error,
        "Interrupt" => AgentStatus::Idle,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_register_every_hooked_event() {
        let settings: Value = serde_json::from_str(&hooks_settings(Path::new("/"), false)).unwrap();
        for event in HOOKED_EVENTS {
            let command = settings["hooks"][event][0]["hooks"][0]["command"].as_str().unwrap();
            assert!(command.contains(EVENTS_VAR), "{event}");
        }
        assert!(settings["statusLine"]["command"].as_str().unwrap().contains(claude_status_line::STATUS_VAR));
        assert_eq!(settings["wheelScrollAccelerationEnabled"], false);
        assert!(settings.get("env").is_none());
    }

    #[test]
    fn settings_can_blank_the_api_key_variables() {
        let settings: Value = serde_json::from_str(&hooks_settings(Path::new("/"), true)).unwrap();
        for var in API_KEY_VARS {
            assert_eq!(settings["env"][var], "", "{var}");
        }
    }

    #[test]
    fn only_a_real_exit_ends_the_agent() {
        let ends = |json: &str| serde_json::from_str(json).ok().is_some_and(|e| ends_session(&e));
        assert!(ends(r#"{"hook_event_name":"SessionEnd","reason":"prompt_input_exit"}"#));
        assert!(ends(r#"{"hook_event_name":"SessionEnd"}"#));
        assert!(!ends(r#"{"hook_event_name":"SessionEnd","reason":"clear"}"#));
        assert!(!ends(r#"{"hook_event_name":"SessionEnd","reason":"resume"}"#));
        assert!(!ends(r#"{"hook_event_name":"SessionStart","source":"clear"}"#));
    }

    #[test]
    fn a_shell_prompt_ends_an_agent_that_had_started() {
        let mut agent = AgentEvents::create(Uuid::new_v4()).unwrap();
        agent.note_shell_prompt();
        assert!(!agent.ended(), "the prompt the agent is started from");
        agent.session_id = Some("s".into());
        agent.note_shell_prompt();
        assert!(agent.ended());
    }

    #[test]
    fn keeps_the_first_line_of_what_the_agent_said() {
        let message = |json: &str| serde_json::from_str(json).ok().and_then(|e| message_from_event(&e));
        assert_eq!(
            message(
                r#"{"hook_event_name":"Stop","last_assistant_message":"\n  Done: 3 tests pass.\nDetails…"}"#
            )
            .as_deref(),
            Some("Done: 3 tests pass.")
        );
        assert_eq!(
            message(r#"{"hook_event_name":"Notification","message":"Claude needs your permission"}"#)
                .as_deref(),
            Some("Claude needs your permission")
        );
        assert_eq!(
            message(r#"{"hook_event_name":"StopFailure","error":"rate limited"}"#).as_deref(),
            Some("rate limited")
        );
        assert_eq!(message(r#"{"hook_event_name":"Stop"}"#), None);
        assert_eq!(message(r#"{"hook_event_name":"UserPromptSubmit","prompt":"hi"}"#), None);
        let long = format!(r#"{{"hook_event_name":"Stop","last_assistant_message":"{}"}}"#, "x".repeat(500));
        assert_eq!(message(&long).unwrap().chars().count(), MESSAGE_CHARS);
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
        assert_eq!(
            status(r#"{"hook_event_name":"PreToolUse","tool_name":"request_user_input"}"#),
            Some(AgentStatus::NeedsInput)
        );
        assert_eq!(status(r#"{"hook_event_name":"PreToolUse","tool_name":"Bash"}"#), None);
        assert_eq!(status(r#"{"hook_event_name":"Interrupt"}"#), Some(AgentStatus::Idle));
        assert_eq!(status("not json"), None);
    }
}
