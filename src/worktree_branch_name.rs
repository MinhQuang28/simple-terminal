//! Names for worktrees made in one step: a temporary `wt-…` branch first, renamed after the task
//! once Claude gets its first prompt there. Claude (Haiku, no tools, nothing saved) suggests the
//! name; the prompt's own English words are the fallback.

use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::worktree;

const TEMP_PREFIX: &str = "wt-";
const MAX_LEN: usize = 60;
const SUGGEST_TIMEOUT: Duration = Duration::from_secs(45);

/// A fresh temporary branch name such as `wt-3f9a1c`.
pub fn temp_name() -> String {
    format!("{TEMP_PREFIX}{}", &uuid::Uuid::new_v4().simple().to_string()[..6])
}

/// Whether `prompt` describes a task worth naming a branch after: not a slash command, not one word.
pub fn worth_naming(prompt: &str) -> bool {
    let prompt = prompt.trim();
    !prompt.starts_with('/') && prompt.split_whitespace().nth(1).is_some()
}

/// A free branch name for the task `prompt` describes in the repo at `path`, or `None` when
/// neither Claude nor the prompt's words give a usable one.
pub fn for_prompt(path: &Path, prompt: &str, env: Option<&HashMap<String, String>>) -> Option<String> {
    let base = suggest(prompt, env).or_else(|| slug(prompt))?;
    unique(path, &base)
}

/// Asks Claude for a name; `None` when it isn't installed, fails or takes too long.
fn suggest(prompt: &str, env: Option<&HashMap<String, String>>) -> Option<String> {
    let task: String = prompt.chars().take(600).collect();
    let question = format!(
        "Suggest a git branch name for this task. Reply with the name only: English, lowercase \
         kebab-case, a conventional prefix such as feat/, fix/, refactor/, docs/ or chore/, at \
         most five words after it.\n\nTask: {task}"
    );
    let mut command = Command::new("claude");
    command
        .args(["-p", &question, "--model", "haiku", "--no-session-persistence", "--tools", ""])
        .current_dir(std::env::temp_dir())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(path) = env.and_then(|e| e.get("PATH")) {
        command.env("PATH", path);
    }
    let mut child = command.spawn().ok()?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(None) if started.elapsed() < SUGGEST_TIMEOUT => thread::sleep(Duration::from_millis(200)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let out = child.wait_with_output().ok()?;
    sanitize(&String::from_utf8_lossy(&out.stdout))
}

/// The first line of a reply cut down to a branch name: lowercase letters, digits, `-` and at
/// most one `/`.
fn sanitize(reply: &str) -> Option<String> {
    let line = reply.lines().map(str::trim).find(|l| !l.is_empty())?;
    let mut name = String::new();
    for c in line.trim_matches(|c: char| "`'\"*.".contains(c)).chars().flat_map(char::to_lowercase) {
        let c = match c {
            'a'..='z' | '0'..='9' => c,
            '/' if !name.contains('/') => '/',
            ' ' | '_' | '-' | '.' => '-',
            _ => continue,
        };
        let joins = matches!(c, '-' | '/');
        if !(joins && (name.is_empty() || name.ends_with(['-', '/']))) {
            name.push(c);
        }
    }
    name.truncate(MAX_LEN);
    let name = name.trim_end_matches(['-', '/']).to_string();
    let words = name.rsplit('/').next().unwrap_or_default();
    let bare_prefix = ["feat", "fix", "refactor", "docs", "chore"].contains(&name.as_str());
    (!bare_prefix && words.chars().any(|c| c.is_ascii_alphabetic())).then_some(name)
}

/// The prompt's first few plain-ASCII words, kebab-cased.
fn slug(prompt: &str) -> Option<String> {
    let words: Vec<String> = prompt
        .split_whitespace()
        .filter(|w| w.chars().all(|c| c.is_ascii_alphanumeric()))
        .take(5)
        .map(str::to_ascii_lowercase)
        .collect();
    sanitize(&words.join("-"))
}

/// `base`, or `base-2`… when it's taken.
fn unique(path: &Path, base: &str) -> Option<String> {
    let candidates = std::iter::once(base.to_string()).chain((2..10).map(|n| format!("{base}-{n}")));
    candidates.into_iter().find(|name| worktree::branch_name_free(path, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies_become_branch_names() {
        assert_eq!(sanitize("`feat/login-timeout`\n").as_deref(), Some("feat/login-timeout"));
        assert_eq!(sanitize("\n Fix/Login Timeout.").as_deref(), Some("fix/login-timeout"));
        assert_eq!(sanitize("feat/a/b c").as_deref(), Some("feat/ab-c"));
        assert_eq!(sanitize("feat//--x__y").as_deref(), Some("feat/x-y"));
        assert_eq!(sanitize("feat/"), None);
        assert_eq!(sanitize("123"), None);
        assert_eq!(sanitize(""), None);
    }

    #[test]
    fn prompts_fall_back_to_their_english_words() {
        assert_eq!(slug("Add dark mode to the settings page").as_deref(), Some("add-dark-mode-to-the"));
        assert_eq!(slug("sửa lỗi login timeout").as_deref(), Some("login-timeout"));
        assert_eq!(slug("sửa lỗi"), None);
    }

    #[test]
    fn only_real_tasks_are_named() {
        assert!(worth_naming("fix the login timeout"));
        assert!(!worth_naming("/init"));
        assert!(!worth_naming("  hi  "));
        assert!(temp_name().starts_with(TEMP_PREFIX));
    }
}
