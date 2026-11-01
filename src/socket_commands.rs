//! Socket commands for driving panes from scripts and agents: split a pane, type into one, read
//! what one shows, start an agent with a task. Fields are tab-separated, since the `repodeck`
//! CLI strips control characters from what it forwards.

use std::path::PathBuf;

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use uuid::Uuid;

use crate::app::RepodeckApp;
use crate::finder_service::FolderRequest;
use crate::model::{PaneKind, SplitDir};
use crate::terminal::content_rows;

/// Most lines `read` returns.
const MAX_READ_LINES: usize = 2000;

/// Runs a tab-separated command; returns the reply text.
pub fn apply(app: &mut RepodeckApp, command: &str) -> String {
    let fields: Vec<&str> = command.split('\t').collect();
    let result = match fields.as_slice() {
        ["claude", path, task] => start_agent(app, path, task),
        ["split", pane, dir, kind, run] => split(app, pane, dir, kind, run),
        ["send", pane, mode, text] => send(app, pane, *mode == "enter", text),
        ["read", pane, lines] => read(app, pane, lines),
        _ => Err(format!("malformed command {:?}", fields.first().unwrap_or(&""))),
    };
    result.unwrap_or_else(|err| format!("error: {err}"))
}

fn pane_id(text: &str) -> Result<Uuid, String> {
    Uuid::parse_str(text)
        .map_err(|_| format!("not a pane id: {text:?} (see $REPODECK_PANE or `repodeck list`)"))
}

/// A new agent tab at `path` whose first prompt is `task`; replies with its pane id.
fn start_agent(app: &mut RepodeckApp, path: &str, task: &str) -> Result<String, String> {
    let path = PathBuf::from(path);
    if !path.is_dir() {
        return Err(format!("not a folder: {}", path.display()));
    }
    // Opened beside the user's work rather than in front of it.
    let request = FolderRequest { path, kind: PaneKind::Agent };
    let pane = app.open_folder_tab(request, false).ok_or("could not open a tab")?;
    if !task.is_empty() {
        app.pool.set_startup(pane, task.to_string());
    }
    start(app, pane)?;
    Ok(pane.to_string())
}

/// Splits `pane` to the right or down with a shell (running `run`, if any) or an agent (given
/// `run` as its first prompt); replies with the new pane's id.
fn split(app: &mut RepodeckApp, pane: &str, dir: &str, kind: &str, run: &str) -> Result<String, String> {
    let pane = pane_id(pane)?;
    let dir = match dir {
        "down" => SplitDir::Down,
        _ => SplitDir::Right,
    };
    let kind = if kind == "claude" { PaneKind::Agent } else { PaneKind::Shell };
    let new = app.split_pane(pane, dir, kind.clone()).ok_or("unknown pane")?;
    // Runs once: what an agent or script sends (a migration, a publish) mustn't rerun on every
    // launch the way a preset the user picked does.
    if !run.is_empty() {
        app.pool.set_startup(new, run.to_string());
    }
    start(app, new)?;
    Ok(new.to_string())
}

/// Starts `pane`'s program now rather than when the pane is first drawn, which for a tab in the
/// background (or a hidden window) could be much later.
fn start(app: &mut RepodeckApp, pane: Uuid) -> Result<(), String> {
    let found = app.state.projects.iter().flat_map(|p| &p.repos).find_map(|r| {
        let p = r.tabs.iter().flat_map(|t| &t.panes).find(|p| p.id == pane)?;
        Some((p.clone(), p.cwd.clone().unwrap_or_else(|| r.path.clone())))
    });
    let (pane, cwd) = found.ok_or("unknown pane")?;
    app.pool.get_or_spawn(&pane, &cwd, &app.state.settings, &app.ctx).map(|_| ())
}

/// Types `text` into `pane` as a paste, then presses Return when `enter`.
fn send(app: &mut RepodeckApp, pane: &str, enter: bool, text: &str) -> Result<String, String> {
    let session = app.pool.get_mut(pane_id(pane)?).ok_or("pane not running (open it once to start it)")?;
    session.paste(text);
    if enter {
        session.send_input(b"\r".to_vec());
    }
    Ok("ok".into())
}

/// The last `lines` lines `pane` shows, history included, without the blank rows below them.
fn read(app: &RepodeckApp, pane: &str, lines: &str) -> Result<String, String> {
    let session = app.pool.get(pane_id(pane)?).ok_or("pane not running (open it once to start it)")?;
    let lines: usize = lines.parse().map_err(|_| format!("not a line count: {lines}"))?;
    let lines = lines.clamp(1, MAX_READ_LINES) as i32;
    let term = session.term.lock();
    let last = content_rows(&term).max(1) as i32 - 1;
    let first = (last - lines + 1).max(-(term.grid().history_size() as i32));
    let end = Point::new(Line(last), Column(term.columns() - 1));
    Ok(term.bounds_to_string(Point::new(Line(first), Column(0)), end).trim_end().to_string())
}
