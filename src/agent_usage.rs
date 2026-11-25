//! Usage that agents other than Claude Code report: Antigravity's quota, which comes with its
//! status line, and Codex's token counts and limits, which it writes to its session file.

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use serde_json::Value;

use crate::claude_status_line::{Limit, StatusLine, Window, now};

const POLL_INTERVAL: Duration = Duration::from_secs(1);
/// The latest token count is among the last records of a session file.
const TAIL_BYTES: u64 = 256 * 1024;
const FIVE_HOURS: u64 = 300;
const SEVEN_DAYS: u64 = 10_080;

/// Puts Antigravity's `quota` (e.g. `gemini-5h`, `3p-weekly`, each with the fraction left) into
/// `status`: Gemini's own windows as the session and weekly limits, the rest beside them.
pub fn apply_antigravity_quota(status: &mut StatusLine, quota: &serde_json::Map<String, Value>) {
    for (key, window) in quota {
        let (Some(left), Some(resets_in)) =
            (window["remaining_fraction"].as_f64(), window["reset_in_seconds"].as_u64())
        else {
            continue;
        };
        let limit = Limit { used_percent: (1.0 - left) * 100.0, resets_at: status.reported_at + resets_in };
        let Some((group, span)) = key.rsplit_once('-') else { continue };
        let span = match span {
            "5h" => "five_hour",
            "weekly" => "seven_day",
            _ => continue,
        };
        match (group, span) {
            ("gemini", "five_hour") => status.five_hour = Some(limit),
            ("gemini", _) => status.seven_day = Some(limit),
            _ => {
                let group = if group == "3p" { "other_models" } else { group };
                status.others.push(Window { key: format!("{span}_{group}"), limit });
            }
        }
    }
}

/// A Codex session file, re-read when it changes for the session's latest token count.
pub struct RolloutUsage {
    path: PathBuf,
    model: Option<String>,
    checked: Instant,
    modified: Option<SystemTime>,
    latest: Option<StatusLine>,
}

impl RolloutUsage {
    /// None unless `path` names a Codex session file.
    pub fn open(path: &str) -> Option<Self> {
        let path = PathBuf::from(path);
        path.file_name()?.to_str()?.starts_with("rollout-").then(|| Self {
            path,
            model: None,
            // The first poll reads right away.
            checked: Instant::now() - POLL_INTERVAL,
            modified: None,
            latest: None,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn set_model(&mut self, model: Option<&str>) {
        if let Some(model) = model {
            self.model = Some(model.to_owned());
        }
    }

    pub fn latest(&self) -> Option<StatusLine> {
        let mut status = self.latest.clone()?;
        status.model = self.model.clone();
        Some(status)
    }

    pub fn poll(&mut self) {
        if self.checked.elapsed() < POLL_INTERVAL {
            return;
        }
        self.checked = Instant::now();
        let Some(modified) = fs::metadata(&self.path).and_then(|m| m.modified()).ok() else { return };
        if self.modified == Some(modified) {
            return;
        }
        self.modified = Some(modified);
        if let Some(status) = tail(&self.path).and_then(|text| parse_rollout(&text, now())) {
            self.latest = Some(status);
        }
    }
}

fn tail(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(TAIL_BYTES))).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// The last token count in a session file's text: context in use and the account's limits.
fn parse_rollout(text: &str, reported_at: u64) -> Option<StatusLine> {
    let count = text
        .lines()
        .rev()
        .filter(|line| line.contains("\"token_count\""))
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|record| record["payload"]["type"] == "token_count")?;
    let payload = &count["payload"];
    let mut status = StatusLine { reported_at, ..Default::default() };
    let used = payload["info"]["last_token_usage"]["total_tokens"].as_f64();
    let size = payload["info"]["model_context_window"].as_f64().filter(|size| *size > 0.0);
    status.context_percent = used.zip(size).map(|(used, size)| (used / size * 100.0).min(100.0));
    for slot in ["primary", "secondary"] {
        let window = &payload["rate_limits"][slot];
        let (Some(used_percent), Some(resets_at)) =
            (window["used_percent"].as_f64(), window["resets_at"].as_u64())
        else {
            continue;
        };
        let limit = Limit { used_percent, resets_at };
        match window["window_minutes"].as_u64() {
            Some(FIVE_HOURS) => status.five_hour = Some(limit),
            Some(SEVEN_DAYS) => status.seven_day = Some(limit),
            minutes => status.others.push(Window { key: window_name(minutes), limit }),
        }
    }
    Some(status)
}

fn window_name(minutes: Option<u64>) -> String {
    match minutes.map(|minutes| minutes / 1_440) {
        Some(28..=31) => "monthly".into(),
        Some(days @ 1..) => format!("{days}_day"),
        _ => "limit".into(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn reads_antigravity_quota() {
        let quota = json!({
            "gemini-5h": { "remaining_fraction": 0.75, "reset_in_seconds": 100 },
            "gemini-weekly": { "remaining_fraction": 0.5, "reset_in_seconds": 200 },
            "3p-5h": { "remaining_fraction": 1.0, "reset_in_seconds": 300 },
        });
        let mut status = StatusLine { reported_at: 1_000, ..Default::default() };
        apply_antigravity_quota(&mut status, quota.as_object().unwrap());
        assert_eq!(status.five_hour, Some(Limit { used_percent: 25.0, resets_at: 1_100 }));
        assert_eq!(status.seven_day, Some(Limit { used_percent: 50.0, resets_at: 1_200 }));
        assert_eq!(status.others[0].label(), "Session · Other models");
    }

    #[test]
    fn reads_the_last_token_count_of_a_codex_session() {
        let record = |used: u64, minutes: u64| {
            json!({ "type": "event_msg", "payload": { "type": "token_count",
                "info": { "last_token_usage": { "total_tokens": used }, "model_context_window": 200_000 },
                "rate_limits": { "primary": { "used_percent": 12.0, "window_minutes": minutes, "resets_at": 99 },
                    "secondary": null } } })
            .to_string()
        };
        let text = format!(
            "{}\n{{\"type\":\"turn_context\"}}\n{}\nhalf a li",
            record(10_000, 300),
            record(50_000, 43_200)
        );
        let status = parse_rollout(&text, 7).unwrap();
        assert_eq!(status.context_percent, Some(25.0));
        assert_eq!(status.five_hour, None);
        assert_eq!(status.others[0].label(), "Monthly");
        assert_eq!(status.others[0].limit, Limit { used_percent: 12.0, resets_at: 99 });
        assert!(parse_rollout("{\"type\":\"session_meta\"}", 7).is_none());
        assert!(RolloutUsage::open("/x/transcript.jsonl").is_none());
        assert!(RolloutUsage::open("/x/rollout-2026.jsonl").is_some());
    }
}
