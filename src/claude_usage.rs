use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, UNIX_EPOCH};

use eframe::egui;
use serde_json::Value;

use crate::claude_history::transcripts_dir;

/// How long a transcript's size and mtime are trusted before being checked again.
const RECHECK_AFTER: Duration = Duration::from_secs(30);

/// Tokens a Claude Code conversation has consumed, summed over its assistant turns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    pub input: u64,
    pub cache_write: u64,
    pub cache_read: u64,
    pub output: u64,
}

impl Usage {
    /// Everything the model read, including cached prompt tokens.
    pub fn total_input(&self) -> u64 {
        self.input + self.cache_write + self.cache_read
    }

    /// Short form for the status bar, e.g. "↑ 1.2M ↓ 34k".
    pub fn summary(&self) -> String {
        format!("↑ {} ↓ {}", compact(self.total_input()), compact(self.output))
    }

    pub fn breakdown(&self) -> String {
        format!(
            "input {} · cache write {} · cache read {} · output {}",
            compact(self.input),
            compact(self.cache_write),
            compact(self.cache_read),
            compact(self.output)
        )
    }
}

pub fn compact(n: u64) -> String {
    let scaled = |value: f64, unit: &str| {
        let text = format!("{value:.1}");
        format!("{}{unit}", text.trim_end_matches(".0"))
    };
    match n {
        0..1_000 => n.to_string(),
        1_000..1_000_000 => scaled(n as f64 / 1_000.0, "k"),
        _ => scaled(n as f64 / 1_000_000.0, "M"),
    }
}

/// Running totals of a transcript, so appended turns are read without rescanning the file.
#[derive(Clone, Default)]
pub struct Progress {
    /// Bytes of the transcript already consumed.
    offset: u64,
    per_message: HashMap<String, Usage>,
    anonymous: Usage,
}

impl Progress {
    /// Reads turns appended since the last call. Streaming writes one entry per content block,
    /// all carrying the message's usage, so the last entry per message id counts. A transcript
    /// that shrank is read from the start again.
    pub fn advance(&mut self, path: &Path) -> Option<()> {
        let mut file = File::open(path).ok()?;
        let len = file.metadata().ok()?.len();
        if len < self.offset {
            *self = Self::default();
        }
        file.seek(SeekFrom::Start(self.offset)).ok()?;
        let mut reader = BufReader::new(file);
        let mut line = String::new();
        loop {
            line.clear();
            let read = reader.read_line(&mut line).ok()?;
            if read == 0 || !line.ends_with('\n') {
                break;
            }
            self.offset += read as u64;
            self.absorb(&line);
        }
        Some(())
    }

    fn absorb(&mut self, line: &str) {
        if !line.contains(r#""type":"assistant""#) {
            return;
        }
        let Ok(entry) = serde_json::from_str::<Value>(line) else { return };
        let message = &entry["message"];
        let Some(usage) = parse_usage(&message["usage"]) else { return };
        match message["id"].as_str() {
            Some(id) => {
                self.per_message.insert(id.to_string(), usage);
            }
            None => self.anonymous = add(self.anonymous, usage),
        }
    }

    pub fn total(&self) -> Usage {
        self.per_message.values().fold(self.anonymous, |acc, u| add(acc, *u))
    }
}

fn parse_usage(value: &Value) -> Option<Usage> {
    let field = |name: &str| value.get(name).and_then(Value::as_u64).unwrap_or(0);
    value.is_object().then(|| Usage {
        input: field("input_tokens"),
        cache_write: field("cache_creation_input_tokens"),
        cache_read: field("cache_read_input_tokens"),
        output: field("output_tokens"),
    })
}

fn add(a: Usage, b: Usage) -> Usage {
    Usage {
        input: a.input + b.input,
        cache_write: a.cache_write + b.cache_write,
        cache_read: a.cache_read + b.cache_read,
        output: a.output + b.output,
    }
}

struct Entry {
    checked_at: Instant,
    /// Size and mtime of the transcript the cached usage was read from; zero before it exists.
    stamp: (u64, u64),
    /// Taken by the reader thread while it works, so a read is never started twice.
    progress: Option<Progress>,
    usage: Option<Usage>,
}

/// Usage per transcript, read incrementally on a background thread when the file grows.
#[derive(Clone)]
pub struct UsageCache {
    entries: Arc<Mutex<HashMap<PathBuf, Entry>>>,
    ctx: egui::Context,
}

impl UsageCache {
    pub fn new(ctx: egui::Context) -> Self {
        Self { entries: Arc::default(), ctx }
    }

    /// Usage of `session` in `repo`; `None` until the transcript has been read once.
    pub fn usage(&self, repo: &Path, session: &str) -> Option<Usage> {
        let path = transcripts_dir(repo)?.join(format!("{session}.jsonl"));
        let Ok(mut entries) = self.entries.lock() else { return None };
        let entry = entries.entry(path.clone()).or_insert_with(|| Entry {
            checked_at: Instant::now() - RECHECK_AFTER,
            stamp: (0, 0),
            progress: Some(Progress::default()),
            usage: None,
        });
        if entry.checked_at.elapsed() >= RECHECK_AFTER {
            entry.checked_at = Instant::now();
            let stamp = stamp(&path).unwrap_or((0, 0));
            if stamp != entry.stamp
                && stamp != (0, 0)
                && let Some(progress) = entry.progress.take()
            {
                entry.stamp = stamp;
                self.spawn_read(path, progress);
            }
        }
        entry.usage
    }

    fn spawn_read(&self, path: PathBuf, mut progress: Progress) {
        let this = self.clone();
        let spawned = thread::Builder::new().name("claude-usage".into()).spawn(move || {
            progress.advance(&path);
            let usage = progress.total();
            if let Ok(mut entries) = this.entries.lock()
                && let Some(entry) = entries.get_mut(&path)
            {
                entry.progress = Some(progress);
                entry.usage = Some(usage);
            }
            this.ctx.request_repaint();
        });
        if let Err(err) = spawned {
            log::error!("Failed to start Claude usage read: {err}");
        }
    }
}

fn stamp(path: &Path) -> Option<(u64, u64)> {
    let meta = path.metadata().ok()?;
    let modified = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some((meta.len(), modified))
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn sums_the_last_usage_per_message_id() {
        let path = std::env::temp_dir().join(format!("repodeck-usage-{}.jsonl", uuid::Uuid::new_v4()));
        let mut file = File::create(&path).unwrap();
        let lines = [
            r#"{"type":"user","message":{"role":"user","content":"hi"}}"#,
            r#"{"type":"assistant","message":{"id":"m1","usage":{"input_tokens":10,"cache_creation_input_tokens":5,"cache_read_input_tokens":100,"output_tokens":1}}}"#,
            r#"{"type":"assistant","message":{"id":"m1","usage":{"input_tokens":10,"cache_creation_input_tokens":5,"cache_read_input_tokens":100,"output_tokens":7}}}"#,
            r#"{"type":"assistant","message":{"id":"m2","usage":{"input_tokens":2,"output_tokens":3}}}"#,
            r#"{"type":"assistant","message":{"usage":{"input_tokens":1,"output_tokens":1}}}"#,
            "not json",
        ];
        for line in lines {
            writeln!(file, "{line}").unwrap();
        }
        let mut progress = Progress::default();
        progress.advance(&path);
        assert_eq!(progress.total(), Usage { input: 13, cache_write: 5, cache_read: 100, output: 11 });
        assert_eq!(progress.total().total_input(), 118);
        writeln!(
            file,
            r#"{{"type":"assistant","message":{{"id":"m3","usage":{{"input_tokens":1,"output_tokens":1}}}}}}"#
        )
        .unwrap();
        write!(file, r#"{{"type":"assistant","message":{{"id":"m4","usage":{{"input_tokens":50"#).unwrap();
        progress.advance(&path);
        assert_eq!(progress.total(), Usage { input: 14, cache_write: 5, cache_read: 100, output: 12 });
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn formats_compact_counts() {
        assert_eq!(compact(999), "999");
        assert_eq!(compact(1_000), "1k");
        assert_eq!(compact(34_500), "34.5k");
        assert_eq!(compact(1_200_000), "1.2M");
    }
}
