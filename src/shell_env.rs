use std::collections::HashMap;
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::{OnceLock, mpsc};
use std::thread;
use std::time::{Duration, Instant};

/// Gives up on startup files that wait for input or hang.
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(10);
const MARKER: &str = "__REPODECK_ENV__";
/// Per-process or per-terminal variables that must not leak into new panes.
const SKIP: &[&str] = &["PWD", "OLDPWD", "SHLVL", "_", "TERM", "TERM_PROGRAM", "COLORTERM", "ZDOTDIR"];

/// The environment a login shell ends up with, captured once so agent panes can run their
/// command without sourcing the user's startup files again (oh-my-zsh plus nvm take ~0.35 s).
/// Edits to those files apply to agents after Repodeck restarts.
static CAPTURED: OnceLock<(String, HashMap<String, String>)> = OnceLock::new();

/// The configured shell, or the user's `$SHELL`.
pub fn program(shell: &str) -> String {
    if shell.is_empty() { std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into()) } else { shell.into() }
}

/// Captures `shell`'s login environment in the background.
pub fn prefetch(shell: String) {
    let spawned = thread::Builder::new().name("shell-env".into()).spawn(move || match capture(&shell) {
        Some(env) => {
            let _ = CAPTURED.set((shell, env));
        }
        None => log::warn!("Could not capture the environment of {shell}; agents start through it"),
    });
    if let Err(err) = spawned {
        log::error!("Failed to start the shell environment capture: {err}");
    }
}

/// The captured environment, if it is ready and was taken from `shell`.
pub fn get(shell: &str) -> Option<&'static HashMap<String, String>> {
    CAPTURED.get().filter(|(captured, _)| captured == shell).map(|(_, env)| env)
}

fn capture(shell: &str) -> Option<HashMap<String, String>> {
    let script = format!("printf '%s' {MARKER}; env -0");
    let mut child = Command::new(shell)
        .args(["-l", "-i", "-c", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    // Drained on its own thread so a chatty startup file can't fill the pipe and stall the shell;
    // a daemon started by those files may keep the pipe open, hence the timed receive below.
    let mut stdout = child.stdout.take()?;
    let (tx, output) = mpsc::channel();
    thread::spawn(move || {
        let mut out = Vec::new();
        let _ = tx.send(stdout.read_to_end(&mut out).map(|_| out));
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().ok()? {
            break status;
        }
        if started.elapsed() > CAPTURE_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        thread::sleep(Duration::from_millis(20));
    };
    let out = output.recv_timeout(CAPTURE_TIMEOUT.saturating_sub(started.elapsed())).ok()?.ok()?;
    status.success().then(|| parse(&out)).flatten()
}

/// Parses `env -0` output following the marker; startup files may print before it.
fn parse(stdout: &[u8]) -> Option<HashMap<String, String>> {
    let text = String::from_utf8_lossy(stdout);
    let (_, vars) = text.split_once(MARKER)?;
    let env: HashMap<String, String> = vars
        .split('\0')
        .filter_map(|entry| entry.split_once('='))
        .filter(|(key, _)| !key.is_empty() && !SKIP.contains(key) && !key.starts_with("REPODECK_"))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();
    env.contains_key("PATH").then_some(env)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_env_after_the_marker() {
        let out =
            b"Welcome!\n__REPODECK_ENV__PATH=/opt/bin:/bin\0A=x=y\0SHLVL=2\0REPODECK_PANE=1\0MULTI=a\nb\0";
        let env = parse(out).unwrap();
        assert_eq!(env.get("PATH").map(String::as_str), Some("/opt/bin:/bin"));
        assert_eq!(env.get("A").map(String::as_str), Some("x=y"));
        assert_eq!(env.get("MULTI").map(String::as_str), Some("a\nb"));
        assert!(!env.contains_key("SHLVL") && !env.contains_key("REPODECK_PANE"));
        assert!(parse(b"no marker").is_none());
    }

    #[test]
    fn captures_a_login_environment() {
        let env = capture("/bin/sh").expect("sh environment");
        assert!(env.contains_key("PATH"));
    }
}
