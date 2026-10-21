use std::path::Path;

use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point};

use super::session::TerminalSession;
use crate::shell_events::Finished;

/// What the shell's prompt hooks tell us about the session.
impl TerminalSession {
    /// Working directory reported by the shell's prompt hook, if integration is active.
    pub fn cwd(&self) -> Option<&Path> {
        self.shell.as_ref()?.cwd.as_deref()
    }

    pub fn last_finished(&self) -> Option<&Finished> {
        self.shell.as_ref()?.last_finished.as_ref()
    }

    pub fn take_long_finished(&mut self) -> Option<Finished> {
        self.shell.as_mut()?.take_long_finished()
    }

    /// Scrolls so the previous (`-1`) or next (`1`) prompt sits at the top of the view.
    pub fn scroll_to_prompt(&self, direction: i32) {
        let Some(shell) = &self.shell else { return };
        let mut term = self.term.lock();
        let history = term.grid().history_size();
        let top = history.saturating_sub(term.grid().display_offset());
        let target = if direction < 0 {
            shell.marks().iter().rev().map(|m| m.line).find(|l| *l < top)
        } else {
            shell.marks().iter().map(|m| m.line).find(|l| *l > top)
        };
        let Some(line) = target else { return };
        let offset = history.saturating_sub(line) as i32;
        let delta = offset - term.grid().display_offset() as i32;
        term.scroll_display(Scroll::Delta(delta));
    }

    /// Text between the last two prompts, which is the previous command and its output.
    pub fn last_output(&self) -> Option<String> {
        let marks = self.shell.as_ref()?.marks();
        let [.., previous, last] = marks else { return None };
        let term = self.term.lock();
        let history = term.grid().history_size() as i32;
        let start = Point::new(Line(previous.line as i32 - history + 1), Column(0));
        let end = Point::new(Line(last.line as i32 - history - 1), Column(term.columns() - 1));
        (start.line <= end.line).then(|| term.bounds_to_string(start, end))
    }
}
