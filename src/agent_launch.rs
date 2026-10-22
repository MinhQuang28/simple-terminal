//! Starts agent CLIs so their lifecycle events reach the pane's event file, whichever agent the
//! command names: Claude Code takes inline settings, Codex session hooks (through
//! `repodeck run-codex`, which pre-trusts them), OpenCode a plugin named in inline config.

use std::path::Path;

use serde_json::{Value, json};

use crate::agent_events::{EVENTS_VAR, HOOKS_VAR};

/// The `repodeck` verb that starts Codex with trusted hooks.
pub const CODEX_VERB: &str = "run-codex";
/// OpenCode reads inline JSON config from this variable.
pub const OPENCODE_CONFIG_VAR: &str = "OPENCODE_CONFIG_CONTENT";
const OPENCODE_PLUGIN: &str = include_str!("opencode_plugin.js");
const OPENCODE_PLUGIN_FILE: &str = "opencode-repodeck.js";
/// Codex hook events Repodeck listens to, with the tool matcher one of them needs.
const CODEX_HOOKS: &[(&str, Option<&str>)] = &[
    ("SessionStart", None),
    ("UserPromptSubmit", None),
    ("PermissionRequest", None),
    ("PreToolUse", Some("^request_user_input$")),
    ("Stop", None),
    ("Interrupt", None),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Agent {
    Claude,
    Codex,
    OpenCode,
}

impl Agent {
    /// The agent a command token names, by its file name.
    pub fn of(token: &str) -> Option<Self> {
        match Path::new(token).file_name()?.to_str()? {
            "claude" => Some(Self::Claude),
            "codex" => Some(Self::Codex),
            "opencode" => Some(Self::OpenCode),
            _ => None,
        }
    }
}

/// Whether `command` starts `agent` anywhere in its pipeline.
pub fn uses(command: &str, agent: Agent) -> bool {
    command_tokens(command).any(|(start, token)| start && Agent::of(token) == Some(agent))
}

/// Rewrites each agent invocation in a shell command line so its hooks are loaded.
pub fn with_hooks(command: &str) -> String {
    let out: Vec<String> = command_tokens(command)
        .map(|(start, token)| match Agent::of(token).filter(|_| start) {
            Some(Agent::Claude) => format!("{token} --settings \"${HOOKS_VAR}\""),
            Some(Agent::Codex) => format!("repodeck {CODEX_VERB} {token}"),
            _ => token.to_string(),
        })
        .collect();
    out.join(" ")
}

/// The command line's words, each flagged when it begins a command.
fn command_tokens(command: &str) -> impl Iterator<Item = (bool, &str)> {
    let mut start = true;
    command.split_whitespace().map(move |token| {
        let at_start = start;
        start = matches!(token, "||" | "&&" | ";" | "|") || token.ends_with(';');
        (at_start, token)
    })
}

/// `-c` overrides registering Repodeck's hooks with Codex, in its TOML syntax. Each appends the
/// hook's JSON payload, newline-terminated, to the file named by `EVENTS_VAR`.
pub fn codex_hook_flags() -> Vec<String> {
    let command = format!("(cat; echo) >> \\\"${EVENTS_VAR}\\\"");
    CODEX_HOOKS
        .iter()
        .flat_map(|(event, matcher)| {
            let matcher = matcher.map(|m| format!("matcher=\"{m}\",")).unwrap_or_default();
            let hook =
                format!("hooks.{event}=[{{{matcher}hooks=[{{type=\"command\",command=\"{command}\"}}]}}]");
            ["-c".to_string(), hook]
        })
        .collect()
}

/// Inline OpenCode config that loads Repodeck's event plugin, merged into any config already in
/// `existing`. None when the plugin file can't be written under `data_dir`.
pub fn opencode_config(data_dir: &Path, existing: Option<&str>) -> Option<String> {
    let path = data_dir.join(OPENCODE_PLUGIN_FILE);
    if std::fs::read_to_string(&path).ok().as_deref() != Some(OPENCODE_PLUGIN) {
        std::fs::create_dir_all(data_dir).ok()?;
        std::fs::write(&path, OPENCODE_PLUGIN).ok()?;
    }
    let uri = json!(format!("file://{}", path.display()));
    let mut config = existing
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    let mut plugins = config["plugin"].as_array().cloned().unwrap_or_default();
    if !plugins.contains(&uri) {
        plugins.push(uri);
    }
    config["plugin"] = Value::Array(plugins);
    Some(config.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hooks_only_the_first_word_of_each_command() {
        assert_eq!(
            with_hooks("claude --continue || claude"),
            "claude --settings \"$REPODECK_AGENT_HOOKS\" --continue || claude --settings \"$REPODECK_AGENT_HOOKS\""
        );
        assert_eq!(with_hooks("echo claude"), "echo claude");
        assert_eq!(
            with_hooks("/opt/bin/claude --model opus"),
            "/opt/bin/claude --settings \"$REPODECK_AGENT_HOOKS\" --model opus"
        );
        assert_eq!(with_hooks("claude-foo"), "claude-foo");
        assert_eq!(
            with_hooks("codex resume --last || codex"),
            "repodeck run-codex codex resume --last || repodeck run-codex codex"
        );
        assert_eq!(with_hooks("opencode -m gpt"), "opencode -m gpt");
        assert!(uses("claude || opencode", Agent::OpenCode));
        assert!(!uses("echo opencode", Agent::OpenCode));
    }

    #[test]
    fn codex_flags_register_each_hook_as_toml() {
        let flags = codex_hook_flags();
        assert_eq!(flags.len(), CODEX_HOOKS.len() * 2);
        assert!(flags.iter().step_by(2).all(|f| f == "-c"));
        let stop = flags.iter().find(|f| f.starts_with("hooks.Stop=")).unwrap();
        assert_eq!(
            stop,
            "hooks.Stop=[{hooks=[{type=\"command\",command=\"(cat; echo) >> \\\"$REPODECK_AGENT_EVENTS\\\"\"}]}]"
        );
        let input = flags.iter().find(|f| f.starts_with("hooks.PreToolUse=")).unwrap();
        assert!(input.contains("matcher=\"^request_user_input$\","));
    }

    #[test]
    fn opencode_config_adds_the_plugin_to_existing_config() {
        let dir = std::env::temp_dir().join(format!("repodeck-opencode-{}", uuid::Uuid::new_v4()));
        let config = opencode_config(&dir, Some(r#"{"theme":"x","plugin":["other"]}"#)).unwrap();
        let config: Value = serde_json::from_str(&config).unwrap();
        assert_eq!(config["theme"], "x");
        let plugins = config["plugin"].as_array().unwrap();
        assert_eq!(plugins[0], "other");
        let uri = plugins[1].as_str().unwrap();
        assert!(uri.starts_with("file://") && uri.ends_with(OPENCODE_PLUGIN_FILE), "{uri}");
        assert_eq!(std::fs::read_to_string(dir.join(OPENCODE_PLUGIN_FILE)).unwrap(), OPENCODE_PLUGIN);
        let again = opencode_config(&dir, Some("not json")).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&again).unwrap()["plugin"].as_array().unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }
}
