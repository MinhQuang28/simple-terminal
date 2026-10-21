use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use uuid::Uuid;

use crate::store;

const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Append-only file a pane's child processes write lines to, tailed by the app.
pub struct EventLog {
    path: PathBuf,
    offset: u64,
    checked: Instant,
}

impl EventLog {
    pub fn create(kind: &str, pane: Uuid) -> Option<Self> {
        let dir = store::data_dir()?.join(kind);
        fs::create_dir_all(&dir).ok()?;
        let path = dir.join(format!("{pane}.log"));
        fs::write(&path, b"").ok()?;
        Some(Self { path, offset: 0, checked: Instant::now() })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Lines appended since the last poll. A writer may still be mid-line; that line is returned
    /// once its newline lands.
    pub fn poll_lines(&mut self) -> Vec<String> {
        if self.checked.elapsed() < POLL_INTERVAL {
            return Vec::new();
        }
        self.checked = Instant::now();
        self.read_new().unwrap_or_default()
    }

    fn read_new(&mut self) -> Option<Vec<String>> {
        let len = fs::metadata(&self.path).ok()?.len();
        if len <= self.offset {
            return None;
        }
        let mut file = File::open(&self.path).ok()?;
        file.seek(SeekFrom::Start(self.offset)).ok()?;
        let mut text = String::new();
        file.read_to_string(&mut text).ok()?;
        let complete = text.rfind('\n').map_or(0, |i| i + 1);
        self.offset += complete as u64;
        Some(text[..complete].lines().filter(|l| !l.is_empty()).map(str::to_string).collect())
    }
}

impl Drop for EventLog {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
