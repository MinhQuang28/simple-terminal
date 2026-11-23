//! The models Claude Code offers, as it lists them in answer to an `initialize` request, and the
//! `claude` command that is asked.

use std::process::Command;

use serde_json::Value;

use crate::shell_env;

const API_KEYS: &[&str] = &["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN"];

/// A model a Claude Code session can be switched to.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelOption {
    /// What `/model` takes: an alias such as "opus", or a model id.
    pub value: String,
    /// The name Claude Code shows for it: "Opus 5.5".
    pub name: String,
}

/// The models in an `initialize` answer; empty when it lists none.
pub fn parse(response: &Value) -> Vec<ModelOption> {
    let model = |entry: &Value| {
        let text = |key: &str| entry[key].as_str().map(str::to_string);
        Some(ModelOption { value: text("value")?, name: text("displayName")? })
    };
    response["models"].as_array().into_iter().flatten().filter_map(model).collect()
}

/// `claude` with `args`, run with the environment of `shell`'s login session.
pub fn claude_command<'a>(shell: &str, hide_api_keys: bool, args: impl Iterator<Item = &'a str>) -> Command {
    // The login environment has the PATH that finds `claude`; until it is captured, a login
    // shell finds it.
    let mut command = match shell_env::get(shell) {
        Some(env) => {
            let mut command = Command::new("claude");
            command.args(args).env_clear().envs(env);
            command
        }
        None => {
            let mut command = Command::new(shell);
            let line = args.fold("exec claude".to_string(), |line, arg| line + " " + arg);
            command.args(["-l", "-c", &line]);
            command
        }
    };
    for key in API_KEYS.iter().filter(|_| hide_api_keys) {
        command.env_remove(key);
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_models_claude_lists() {
        let answer = serde_json::json!({"models": [
            {"value": "opus", "displayName": "Opus 5.5", "resolvedModel": "claude-opus-5-5"},
            {"value": "broken"},
            {"value": "claude-haiku-4-5", "displayName": "Haiku 4.5"},
        ]});
        let names: Vec<_> = parse(&answer).into_iter().map(|m| (m.value, m.name)).collect();
        let expected = [("opus".into(), "Opus 5.5".into()), ("claude-haiku-4-5".into(), "Haiku 4.5".into())];
        assert_eq!(names, expected);
        assert!(parse(&serde_json::json!({"rate_limits": {}})).is_empty());
    }
}
