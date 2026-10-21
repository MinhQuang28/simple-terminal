use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use eframe::egui::{self, Galley};

use crate::git_status::GitStatus;
use crate::ui::diff_comments::Comment;

/// Lines of a unified diff beyond this are dropped so huge diffs stay responsive.
const MAX_LINES: usize = 20_000;

#[derive(Default)]
struct Entry {
    /// Status generation the diff was computed for; a newer one means the tree changed.
    generation: u64,
    /// Generation of the diff currently being computed, if any.
    computing: Option<u64>,
    text: Option<Arc<str>>,
    /// Laid-out text for the current `text` and the look it was laid out with.
    galley: Option<(u64, Arc<Galley>)>,
    comments: Vec<Comment>,
}

/// Unified diffs of changed files, computed on a background thread whenever the repo's git
/// status changes; the previous text stays visible until the new one arrives.
pub struct DiffCache {
    entries: HashMap<PathBuf, Entry>,
    results: Receiver<(PathBuf, u64, String)>,
    tx: Sender<(PathBuf, u64, String)>,
    ctx: egui::Context,
}

impl DiffCache {
    pub fn new(ctx: egui::Context) -> Self {
        let (tx, results) = mpsc::channel();
        Self { entries: HashMap::new(), results, tx, ctx }
    }

    /// The diff of `file`, or `None` until its first computation finishes.
    pub fn get(&mut self, repo: &Path, file: &Path, status: Option<&Arc<GitStatus>>) -> Option<Arc<str>> {
        while let Ok((path, generation, text)) = self.results.try_recv() {
            let entry = self.entries.entry(path).or_default();
            if generation >= entry.generation {
                entry.generation = generation;
                entry.text = Some(text.into());
                entry.galley = None;
            }
            if entry.computing == Some(generation) {
                entry.computing = None;
            }
        }
        let generation = status.map_or(0, |s| s.generation);
        let entry = self.entries.entry(file.to_path_buf()).or_default();
        let stale = entry.text.is_none() || entry.generation < generation;
        if stale && entry.computing.is_none_or(|g| g < generation) {
            entry.computing = Some(generation);
            // Untracked directories are reported as one entry, so match by prefix too.
            let untracked = status.is_some_and(|s| {
                s.files.iter().any(|f| f.status == 'U' && file.starts_with(repo.join(&f.path)))
            });
            let (repo, file, tx, ctx) =
                (repo.to_path_buf(), file.to_path_buf(), self.tx.clone(), self.ctx.clone());
            let spawned = thread::Builder::new().name("git-diff".into()).spawn(move || {
                let text = diff(&repo, &file, untracked);
                let _ = tx.send((file, generation, text));
                ctx.request_repaint();
            });
            if let Err(err) = spawned {
                log::error!("Failed to start git diff: {err}");
            }
        }
        entry.text.clone()
    }

    /// Where the view keeps the laid-out diff of `file` (keyed by the look it was built for) and
    /// the user's comments on it.
    pub fn view_slots(&mut self, file: &Path) -> (&mut Option<(u64, Arc<Galley>)>, &mut Vec<Comment>) {
        let entry = self.entries.entry(file.to_path_buf()).or_default();
        (&mut entry.galley, &mut entry.comments)
    }

    pub fn forget(&mut self, file: &Path) {
        self.entries.remove(file);
    }
}

/// Git's well-known empty tree, which stands in for `HEAD` in a repo without commits.
const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

fn diff(repo: &Path, file: &Path, untracked: bool) -> String {
    let run = |base: &str| {
        let mut command = Command::new("git");
        command.arg("-C").arg(repo).arg("--no-optional-locks");
        if untracked {
            command.args(["diff", "--no-index", "--", "/dev/null"]).arg(file);
        } else {
            command.args(["diff", base, "--"]).arg(file);
        }
        command.output()
    };
    let output = match run("HEAD") {
        Ok(output) if String::from_utf8_lossy(&output.stderr).contains("bad revision 'HEAD'") => {
            match run(EMPTY_TREE) {
                Ok(output) => output,
                Err(err) => return format!("Cannot run git: {err}"),
            }
        }
        Ok(output) => output,
        Err(err) => return format!("Cannot run git: {err}"),
    };
    // `--no-index` exits 1 when the files differ, so the status alone doesn't signal failure.
    if output.stdout.is_empty() && !output.stderr.is_empty() {
        return String::from_utf8_lossy(&output.stderr).into_owned();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut lines = text.lines();
    let mut kept: Vec<&str> = lines.by_ref().take(MAX_LINES).collect();
    if lines.next().is_some() {
        kept.push("… diff truncated");
    }
    kept.join("\n")
}
