use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::library::{self, McpServer, Skill};

/// How long a repo's library is reused before it is rescanned in the background. The user's
/// skills alone run to hundreds of KB of `SKILL.md`, too much to read every frame.
const REFRESH_AFTER: Duration = Duration::from_secs(3);

/// Skills and MCP servers available to agents in a repo.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Library {
    pub skills: Vec<Skill>,
    pub servers: Vec<McpServer>,
}

struct Entry {
    scanned_at: Instant,
    library: Option<Arc<Library>>,
}

/// Per-repo libraries, scanned on background threads and shared while a rescan runs.
#[derive(Clone)]
pub struct LibraryCache {
    entries: Arc<Mutex<HashMap<PathBuf, Entry>>>,
    ctx: egui::Context,
}

impl LibraryCache {
    pub fn new(ctx: egui::Context) -> Self {
        Self { entries: Arc::default(), ctx }
    }

    /// The library for `repo`; `None` until the first scan finishes. A stale one is returned
    /// while a rescan runs.
    pub fn get(&self, repo: &Path) -> Option<Arc<Library>> {
        let Ok(mut entries) = self.entries.lock() else { return None };
        let stale = entries.get(repo).is_none_or(|e| e.scanned_at.elapsed() > REFRESH_AFTER);
        if stale {
            let previous = entries.get(repo).and_then(|e| e.library.clone());
            entries.insert(repo.to_path_buf(), Entry { scanned_at: Instant::now(), library: previous });
            self.spawn_scan(repo.to_path_buf());
        }
        entries.get(repo).and_then(|e| e.library.clone())
    }

    /// Rescans every repo on its next `get`, e.g. after a skill was toggled: user skills are
    /// shared, so the change shows in all repos.
    pub fn invalidate(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            for entry in entries.values_mut() {
                entry.scanned_at = Instant::now() - REFRESH_AFTER - Duration::from_millis(1);
            }
        }
    }

    fn spawn_scan(&self, repo: PathBuf) {
        let this = self.clone();
        let spawned = thread::Builder::new().name("library-scan".into()).spawn(move || {
            let library = Library { skills: library::skills(&repo), servers: library::mcp_servers(&repo) };
            if let Ok(mut entries) = this.entries.lock() {
                entries.insert(repo, Entry { scanned_at: Instant::now(), library: Some(Arc::new(library)) });
            }
            this.ctx.request_repaint();
        });
        if let Err(err) = spawned {
            log::error!("Failed to start library scan: {err}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_once_then_serves_the_cached_library_until_invalidated() {
        let cache = LibraryCache::new(egui::Context::default());
        let repo = std::env::temp_dir().join(format!("repodeck-libcache-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&repo).unwrap();
        assert!(cache.get(&repo).is_none(), "nothing before the first scan lands");
        let deadline = Instant::now() + Duration::from_secs(5);
        while cache.get(&repo).is_none() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        let first = cache.get(&repo).expect("scan finished");
        assert!(Arc::ptr_eq(&first, &cache.get(&repo).unwrap()), "no rescan within the refresh window");
        cache.invalidate();
        let _ = cache.get(&repo);
        let stale_served = cache.entries.lock().unwrap()[&repo].library.clone().unwrap();
        assert!(Arc::ptr_eq(&first, &stale_served), "the old library is served while rescanning");
        std::fs::remove_dir_all(&repo).unwrap();
    }
}
