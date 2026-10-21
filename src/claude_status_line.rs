use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use uuid::Uuid;

use crate::store;

/// Environment variable naming the file Claude Code's status line command writes its JSON to.
pub const STATUS_VAR: &str = "REPODECK_AGENT_STATUS";
const POLL_INTERVAL: Duration = Duration::from_secs(1);
/// Usage at or above this share of a limit is highlighted.
const HIGH_USAGE: f64 = 80.0;

/// The `statusLine` setting for `claude --settings`: saves Claude Code's status JSON to the file
/// named by `STATUS_VAR`, then pipes it to the user's own status line command so that still shows.
pub fn settings(cwd: &Path) -> Value {
    let mut status_line = user_status_line(cwd).unwrap_or_else(|| json!({ "type": "command" }));
    let command = match status_line["command"].as_str() {
        Some(own) => format!("tee \"${STATUS_VAR}\" | ({own})"),
        None => format!("cat > \"${STATUS_VAR}\""),
    };
    status_line["command"] = command.into();
    status_line
}

/// The status line the user configured, by Claude Code's precedence: local, project, then user.
fn user_status_line(cwd: &Path) -> Option<Value> {
    let home = std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".claude/settings.json"));
    [cwd.join(".claude/settings.local.json"), cwd.join(".claude/settings.json")]
        .into_iter()
        .chain(home)
        .filter_map(|file| serde_json::from_str::<Value>(&fs::read_to_string(file).ok()?).ok())
        .find_map(|settings| settings.get("statusLine").filter(|s| s["type"] == "command").cloned())
}

/// One of the subscription's rolling usage windows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limit {
    pub used_percent: f64,
    /// Unix seconds.
    pub resets_at: u64,
}

impl Limit {
    /// Share used now: nothing once the window has reset since Claude Code reported it.
    pub fn used_at(&self, now: u64) -> f64 {
        if now >= self.resets_at { 0.0 } else { self.used_percent }
    }
}

/// What Claude Code last reported through its status line. Limits are only present for Pro/Max
/// subscriptions, after the session's first response.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StatusLine {
    pub five_hour: Option<Limit>,
    pub seven_day: Option<Limit>,
    pub context_percent: Option<f64>,
    /// Unix seconds.
    pub reported_at: u64,
}

impl StatusLine {
    fn parse(json: &Value, reported_at: u64) -> Self {
        let limit = |key: &str| {
            let window = &json["rate_limits"][key];
            Some(Limit {
                used_percent: window["used_percentage"].as_f64()?,
                resets_at: window["resets_at"].as_u64()?,
            })
        };
        Self {
            five_hour: limit("five_hour"),
            seven_day: limit("seven_day"),
            context_percent: json["context_window"]["used_percentage"].as_f64(),
            reported_at,
        }
    }

    pub fn has_limits(&self) -> bool {
        self.five_hour.is_some() || self.seven_day.is_some()
    }

    /// Short form for the status bar, e.g. "5h 15% · 7d 2%".
    pub fn summary(&self) -> String {
        let now = now();
        let windows = [("5h", self.five_hour), ("7d", self.seven_day)];
        let parts: Vec<String> = windows
            .iter()
            .filter_map(|(name, l)| Some(format!("{name} {:.0}%", l.as_ref()?.used_at(now))))
            .collect();
        parts.join(" · ")
    }

    pub fn details(&self) -> String {
        let now = now();
        let windows = [("5-hour limit", self.five_hour), ("Weekly limit", self.seven_day)];
        let lines: Vec<String> = windows
            .iter()
            .filter_map(|(name, limit)| {
                let limit = limit.as_ref()?;
                let resets = limit.resets_at.saturating_sub(now);
                Some(format!("{name}: {:.0}% used · resets in {}", limit.used_at(now), duration(resets)))
            })
            .collect();
        lines.join("\n")
    }

    pub fn is_high(&self) -> bool {
        let now = now();
        [self.five_hour, self.seven_day].iter().flatten().any(|l| l.used_at(now) >= HIGH_USAGE)
    }
}

/// The file a pane's Claude Code status line writes to, re-read when it changes.
pub struct StatusFile {
    path: PathBuf,
    checked: Instant,
    modified: Option<SystemTime>,
    latest: Option<StatusLine>,
}

impl StatusFile {
    pub fn create(pane: Uuid) -> Option<Self> {
        let dir = store::data_dir()?.join("agent-status");
        fs::create_dir_all(&dir).ok()?;
        let path = dir.join(format!("{pane}.json"));
        let _ = fs::remove_file(&path);
        Some(Self { path, checked: Instant::now(), modified: None, latest: None })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn latest(&self) -> Option<StatusLine> {
        self.latest
    }

    /// Reads the file if it changed. A half-written file fails to parse and is read again later.
    pub fn poll(&mut self) {
        if self.checked.elapsed() < POLL_INTERVAL {
            return;
        }
        self.checked = Instant::now();
        let Some(modified) = fs::metadata(&self.path).and_then(|m| m.modified()).ok() else { return };
        if self.modified == Some(modified) {
            return;
        }
        let Some(json) = fs::read_to_string(&self.path).ok().and_then(|t| serde_json::from_str(&t).ok())
        else {
            return;
        };
        self.modified = Some(modified);
        let mut status = StatusLine::parse(&json, now());
        // A report without limits doesn't mean they went away.
        if let Some(previous) = self.latest {
            status.five_hour = status.five_hour.or(previous.five_hour);
            status.seven_day = status.seven_day.or(previous.seven_day);
        }
        self.latest = Some(status);
    }
}

impl Drop for StatusFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// e.g. "45m", "2h 13m", "6d 3h".
fn duration(secs: u64) -> String {
    let (days, hours, minutes) = (secs / 86_400, secs / 3_600 % 24, secs / 60 % 60);
    match (days, hours) {
        (0, 0) => format!("{minutes}m"),
        (0, _) => format!("{hours}h {minutes}m"),
        _ => format!("{days}d {hours}h"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_limits_and_context() {
        let json: Value = serde_json::from_str(
            r#"{"rate_limits":{"five_hour":{"used_percentage":15,"resets_at":200},"seven_day":{"used_percentage":2.5,"resets_at":900}},
                "context_window":{"used_percentage":4}}"#,
        )
        .unwrap();
        let status = StatusLine::parse(&json, 100);
        assert_eq!(status.five_hour, Some(Limit { used_percent: 15.0, resets_at: 200 }));
        assert_eq!(status.seven_day.map(|l| l.used_percent), Some(2.5));
        assert_eq!(status.context_percent, Some(4.0));
        assert!(status.has_limits());
        assert!(!StatusLine::parse(&json!({}), 100).has_limits());
    }

    #[test]
    fn a_reset_window_counts_as_unused() {
        let limit = Limit { used_percent: 90.0, resets_at: 1_000 };
        assert_eq!(limit.used_at(999), 90.0);
        assert_eq!(limit.used_at(1_000), 0.0);
    }

    #[test]
    fn wraps_the_users_own_status_line() {
        let dir = std::env::temp_dir().join(format!("repodeck-statusline-{}", Uuid::new_v4()));
        fs::create_dir_all(dir.join(".claude")).unwrap();
        fs::write(
            dir.join(".claude/settings.json"),
            r#"{"statusLine":{"type":"command","command":"~/bin/line.sh","padding":0}}"#,
        )
        .unwrap();
        let wrapped = settings(&dir);
        assert_eq!(wrapped["command"], format!("tee \"${STATUS_VAR}\" | (~/bin/line.sh)"));
        assert_eq!(wrapped["padding"], 0);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn formats_durations() {
        assert_eq!(duration(45 * 60), "45m");
        assert_eq!(duration(2 * 3_600 + 13 * 60), "2h 13m");
        assert_eq!(duration(6 * 86_400 + 3 * 3_600), "6d 3h");
    }
}
