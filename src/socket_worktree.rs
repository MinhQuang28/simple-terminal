//! Socket commands for worktrees: make one, show one (adding it to the sidebar when needed),
//! list them, remove one, and remove a finished one with its branch. Those that run git before
//! they can answer do it off the UI thread.

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use uuid::Uuid;

use crate::app::{Job, RepodeckApp};
use crate::model::PaneKind;
use crate::socket_control::{apply, folder};
use crate::socket_queries::reply;
use crate::ui::Action;
use crate::worktree::{self, BranchFate};

/// A worktree of the repo at `path` on `branch` (or a name made from `task`), opening Claude
/// there (on `task`, if given) or a shell. Made in the background.
pub fn worktree_new(
    app: &mut RepodeckApp,
    path: &str,
    branch: &str,
    task: &str,
    shell: bool,
    from: &str,
) -> Result<String, String> {
    let path = folder(path)?;
    let repo = app.repo_containing(&path).unwrap_or_else(|| app.add_repo_to(None, &path));
    let name = app.state.repo(repo).map(|r| r.name.clone()).unwrap_or_default();
    let kind = if shell && task.is_empty() { PaneKind::Shell } else { PaneKind::Agent };
    let branch = Some(branch).filter(|b| !b.is_empty());
    let task = Some(task.to_string()).filter(|t| !t.is_empty());
    app.create_worktree(repo, branch, task, Uuid::parse_str(from).ok(), kind);
    Ok(format!("creating a worktree of {name}; its tab appears when it's ready"))
}

/// Shows the checkout at `path`, adding it to the sidebar (under its main checkout) if needed.
pub fn worktree_open(app: &mut RepodeckApp, path: &str) -> Result<String, String> {
    let path = folder(path)?;
    // The innermost checkout holding it: a worktree can sit inside the main checkout.
    let checkouts = worktree::list(&path).into_iter().map(|c| c.path);
    let inner = checkouts.filter(|p| path.starts_with(p)).max_by_key(|p| p.components().count());
    let path = inner.unwrap_or(path);
    let repos = || app.state.projects.iter().flat_map(|p| &p.repos);
    let main = worktree::main_repo(&path);
    let from = repos()
        .find(|r| r.path == path)
        .or_else(|| repos().find(|r| Some(&r.path) == main.as_ref()))
        .map(|r| r.id);
    let from = from.unwrap_or_else(|| app.add_repo_to(None, &path));
    app.open_worktree(from, &path);
    // After selecting it: the view is the selected repo's.
    apply(app, [Action::ShowCode(false)]);
    let repo = app.state.selected_repo.ok_or("worktree not opened")?;
    if app.state.repo(repo).is_some_and(|r| r.tabs.is_empty()) {
        app.new_tab(repo, PaneKind::Shell, None);
    }
    Ok(app.state.repo(repo).map(|r| r.name.clone()).unwrap_or_default())
}

/// Whether `command` is one of the worktree commands that run git before they can answer.
pub fn runs_git(command: &str) -> bool {
    ["worktree.list\t", "worktree.remove\t", "worktree.done\t"].iter().any(|verb| command.starts_with(verb))
}

/// Answers a [`runs_git`] command from a background thread, so git doesn't hold up a frame; a
/// removal that passed its checks then goes ahead in the app.
pub fn apply_in_background(app: &RepodeckApp, command: &str, mut stream: UnixStream) {
    let command = command.to_string();
    let open: Vec<PathBuf> =
        app.state.projects.iter().flat_map(|p| &p.repos).map(|r| r.path.clone()).collect();
    app.spawn_job("socket-worktree", move || {
        let fields: Vec<&str> = command.split('\t').collect();
        let mut removal = None;
        let reply = match fields.as_slice() {
            ["worktree.list", path, json] => worktrees(&open, path, *json == "json"),
            ["worktree.remove", path, branch, force] => {
                let fate = match (*branch == "delete-branch", *force == "force") {
                    (false, _) => BranchFate::Keep,
                    (true, false) => BranchFate::DeleteIfMerged,
                    // Forced, the branch goes even with commits nothing else holds.
                    (true, true) => BranchFate::Delete,
                };
                removable(Path::new(path), *force == "force", false)
                    .map(|()| removal = Some((PathBuf::from(path), fate, *force == "force")))
                    .map(|()| "removing the worktree".to_string())
            }
            ["worktree.done", path] => removable(Path::new(path), false, true)
                .map(|()| removal = Some((PathBuf::from(path), BranchFate::DeleteIfMerged, false)))
                .map(|()| "removing the worktree".to_string()),
            _ => Err(format!("malformed command {:?}", fields.first().unwrap_or(&""))),
        };
        let _ = writeln!(stream, "{}", reply.unwrap_or_else(|err| format!("error: {err}")));
        Box::new(move |app: &mut RepodeckApp| {
            if let Some((path, fate, force)) = removal {
                remove(app, path, fate, force);
            }
        }) as Job
    });
}

/// The checkouts of the repository at `path`, whether each is open in Repodeck and merged.
fn worktrees(open: &[PathBuf], path: &str, json: bool) -> Result<String, String> {
    let checkouts = worktree::list(Path::new(path));
    if checkouts.is_empty() {
        return Err(format!("not a git repository: {path}"));
    }
    let mut lines = Vec::new();
    let mut out = Vec::new();
    for c in &checkouts {
        let branch = c.branch.clone().unwrap_or_else(|| "(detached)".into());
        let (open, merged) = (open.contains(&c.path), !c.main && worktree::is_merged(&c.path));
        let kind = if c.main { "main" } else { "worktree" };
        let flags: Vec<&str> =
            [open.then_some("open"), merged.then_some("merged")].into_iter().flatten().collect();
        lines.push(format!("{}\t{branch}\t{kind}\t{}", c.path.display(), flags.join(",")));
        out.push(
            json!({ "path": c.path, "branch": c.branch, "main": c.main, "open": open, "merged": merged }),
        );
    }
    reply(json, Value::Array(out), lines)
}

/// Whether the linked worktree at `path` may be removed: nothing uncommitted unless `force`,
/// and with `contained` (for `done`) no commits the default branch lacks. Checked here so the
/// caller hears it; git would refuse later, out of its sight.
fn removable(path: &Path, force: bool, contained: bool) -> Result<(), String> {
    if !worktree::is_worktree(path) {
        return Err(format!("not a linked worktree: {}", path.display()));
    }
    if !force && crate::git_status::read_status(path).is_some_and(|s| !s.files.is_empty()) {
        return Err("the worktree has uncommitted changes (`worktree remove --force` drops them)".into());
    }
    if contained && !worktree::is_contained(path) {
        return Err(
            "its branch isn't merged into the default branch yet (`worktree remove --delete-branch --force` drops it anyway)"
                .into(),
        );
    }
    Ok(())
}

/// Deletes the linked worktree at `path`, closing its tabs when it is open in the sidebar.
fn remove(app: &mut RepodeckApp, path: PathBuf, fate: BranchFate, force: bool) {
    if let Some(repo) =
        app.state.projects.iter().flat_map(|p| &p.repos).find(|r| r.path == path).map(|r| r.id)
    {
        return app.remove_worktree(repo, fate, force);
    }
    let shown = path.display().to_string();
    app.spawn_job("worktree-remove", move || {
        let result = worktree::main_repo(&path)
            .ok_or_else(|| "main checkout not found".to_string())
            .and_then(|main| worktree::remove(&main, &path, fate, force));
        Box::new(move |app: &mut RepodeckApp| {
            if let Err(err) = result {
                app.report_error(&format!("Removing worktree {shown}"), err);
            }
        }) as Job
    });
}
