//! Read-only socket commands: what is open (projects, tabs, panes), an agent's status, a repo's
//! worktrees and Claude sessions, the inbox. Each replies with tab-separated lines, or JSON when
//! asked for, so agents can parse it either way.

use std::path::Path;

use serde_json::{Value, json};
use uuid::Uuid;

use crate::app::RepodeckApp;
use crate::model::{PaneKind, Repo, Tab};

/// The repo at `path`, or the one holding it.
pub(crate) fn repo_at(app: &RepodeckApp, path: &str) -> Result<Uuid, String> {
    let path = Path::new(path);
    let exact = app.state.projects.iter().flat_map(|p| &p.repos).find(|r| r.path == path).map(|r| r.id);
    exact.or_else(|| app.repo_containing(path)).ok_or_else(|| format!("no repo open at {}", path.display()))
}

/// The repo holding tab `id`, and the tab.
pub(crate) fn find_tab<'a>(app: &'a RepodeckApp, id: &str) -> Result<(Uuid, &'a Tab), String> {
    let id = Uuid::parse_str(id).map_err(|_| format!("not a tab id: {id:?} (see `repodeck tabs`)"))?;
    let repos = app.state.projects.iter().flat_map(|p| &p.repos);
    repos
        .flat_map(|r| r.tabs.iter().map(move |t| (r.id, t)))
        .find(|(_, t)| t.id == id)
        .ok_or_else(|| "unknown tab".to_string())
}

/// The tab holding pane `id`.
pub(crate) fn tab_of_pane(app: &RepodeckApp, id: Uuid) -> Option<Uuid> {
    let tabs = app.state.projects.iter().flat_map(|p| &p.repos).flat_map(|r| &r.tabs);
    tabs.into_iter().find(|t| t.panes.iter().any(|p| p.id == id)).map(|t| t.id)
}

pub(crate) fn reply(json: bool, value: Value, lines: Vec<String>) -> Result<String, String> {
    Ok(if json { value.to_string() } else { lines.join("\n") })
}

/// `text` as one field of a tab-separated line: control characters (tabs, newlines) as spaces.
fn field(text: &str) -> String {
    text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect()
}

fn tab_kind(tab: &Tab) -> &'static str {
    match (&tab.file, &tab.diff, tab.kind()) {
        (Some(_), _, _) => "file",
        (_, Some(_), _) => "diff",
        (_, _, PaneKind::Agent) => "claude",
        _ => "shell",
    }
}

fn pane_kind(kind: &PaneKind) -> &'static str {
    if *kind == PaneKind::Agent { "claude" } else { "shell" }
}

fn tab_title(tab: &Tab) -> &str {
    tab.name.as_deref().unwrap_or(&tab.title)
}

/// Projects and their repos; a worktree names the repo it was made from.
pub fn projects(app: &RepodeckApp, json: bool) -> Result<String, String> {
    let name_of = |id: Option<Uuid>| id.and_then(|id| app.state.repo(id)).map(|r| r.name.clone());
    let mut lines = Vec::new();
    let mut out = Vec::new();
    for project in &app.state.projects {
        let mut repos = Vec::new();
        for repo in &project.repos {
            let selected = app.state.selected_repo == Some(repo.id);
            let of = name_of(repo.worktree_of).unwrap_or_default();
            lines.push(format!(
                "{}\t{}\t{}\t{of}\t{}\t{}",
                field(&project.name),
                field(&repo.name),
                repo.path.display(),
                repo.tabs.len(),
                if selected { "selected" } else { "" }
            ));
            repos.push(json!({
                "name": repo.name, "path": repo.path, "worktree_of": name_of(repo.worktree_of),
                "tabs": repo.tabs.len(), "selected": selected, "code_view": repo.code_view,
            }));
        }
        out.push(json!({ "name": project.name, "repos": repos }));
    }
    reply(json, Value::Array(out), lines)
}

/// Tabs of every repo (or of the repo at `path`), with their panes and agent statuses.
pub fn tabs(app: &RepodeckApp, path: &str, json: bool) -> Result<String, String> {
    let only = if path.is_empty() { None } else { Some(repo_at(app, path)?) };
    let repos = app.state.projects.iter().flat_map(|p| &p.repos).filter(|r| only.is_none_or(|id| r.id == id));
    let mut lines = Vec::new();
    let mut out = Vec::new();
    for repo in repos {
        for tab in &repo.tabs {
            let shown = app.state.selected_repo == Some(repo.id) && repo.active_tab == Some(tab.id);
            let panes: Vec<Value> = tab.panes.iter().map(|p| pane_json(app, p.id, &p.kind)).collect();
            let ids: Vec<String> = tab.panes.iter().map(|p| p.id.to_string()).collect();
            lines.push(format!(
                "{}\t{}\t{}\t{}\t{}\t{}",
                field(&repo.name),
                tab.id,
                field(tab_title(tab)),
                tab_kind(tab),
                if shown { "shown" } else { "" },
                ids.join(",")
            ));
            out.push(tab_json(repo, tab, shown, panes));
        }
    }
    reply(json, Value::Array(out), lines)
}

fn tab_json(repo: &Repo, tab: &Tab, shown: bool, panes: Vec<Value>) -> Value {
    json!({
        "id": tab.id, "repo": repo.name, "repo_path": repo.path, "title": tab_title(tab),
        "kind": tab_kind(tab), "shown": shown, "pinned": tab.pinned, "settled": tab.settled,
        "file": tab.file.as_ref().or(tab.diff.as_ref()), "panes": panes,
    })
}

fn pane_json(app: &RepodeckApp, id: Uuid, kind: &PaneKind) -> Value {
    let status = app.pool.agent_status(id).map(|s| s.label()).unwrap_or_default();
    json!({ "id": id, "kind": pane_kind(kind), "status": status, "running": app.pool.get(id).is_some() })
}

/// Every pane: repo, path, tab title, pane id, kind, agent status.
pub fn list(app: &RepodeckApp, json: bool) -> Result<String, String> {
    let mut lines = Vec::new();
    let mut out = Vec::new();
    for repo in app.state.projects.iter().flat_map(|p| &p.repos) {
        for tab in &repo.tabs {
            for pane in &tab.panes {
                let mut row = pane_json(app, pane.id, &pane.kind);
                lines.push(format!(
                    "{}\t{}\t{}\t{}\t{}\t{}",
                    field(&repo.name),
                    repo.path.display(),
                    field(tab_title(tab)),
                    pane.id,
                    pane_kind(&pane.kind),
                    row["status"].as_str().unwrap_or_default()
                ));
                row["repo"] = json!(repo.name);
                row["repo_path"] = json!(repo.path);
                row["tab"] = json!(tab.id);
                row["tab_title"] = json!(tab_title(tab));
                out.push(row);
            }
        }
    }
    reply(json, Value::Array(out), lines)
}

/// An agent pane's status, whether it has been given a turn, and the approval it waits on.
pub fn status(app: &RepodeckApp, pane: Uuid) -> Result<String, String> {
    let session = app.pool.get(pane).ok_or("pane not running (open it once to start it)")?;
    let status = session.agent_status().map(|s| s.label());
    Ok(json!({
        "pane": pane, "status": status.unwrap_or_default(), "agent": status.is_some(),
        "asked": session.agent_prompt().is_some(), "approval": session.approval_request(),
    })
    .to_string())
}

/// Past Claude conversations in the repo at `path`, newest first.
pub fn sessions(app: &RepodeckApp, path: &str, json: bool) -> Result<String, String> {
    let repo = app.state.repo(repo_at(app, path)?).ok_or("unknown repo")?;
    let sessions =
        app.claude_history.sessions(&repo.path).ok_or("still reading Claude's history; try again")?;
    let lines = sessions.iter().map(|s| format!("{}\t{}\t{}", s.id, s.modified, field(&s.title))).collect();
    let out =
        sessions.iter().map(|s| json!({ "id": s.id, "title": s.title, "modified": s.modified })).collect();
    reply(json, Value::Array(out), lines)
}

/// The inbox, oldest first.
pub fn inbox(app: &RepodeckApp, json: bool) -> Result<String, String> {
    let notices = app.inbox.notices();
    let lines = notices
        .iter()
        .map(|n| {
            let read = if n.read { "read" } else { "unread" };
            let (label, title, detail) = (field(&n.label), field(&n.title), field(&n.detail));
            format!("{}\t{read}\t{}\t{label}\t{title}\t{detail}", n.id, n.source.pane)
        })
        .collect();
    let out = notices
        .iter()
        .map(|n| {
            json!({
                "id": n.id, "at": n.at, "pane": n.source.pane, "label": n.label,
                "title": n.title, "detail": n.detail, "read": n.read,
            })
        })
        .collect();
    reply(json, Value::Array(out), lines)
}
