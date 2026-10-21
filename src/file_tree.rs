use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui;
use ignore::WalkBuilder;

/// Re-read a directory at most this often so external changes show up without polling hard.
const CACHE_TTL: Duration = Duration::from_secs(3);

#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
}

/// Lazily listed, `.gitignore`-aware directory tree with per-directory expansion state. A
/// directory is listed synchronously the first time it's shown and refreshed off-thread after
/// that, so slow disks never stall a frame.
pub struct FileTree {
    cache: HashMap<PathBuf, (Instant, Arc<[Entry]>)>,
    expanded: HashSet<PathBuf>,
    pending: HashSet<PathBuf>,
    requests: Sender<PathBuf>,
    results: Receiver<(PathBuf, Vec<Entry>)>,
}

impl FileTree {
    pub fn new(ctx: egui::Context) -> Self {
        let (requests, request_rx) = mpsc::channel::<PathBuf>();
        let (result_tx, results) = mpsc::channel();
        thread::Builder::new()
            .name("file-tree".into())
            .spawn(move || {
                while let Ok(dir) = request_rx.recv() {
                    let entries = list_dir(&dir);
                    if result_tx.send((dir, entries)).is_err() {
                        return;
                    }
                    ctx.request_repaint();
                }
            })
            .expect("failed to spawn file tree thread");
        Self { cache: HashMap::new(), expanded: HashSet::new(), pending: HashSet::new(), requests, results }
    }

    /// The directory's entries, shared so showing them each frame doesn't copy the list.
    pub fn children(&mut self, dir: &Path) -> Arc<[Entry]> {
        while let Ok((path, entries)) = self.results.try_recv() {
            self.pending.remove(&path);
            self.cache.insert(path, (Instant::now(), entries.into()));
        }
        match self.cache.get(dir) {
            None => {
                self.cache.insert(dir.to_path_buf(), (Instant::now(), list_dir(dir).into()));
            }
            Some((at, _)) if at.elapsed() >= CACHE_TTL && !self.pending.contains(dir) => {
                self.pending.insert(dir.to_path_buf());
                let _ = self.requests.send(dir.to_path_buf());
            }
            Some(_) => {}
        }
        self.cache[dir].1.clone()
    }

    pub fn is_expanded(&self, dir: &Path) -> bool {
        self.expanded.contains(dir)
    }

    pub fn toggle(&mut self, dir: &Path) {
        if !self.expanded.remove(dir) {
            self.expanded.insert(dir.to_path_buf());
        }
    }
}

fn list_dir(dir: &Path) -> Vec<Entry> {
    let walker = WalkBuilder::new(dir)
        .max_depth(Some(1))
        .hidden(false)
        .git_ignore(true)
        .git_exclude(true)
        .parents(true)
        .require_git(false)
        .filter_entry(|e| e.file_name() != ".git")
        .build();
    let mut entries: Vec<Entry> = walker
        .filter_map(Result::ok)
        .filter(|e| e.depth() == 1)
        .map(|e| Entry {
            name: e.file_name().to_string_lossy().into_owned(),
            is_dir: e.file_type().is_some_and(|t| t.is_dir()),
            path: e.into_path(),
        })
        .collect();
    entries.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_dirs_first_and_respects_gitignore() {
        let root = std::env::temp_dir().join(format!("repodeck-tree-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("target")).unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
        std::fs::write(root.join("b.txt"), "").unwrap();
        std::fs::write(root.join("A.md"), "").unwrap();

        let names: Vec<_> = list_dir(&root).into_iter().map(|e| e.name).collect();
        assert_eq!(names, ["src", ".gitignore", "A.md", "b.txt"]);
        std::fs::remove_dir_all(root).unwrap();
    }
}
