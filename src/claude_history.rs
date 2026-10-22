use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, UNIX_EPOCH};

use eframe::egui;
use serde_json::Value;

use crate::worktree::copy_path;

/// How long a repo's session list is reused before it is rescanned in the background.
const REFRESH_AFTER: Duration = Duration::from_secs(15);
const MAX_SESSIONS: usize = 30;

/// A past Claude Code conversation in a repo.
#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub id: String,
    pub title: String,
    /// Unix seconds of the last write to the transcript.
    pub modified: u64,
    /// Transcript size in bytes, a rough measure of how long the conversation ran.
    pub size: u64,
}

struct Entry {
    scanned_at: Instant,
    sessions: Option<Arc<[Session]>>,
}

/// A transcript's title as of its size and mtime, so rescans skip files that haven't changed.
struct CachedTitle {
    stamp: (u64, u64),
    title: Option<String>,
}

/// Claude Code transcripts per repo, scanned on background threads because they can be large.
#[derive(Clone)]
pub struct ClaudeHistory {
    entries: Arc<Mutex<HashMap<PathBuf, Entry>>>,
    titles: Arc<Mutex<HashMap<PathBuf, CachedTitle>>>,
    ctx: egui::Context,
}

impl ClaudeHistory {
    pub fn new(ctx: egui::Context) -> Self {
        Self { entries: Arc::default(), titles: Arc::default(), ctx }
    }

    /// Sessions for `repo`, newest first; `None` until the first scan finishes. Stale lists are
    /// returned while a rescan runs.
    pub fn sessions(&self, repo: &Path) -> Option<Arc<[Session]>> {
        let Ok(mut entries) = self.entries.lock() else { return None };
        let stale = entries.get(repo).is_none_or(|e| e.scanned_at.elapsed() > REFRESH_AFTER);
        if stale {
            let previous = entries.get(repo).and_then(|e| e.sessions.clone());
            entries.insert(repo.to_path_buf(), Entry { scanned_at: Instant::now(), sessions: previous });
            self.spawn_scan(repo.to_path_buf());
        }
        entries.get(repo).and_then(|e| e.sessions.clone())
    }

    fn spawn_scan(&self, repo: PathBuf) {
        let this = self.clone();
        let spawned = thread::Builder::new().name("claude-history".into()).spawn(move || {
            let sessions = scan(&repo, &this.titles);
            if let Ok(mut entries) = this.entries.lock() {
                entries.insert(repo, Entry { scanned_at: Instant::now(), sessions: Some(sessions.into()) });
            }
            this.ctx.request_repaint();
        });
        if let Err(err) = spawned {
            log::error!("Failed to start Claude history scan: {err}");
        }
    }
}

/// Claude Code's transcript directory for a working directory: every non-alphanumeric character of
/// the path becomes `-`.
pub(crate) fn transcripts_dir(repo: &Path) -> Option<PathBuf> {
    let encoded: String =
        repo.to_string_lossy().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let home = std::env::var_os("HOME")?;
    Some(Path::new(&home).join(".claude/projects").join(encoded))
}

fn scan(repo: &Path, titles: &Mutex<HashMap<PathBuf, CachedTitle>>) -> Vec<Session> {
    let Some(dir) = transcripts_dir(repo) else { return Vec::new() };
    let Ok(entries) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut files: Vec<(PathBuf, u64, u64)> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "jsonl"))
        .filter_map(|p| {
            let meta = p.metadata().ok()?;
            let modified = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs();
            Some((p, meta.len(), modified))
        })
        .collect();
    files.sort_by_key(|(_, _, modified)| std::cmp::Reverse(*modified));
    files
        .into_iter()
        .filter_map(|(path, len, modified)| {
            let id = path.file_stem()?.to_string_lossy().into_owned();
            if !is_session_id(&id) {
                return None;
            }
            let title = cached_title(titles, path, (len, modified))?;
            Some(Session { id, title, modified, size: len })
        })
        .take(MAX_SESSIONS)
        .collect()
}

/// Transcripts run to tens of megabytes, so a title is only read again once the file changed.
fn cached_title(
    titles: &Mutex<HashMap<PathBuf, CachedTitle>>,
    path: PathBuf,
    stamp: (u64, u64),
) -> Option<String> {
    let cached = titles.lock().ok()?.get(&path).filter(|c| c.stamp == stamp).map(|c| c.title.clone());
    if let Some(title) = cached {
        return title;
    }
    let title = read_title(&path);
    titles.lock().ok()?.insert(path, CachedTitle { stamp, title: title.clone() });
    title
}

/// Copies `from`'s Claude Code transcripts into `to`'s transcript directory, so conversations
/// started in a repo can be resumed from a new worktree of it. Entries already there are kept.
pub fn copy_transcripts(from: &Path, to: &Path) -> Result<(), String> {
    let (Some(src), Some(dst)) = (transcripts_dir(from), transcripts_dir(to)) else { return Ok(()) };
    let Ok(entries) = fs::read_dir(&src) else { return Ok(()) };
    for entry in entries.flatten() {
        let target = dst.join(entry.file_name());
        if !target.exists() {
            copy_path(&entry.path(), &target).map_err(|err| format!("{}: {err}", entry.path().display()))?;
        }
    }
    Ok(())
}

/// Session ids are interpolated into a shell command, so only accept UUID-like names.
pub fn is_session_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

/// The user's rename, else Claude's generated title, else the latest prompt. `None` for transcripts
/// without any prompt, which Claude can't meaningfully resume.
fn read_title(path: &Path) -> Option<String> {
    let mut reader = BufReader::new(File::open(path).ok()?);
    let (mut custom, mut generated, mut prompt) = (None, None, None);
    // One buffer for every line: message entries can be megabytes each.
    let mut line = String::new();
    loop {
        line.clear();
        if !matches!(reader.read_line(&mut line), Ok(n) if n > 0) {
            break;
        }
        // Cheap substring checks first: transcripts are mostly large message entries.
        let slot = if line.contains(r#""type":"custom-title""#) {
            (&mut custom, "customTitle")
        } else if line.contains(r#""type":"ai-title""#) {
            (&mut generated, "aiTitle")
        } else if line.contains(r#""type":"last-prompt""#) {
            (&mut prompt, "lastPrompt")
        } else {
            continue;
        };
        let value: Option<String> = serde_json::from_str::<Value>(&line)
            .ok()
            .and_then(|v| v.get(slot.1)?.as_str().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty());
        if value.is_some() {
            *slot.0 = value;
        }
    }
    let title = custom.or(generated).or(prompt)?;
    Some(title.lines().next().unwrap_or_default().to_string())
}

#[cfg(test)]
#[path = "claude_history_tests.rs"]
mod tests;
