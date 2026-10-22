use std::fs;
use std::path::{Path, PathBuf};

use crate::store;

/// Environment variable naming the file the shell appends command events to.
pub const EVENTS_VAR: &str = "REPODECK_SHELL_EVENTS";

const ZSHENV: &str = r#"# Repodeck shell integration: load the user's config, then add prompt hooks.
REPODECK_ZDOTDIR="$ZDOTDIR"
if [[ -f "$REPODECK_USER_ZDOTDIR/.zshenv" ]]; then
  ZDOTDIR="$REPODECK_USER_ZDOTDIR"
  source "$ZDOTDIR/.zshenv"
  ZDOTDIR="$REPODECK_ZDOTDIR"
fi
"#;

const ZSHRC: &str = r#"if [[ -f "$REPODECK_USER_ZDOTDIR/.zshrc" ]]; then
  ZDOTDIR="$REPODECK_USER_ZDOTDIR"
  source "$ZDOTDIR/.zshrc"
fi
# Later shells started from here read the user's own files again.
ZDOTDIR="$REPODECK_USER_ZDOTDIR"
if [[ -n "$REPODECK_SHELL_EVENTS" ]]; then
  zmodload zsh/datetime 2>/dev/null
  autoload -Uz add-zsh-hook
  __repodeck_preexec() {
    print -r -- "S	$EPOCHREALTIME	${1//$'\n'/ }" >> "$REPODECK_SHELL_EVENTS" 2>/dev/null
  }
  __repodeck_precmd() {
    local code=$?
    print -r -- "E	$EPOCHREALTIME	$code	$PWD" >> "$REPODECK_SHELL_EVENTS" 2>/dev/null
  }
  add-zsh-hook preexec __repodeck_preexec
  add-zsh-hook precmd __repodeck_precmd
fi
"#;

fn user_file_loader(name: &str) -> String {
    format!(
        "if [[ -f \"$REPODECK_USER_ZDOTDIR/{name}\" ]]; then\n  ZDOTDIR=\"$REPODECK_USER_ZDOTDIR\"\n  \
         source \"$ZDOTDIR/{name}\"\n  ZDOTDIR=\"$REPODECK_ZDOTDIR\"\nfi\n"
    )
}

/// Environment that makes zsh load the user's own startup files plus Repodeck's prompt hooks.
/// Other shells get only the events variable and no hooks.
pub fn env(shell_program: &str, events: &Path) -> Vec<(String, String)> {
    let mut env = vec![(EVENTS_VAR.to_string(), events.to_string_lossy().into_owned())];
    let is_zsh = Path::new(shell_program).file_name().is_some_and(|n| n == "zsh");
    if !is_zsh {
        return env;
    }
    let Some(dir) = zsh_dir() else { return env };
    let user_zdotdir = std::env::var_os("ZDOTDIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .unwrap_or_default();
    env.push(("ZDOTDIR".into(), dir.to_string_lossy().into_owned()));
    env.push(("REPODECK_USER_ZDOTDIR".into(), user_zdotdir.to_string_lossy().into_owned()));
    env
}

/// Writes (or refreshes) the wrapper startup files and returns their directory.
fn zsh_dir() -> Option<PathBuf> {
    let dir = store::data_dir()?.join("shell-integration").join("zsh");
    fs::create_dir_all(&dir).ok()?;
    let files = [
        (".zshenv", ZSHENV.to_string()),
        (".zprofile", user_file_loader(".zprofile")),
        (".zshrc", ZSHRC.to_string()),
        (".zlogin", user_file_loader(".zlogin")),
        (".zlogout", user_file_loader(".zlogout")),
    ];
    for (name, content) in files {
        let path = dir.join(name);
        if fs::read_to_string(&path).ok().as_deref() != Some(content.as_str()) {
            fs::write(&path, content).ok()?;
        }
    }
    Some(dir)
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::process::{Command, Stdio};

    use super::*;

    #[test]
    fn zsh_loads_user_config_and_reports_prompts() {
        let root = std::env::temp_dir().join(format!("repodeck-zsh-{}", uuid::Uuid::new_v4()));
        let home = root.join("home");
        fs::create_dir_all(&home).unwrap();
        fs::write(home.join(".zshrc"), "export REPODECK_TEST_RC=loaded\n").unwrap();
        // SAFETY: the only test reading this variable; it is set before the data dir is used.
        unsafe { std::env::set_var("REPODECK_DATA_DIR", &root) };
        let events = root.join("events.log");
        fs::write(&events, "").unwrap();

        let mut child = Command::new("/bin/zsh")
            .arg("-i")
            .env_clear()
            .env("HOME", &home)
            .env("PATH", "/usr/bin:/bin")
            .env("TERM", "dumb")
            .envs(env("/bin/zsh", &events))
            .env("REPODECK_USER_ZDOTDIR", &home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(b"echo rc=$REPODECK_TEST_RC\nfalse\nexit\n").unwrap();
        let output = child.wait_with_output().unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("rc=loaded"), "{stdout:?}");

        let log = fs::read_to_string(&events).unwrap();
        assert!(log.lines().any(|l| l.starts_with("S\t") && l.ends_with("\tfalse")), "{log}");
        assert!(log.lines().any(|l| l.starts_with("E\t") && l.contains("\t1\t")), "{log}");
        // Tests running alongside may still be writing event logs under the data dir this test
        // pointed at itself, so a leftover file is not a failure.
        let _ = fs::remove_dir_all(root);
    }
}
