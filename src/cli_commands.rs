//! The CLI verbs for driving the app beyond panes: queries (`projects`, `tabs`, `status`,
//! `sessions`, `inbox`), app commands by name (`do`, `commands`), tabs, worktrees, agents, the
//! Code view and projects. Each becomes one tab-separated socket command.

use std::path::Path;

use crate::cli::{clean, take_flag, take_value, this_pane};

/// What a verb does: a command for the app, or text printed without asking it.
pub enum Request {
    Send(String),
    Print(String),
}

/// The request for `verb` with `args`; `None` when it isn't one of these verbs.
pub fn build(verb: &str, args: &mut Vec<String>) -> Option<Result<Request, String>> {
    let send = |command: String| Some(Ok(Request::Send(command)));
    match verb {
        "commands" => {
            let names: Vec<&str> = crate::app_commands::names().collect();
            Some(Ok(Request::Print(names.join("\n"))))
        }
        "do" => {
            let pane = take_value(args, "--pane").unwrap_or_default();
            let Some(name) = args.first() else {
                return Some(Err("usage: repodeck do <command> [--pane <id>]".into()));
            };
            send(format!("do\t{}\t{}", clean(name), clean(&pane)))
        }
        "status" => {
            let pane = take_value(args, "--pane").unwrap_or_else(this_pane);
            send(format!("status.json\t{}", clean(&pane)))
        }
        "projects" => send(format!("projects\t{}", json(args))),
        "tabs" => {
            let json = json(args);
            let repo = take_value(args, "--repo").map(|r| absolute(&r)).unwrap_or_default();
            send(format!("tabs\t{repo}\t{json}"))
        }
        "sessions" => {
            let json = json(args);
            send(format!("sessions\t{}\t{json}", folder_arg(args)))
        }
        "resume" => {
            let session = take_value(args, "--session").unwrap_or_default();
            send(format!("resume\t{}\t{}", folder_arg(args), clean(&session)))
        }
        "inbox" => match args.first().map(String::as_str) {
            Some(verb @ ("read" | "clear")) => send(format!("inbox.mark\t{verb}")),
            _ => send(format!("inbox\t{}", json(args))),
        },
        "interrupt" | "approve" => {
            let deny = take_flag(args, "--deny");
            let Some(pane) = take_value(args, "--pane") else {
                return Some(Err(format!(
                    "usage: repodeck {verb} --pane <id>{}",
                    if verb == "approve" { " [--deny]" } else { "" }
                )));
            };
            match verb {
                "interrupt" => send(format!("interrupt\t{}", clean(&pane))),
                _ => send(format!(
                    "approve\t{}\t{}\t{}",
                    clean(&pane),
                    if deny { "deny" } else { "allow" },
                    this_pane()
                )),
            }
        }
        "tab" => tab(args),
        "pane" => {
            let force = if take_flag(args, "--force") { "force" } else { "" };
            match (args.first().map(String::as_str), args.get(1)) {
                (Some("close"), Some(pane)) => send(format!("pane.close\t{}\t{force}", clean(pane))),
                _ => Some(Err("usage: repodeck pane close <id> [--force]".into())),
            }
        }
        "code" => code(args),
        "repo" => {
            let project = take_value(args, "--project").unwrap_or_default();
            match args.first().map(String::as_str) {
                Some("add") if args.len() > 1 => {
                    send(format!("repo.add\t{}\t{}", absolute(&args[1..].join(" ")), clean(&project)))
                }
                _ => Some(Err("usage: repodeck repo add <folder> [--project <name>]".into())),
            }
        }
        "project" => {
            let folder = take_value(args, "--folder").map(|f| absolute(&f)).unwrap_or_default();
            match args.first().map(String::as_str) {
                Some("new") if args.len() > 1 => {
                    send(format!("project.new\t{}\t{folder}", clean(&args[1..].join(" "))))
                }
                _ => Some(Err("usage: repodeck project new <name> [--folder <path>]".into())),
            }
        }
        "worktree" => worktree(args),
        _ => None,
    }
}

fn tab(args: &mut Vec<String>) -> Option<Result<Request, String>> {
    let force = take_flag(args, "--force");
    let to = take_value(args, "--repo");
    let usage = "usage: repodeck tab select|close [--force]|rename <name>|move --repo <folder> <tab id>";
    let (Some(sub), Some(id)) = (args.first().cloned(), args.get(1).map(|i| clean(i))) else {
        return Some(Err(usage.into()));
    };
    let command = match sub.as_str() {
        "select" => format!("tab.select\t{id}"),
        "close" => format!("tab.close\t{id}\t{}", if force { "force" } else { "" }),
        "rename" => format!("tab.rename\t{id}\t{}", clean(&args[2..].join(" "))),
        "move" => match to {
            Some(to) => format!("tab.move\t{id}\t{}", absolute(&to)),
            None => return Some(Err(usage.into())),
        },
        _ => return Some(Err(usage.into())),
    };
    Some(Ok(Request::Send(command)))
}

fn code(args: &[String]) -> Option<Result<Request, String>> {
    let sub = args.first().cloned().unwrap_or_default();
    let rest = args.get(1..).map(|r| r.join(" ")).unwrap_or_default();
    let command = match sub.as_str() {
        "open" if !rest.is_empty() => {
            let (file, line, col) = location(&rest);
            format!("code.open\t{}\t{line}\t{col}", absolute(file))
        }
        "diff" => format!("code.diff\t{}", absolute(if rest.is_empty() { "." } else { &rest })),
        _ => return Some(Err("usage: repodeck code open <file>[:line[:col]] | code diff [path]".into())),
    };
    Some(Ok(Request::Send(command)))
}

fn worktree(args: &mut Vec<String>) -> Option<Result<Request, String>> {
    let sub = args.first().cloned().unwrap_or_default();
    let command = match sub.as_str() {
        "list" => {
            args.remove(0);
            let json = json(args);
            format!("worktree.list\t{}\t{json}", folder_arg(args))
        }
        "new" => {
            args.remove(0);
            let branch = take_value(args, "--branch").unwrap_or_default();
            let task = take_value(args, "--task").unwrap_or_default();
            let kind = if take_flag(args, "--shell") { "shell" } else { "claude" };
            let folder = folder_arg(args);
            format!("worktree.new\t{folder}\t{}\t{}\t{kind}\t{}", clean(&branch), clean(&task), this_pane())
        }
        "open" | "done" => {
            args.remove(0);
            format!("worktree.{sub}\t{}", folder_arg(args))
        }
        "remove" => {
            args.remove(0);
            let delete = if take_flag(args, "--delete-branch") { "delete-branch" } else { "" };
            let force = if take_flag(args, "--force") { "force" } else { "" };
            format!("worktree.remove\t{}\t{delete}\t{force}", folder_arg(args))
        }
        // `repodeck worktree [folder] [--task <text>]`, handled by the older command.
        _ => return None,
    };
    Some(Ok(Request::Send(command)))
}

/// `json` when `--json` was given, which it takes out of `args`.
fn json(args: &mut Vec<String>) -> &'static str {
    if take_flag(args, "--json") { "json" } else { "" }
}

/// The folder the remaining arguments name, the current one when they name none.
fn folder_arg(args: &[String]) -> String {
    absolute(&if args.is_empty() { ".".to_string() } else { args.join(" ") })
}

/// `path` made absolute (symlinks resolved when it exists), cleaned for the socket.
fn absolute(path: &str) -> String {
    let resolved = std::fs::canonicalize(path).or_else(|_| std::path::absolute(path));
    clean(&resolved.unwrap_or_else(|_| Path::new(path).to_path_buf()).to_string_lossy())
}

/// `file`, `file:line` or `file:line:col`, split into its parts (empty when absent).
fn location(text: &str) -> (&str, &str, &str) {
    let numeric = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let mut parts = text.rsplitn(3, ':');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(c), Some(l), Some(f)) if numeric(c) && numeric(l) => (f, l, c),
        _ => match text.rsplit_once(':') {
            Some((f, l)) if numeric(l) => (f, l, ""),
            _ => (text, "", ""),
        },
    }
}

#[cfg(test)]
#[path = "cli_commands_tests.rs"]
mod tests;
