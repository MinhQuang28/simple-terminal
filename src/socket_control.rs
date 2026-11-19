//! Socket commands that change what Repodeck shows or runs: app commands by name, tabs, panes,
//! worktrees, agents (interrupt, approve, resume), the Code view, projects and the inbox. They
//! act like the matching click or key, through the same actions.

use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::app::RepodeckApp;
use crate::app_shortcuts::CODE_TAKES;
use crate::model::{Project, SidePanel};
use crate::socket_queries::{find_tab, repo_at, tab_of_pane};
use crate::ui::Action;
use crate::ui::add_project_dialog::ProjectSource;

pub(crate) fn apply(app: &mut RepodeckApp, actions: impl IntoIterator<Item = Action>) {
    let ctx = app.ctx.clone();
    for action in actions {
        app.apply(action, &ctx);
    }
    ctx.request_repaint();
}

pub(crate) fn folder(path: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(path);
    if path.is_dir() { Ok(path) } else { Err(format!("not a folder: {}", path.display())) }
}

/// Runs app command `name` as its key would, after showing `pane` when one is given.
pub fn run_command(app: &mut RepodeckApp, name: &str, pane: Option<Uuid>) -> Result<String, String> {
    if !crate::app_commands::names().any(|n| n == name) {
        return Err(format!("unknown command {name:?} (see `repodeck commands`)"));
    }
    if let Some(pane) = pane {
        tab_of_pane(app, pane).ok_or("unknown pane")?;
        apply(app, [Action::GoToPane(pane)]);
    }
    // The guards a key press has: nothing acts beneath a dialog or peek, and pane commands have
    // no panes in the Code view.
    if app.dialog.is_some() || app.peek.is_some() {
        return Err("a dialog is open in the app; close it first".into());
    }
    let pane_command = name.starts_with("pane.") && CODE_TAKES.iter().any(|taken| name.starts_with(taken));
    if pane_command && app.code_view() {
        return Err(format!(
            "{name} has no panes to act on in the Code view (`repodeck do view.terminals` first)"
        ));
    }
    let actions = app.command_actions(name).unwrap_or_default();
    apply(app, actions);
    Ok("ok".into())
}

pub fn tab_select(app: &mut RepodeckApp, id: &str) -> Result<String, String> {
    let (repo, tab) = find_tab(app, id).map(|(r, t)| (r, t.id))?;
    let document = app
        .state
        .repo(repo)
        .and_then(|r| r.tabs.iter().find(|t| t.id == tab))
        .is_some_and(|t| t.is_document());
    apply(app, [Action::SelectTab { repo, tab }, Action::ShowCode(document)]);
    Ok("ok".into())
}

pub fn tab_close(app: &mut RepodeckApp, id: &str, force: bool) -> Result<String, String> {
    let (repo, tab) = find_tab(app, id).map(|(r, t)| (r, t.id))?;
    apply(
        app,
        [if force { Action::DiscardAndCloseTab { repo, tab } } else { Action::CloseTab { repo, tab } }],
    );
    Ok("ok".into())
}

/// Names the tab; an empty name hands the title back to the program running in it.
pub fn tab_rename(app: &mut RepodeckApp, id: &str, name: &str) -> Result<String, String> {
    let (repo, tab) = find_tab(app, id).map(|(r, t)| (r, t.id))?;
    let tab = app
        .state
        .repo_mut(repo)
        .and_then(|r| r.tabs.iter_mut().find(|t| t.id == tab))
        .ok_or("unknown tab")?;
    tab.name = Some(name.trim().to_string()).filter(|n| !n.is_empty());
    app.dirty = true;
    app.ctx.request_repaint();
    Ok("ok".into())
}

/// Moves the tab to the repo or worktree at `path`, where it then shows.
pub fn tab_move(app: &mut RepodeckApp, id: &str, path: &str) -> Result<String, String> {
    let (from, tab) = find_tab(app, id).map(|(r, t)| (r, t.id))?;
    let to = repo_at(app, path)?;
    if to != from {
        apply(app, [Action::MoveTabToRepo { from, tab, to }]);
    }
    Ok("ok".into())
}

pub fn pane_close(app: &mut RepodeckApp, pane: Uuid) -> Result<String, String> {
    let tab = tab_of_pane(app, pane).ok_or("unknown pane")?;
    apply(app, [Action::ClosePane { tab, pane }]);
    Ok("ok".into())
}

/// Shows a file in the Code view, at `line` and `col` when given.
pub fn code_open(app: &mut RepodeckApp, file: &str, line: &str, col: &str) -> Result<String, String> {
    let path = PathBuf::from(file);
    if !path.is_file() {
        return Err(format!("not a file: {file}"));
    }
    let repo = repo_at(app, file)?;
    let (line, col) = (line.parse().ok(), col.parse().ok());
    apply(
        app,
        [Action::SelectRepo(repo), Action::ShowCode(true), Action::OpenFileAt { repo, path, line, col }],
    );
    Ok("ok".into())
}

/// A file's diff against HEAD in a tab; for a folder, the repo's list of changes.
pub fn code_diff(app: &mut RepodeckApp, path: &str) -> Result<String, String> {
    let repo = repo_at(app, path)?;
    let mut actions = vec![Action::SelectRepo(repo), Action::ShowCode(true)];
    actions.push(match Path::new(path).is_file() {
        true => Action::OpenDiff { repo, path: PathBuf::from(path) },
        false => Action::SetSidePanel(SidePanel::Changes),
    });
    apply(app, actions);
    Ok("ok".into())
}

/// Adds the folder at `path` to project `project` (created when missing), or to the current one.
pub fn repo_add(app: &mut RepodeckApp, path: &str, project: &str) -> Result<String, String> {
    let path = folder(path)?;
    let project = match project {
        "" => None,
        name => Some(project_named(app, name)),
    };
    let repo = app.add_repo_to(project, &path);
    app.dirty = true;
    app.ctx.request_repaint();
    Ok(app.state.repo(repo).map(|r| r.name.clone()).unwrap_or_default())
}

/// A new project, around the folder at `path` when given.
pub fn project_new(app: &mut RepodeckApp, name: &str, path: &str) -> Result<String, String> {
    if path.is_empty() {
        project_named(app, name);
    } else {
        app.add_project(name.to_string(), ProjectSource::Folder(folder(path)?));
    }
    app.dirty = true;
    app.ctx.request_repaint();
    Ok("ok".into())
}

fn project_named(app: &mut RepodeckApp, name: &str) -> Uuid {
    if let Some(project) = app.state.projects.iter().find(|p| p.name == name) {
        return project.id;
    }
    let project = Project::new(name.to_string());
    let id = project.id;
    app.state.projects.push(project);
    id
}

pub fn inbox(app: &mut RepodeckApp, verb: &str) -> Result<String, String> {
    match verb {
        "read" => apply(app, [Action::MarkNoticesRead]),
        "clear" => apply(app, [Action::ClearNotices]),
        other => return Err(format!("unknown inbox command {other:?}")),
    }
    Ok("ok".into())
}
