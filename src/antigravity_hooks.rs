//! Antigravity CLI (`agy`) takes hooks from plugins only, not per session, so Repodeck keeps a
//! small plugin in Antigravity's global config. Its hooks do nothing outside a Repodeck pane.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::agent_events::EVENTS_VAR;

/// Environment variable holding the inline settings JSON `agy --settings` takes in a pane.
pub const SETTINGS_VAR: &str = "REPODECK_AGY_SETTINGS";
/// The `repodeck` verb Antigravity runs as its status line when the user has none of their own.
pub const STATUS_VERB: &str = "agy-status";
const PLUGIN: &str = "repodeck";
/// Antigravity hook events Repodeck listens to; all take handlers as a flat list.
const HOOKED_EVENTS: &[&str] = &["SessionStart", "PreInvocation", "Stop"];

/// Writes (or refreshes) the plugin, when Antigravity is set up on this machine.
pub fn install() {
    if let Some(home) = std::env::var_os("HOME") {
        install_under(&PathBuf::from(home).join(".gemini").join("config"));
    }
}

/// Settings for `agy --settings`: a status line that saves Antigravity's status JSON (context in
/// use, quota), then pipes it to the user's own status line command so that still shows. Without
/// one, `exe` prints a line in place of the footer Antigravity drops for any status line.
pub fn session_settings(exe: Option<&Path>) -> String {
    let own = std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".gemini/antigravity-cli/settings.json"))
        .and_then(|file| serde_json::from_str::<Value>(&fs::read_to_string(file).ok()?).ok())
        .and_then(|settings| settings.get("statusLine").filter(|s| s["type"] == "command").cloned());
    let own = own.or_else(|| {
        let exe = exe?.to_str()?.replace('\'', r"'\''");
        Some(json!({ "type": "command", "command": format!("'{exe}' {STATUS_VERB}") }))
    });
    json!({ "statusLine": crate::claude_status_line::saving(own) }).to_string()
}

/// `repodeck agy-status`: reads Antigravity's status JSON and prints the mode and model, as its
/// own footer does.
pub fn print_status_line() {
    let status: Value = serde_json::from_reader(std::io::stdin()).unwrap_or_default();
    println!("{}", footer(&status));
}

fn footer(status: &Value) -> String {
    let parts = [&status["cycle_mode"], &status["model"]["display_name"]];
    parts.iter().filter_map(|part| part.as_str()).collect::<Vec<_>>().join(" · ")
}

fn install_under(config: &Path) -> Option<()> {
    if !config.is_dir() {
        return None;
    }
    let dir = config.join("plugins").join(PLUGIN);
    fs::create_dir_all(&dir).ok()?;
    let manifest =
        json!({ "name": PLUGIN, "version": "1.0.0", "description": "Reports agent status to Repodeck" });
    for (name, content) in [("plugin.json", manifest), ("hooks.json", hooks())] {
        let (path, text) = (dir.join(name), format!("{content:#}\n"));
        if fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
            fs::write(&path, text).ok()?;
        }
    }
    Some(())
}

/// Each hook appends its JSON payload, newline-terminated, to the file named by `EVENTS_VAR`,
/// and answers with the empty object Antigravity expects.
fn hooks() -> Value {
    let command = format!("[ -n \"${EVENTS_VAR}\" ] && (cat; echo) >> \"${EVENTS_VAR}\"; echo '{{}}'");
    let handlers = json!([{ "type": "command", "command": command, "timeout": 5 }]);
    let events: serde_json::Map<String, Value> =
        HOOKED_EVENTS.iter().map(|event| (event.to_string(), handlers.clone())).collect();
    json!({ PLUGIN: events })
}

/// An Antigravity hook payload in the shape of Claude Code's, which the rest of Repodeck reads.
/// None for anything else.
pub fn as_claude_event(event: &Value) -> Option<Value> {
    let name = match event["hookEventName"].as_str()? {
        "SessionStart" => "SessionStart",
        "PreInvocation" => "UserPromptSubmit",
        "Stop" => match event["terminationReason"].as_str() {
            Some("USER_CANCELED") => "Interrupt",
            Some("ERROR") => "StopFailure",
            _ => "Stop",
        },
        _ => return None,
    };
    let mut out = json!({ "hook_event_name": name, "session_id": event["conversationId"] });
    match name {
        "UserPromptSubmit" => out["prompt"] = event["lastUserInput"].clone(),
        "StopFailure" => out["error"] = event["error"].clone(),
        _ => {}
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_only_where_antigravity_is_set_up() {
        let root = std::env::temp_dir().join(format!("repodeck-agy-{}", uuid::Uuid::new_v4()));
        assert!(install_under(&root.join("config")).is_none());
        assert!(!root.exists());

        fs::create_dir_all(root.join("config")).unwrap();
        install_under(&root.join("config")).unwrap();
        let plugin = root.join("config/plugins/repodeck");
        let hooks: Value =
            serde_json::from_str(&fs::read_to_string(plugin.join("hooks.json")).unwrap()).unwrap();
        for event in HOOKED_EVENTS {
            assert!(hooks[PLUGIN][event][0]["command"].as_str().unwrap().contains(EVENTS_VAR), "{event}");
        }
        assert!(plugin.join("plugin.json").is_file());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reads_antigravity_payloads_as_claude_events() {
        let read = |json: &str| as_claude_event(&serde_json::from_str(json).unwrap());
        let prompt =
            read(r#"{"hookEventName":"PreInvocation","conversationId":"c1","lastUserInput":"fix it"}"#);
        assert_eq!(
            prompt,
            Some(json!({ "hook_event_name": "UserPromptSubmit", "session_id": "c1", "prompt": "fix it" }))
        );
        let name = |json: &str| read(json).map(|e| e["hook_event_name"].as_str().unwrap().to_string());
        assert_eq!(
            name(r#"{"hookEventName":"Stop","terminationReason":"NO_TOOL_CALL"}"#).as_deref(),
            Some("Stop")
        );
        assert_eq!(
            name(r#"{"hookEventName":"Stop","terminationReason":"USER_CANCELED"}"#).as_deref(),
            Some("Interrupt")
        );
        let failed = read(r#"{"hookEventName":"Stop","terminationReason":"ERROR","error":"quota"}"#).unwrap();
        assert_eq!(
            (failed["hook_event_name"].as_str(), failed["error"].as_str()),
            (Some("StopFailure"), Some("quota"))
        );
        assert_eq!(read(r#"{"hook_event_name":"Stop"}"#), None);
    }

    #[test]
    fn the_status_line_names_mode_and_model() {
        let status = json!({ "cycle_mode": "auto", "model": { "display_name": "Gemini 4 Carbon (Medium)" } });
        assert_eq!(footer(&status), "auto · Gemini 4 Carbon (Medium)");
        assert_eq!(footer(&json!({})), "");
    }
}
