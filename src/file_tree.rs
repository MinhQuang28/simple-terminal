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
    /// Matched by `.gitignore` or `.git/info/exclude`; only listed when ignored files are shown.
    pub ignored: bool,
}

/// Lazily listed, `.gitignore`-aware directory tree with per-directory expansion state. A
/// directory is listed synchronously the first time it's shown and refreshed off-thread after
/// that, so slow disks never stall a frame.
pub struct FileTree {
    cache: HashMap<PathBuf, (Instant, Arc<[Entry]>)>,
    expanded: HashSet<PathBuf>,
    pending: HashSet<PathBuf>,
    show_ignored: bool,
    requests: Sender<(PathBuf, bool)>,
    results: Receiver<(PathBuf, bool, Vec<Entry>)>,
}

impl FileTree {
    pub fn new(ctx: egui::Context) -> Self {
        let (requests, request_rx) = mpsc::channel::<(PathBuf, bool)>();
        let (result_tx, results) = mpsc::channel();
        thread::Builder::new()
            .name("file-tree".into())
            .spawn(move || {
                while let Ok((dir, show_ignored)) = request_rx.recv() {
                    let entries = list_dir(&dir, show_ignored);
                    if result_tx.send((dir, show_ignored, entries)).is_err() {
                        return;
                    }
                    ctx.request_repaint();
                }
            })
            .expect("failed to spawn file tree thread");
        Self {
            cache: HashMap::new(),
            expanded: HashSet::new(),
            pending: HashSet::new(),
            show_ignored: false,
            requests,
            results,
        }
    }

    /// Lists gitignored entries too (muted) when set; changing it drops every cached listing.
    pub fn set_show_ignored(&mut self, show_ignored: bool) {
        if self.show_ignored != show_ignored {
            self.show_ignored = show_ignored;
            self.cache.clear();
            self.pending.clear();
        }
    }

    /// The directory's entries, shared so showing them each frame doesn't copy the list.
    pub fn children(&mut self, dir: &Path) -> Arc<[Entry]> {
        while let Ok((path, show_ignored, entries)) = self.results.try_recv() {
            // A listing requested before the toggle flipped would mix modes; let it refresh.
            if show_ignored != self.show_ignored {
                continue;
            }
            self.pending.remove(&path);
            self.cache.insert(path, (Instant::now(), entries.into()));
        }
        match self.cache.get(dir) {
            None => {
                let entries = list_dir(dir, self.show_ignored).into();
                self.cache.insert(dir.to_path_buf(), (Instant::now(), entries));
            }
            Some((at, _)) if at.elapsed() >= CACHE_TTL && !self.pending.contains(dir) => {
                self.pending.insert(dir.to_path_buf());
                let _ = self.requests.send((dir.to_path_buf(), self.show_ignored));
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

/// The directory's immediate entries, honouring git ignore rules unless `with_ignored` is set,
/// in which case ignored entries are listed too and flagged.
fn list_dir(dir: &Path, with_ignored: bool) -> Vec<Entry> {
    let tracked = list_paths(dir, true);
    let (listed, tracked): (Vec<_>, HashSet<PathBuf>) = if with_ignored {
        (list_paths(dir, false), tracked.into_iter().map(|(path, _)| path).collect())
    } else {
        (tracked, HashSet::new())
    };
    let mut entries: Vec<Entry> = listed
        .into_iter()
        .map(|(path, is_dir)| Entry {
            name: path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
            ignored: with_ignored && !tracked.contains(&path),
            is_dir,
            path,
        })
        .collect();
    entries.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    entries
}

fn list_paths(dir: &Path, git_ignore: bool) -> Vec<(PathBuf, bool)> {
    WalkBuilder::new(dir)
        .max_depth(Some(1))
        .hidden(false)
        .git_ignore(git_ignore)
        .git_exclude(git_ignore)
        .parents(git_ignore)
        .require_git(false)
        .filter_entry(|e| e.file_name() != ".git")
        .build()
        .filter_map(Result::ok)
        .filter(|e| e.depth() == 1)
        .map(|e| (e.path().to_path_buf(), e.file_type().is_some_and(|t| t.is_dir())))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        let root = std::env::temp_dir().join(format!("repodeck-tree-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("target")).unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join(".gitignore"), "target/\n.env\n").unwrap();
        std::fs::write(root.join(".env"), "").unwrap();
        std::fs::write(root.join("b.txt"), "").unwrap();
        std::fs::write(root.join("A.md"), "").unwrap();
        root
    }

    #[test]
    fn lists_dirs_first_and_respects_gitignore() {
        let root = fixture();
        let names: Vec<_> = list_dir(&root, false).into_iter().map(|e| e.name).collect();
        assert_eq!(names, ["src", ".gitignore", "A.md", "b.txt"]);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn shows_ignored_entries_flagged_when_asked() {
        let root = fixture();
        let entries = list_dir(&root, true);
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["src", "target", ".env", ".gitignore", "A.md", "b.txt"]);
        let ignored: Vec<_> = entries.iter().filter(|e| e.ignored).map(|e| e.name.as_str()).collect();
        assert_eq!(ignored, ["target", ".env"]);
        std::fs::remove_dir_all(root).unwrap();
    }
}
