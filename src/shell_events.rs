use std::path::PathBuf;
use std::time::{Duration, Instant};

use uuid::Uuid;

use crate::event_log::EventLog;

/// Commands that ran at least this long are reported when they finish.
const LONG_COMMAND: Duration = Duration::from_secs(15);
const MAX_MARKS: usize = 2000;

/// A prompt the shell printed, recorded by the grid line it appeared on.
#[derive(Clone, Debug)]
pub struct Mark {
    /// Line counted from the top of scrollback at the time of recording.
    pub line: usize,
}

/// A command that finished, for notifications and the exit badge.
#[derive(Clone, Debug)]
pub struct Finished {
    pub command: String,
    pub exit_code: i32,
    pub duration: Duration,
    pub at: Instant,
}

/// Tracks prompts, running commands and the working directory from shell hook events.
pub struct ShellEvents {
    log: EventLog,
    marks: Vec<Mark>,
    running: Option<(f64, String)>,
    pub cwd: Option<PathBuf>,
    pub last_finished: Option<Finished>,
    long_finished: Option<Finished>,
}

impl ShellEvents {
    pub fn create(pane: Uuid) -> Option<Self> {
        Some(Self {
            log: EventLog::create("shell-events", pane)?,
            marks: Vec::new(),
            running: None,
            cwd: None,
            last_finished: None,
            long_finished: None,
        })
    }

    pub fn path(&self) -> &std::path::Path {
        self.log.path()
    }

    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }

    /// Applies new events; `prompt_line` is where the next prompt is being drawn right now.
    /// Returns whether the shell printed a prompt.
    pub fn poll(&mut self, prompt_line: impl Fn() -> usize) -> bool {
        let mut prompted = false;
        for line in self.log.poll_lines() {
            let mut fields = line.split('\t');
            match (fields.next(), fields.next().and_then(|t| t.parse::<f64>().ok())) {
                (Some("S"), Some(started)) => {
                    self.running = Some((started, fields.next().unwrap_or_default().to_string()));
                }
                (Some("E"), Some(ended)) => {
                    let exit_code = fields.next().and_then(|c| c.parse().ok()).unwrap_or(0);
                    if let Some(cwd) = fields.next().filter(|c| !c.is_empty()) {
                        self.cwd = Some(PathBuf::from(cwd));
                    }
                    if let Some((started, command)) = self.running.take() {
                        let duration = Duration::from_secs_f64((ended - started).max(0.0));
                        let finished = Finished { command, exit_code, duration, at: Instant::now() };
                        if duration >= LONG_COMMAND {
                            self.long_finished = Some(finished.clone());
                        }
                        self.last_finished = Some(finished);
                    }
                    self.marks.push(Mark { line: prompt_line() });
                    if self.marks.len() > MAX_MARKS {
                        self.marks.remove(0);
                    }
                    prompted = true;
                }
                _ => {}
            }
        }
        prompted
    }

    /// A long-running command that finished since the last call.
    pub fn take_long_finished(&mut self) -> Option<Finished> {
        self.long_finished.take()
    }
}
