use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use eframe::egui;
use serde_json::Value;

/// How often each repo's pull request is re-read from GitHub.
const POLL_INTERVAL: Duration = Duration::from_secs(90);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckState {
    Passing,
    Failing,
    Pending,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    pub name: String,
    pub state: CheckState,
    pub url: String,
}

/// The open pull request for a repo's current branch, as `gh pr view` reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PullRequest {
    pub number: u64,
    pub url: String,
    pub title: String,
    /// `OPEN`, `MERGED` or `CLOSED`.
    pub state: String,
    pub draft: bool,
    /// `APPROVED`, `CHANGES_REQUESTED`, `REVIEW_REQUIRED` or empty.
    pub review: String,
    pub checks: Vec<Check>,
}

impl PullRequest {
    /// Worst state across checks: failing beats pending beats passing; `None` without checks.
    pub fn ci(&self) -> Option<CheckState> {
        let states = self.checks.iter().map(|c| c.state);
        if states.clone().any(|s| s == CheckState::Failing) {
            Some(CheckState::Failing)
        } else if states.clone().any(|s| s == CheckState::Pending) {
            Some(CheckState::Pending)
        } else {
            self.checks.first().map(|_| CheckState::Passing)
        }
    }

    pub fn failing(&self) -> impl Iterator<Item = &Check> {
        self.checks.iter().filter(|c| c.state == CheckState::Failing)
    }
}

enum Msg {
    Paths(Vec<PathBuf>),
    Refresh(PathBuf),
}

/// Pull request state per repo, polled with the GitHub CLI on a background thread. Repos without
/// a PR (or without `gh`) are recorded as `None` so the UI can tell "no PR" from "not checked".
pub struct PrWatcher {
    shared: Arc<Mutex<HashMap<PathBuf, Option<Arc<PullRequest>>>>>,
    tx: Sender<Msg>,
}

impl PrWatcher {
    pub fn spawn(ctx: egui::Context) -> Self {
        let shared: Arc<Mutex<HashMap<PathBuf, Option<Arc<PullRequest>>>>> = Arc::default();
        let (tx, rx) = mpsc::channel();
        let worker = shared.clone();
        thread::Builder::new()
            .name("pr-status".into())
            .spawn(move || run(worker, rx, ctx))
            .expect("failed to spawn pr status thread");
        Self { shared, tx }
    }

    pub fn set_paths(&self, paths: Vec<PathBuf>) {
        let _ = self.tx.send(Msg::Paths(paths));
    }

    pub fn refresh(&self, path: &Path) {
        let _ = self.tx.send(Msg::Refresh(path.to_path_buf()));
    }

    /// `Some(None)` once the repo was checked and has no PR.
    pub fn pr(&self, path: &Path) -> Option<Option<Arc<PullRequest>>> {
        self.shared.lock().ok()?.get(path).cloned()
    }
}

fn run(
    shared: Arc<Mutex<HashMap<PathBuf, Option<Arc<PullRequest>>>>>,
    rx: Receiver<Msg>,
    ctx: egui::Context,
) {
    if !gh_available() {
        log::info!("GitHub CLI not found; pull request status disabled");
        return;
    }
    let mut repos: Vec<PathBuf> = Vec::new();
    loop {
        let due: Vec<PathBuf> = match rx.recv_timeout(POLL_INTERVAL) {
            Ok(Msg::Paths(paths)) => {
                if let Ok(mut map) = shared.lock() {
                    map.retain(|p, _| paths.contains(p));
                }
                let new: Vec<_> = paths.iter().filter(|p| !repos.contains(p)).cloned().collect();
                repos = paths;
                new
            }
            Ok(Msg::Refresh(path)) => vec![path],
            Err(RecvTimeoutError::Timeout) => repos.clone(),
            Err(RecvTimeoutError::Disconnected) => return,
        };
        let mut changed = false;
        for path in due {
            let pr = read_pr(&path).map(Arc::new);
            if let Ok(mut map) = shared.lock()
                && map.get(&path) != Some(&pr)
            {
                map.insert(path, pr);
                changed = true;
            }
        }
        if changed {
            ctx.request_repaint();
        }
    }
}

fn gh_available() -> bool {
    Command::new("gh").arg("--version").output().is_ok_and(|o| o.status.success())
}

fn read_pr(repo: &Path) -> Option<PullRequest> {
    let output = Command::new("gh")
        .current_dir(repo)
        .args(["pr", "view", "--json", "number,url,title,state,isDraft,reviewDecision,statusCheckRollup"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_pr(&String::from_utf8_lossy(&output.stdout))
}

fn parse_pr(json: &str) -> Option<PullRequest> {
    let value: Value = serde_json::from_str(json).ok()?;
    let text = |key: &str| value[key].as_str().unwrap_or_default().to_string();
    let checks = value["statusCheckRollup"]
        .as_array()
        .map(|items| items.iter().filter_map(parse_check).collect())
        .unwrap_or_default();
    Some(PullRequest {
        number: value["number"].as_u64()?,
        url: text("url"),
        title: text("title"),
        state: text("state"),
        draft: value["isDraft"].as_bool().unwrap_or(false),
        review: text("reviewDecision"),
        checks,
    })
}

/// Handles both GitHub Actions check runs and legacy commit statuses.
fn parse_check(item: &Value) -> Option<Check> {
    let name = item["name"].as_str().or_else(|| item["context"].as_str())?.to_string();
    let url = item["detailsUrl"].as_str().or_else(|| item["targetUrl"].as_str()).unwrap_or_default();
    let state = match (item["status"].as_str(), item["conclusion"].as_str(), item["state"].as_str()) {
        (_, Some("SUCCESS" | "NEUTRAL" | "SKIPPED"), _) | (_, _, Some("SUCCESS")) => CheckState::Passing,
        (_, Some("FAILURE" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED" | "STARTUP_FAILURE"), _)
        | (_, _, Some("FAILURE" | "ERROR")) => CheckState::Failing,
        _ => CheckState::Pending,
    };
    Some(Check { name, state, url: url.to_string() })
}

/// The GitHub Actions run id in a check's details URL, if it points at one.
pub fn run_id(url: &str) -> Option<&str> {
    url.split("/actions/runs/").nth(1)?.split('/').next().filter(|id| !id.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_checks_and_summarises_ci() {
        let json = r#"{"number":7,"url":"u","title":"t","state":"OPEN","isDraft":false,"reviewDecision":"",
            "statusCheckRollup":[
              {"__typename":"CheckRun","name":"build","status":"COMPLETED","conclusion":"SUCCESS","detailsUrl":"https://github.com/o/r/actions/runs/42/job/1"},
              {"__typename":"CheckRun","name":"test","status":"IN_PROGRESS","conclusion":"","detailsUrl":""},
              {"__typename":"StatusContext","context":"lint","state":"FAILURE","targetUrl":"x"}]}"#;
        let pr = parse_pr(json).unwrap();
        assert_eq!(pr.number, 7);
        assert_eq!(pr.ci(), Some(CheckState::Failing));
        assert_eq!(pr.failing().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["lint"]);
        assert_eq!(run_id(&pr.checks[0].url), Some("42"));
        assert_eq!(run_id("https://example.com"), None);
    }

    #[test]
    fn no_checks_means_no_ci_state() {
        let pr = parse_pr(r#"{"number":1,"statusCheckRollup":[]}"#).unwrap();
        assert_eq!(pr.ci(), None);
    }
}
