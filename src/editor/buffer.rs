use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result};
use egui_commonmark::CommonMarkCache;

use super::find_bar::EditorBar;

/// Files larger than this open read-only to keep editing and highlighting responsive.
const MAX_EDITABLE_BYTES: u64 = 2 * 1024 * 1024;
/// How often an open file is checked for changes on disk.
const DISK_CHECK_INTERVAL: Duration = Duration::from_secs(1);

pub enum Content {
    Text(String),
    /// Why the file can't be shown as text.
    Unavailable(String),
}

pub struct Buffer {
    pub content: Content,
    saved: String,
    modified_at: Option<SystemTime>,
    checked_at: Instant,
    pub save_error: Option<String>,
    /// Shows rendered Markdown instead of the text editor.
    pub preview: bool,
    pub markdown: CommonMarkCache,
    /// Line-number column for the current line count.
    gutter: (usize, String),
    /// Find or go-to-line bar, while open.
    pub bar: Option<EditorBar>,
}

/// Whether `path` is a Markdown document, which opens in preview.
pub fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"))
}

impl Buffer {
    fn load(path: &Path) -> Self {
        let modified_at = fs::metadata(path).and_then(|m| m.modified()).ok();
        let content = match read_text(path) {
            Ok(text) => Content::Text(text),
            Err(err) => Content::Unavailable(format!("{err:#}")),
        };
        let saved = match &content {
            Content::Text(text) => text.clone(),
            Content::Unavailable(_) => String::new(),
        };
        Self {
            content,
            saved,
            modified_at,
            checked_at: Instant::now(),
            save_error: None,
            preview: is_markdown(path),
            markdown: CommonMarkCache::default(),
            gutter: (0, String::new()),
            bar: None,
        }
    }

    /// Right-aligned line numbers, one per line, rebuilt only when the line count changes.
    pub fn gutter(&mut self, lines: usize) -> &str {
        if self.gutter.0 != lines {
            let digits = lines.to_string().len();
            let text: String = (1..=lines).map(|n| format!("{n:>digits$}\n")).collect();
            self.gutter = (lines, text.trim_end().to_string());
        }
        &self.gutter.1
    }

    pub fn is_dirty(&self) -> bool {
        matches!(&self.content, Content::Text(text) if *text != self.saved)
    }

    pub fn save(&mut self, path: &Path) {
        let Content::Text(text) = &self.content else { return };
        match fs::write(path, text) {
            Ok(()) => {
                self.saved = text.clone();
                self.modified_at = fs::metadata(path).and_then(|m| m.modified()).ok();
                self.save_error = None;
            }
            Err(err) => self.save_error = Some(format!("Save failed: {err}")),
        }
    }
}

fn read_text(path: &Path) -> Result<String> {
    let size = fs::metadata(path).with_context(|| format!("Cannot open {}", path.display()))?.len();
    if size > MAX_EDITABLE_BYTES {
        anyhow::bail!("File is too large to edit ({:.1} MB)", size as f64 / 1_048_576.0);
    }
    let bytes = fs::read(path).with_context(|| format!("Cannot read {}", path.display()))?;
    if bytes.iter().take(8192).any(|b| *b == 0) {
        anyhow::bail!("Binary file");
    }
    String::from_utf8(bytes).context("File is not valid UTF-8")
}

/// Open file contents keyed by absolute path, shared by all tabs showing the same file.
#[derive(Default)]
pub struct Buffers {
    open: HashMap<PathBuf, Buffer>,
}

impl Buffers {
    /// Returns the buffer, loading it on first use and reloading clean buffers changed on disk.
    pub fn get(&mut self, path: &Path) -> &mut Buffer {
        let reload = match self.open.get_mut(path) {
            None => true,
            Some(b) if b.is_dirty() || b.checked_at.elapsed() < DISK_CHECK_INTERVAL => false,
            Some(b) => {
                b.checked_at = Instant::now();
                b.modified_at != fs::metadata(path).and_then(|m| m.modified()).ok()
            }
        };
        if reload {
            self.open.insert(path.to_path_buf(), Buffer::load(path));
        }
        self.open.get_mut(path).expect("buffer inserted above")
    }

    pub fn is_dirty(&self, path: &Path) -> bool {
        self.open.get(path).is_some_and(Buffer::is_dirty)
    }

    pub fn dirty_count(&self) -> usize {
        self.open.values().filter(|b| b.is_dirty()).count()
    }

    pub fn save_all(&mut self) {
        for (path, buffer) in &mut self.open {
            if buffer.is_dirty() {
                buffer.save(path);
            }
        }
    }

    pub fn save(&mut self, path: &Path) {
        if let Some(buffer) = self.open.get_mut(path) {
            buffer.save(path);
        }
    }

    pub fn close(&mut self, path: &Path) {
        self.open.remove(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_dirty_state_and_saves() {
        let path = std::env::temp_dir().join(format!("repodeck-buf-{}.txt", uuid::Uuid::new_v4()));
        fs::write(&path, "hello").unwrap();
        let mut buffers = Buffers::default();
        let Content::Text(text) = &mut buffers.get(&path).content else { panic!("expected text") };
        text.push_str(" world");
        assert!(buffers.is_dirty(&path));
        buffers.save(&path);
        assert!(!buffers.is_dirty(&path));
        assert_eq!(fs::read_to_string(&path).unwrap(), "hello world");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn refuses_binary_files() {
        let path = std::env::temp_dir().join(format!("repodeck-bin-{}", uuid::Uuid::new_v4()));
        fs::write(&path, [0u8, 1, 2]).unwrap();
        let mut buffers = Buffers::default();
        assert!(matches!(&buffers.get(&path).content, Content::Unavailable(msg) if msg == "Binary file"));
        fs::remove_file(path).unwrap();
    }
}
