use alacritty_terminal::vte::ansi::CursorShape;
use eframe::egui::{Key, Modifiers, Pos2};

use super::input::key_to_bytes;
use super::render::Grid;
use super::session::TerminalSession;
use super::snapshot::Frame;

/// Columns Claude Code's prompt box keeps before the text on every row ("❯ " or two spaces).
const TEXT_START: usize = 2;
/// How long each step waits for the agent to move its caret, in seconds.
const TIMEOUT: f64 = 0.5;
/// How long the caret has to stay put after keys were sent before the next step reads it, so a
/// step never works from a screen the agent is still redrawing.
const SETTLE: f64 = 0.03;
/// What Claude Code draws above and below its prompt box.
const RULE: char = '─';

/// Edits in an agent's prompt box driven from the mouse. The agent only knows keys, so a click
/// sends arrows (row first, as Up/Down keep the column, then along the row) and erasing a
/// selection moves the caret to its end and sends Backspace one row at a time, reading where
/// the caret landed after each.
#[derive(Default)]
pub struct PromptClick {
    job: Option<Job>,
}

struct Job {
    phase: Phase,
    /// The caret when keys were last sent; the next step waits for it to move and settle.
    sent_from: Option<(i32, usize)>,
    caret: (i32, usize),
    caret_since: f64,
    until: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Move {
        row: i32,
        col: usize,
        rows_sent: bool,
        then_erase: Option<(i32, usize)>,
    },
    /// Backspace until the caret is back at this cell.
    Erase {
        row: i32,
        col: usize,
    },
}

#[derive(Debug, PartialEq)]
enum Step {
    Wait,
    /// Keys to send, then the phase that follows them, if any.
    Send(Key, usize, Option<Phase>),
}

impl PromptClick {
    /// Moves the caret to the character boundary nearest `pos`, when that lies inside the prompt
    /// box holding the cursor.
    pub fn click(&mut self, frame: &Frame, grid: Grid, pos: Pos2, now: f64) {
        let rel = pos - grid.origin;
        let row = (rel.y / grid.cell.y).floor() as i32;
        let col = (rel.x / grid.cell.x).round().max(0.0) as usize;
        self.job = in_prompt_box(frame, row).then(|| {
            let col = clamp_col(frame, row, col);
            Job::new(frame, Phase::Move { row, col, rows_sent: false, then_erase: None }, now)
        });
    }

    /// Erases the selection when it lies inside the prompt box, clearing it. Returns whether it
    /// did, so the key that asked for it isn't sent as well.
    pub fn erase_selection(&mut self, session: &mut TerminalSession, now: f64) -> bool {
        if !session.frame.has_selection || !session.is_agent() {
            return false;
        }
        let range = {
            let term = session.term.lock();
            term.selection.as_ref().and_then(|s| s.to_range(&term))
        };
        let Some(range) = range else { return false };
        let offset = session.frame.display_offset as i32;
        let frame = &session.frame;
        let (start_row, end_row) = (range.start.line.0 + offset, range.end.line.0 + offset);
        if !in_prompt_box(frame, start_row) || !in_prompt_box(frame, end_row) {
            return false;
        }
        let start_col = clamp_col(frame, start_row, range.start.column.0);
        let end_col = clamp_col(frame, end_row, range.end.column.0 + 1);
        let erase = Some((start_row, start_col));
        let phase = Phase::Move { row: end_row, col: end_col, rows_sent: false, then_erase: erase };
        self.job = Some(Job::new(frame, phase, now));
        session.term.lock().selection = None;
        session.frame.has_selection = false;
        true
    }

    /// Sends the next keys of the job. Returns whether it is still waiting on the agent, so the
    /// caller keeps repainting.
    pub fn step(&mut self, session: &TerminalSession, now: f64) -> bool {
        let Some(job) = self.job.as_mut() else { return false };
        let frame = &session.frame;
        let caret = (frame.cursor.row, frame.cursor.col);
        if caret != job.caret {
            (job.caret, job.caret_since) = (caret, now);
        }
        if now > job.until {
            self.job = None;
            return false;
        }
        if job.sent_from.is_some_and(|from| from == caret || now - job.caret_since < SETTLE) {
            return true;
        }
        let Step::Send(key, count, next) = next_step(frame, job.phase) else { return true };
        if let Some(bytes) = key_to_bytes(key, Modifiers::NONE, frame.mode).filter(|_| count > 0) {
            session.write(bytes.repeat(count));
        }
        match next {
            Some(phase) => {
                job.phase = phase;
                job.sent_from = (count > 0).then_some(caret);
                job.until = now + TIMEOUT;
            }
            None => self.job = None,
        }
        self.job.is_some()
    }
}

impl Job {
    fn new(frame: &Frame, phase: Phase, now: f64) -> Self {
        let caret = (frame.cursor.row, frame.cursor.col);
        Self { phase, sent_from: None, caret, caret_since: now, until: now + TIMEOUT }
    }
}

fn next_step(frame: &Frame, phase: Phase) -> Step {
    let (row, col) = (frame.cursor.row, frame.cursor.col);
    match phase {
        Phase::Move { row: target, rows_sent: true, .. } if row != target => Step::Wait,
        Phase::Move { row: target, col: target_col, then_erase, .. } if row != target => {
            let key = if row > target { Key::ArrowUp } else { Key::ArrowDown };
            let next = Phase::Move { row: target, col: target_col, rows_sent: true, then_erase };
            Step::Send(key, row.abs_diff(target) as usize, Some(next))
        }
        Phase::Move { col: target, then_erase, .. } => {
            let key = if target < col { Key::ArrowLeft } else { Key::ArrowRight };
            let next = then_erase.map(|(row, col)| Phase::Erase { row, col });
            Step::Send(key, chars_between(frame, row, col, target), next)
        }
        Phase::Erase { row: start, col: start_col } if row < start || row == start && col <= start_col => {
            Step::Send(Key::Backspace, 0, None)
        }
        Phase::Erase { row: start, col: start_col } if row == start => {
            Step::Send(Key::Backspace, chars_between(frame, row, col, start_col), None)
        }
        // A row below the start: its text before the caret, or else the break joining it to the
        // row above (a newline, or the space a wrapped row ended on).
        Phase::Erase { .. } => {
            let count = chars_between(frame, row, col, TEXT_START).max(1);
            Step::Send(Key::Backspace, count, Some(phase))
        }
    }
}

/// Characters between two columns of `row`, one arrow or Backspace each; wide characters fill
/// one cell of the frame.
fn chars_between(frame: &Frame, row: i32, a: usize, b: usize) -> usize {
    let cols = a.min(b)..a.max(b);
    frame.cells.iter().filter(|c| c.row == row && cols.contains(&c.col)).count()
}

/// Whether `row` and the visible cursor share a prompt box: rules above and below them both,
/// none between.
fn in_prompt_box(frame: &Frame, row: i32) -> bool {
    let cursor = &frame.cursor;
    if cursor.shape == CursorShape::Hidden {
        return false;
    }
    let (top, bottom) = (cursor.row.min(row), cursor.row.max(row));
    let rule_rows = || frame.cells.iter().filter(|c| c.col == 0 && c.c == RULE).map(|c| c.row);
    rule_rows().any(|r| r < top)
        && rule_rows().any(|r| r > bottom)
        && !rule_rows().any(|r| (top..=bottom).contains(&r))
}

/// Keeps a column off the prompt's prefix and short of the row's end, past which Right would
/// carry on into the next row.
fn clamp_col(frame: &Frame, row: i32, col: usize) -> usize {
    let end = frame.cells.iter().filter(|c| c.row == row && c.c != ' ').map(|c| c.col + c.width).max();
    col.clamp(TEXT_START, end.unwrap_or(TEXT_START).max(TEXT_START))
}

#[cfg(test)]
#[path = "prompt_click_tests.rs"]
mod tests;
