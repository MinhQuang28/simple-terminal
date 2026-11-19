//! Notices a turn stopped with Escape. Claude Code reports no hook for an interrupted turn, so
//! after Escape reaches a working agent its screen is watched for a moment: the turn stopped
//! once it prints "Interrupted" and no longer offers "esc to interrupt".

use std::time::{Duration, Instant};

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};

use super::session::TerminalSession;
use crate::agent_events::AgentStatus;

/// How long after Escape the screen is watched; the agent redraws well within it.
const WATCH: Duration = Duration::from_secs(5);

impl TerminalSession {
    /// Presses Escape for the agent, which ends the turn it is working on (`repodeck interrupt`).
    pub fn interrupt_agent(&mut self) {
        self.send_input(b"\x1b".to_vec());
        if let Some(agent) = &mut self.agent {
            agent.note_interrupted();
        }
    }

    /// Input on its way to the pane: a lone Escape for a working agent may stop its turn.
    pub(super) fn note_escape(&mut self, bytes: &[u8]) {
        if bytes == b"\x1b" && self.agent_status() == Some(AgentStatus::Working) {
            self.escape_at = Some(Instant::now());
        }
    }

    /// Marks the turn stopped once the screen says so; gives up when the agent moved on or the
    /// Escape did something else (closed a menu, left insert mode).
    pub(super) fn check_interrupted(&mut self) {
        let Some(at) = self.escape_at else { return };
        if at.elapsed() > WATCH || self.agent_status() != Some(AgentStatus::Working) {
            self.escape_at = None;
            return;
        }
        let screen = {
            let term = self.term.lock();
            let end = Point::new(Line(term.screen_lines() as i32 - 1), Column(term.columns() - 1));
            term.bounds_to_string(Point::new(Line(0), Column(0)), end)
        };
        if shows_interrupted_turn(&screen)
            && let Some(agent) = &mut self.agent
        {
            agent.note_interrupted();
            self.escape_at = None;
        }
    }
}

/// Whether an agent's screen shows a turn stopped by Escape rather than one still running.
fn shows_interrupted_turn(screen: &str) -> bool {
    screen.contains("Interrupted") && !screen.contains("esc to interrupt")
}

#[cfg(test)]
mod tests {
    use super::shows_interrupted_turn;

    #[test]
    fn a_turn_is_interrupted_once_the_screen_says_so_and_stops_offering_it() {
        let working = "✻ Doodling… (12s)\n❯ \n  ⏵⏵ auto mode on · esc to interrupt";
        let stopped = "  ⎿  Interrupted · What should Claude do instead?\n❯ \n  ⏵⏵ auto mode on";
        // An earlier interruption is still on screen while a new turn runs.
        let again = "  ⎿  Interrupted · What should Claude do instead?\n✻ Doodling…\n  esc to interrupt";
        assert!(!shows_interrupted_turn(working));
        assert!(shows_interrupted_turn(stopped));
        assert!(!shows_interrupted_turn(again));
    }
}
