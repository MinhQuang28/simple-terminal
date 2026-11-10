//! Command-line mode, used by scripts, hooks and agents inside panes. `notify` emits an OSC 9
//! sequence on stdout, which Repodeck (and most other terminals) show as a notification for the
//! pane it runs in; the rest talk to the running app over its socket.

use std::io::Write;
use std::time::{Duration, Instant};

use crate::{agent_launch, codex_launch, socket_api};

pub const USAGE: &str = "usage: repodeck notify <text> | open [folder] | claude [folder] [--task <text>] | \
worktree [folder] [--task <text>] | \
focus [pane] | list | split [--down] [--claude] [--pane <id>] [-- <command or task>] | \
send [--pane <id>] [--no-enter] <text> | read [--pane <id>] [--lines <n>] | \
wait --pane <id> [--timeout <seconds>] | shot [file.png] | --skill";

/// How to drive Repodeck from inside a pane, written for agents.
const GUIDE: &str = include_str!("cli-guide.md");

/// Runs a CLI invocation; false when the arguments aren't one, so the app starts instead.
pub fn run() -> bool {
    let mut args: Vec<String> =
        std::env::args_os().skip(1).map(|a| a.to_string_lossy().into_owned()).collect();
    let Some(verb) = (!args.is_empty()).then(|| args.remove(0)) else { return false };
    if verb == agent_launch::CODEX_VERB {
        codex_launch::run(args.into_iter());
    }
    let command = match verb.as_str() {
        // Inside a Repodeck pane the socket files it against this pane even when stdout is
        // captured (an agent's shell tool); elsewhere OSC 9 reaches whatever terminal shows it.
        "notify" if std::env::var_os(socket_api::SOCKET_VAR).is_some() => {
            format!("notify {} {}", this_pane(), clean(&args.join(" ")))
        }
        "notify" => {
            let mut out = std::io::stdout().lock();
            let _ = write!(out, "\x1b]9;{}\x07", clean(&args.join(" ")));
            let _ = out.flush();
            return true;
        }
        "open" | "claude" => {
            let task = take_value(&mut args, "--task");
            let folder = if args.is_empty() { ".".to_string() } else { args.join(" ") };
            let path = std::fs::canonicalize(&folder).unwrap_or_else(|_| folder.into());
            match task.filter(|_| verb == "claude") {
                Some(task) => format!("claude\t{}\t{}\t{}", path.display(), clean(&task), this_pane()),
                None => format!("{verb} {}", path.display()),
            }
        }
        "worktree" => {
            let task = take_value(&mut args, "--task").unwrap_or_default();
            let folder = if args.is_empty() { ".".to_string() } else { args.join(" ") };
            let path = std::fs::canonicalize(&folder).unwrap_or_else(|_| folder.into());
            format!("worktree\t{}\t{}\t{}", path.display(), clean(&task), this_pane())
        }
        "focus" => format!("focus {}", if args.is_empty() { this_pane() } else { clean(&args.join(" ")) }),
        "list" => "list".to_string(),
        "split" => {
            let run = args.iter().position(|a| a == "--").map(|i| args.split_off(i)[1..].join(" "));
            let pane = take_value(&mut args, "--pane").unwrap_or_else(this_pane);
            let dir = if take_flag(&mut args, "--down") { "down" } else { "right" };
            let kind = if take_flag(&mut args, "--claude") { "claude" } else { "shell" };
            format!("split\t{pane}\t{dir}\t{kind}\t{}\t{}", clean(&run.unwrap_or_default()), this_pane())
        }
        "send" => {
            let pane = take_value(&mut args, "--pane").unwrap_or_else(this_pane);
            let mode = if take_flag(&mut args, "--no-enter") { "raw" } else { "enter" };
            format!("send\t{pane}\t{mode}\t{}", clean(&args.join(" ")))
        }
        "read" => {
            let pane = take_value(&mut args, "--pane").unwrap_or_else(this_pane);
            let lines = take_value(&mut args, "--lines").unwrap_or_else(|| "50".into());
            format!("read\t{pane}\t{}", clean(&lines))
        }
        "wait" => {
            let pane = take_value(&mut args, "--pane").unwrap_or_default();
            let timeout = take_value(&mut args, "--timeout").and_then(|t| t.parse().ok()).unwrap_or(1800);
            wait(&clean(&pane), Duration::from_secs(timeout));
            return true;
        }
        "shot" => {
            let file = match args.is_empty() {
                true => std::env::temp_dir().join(format!("repodeck-shot-{}.png", std::process::id())),
                false => std::path::absolute(args.join(" ")).unwrap_or_else(|_| args.join(" ").into()),
            };
            format!("shot\t{}", clean(&file.to_string_lossy()))
        }
        "--skill" => {
            print!("{GUIDE}");
            return true;
        }
        "-h" | "--help" | "help" => {
            println!("{USAGE}");
            return true;
        }
        _ => return false,
    };
    reply(&command);
    true
}

fn reply(command: &str) {
    match socket_api::send(command) {
        Ok(reply) if reply.starts_with("error:") => {
            eprintln!("{reply}");
            std::process::exit(1);
        }
        Ok(reply) => {
            if !reply.is_empty() {
                println!("{reply}");
            }
        }
        Err(err) => {
            eprintln!("repodeck: {err}");
            std::process::exit(1);
        }
    }
}

/// Blocks until the agent in `pane` has finished the turn it was asked for (or needs the user),
/// then prints how it stopped. The first look comes late enough for a prompt just sent to count.
fn wait(pane: &str, timeout: Duration) {
    let fail = |message: &str| -> ! {
        eprintln!("{message}");
        std::process::exit(1)
    };
    let started = Instant::now();
    std::thread::sleep(Duration::from_millis(700));
    loop {
        let reply = socket_api::send(&format!("status\t{pane}"))
            .unwrap_or_else(|err| fail(&format!("repodeck: {err}")));
        if reply.starts_with("error:") {
            fail(&reply);
        }
        if let Some(outcome) = turn_outcome(&reply) {
            println!("{outcome}");
            return;
        }
        if started.elapsed() >= timeout {
            fail(&format!("error: no finished turn after {}s", timeout.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(400));
    }
}

/// How a turn ended, from a `status` reply (`label\t<asked>`): `None` while the agent is working
/// or hasn't been asked anything yet. A finished turn the user already looked at reads as idle.
fn turn_outcome(reply: &str) -> Option<&str> {
    match reply.split_once('\t').unwrap_or((reply, "0")) {
        ("Working", _) | ("", "0") => None,
        ("", _) => Some("Done"),
        (label, _) => Some(label),
    }
}

/// The pane this command runs in.
fn this_pane() -> String {
    std::env::var(socket_api::PANE_VAR).unwrap_or_default()
}

/// Control characters (newlines, tabs) become spaces: the socket protocol is one line, its
/// fields split on tabs.
fn clean(text: &str) -> String {
    text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect()
}

/// Removes `--flag` from `args`; whether it was there.
fn take_flag(args: &mut Vec<String>, flag: &str) -> bool {
    let at = args.iter().position(|a| a == flag);
    at.map(|i| args.remove(i)).is_some()
}

/// Removes `--name value` from `args` and returns the value.
fn take_value(args: &mut Vec<String>, name: &str) -> Option<String> {
    let at = args.iter().position(|a| a == name)?;
    args.remove(at);
    (at < args.len()).then(|| args.remove(at))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_are_taken_out_of_the_arguments() {
        let mut args: Vec<String> = ["fix", "--task", "a b", "--down", "x"].map(String::from).to_vec();
        assert_eq!(take_value(&mut args, "--task").as_deref(), Some("a b"));
        assert!(take_flag(&mut args, "--down"));
        assert!(!take_flag(&mut args, "--down"));
        assert_eq!(args, ["fix", "x"]);
        assert_eq!(clean("one\ntwo\tthree"), "one two three");
    }

    #[test]
    fn a_wait_ends_with_the_turn() {
        assert_eq!(turn_outcome("\t0"), None, "not asked anything yet");
        assert_eq!(turn_outcome("Working\t1"), None);
        assert_eq!(turn_outcome("Done\t1"), Some("Done"));
        assert_eq!(turn_outcome("\t1"), Some("Done"), "seen by the user already");
        assert_eq!(turn_outcome("Approve?\t1"), Some("Approve?"));
    }
}
