//! Command-line mode, used by scripts, hooks and agents inside panes. `notify` emits an OSC 9
//! sequence on stdout, which Repodeck (and most other terminals) show as a notification for the
//! pane it runs in; the rest talk to the running app over its socket.

use std::io::Write;

use crate::{agent_launch, codex_launch, socket_api};

pub const USAGE: &str = "usage: repodeck notify <text> | open [folder] | claude [folder] [--task <text>] | \
worktree [folder] [--task <text>] | \
focus [pane] | list | split [--down] [--claude] [--pane <id>] [-- <command or task>] | \
send [--pane <id>] [--no-enter] <text> | read [--pane <id>] [--lines <n>] | --skill";

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
                Some(task) => format!("claude\t{}\t{}", path.display(), clean(&task)),
                None => format!("{verb} {}", path.display()),
            }
        }
        "worktree" => {
            let task = take_value(&mut args, "--task").unwrap_or_default();
            let folder = if args.is_empty() { ".".to_string() } else { args.join(" ") };
            let path = std::fs::canonicalize(&folder).unwrap_or_else(|_| folder.into());
            format!("worktree\t{}\t{}", path.display(), clean(&task))
        }
        "focus" => format!("focus {}", if args.is_empty() { this_pane() } else { clean(&args.join(" ")) }),
        "list" => "list".to_string(),
        "split" => {
            let run = args.iter().position(|a| a == "--").map(|i| args.split_off(i)[1..].join(" "));
            let pane = take_value(&mut args, "--pane").unwrap_or_else(this_pane);
            let dir = if take_flag(&mut args, "--down") { "down" } else { "right" };
            let kind = if take_flag(&mut args, "--claude") { "claude" } else { "shell" };
            format!("split\t{pane}\t{dir}\t{kind}\t{}", clean(&run.unwrap_or_default()))
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
}
