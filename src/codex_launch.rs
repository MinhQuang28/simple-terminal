//! `repodeck run-codex <codex> [args…]`: starts Codex with Repodeck's session hooks marked
//! trusted, so Codex doesn't stop at its hook review screen. Codex keys trust by a hash only it
//! computes, so the hashes are asked for once per binary through `codex app-server` and cached.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::agent_launch::codex_hook_flags;
use crate::store;

const TIMEOUT: Duration = Duration::from_secs(15);
const CACHE_DIR: &str = "codex-trust";
const INITIALIZE: &str = r#"{"method":"initialize","id":1,"params":{"clientInfo":{"name":"repodeck","title":"Repodeck","version":"1"}}}"#;
const INITIALIZED: &str = r#"{"method":"initialized","params":{}}"#;
const LIST: &str = r#"{"method":"hooks/list","id":2,"params":{}}"#;

/// Replaces this process with Codex; only returns (by exiting) when it can't be started.
pub fn run(mut args: impl Iterator<Item = String>) -> ! {
    let Some(codex) = args.next() else {
        eprintln!("usage: repodeck run-codex <codex> [args…]");
        std::process::exit(2)
    };
    let flags = codex_hook_flags();
    let mut command = Command::new(&codex);
    command.args(&flags);
    match trust_state(&codex, &flags) {
        Some(state) => {
            command.arg("-c").arg(state);
        }
        None => eprintln!("repodeck: could not pre-trust its Codex hooks; Codex may ask you to review them"),
    }
    command.args(args);
    let err = command.exec();
    eprintln!("repodeck: cannot start {codex}: {err}");
    std::process::exit(127)
}

fn trust_state(codex: &str, flags: &[String]) -> Option<String> {
    let cache = cache_path(codex, flags);
    if let Some(path) = &cache
        && let Ok(state) = std::fs::read_to_string(path)
        && !state.is_empty()
    {
        return Some(state);
    }
    let state = query(codex, flags)?;
    if let Some(path) = cache
        && let Some(dir) = path.parent()
        && std::fs::create_dir_all(dir).is_ok()
    {
        let _ = std::fs::write(path, &state);
    }
    Some(state)
}

/// A cache file per Codex binary (path, size, modification time) and hook set.
fn cache_path(codex: &str, flags: &[String]) -> Option<PathBuf> {
    let exe = resolve(codex)?;
    let meta = std::fs::metadata(&exe).ok()?;
    let mut hasher = DefaultHasher::new();
    exe.hash(&mut hasher);
    meta.len().hash(&mut hasher);
    meta.modified().ok()?.hash(&mut hasher);
    flags.hash(&mut hasher);
    Some(store::data_dir()?.join(CACHE_DIR).join(format!("{:016x}", hasher.finish())))
}

fn resolve(program: &str) -> Option<PathBuf> {
    if program.contains('/') {
        return Some(PathBuf::from(program));
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|dir| dir.join(program)).find(|candidate| candidate.is_file())
}

/// Asks Codex which hooks the flags register and how it hashes them.
fn query(codex: &str, flags: &[String]) -> Option<String> {
    let mut child = Command::new(codex)
        .args(flags)
        .arg("app-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdin = child.stdin.take()?;
    let reader = BufReader::new(child.stdout.take()?);
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in reader.lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let mut state = None;
    if [INITIALIZE, INITIALIZED, LIST].iter().all(|line| writeln!(stdin, "{line}").is_ok()) {
        let deadline = Instant::now() + TIMEOUT;
        while let Ok(line) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            let Ok(message) = serde_json::from_str::<Value>(&line) else { continue };
            if message["id"] == 2 {
                state = trusted_entries(&message["result"]["data"]);
                break;
            }
        }
    }
    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
    state
}

/// `hooks.state={…}` naming every hook passed on the command line with the hash Codex computed.
fn trusted_entries(data: &Value) -> Option<String> {
    let hooks =
        data.as_array()?.iter().flat_map(|entry| entry["hooks"].as_array().cloned().unwrap_or_default());
    let entries: Vec<String> = hooks
        .filter(|hook| hook["source"] == "sessionFlags")
        .filter_map(|hook| {
            let key = hook["key"].as_str()?.replace('\\', "\\\\").replace('"', "\\\"");
            let hash = hook["currentHash"].as_str()?;
            hash.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == ':')
                .then(|| format!("\"{key}\"={{trusted_hash=\"{hash}\"}}"))
        })
        .collect();
    (!entries.is_empty()).then(|| format!("hooks.state={{{}}}", entries.join(",")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trusts_only_the_hooks_passed_on_the_command_line() {
        let data: Value = serde_json::from_str(
            r#"[{"hooks":[
                {"key":"/Users/me/.codex/hooks.json:stop:0:0","source":"user","currentHash":"sha256:aa"},
                {"key":"/<session-flags>/config.toml:stop:0:0","source":"sessionFlags","currentHash":"sha256:bb"},
                {"key":"/<session-flags>/config.toml:interrupt:0:0","source":"sessionFlags","currentHash":"sha256:cc"},
                {"key":"/<session-flags>/config.toml:bad:0:0","source":"sessionFlags","currentHash":"sha256:\"x"}
            ]}]"#,
        )
        .unwrap();
        assert_eq!(
            trusted_entries(&data).unwrap(),
            "hooks.state={\"/<session-flags>/config.toml:stop:0:0\"={trusted_hash=\"sha256:bb\"},\
             \"/<session-flags>/config.toml:interrupt:0:0\"={trusted_hash=\"sha256:cc\"}}"
        );
        assert_eq!(trusted_entries(&Value::Array(vec![])), None);
    }
}
