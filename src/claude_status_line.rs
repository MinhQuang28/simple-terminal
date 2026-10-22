use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use uuid::Uuid;

use crate::store;

/// Environment variable naming the file Claude Code's status line command writes its JSON to.
pub const STATUS_VAR: &str = "REPODECK_AGENT_STATUS";
const POLL_INTERVAL: Duration = Duration::from_secs(1);

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

/// A usage window with the name Claude Code reports it under, e.g. `seven_day_opus`.
#[derive(Clone, Debug, PartialEq)]
pub struct Window {
    pub key: String,
    pub limit: Limit,
}

impl Window {
    /// "Session", "Weekly", or "Weekly · Opus" for a per-model weekly window.
    pub fn label(&self) -> String {
        match self.key.as_str() {
            "five_hour" => "Session".into(),
            "seven_day" => "Weekly".into(),
            key => match key.strip_prefix("seven_day_") {
                Some(model) => format!("Weekly · {}", capitalize(&model.replace('_', " "))),
                None => capitalize(&key.replace('_', " ")),
            },
        }
    }
}

/// What Claude Code last reported through its status line. Limits are only present for Pro/Max
/// subscriptions, after the session's first response.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StatusLine {
    pub five_hour: Option<Limit>,
    pub seven_day: Option<Limit>,
    /// Further windows, such as per-model weekly limits, in the order reported.
    pub others: Vec<Window>,
    pub context_percent: Option<f64>,
    /// Unix seconds.
    pub reported_at: u64,
}

impl StatusLine {
    fn parse(json: &Value, reported_at: u64) -> Self {
        let limit = |window: &Value| {
            Some(Limit {
                used_percent: window["used_percentage"].as_f64()?,
                resets_at: window["resets_at"].as_u64()?,
            })
        };
        let windows = json["rate_limits"].as_object();
        let others = windows
            .into_iter()
            .flatten()
            .filter(|(key, _)| !matches!(key.as_str(), "five_hour" | "seven_day"))
            .filter_map(|(key, window)| Some(Window { key: key.clone(), limit: limit(window)? }))
            .collect();
        Self {
            five_hour: limit(&json["rate_limits"]["five_hour"]),
            seven_day: limit(&json["rate_limits"]["seven_day"]),
            others,
            context_percent: json["context_window"]["used_percentage"].as_f64(),
            reported_at,
        }
    }

    pub fn has_limits(&self) -> bool {
        self.five_hour.is_some() || self.seven_day.is_some() || !self.others.is_empty()
    }

    /// Every window in display order: session, weekly, then the rest.
    pub fn windows(&self) -> Vec<Window> {
        let named = |key: &str, limit: Option<Limit>| limit.map(|limit| Window { key: key.into(), limit });
        named("five_hour", self.five_hour)
            .into_iter()
            .chain(named("seven_day", self.seven_day))
            .chain(self.others.iter().cloned())
            .collect()
    }

    /// The largest share used across windows now.
    pub fn peak_used(&self) -> f64 {
        let now = now();
        self.windows().iter().map(|w| w.limit.used_at(now)).fold(0.0, f64::max)
    }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
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
        self.latest.clone()
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
        if let Some(previous) = self.latest.take() {
            status.five_hour = status.five_hour.or(previous.five_hour);
            status.seven_day = status.seven_day.or(previous.seven_day);
            if status.others.is_empty() {
                status.others = previous.others;
            }
        }
        self.latest = Some(status);
    }
}

impl Drop for StatusFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub(crate) fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// e.g. "45m", "2h 13m", "6d 3h".
pub fn duration(secs: u64) -> String {
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
    fn keeps_per_model_windows_with_readable_labels() {
        let json: Value = serde_json::from_str(
            r#"{"rate_limits":{"seven_day":{"used_percentage":7,"resets_at":900},
                "seven_day_opus":{"used_percentage":11,"resets_at":900},"overage":{"enabled":true}}}"#,
        )
        .unwrap();
        let status = StatusLine::parse(&json, 100);
        let labels: Vec<String> = status.windows().iter().map(Window::label).collect();
        assert_eq!(labels, ["Weekly", "Weekly · Opus"]);
        assert_eq!(status.peak_used(), 11.0);
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
