use std::path::Path;
use std::process::Command;

use uuid::Uuid;

use crate::app::{Job, RepodeckApp};
use crate::pr_status::{self, PullRequest};

/// Lines of a failed job's log kept when handing it to Claude.
const LOG_TAIL: usize = 150;

impl RepodeckApp {
    /// Opens GitHub's new-pull-request page for the repo's branch.
    pub(crate) fn create_pull_request(&mut self, repo_id: Uuid) {
        let Some(repo) = self.state.repo(repo_id) else { return };
        let path = repo.path.clone();
        let spawned = Command::new("gh").current_dir(&path).args(["pr", "create", "--web"]).spawn();
        if let Err(err) = spawned {
            self.report_error("Could not run gh pr create", err.to_string());
        }
        self.prs.refresh(&path);
    }

    /// Collects the failing checks' logs in the background and pastes them into the repo's Claude
    /// pane with a request to fix them.
    pub(crate) fn auto_fix_ci(&mut self, repo_id: Uuid) {
        let Some(repo) = self.state.repo(repo_id) else { return };
        let Some(Some(pr)) = self.prs.pr(&repo.path) else { return };
        let path = repo.path.clone();
        self.spawn_job("ci-logs", move || {
            let prompt = ci_prompt(&path, &pr);
            Box::new(move |app: &mut RepodeckApp| app.send_to_agent(repo_id, prompt)) as Job
        });
    }

    /// Pastes a library prompt into the active terminal pane, or hands it to the repo's Claude
    /// pane when a document tab is showing.
    pub(crate) fn insert_prompt(&mut self, text: String) {
        let Some(repo) = self.state.selected_repo.and_then(|id| self.state.repo(id)) else { return };
        let tab = repo.tabs.iter().find(|t| Some(t.id) == repo.active_tab);
        let pane = tab.filter(|t| !t.is_document()).and_then(|t| t.active_pane);
        match pane.and_then(|p| self.pool.get(p).map(|s| (p, s))) {
            Some((pane, session)) => {
                session.paste(&text);
                self.pending_focus = Some(pane);
            }
            None => self.send_to_agent(repo.id, text),
        }
    }
}

fn ci_prompt(repo: &Path, pr: &PullRequest) -> String {
    let mut prompt = format!(
        "CI is failing on pull request #{} ({}). Find the cause and fix it.\n\n",
        pr.number, pr.title
    );
    for check in pr.failing() {
        prompt.push_str(&format!("## {}\n", check.name));
        let log = pr_status::run_id(&check.url).map(|id| failed_log(repo, id)).unwrap_or_default();
        if log.is_empty() {
            prompt.push_str(&format!("{}\n\n", check.url));
        } else {
            prompt.push_str(&format!("```\n{log}\n```\n\n"));
        }
    }
    prompt
}

/// The tail of `gh run view --log-failed` for a GitHub Actions run.
fn failed_log(repo: &Path, run_id: &str) -> String {
    let output = Command::new("gh").current_dir(repo).args(["run", "view", run_id, "--log-failed"]).output();
    let Ok(output) = output else { return String::new() };
    let text = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(LOG_TAIL);
    lines[start..].join("\n")
}
