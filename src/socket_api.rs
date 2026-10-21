use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::thread;

use eframe::egui;
use uuid::Uuid;

use crate::app::{Job, RepodeckApp};
use crate::finder_service::FolderRequest;
use crate::model::PaneKind;
use crate::store;

/// Environment variable naming the socket, so `repodeck` run inside a pane finds its app.
pub const SOCKET_VAR: &str = "REPODECK_SOCKET";
/// Environment variable holding the id of the pane a command runs in.
pub const PANE_VAR: &str = "REPODECK_PANE";

pub fn socket_path() -> Option<PathBuf> {
    Some(store::data_dir()?.join("repodeck.sock"))
}

/// Accepts one-line commands on the app's socket and hands each to the UI thread as a job that
/// applies it and writes the reply back.
pub fn listen(jobs: Sender<Job>, ctx: egui::Context) {
    let Some(path) = socket_path() else { return };
    let _ = std::fs::remove_file(&path);
    let listener = match UnixListener::bind(&path) {
        Ok(listener) => listener,
        Err(err) => {
            log::warn!("Socket API unavailable at {}: {err}", path.display());
            return;
        }
    };
    let spawned = thread::Builder::new().name("socket-api".into()).spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut line = String::new();
            if BufReader::new(&stream).read_line(&mut line).is_err() {
                continue;
            }
            let command = line.trim().to_string();
            let job: Job = Box::new(move |app| {
                let reply = apply(app, &command);
                let mut stream = stream;
                let _ = writeln!(stream, "{reply}");
            });
            if jobs.send(job).is_err() {
                return;
            }
            ctx.request_repaint();
        }
    });
    if let Err(err) = spawned {
        log::error!("Failed to start the socket API: {err}");
    }
}

/// Runs a command line of the form `verb args…`; returns the reply text.
fn apply(app: &mut RepodeckApp, command: &str) -> String {
    let (verb, rest) = command.split_once(' ').unwrap_or((command, ""));
    let rest = rest.trim();
    match verb {
        "open" | "claude" => {
            let path = PathBuf::from(rest);
            if !path.is_dir() {
                return format!("error: not a folder: {rest}");
            }
            let kind = if verb == "claude" { PaneKind::Agent } else { PaneKind::Shell };
            app.open_folder(FolderRequest { path, kind });
            app.ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            "ok".into()
        }
        "focus" => match Uuid::parse_str(rest) {
            Ok(pane) if app.focus_pane(pane) => {
                app.ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                "ok".into()
            }
            Ok(_) => "error: unknown pane".into(),
            Err(_) => "error: focus needs a pane id (see $REPODECK_PANE)".into(),
        },
        "notify" => {
            let (pane, text) = match rest.split_once(' ') {
                Some((id, text)) if Uuid::parse_str(id).is_ok() => (Uuid::parse_str(id).ok(), text.trim()),
                _ => (None, rest),
            };
            app.notify_from_cli(pane, text.to_string());
            "ok".into()
        }
        "list" => list(app),
        "" => "error: empty command".into(),
        other => format!("error: unknown command {other}"),
    }
}

/// One tab-separated line per pane: repo, path, tab title, pane id, kind, agent status.
fn list(app: &RepodeckApp) -> String {
    let mut lines = Vec::new();
    for repo in app.state.projects.iter().flat_map(|p| &p.repos) {
        for tab in &repo.tabs {
            for pane in &tab.panes {
                let kind = match pane.kind {
                    PaneKind::Shell => "shell",
                    PaneKind::Agent => "claude",
                };
                let status = app.pool.agent_status(pane.id).map(|s| s.label()).unwrap_or_default();
                let path = repo.path.display();
                lines.push(format!("{}\t{path}\t{}\t{}\t{kind}\t{status}", repo.name, tab.title, pane.id));
            }
        }
    }
    lines.join("\n")
}

/// Sends `command` to the running app and returns its reply.
pub fn send(command: &str) -> Result<String, String> {
    let path = std::env::var_os(SOCKET_VAR).map(PathBuf::from).or_else(socket_path);
    let path = path.ok_or("no data directory")?;
    let mut stream = UnixStream::connect(&path).map_err(|_| "Repodeck is not running".to_string())?;
    writeln!(stream, "{command}").map_err(|e| e.to_string())?;
    let mut reply = String::new();
    BufReader::new(stream).read_to_string(&mut reply).map_err(|e| e.to_string())?;
    Ok(reply.trim_end().to_string())
}
