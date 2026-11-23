//! The subscription's usage limits and the models on offer, asked of Claude Code directly so they
//! show before any session reports them.

use std::io::{Read, Write};
use std::process::Stdio;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui;
use serde_json::{Value, json};

use crate::claude_models::{self, ModelOption, claude_command};
use crate::claude_status_line::{Limit, StatusLine, Window, now};

/// How often the limits are asked for again while the app stays open.
const REFRESH: Duration = Duration::from_secs(10 * 60);
/// How long Claude gets to answer before it is taken to hang, e.g. on a dead network.
const TIMEOUT: Duration = Duration::from_secs(60);
/// A Claude that answers control requests and exits, leaving no conversation behind.
const ARGS: &[&str] = &[
    "-p",
    "--input-format",
    "stream-json",
    "--output-format",
    "stream-json",
    "--verbose",
    "--no-session-persistence",
];

/// Keeps the account's limits current on a background thread.
pub struct UsageLimits {
    latest: Arc<Mutex<Option<StatusLine>>>,
    models: Arc<Mutex<Vec<ModelOption>>>,
    wake: Sender<()>,
}

impl UsageLimits {
    /// Asks now, then every [`REFRESH`], running `claude` through `shell`'s environment.
    pub fn spawn(ctx: egui::Context, shell: String, hide_api_keys: bool) -> Self {
        let latest = Arc::new(Mutex::new(None));
        let (wake, woken) = mpsc::channel();
        let models = Arc::new(Mutex::new(Vec::new()));
        let (slot, offered) = (latest.clone(), models.clone());
        let spawned = thread::Builder::new().name("usage-limits".into()).spawn(move || {
            loop {
                match fetch(&shell, hide_api_keys) {
                    Ok(answer) => {
                        // No limits: an API key or a plan without them.
                        if let (Some(limits), Ok(mut slot)) = (answer.limits, slot.lock()) {
                            *slot = Some(limits);
                        }
                        if let (false, Ok(mut offered)) = (answer.models.is_empty(), offered.lock()) {
                            *offered = answer.models;
                        }
                        ctx.request_repaint();
                    }
                    Err(err) => log::warn!("Could not ask Claude for its usage limits: {err}"),
                }
                if woken.recv_timeout(REFRESH) == Err(RecvTimeoutError::Disconnected) {
                    return;
                }
            }
        });
        if let Err(err) = spawned {
            log::error!("Failed to start the usage limits thread: {err}");
        }
        Self { latest, models, wake }
    }

    pub fn latest(&self) -> Option<StatusLine> {
        self.latest.lock().ok()?.clone()
    }

    /// The models Claude offers; empty until it has answered.
    pub fn models(&self) -> Vec<ModelOption> {
        self.models.lock().map(|models| models.clone()).unwrap_or_default()
    }

    /// Asks again now rather than at the next interval.
    pub fn refresh(&self) {
        let _ = self.wake.send(());
    }
}

/// What one asking gave.
struct Answer {
    limits: Option<StatusLine>,
    models: Vec<ModelOption>,
}

fn fetch(shell: &str, hide_api_keys: bool) -> std::io::Result<Answer> {
    let mut command = claude_command(shell, hide_api_keys, ARGS.iter().copied());
    if let Some(home) = std::env::var_os("HOME") {
        command.current_dir(home);
    }
    let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?;
    let request = |subtype: &str| json!({"type": "control_request", "request_id": subtype, "request": {"subtype": subtype}});
    // Closing the pipe after the requests lets Claude answer and exit.
    if let Some(mut stdin) = child.stdin.take() {
        writeln!(stdin, "{}\n{}", request("initialize"), request("get_usage"))?;
    }
    // Read on a thread of its own, so a Claude that hangs can be given up on.
    let stdout = child.stdout.take();
    let reader = thread::Builder::new().name("usage-answer".into()).spawn(move || {
        let mut answers = String::new();
        if let Some(mut stdout) = stdout {
            let _ = stdout.read_to_string(&mut answers);
        }
        answers
    })?;
    let deadline = Instant::now() + TIMEOUT;
    while child.try_wait()?.is_none() {
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "Claude did not answer"));
        }
        thread::sleep(Duration::from_millis(100));
    }
    let answers = reader.join().unwrap_or_default();
    let mut answer = Answer { limits: None, models: Vec::new() };
    for line in answers.lines().filter(|l| l.contains("control_response")) {
        let Ok(value) = serde_json::from_str::<Value>(line) else { continue };
        let response = &value["response"]["response"];
        answer.limits = answer.limits.or_else(|| parse(response, now()));
        if answer.models.is_empty() {
            answer.models = claude_models::parse(response);
        }
    }
    Ok(answer)
}

/// The limits in a `get_usage` answer; `None` when the account reports none.
fn parse(usage: &Value, reported_at: u64) -> Option<StatusLine> {
    let mut status = StatusLine { reported_at, ..Default::default() };
    for entry in usage["rate_limits"]["limits"].as_array().into_iter().flatten() {
        let (Some(percent), Some(resets)) = (entry["percent"].as_f64(), entry["resets_at"].as_str()) else {
            continue;
        };
        let Some(resets_at) = unix_seconds(resets) else { continue };
        let limit = Limit { used_percent: percent, resets_at };
        let model = entry["scope"]["model"]["display_name"].as_str();
        match (entry["group"].as_str(), model) {
            (Some("session"), None) => status.five_hour = Some(limit),
            (Some("weekly"), None) => status.seven_day = Some(limit),
            (Some("weekly"), Some(model)) => {
                let key = format!("seven_day_{}", model.to_lowercase().replace(' ', "_"));
                status.others.push(Window { key, limit });
            }
            (group, _) => {
                let key = entry["kind"].as_str().or(group).unwrap_or("limit").to_string();
                status.others.push(Window { key, limit });
            }
        }
    }
    status.has_limits().then_some(status)
}

/// Unix seconds of an RFC 3339 time such as `2026-11-26T13:00:00.815455+00:00`.
fn unix_seconds(time: &str) -> Option<u64> {
    let number = |range: std::ops::Range<usize>| time.get(range)?.parse::<i64>().ok();
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);
    // The offset follows the seconds and their fraction: "Z", "+07:00" or "-05:30".
    let zone = time.get(19..)?.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    let offset = match zone.as_bytes().first()? {
        b'Z' | b'z' => 0,
        sign @ (b'+' | b'-') => {
            let minutes = zone.get(1..3)?.parse::<i64>().ok()? * 60 + zone.get(4..6)?.parse::<i64>().ok()?;
            if *sign == b'+' { minutes * 60 } else { -minutes * 60 }
        }
        _ => return None,
    };
    // Days since 1970 of a civil date, counting years from March so leap days fall at the end.
    let years = if month <= 2 { year - 1 } else { year };
    let era = years.div_euclid(400);
    let year_of_era = years.rem_euclid(400);
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    u64::try_from(days * 86_400 + hour * 3_600 + minute * 60 + second - offset).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_rfc_3339_times() {
        assert_eq!(unix_seconds("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(unix_seconds("2026-11-26T13:00:00.815455+00:00"), Some(1_795_698_000));
        assert_eq!(unix_seconds("2026-11-26T20:00:00+07:00"), Some(1_795_698_000));
        assert_eq!(unix_seconds("2024-02-29T23:59:59-00:30"), Some(1_709_252_999));
        assert_eq!(unix_seconds("soon"), None);
    }

    #[test]
    fn reads_the_limits_of_a_usage_answer() {
        let usage = json!({"rate_limits": {"limits": [
            {"kind": "weekly_all", "group": "weekly", "percent": 30, "resets_at": "2026-11-26T13:00:00+00:00", "scope": null},
            {"kind": "weekly_scoped", "group": "weekly", "percent": 27, "resets_at": "2026-11-26T13:00:00+00:00",
             "scope": {"model": {"id": null, "display_name": "Fable"}}},
            {"kind": "session", "group": "session", "percent": 5, "resets_at": "2026-11-23T13:00:00+00:00", "scope": null},
            {"kind": "broken", "group": "weekly", "percent": 1, "resets_at": null}
        ]}});
        let status = parse(&usage, 100).expect("limits");
        assert_eq!(status.seven_day, Some(Limit { used_percent: 30.0, resets_at: 1_795_698_000 }));
        assert_eq!(status.five_hour.map(|l| l.used_percent), Some(5.0));
        let labels: Vec<String> = status.windows().iter().map(Window::label).collect();
        assert_eq!(labels, ["Session", "Weekly", "Weekly · Fable"]);
        assert_eq!(parse(&json!({"rate_limits": {"limits": []}}), 100), None);
    }
}
