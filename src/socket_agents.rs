//! Socket commands that steer agents: interrupt a turn, answer a permission prompt, resume a
//! past conversation.

use uuid::Uuid;

use crate::app::RepodeckApp;
use crate::model::PaneKind;
use crate::socket_control::apply;
use crate::socket_queries::repo_at;
use crate::ui::Action;

/// Esc for an agent (stops its turn), Ctrl-C for a shell.
pub fn interrupt(app: &mut RepodeckApp, pane: Uuid) -> Result<String, String> {
    let agent = app
        .state
        .projects
        .iter()
        .flat_map(|p| &p.repos)
        .flat_map(|r| &r.tabs)
        .flat_map(|t| &t.panes)
        .find(|p| p.id == pane)
        .map(|p| p.kind == PaneKind::Agent)
        .ok_or("unknown pane")?;
    let session = app.pool.get_mut(pane).ok_or("pane not running")?;
    if agent {
        session.interrupt_agent();
    } else {
        session.send_input(b"\x03".to_vec());
    }
    Ok("ok".into())
}

/// Answers the permission prompt of an agent the caller started; an agent can't approve its
/// own requests or those of agents the user started.
pub fn approve(app: &mut RepodeckApp, pane: Uuid, allow: bool, from: &str) -> Result<String, String> {
    let caller = Uuid::parse_str(from).map_err(|_| "approve works only from inside a Repodeck pane")?;
    let parent = app
        .state
        .projects
        .iter()
        .flat_map(|p| &p.repos)
        .flat_map(|r| &r.tabs)
        .flat_map(|t| &t.panes)
        .find(|p| p.id == pane)
        .map(|p| p.parent)
        .ok_or("unknown pane")?;
    if parent != Some(caller) {
        return Err(
            "only an agent you started (split --claude, claude --task, worktree --task) can be answered"
                .into(),
        );
    }
    let waiting = app.pool.get(pane).is_some_and(|s| s.approval_request().is_some());
    if !waiting {
        return Err("that agent isn't waiting for an approval".into());
    }
    apply(app, [Action::AnswerApproval { pane, allow }]);
    Ok(if allow { "allowed" } else { "denied" }.into())
}

/// Opens a Claude tab resuming `session` in the repo at `path` (the latest one when empty).
pub fn resume(app: &mut RepodeckApp, path: &str, session: &str) -> Result<String, String> {
    let repo = repo_at(app, path)?;
    let repo_path = app.state.repo(repo).map(|r| r.path.clone()).ok_or("unknown repo")?;
    let sessions =
        app.claude_history.sessions(&repo_path).ok_or("still reading Claude's history; try again")?;
    let found = if session.is_empty() { sessions.first() } else { sessions.iter().find(|s| s.id == session) };
    let found = found.ok_or("no such Claude session in that repo (see `repodeck sessions`)")?;
    let (id, title) = (found.id.clone(), found.title.clone());
    app.resume_session(repo, id.clone(), title);
    app.ctx.request_repaint();
    Ok(id)
}
