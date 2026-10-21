use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};

use crate::git_status::{GitStatus, read_status};

/// Bursts of file events (a build, a checkout) collapse into one status run.
const DEBOUNCE: Duration = Duration::from_millis(500);
/// A repo is never re-checked more often than this, however busy its working tree is.
const MIN_INTERVAL: Duration = Duration::from_secs(2);
/// Safety net for changes the watcher can't see (network mounts, dropped events).
const FALLBACK_INTERVAL: Duration = Duration::from_secs(60);

enum Msg {
    Paths(Vec<PathBuf>),
    Changed(Vec<PathBuf>),
}

#[derive(Default)]
struct Shared {
    statuses: HashMap<PathBuf, Arc<GitStatus>>,
}

/// Keeps `git status` of the watched repos up to date from file-system events, falling back to
/// a slow poll. Statuses are shared so reading one each frame doesn't copy its file list.
pub struct GitWatcher {
    shared: Arc<Mutex<Shared>>,
    tx: Sender<Msg>,
    paths: Mutex<Vec<PathBuf>>,
}

impl GitWatcher {
    pub fn spawn(ctx: egui::Context) -> Self {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let (tx, rx) = mpsc::channel();
        let worker = Worker { shared: shared.clone(), ctx, rx, tx: tx.clone(), generation: Cell::new(0) };
        thread::Builder::new()
            .name("git-status".into())
            .spawn(move || worker.run())
            .expect("failed to spawn git status thread");
        Self { shared, tx, paths: Mutex::default() }
    }

    pub fn set_paths(&self, paths: Vec<PathBuf>) {
        let Ok(mut current) = self.paths.lock() else { return };
        if *current != paths {
            if let Ok(mut shared) = self.shared.lock() {
                shared.statuses.retain(|p, _| paths.contains(p));
            }
            let _ = self.tx.send(Msg::Paths(paths.clone()));
            *current = paths;
        }
    }

    pub fn status(&self, path: &Path) -> Option<Arc<GitStatus>> {
        self.shared.lock().ok()?.statuses.get(path).cloned()
    }
}

struct Worker {
    shared: Arc<Mutex<Shared>>,
    ctx: egui::Context,
    rx: Receiver<Msg>,
    tx: Sender<Msg>,
    generation: Cell<u64>,
}

impl Worker {
    fn run(self) {
        let mut watcher = self.watcher();
        let mut repos: Vec<PathBuf> = Vec::new();
        let mut checked: HashMap<PathBuf, Instant> = HashMap::new();
        let mut dirty: HashSet<PathBuf> = HashSet::new();
        let mut dirty_since: Option<Instant> = None;
        let mut last_full = Instant::now();
        loop {
            let wait = if dirty.is_empty() { FALLBACK_INTERVAL } else { DEBOUNCE };
            let quiet = match self.rx.recv_timeout(wait) {
                Ok(Msg::Paths(paths)) => {
                    rewatch(watcher.as_mut(), &repos, &paths);
                    repos = paths;
                    dirty.extend(repos.iter().cloned());
                    false
                }
                Ok(Msg::Changed(paths)) => {
                    dirty.extend(paths.into_iter().filter_map(|p| repo_for(&repos, &p)));
                    false
                }
                Err(RecvTimeoutError::Timeout) => true,
                Err(RecvTimeoutError::Disconnected) => return,
            };
            if !dirty.is_empty() {
                // A steady stream of events (a build writing files) must not postpone the check
                // forever: the debounce only holds until the minimum interval has passed.
                let since = *dirty_since.get_or_insert_with(Instant::now);
                if !quiet && since.elapsed() < MIN_INTERVAL {
                    continue;
                }
            }
            if last_full.elapsed() >= FALLBACK_INTERVAL {
                dirty.extend(repos.iter().cloned());
                last_full = Instant::now();
            }
            let now = Instant::now();
            let due: Vec<_> = dirty
                .iter()
                .filter(|p| checked.get(*p).is_none_or(|at| now - *at >= MIN_INTERVAL))
                .cloned()
                .collect();
            let mut changed = false;
            for path in due {
                dirty.remove(&path);
                checked.insert(path.clone(), Instant::now());
                changed |= self.refresh(&path);
            }
            if dirty.is_empty() {
                dirty_since = None;
            }
            if changed {
                self.ctx.request_repaint();
            }
        }
    }

    fn refresh(&self, path: &Path) -> bool {
        let Some(mut status) = read_status(path) else { return false };
        let Ok(mut shared) = self.shared.lock() else { return false };
        if let Some(old) = shared.statuses.get(path) {
            status.generation = old.generation;
            if **old == status {
                return false;
            }
        }
        self.generation.set(self.generation.get() + 1);
        status.generation = self.generation.get();
        shared.statuses.insert(path.to_path_buf(), Arc::new(status));
        true
    }

    fn watcher(&self) -> Option<RecommendedWatcher> {
        let tx = self.tx.clone();
        let handler = move |event: notify::Result<Event>| {
            if let Ok(event) = event {
                let paths: Vec<_> = event.paths.into_iter().filter(|p| affects_status(p)).collect();
                if !paths.is_empty() {
                    let _ = tx.send(Msg::Changed(paths));
                }
            }
        };
        match notify::recommended_watcher(handler) {
            Ok(watcher) => Some(watcher),
            Err(err) => {
                log::warn!("File watching unavailable, polling git status instead: {err}");
                None
            }
        }
    }
}

fn rewatch(watcher: Option<&mut RecommendedWatcher>, old: &[PathBuf], new: &[PathBuf]) {
    let Some(watcher) = watcher else { return };
    for path in old.iter().filter(|p| !new.contains(p)) {
        let _ = watcher.unwatch(path);
    }
    for path in new.iter().filter(|p| !old.contains(p)) {
        if let Err(err) = watcher.watch(path, RecursiveMode::Recursive) {
            log::warn!("Cannot watch {}: {err}", path.display());
        }
    }
}

/// Inside `.git`, only the index, HEAD pointers and refs change what `git status` reports;
/// objects, logs and lock files churn constantly without doing so.
fn affects_status(path: &Path) -> bool {
    let mut inside_git = false;
    for part in path.components().map(|c| c.as_os_str()) {
        if inside_git {
            return matches!(part.to_str(), Some("index" | "HEAD" | "ORIG_HEAD" | "packed-refs" | "refs"));
        }
        inside_git = part == ".git";
    }
    !path.extension().is_some_and(|e| e == "lock")
}

/// The longest watched root containing `path`, so nested repos map to the inner one.
fn repo_for(repos: &[PathBuf], path: &Path) -> Option<PathBuf> {
    repos.iter().filter(|r| path.starts_with(r)).max_by_key(|r| r.as_os_str().len()).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_git_internals_that_do_not_change_status() {
        assert!(affects_status(Path::new("/r/src/main.rs")));
        assert!(affects_status(Path::new("/r/.git/index")));
        assert!(affects_status(Path::new("/r/.git/refs/heads/main")));
        assert!(!affects_status(Path::new("/r/.git/objects/ab/cd")));
        assert!(!affects_status(Path::new("/r/.git/index.lock")));
        assert!(!affects_status(Path::new("/r/.git/logs/HEAD")));
    }

    #[test]
    fn maps_paths_to_the_innermost_repo() {
        let repos = [PathBuf::from("/a"), PathBuf::from("/a/b")];
        assert_eq!(repo_for(&repos, Path::new("/a/b/c.rs")), Some(PathBuf::from("/a/b")));
        assert_eq!(repo_for(&repos, Path::new("/a/x.rs")), Some(PathBuf::from("/a")));
        assert_eq!(repo_for(&repos, Path::new("/z")), None);
    }
}
