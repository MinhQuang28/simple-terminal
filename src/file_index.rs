use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui;
use ignore::WalkBuilder;

/// A listing older than this is walked again, in the background, the next time it's asked for.
const REFRESH: Duration = Duration::from_secs(30);
/// Enough for a large monorepo; anything past it is cut and the listing says so.
const MAX_FILES: usize = 50_000;

/// A repo's files for quick open, relative to its root and `.gitignore`-aware.
pub struct Listing {
    pub paths: Vec<String>,
    /// `paths` lowercased, so matching a query doesn't lowercase every path per keystroke.
    lowered: Vec<String>,
    pub truncated: bool,
}

impl Listing {
    fn new(paths: Vec<String>, truncated: bool) -> Self {
        let lowered = paths.iter().map(|p| p.to_lowercase()).collect();
        Self { paths, lowered, truncated }
    }

    /// Indexes of the `limit` best matches for `query`, best first.
    pub fn search(&self, query: &str, limit: usize, score: impl Fn(&str, &str) -> Option<i64>) -> Vec<usize> {
        let query: String = query.to_lowercase().chars().filter(|c| !c.is_whitespace()).collect();
        let mut scored: Vec<(i64, usize)> =
            self.lowered.iter().enumerate().filter_map(|(i, p)| score(&query, p).map(|s| (s, i))).collect();
        if scored.len() > limit {
            scored.select_nth_unstable_by_key(limit, |(s, i)| (std::cmp::Reverse(*s), *i));
            scored.truncate(limit);
        }
        scored.sort_by_key(|(s, i)| (std::cmp::Reverse(*s), *i));
        scored.into_iter().map(|(_, i)| i).collect()
    }
}

/// Walks repos off the UI thread and keeps each listing shared, so quick open never stalls a
/// frame on the disk.
pub struct FileIndex {
    listings: HashMap<PathBuf, (Instant, Arc<Listing>)>,
    pending: HashSet<PathBuf>,
    requests: Sender<PathBuf>,
    results: Receiver<(PathBuf, Listing)>,
}

impl FileIndex {
    pub fn new(ctx: egui::Context) -> Self {
        let (requests, request_rx) = mpsc::channel::<PathBuf>();
        let (result_tx, results) = mpsc::channel();
        thread::Builder::new()
            .name("file-index".into())
            .spawn(move || {
                while let Ok(root) = request_rx.recv() {
                    let listing = walk(&root);
                    if result_tx.send((root, listing)).is_err() {
                        return;
                    }
                    ctx.request_repaint();
                }
            })
            .expect("failed to spawn file index thread");
        Self { listings: HashMap::new(), pending: HashSet::new(), requests, results }
    }

    /// The repo's listing once walked; asks for a walk when there is none or it has gone stale.
    pub fn get(&mut self, root: &Path) -> Option<Arc<Listing>> {
        while let Ok((path, listing)) = self.results.try_recv() {
            self.pending.remove(&path);
            self.listings.insert(path, (Instant::now(), Arc::new(listing)));
        }
        let stale = self.listings.get(root).is_none_or(|(at, _)| at.elapsed() >= REFRESH);
        if stale && !self.pending.contains(root) {
            self.pending.insert(root.to_path_buf());
            let _ = self.requests.send(root.to_path_buf());
        }
        self.listings.get(root).map(|(_, listing)| listing.clone())
    }
}

fn walk(root: &Path) -> Listing {
    let walker = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .git_exclude(true)
        .require_git(false)
        .filter_entry(|e| e.file_name() != ".git")
        .build();
    let mut paths = Vec::new();
    let mut truncated = false;
    for entry in walker.filter_map(Result::ok) {
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        if paths.len() == MAX_FILES {
            truncated = true;
            break;
        }
        let relative = entry.path().strip_prefix(root).unwrap_or(entry.path());
        paths.push(relative.to_string_lossy().into_owned());
    }
    paths.sort_unstable();
    Listing::new(paths, truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_files_only_and_ranks_the_best_matches_first() {
        let root = std::env::temp_dir().join(format!("repodeck-index-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("src/ui")).unwrap();
        std::fs::create_dir_all(root.join("target")).unwrap();
        std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
        std::fs::write(root.join("src/ui/palette.rs"), "").unwrap();
        std::fs::write(root.join("src/main.rs"), "").unwrap();
        std::fs::write(root.join("target/out.rs"), "").unwrap();

        let listing = walk(&root);
        assert_eq!(listing.paths, [".gitignore", "src/main.rs", "src/ui/palette.rs"]);
        assert!(!listing.truncated);
        let score = |q: &str, t: &str| if t.contains(q) { Some(-(t.len() as i64)) } else { None };
        assert_eq!(listing.search("rs", 1, score), [1]);
        assert_eq!(listing.search("zzz", 5, score), Vec::<usize>::new());
        std::fs::remove_dir_all(root).unwrap();
    }
}
