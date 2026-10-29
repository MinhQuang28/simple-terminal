use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::{Term, TermMode};
use alacritty_terminal::vte::ansi::{ClearMode, Handler};

use super::session::{EventProxy, TerminalSession};

/// What Claude Code draws above its prompt box.
const RULE: char = '─';

impl TerminalSession {
    /// Clears scrollback and the screen above the line being typed on (⌘K), which moves to the
    /// top: a shell's prompt line, or an agent's whole prompt box. The program keeps drawing
    /// where it was, as everything it still addresses moves with the cursor.
    pub fn clear_scrollback(&mut self) {
        clear(&mut self.term.lock(), self.is_agent());
        if let Some(shell) = &mut self.shell {
            shell.clear_marks();
        }
        self.frame.invalidate();
    }
}

fn clear(term: &mut Term<EventProxy>, agent: bool) {
    if !term.mode().contains(TermMode::ALT_SCREEN) {
        let keep = keep_from(term, agent);
        if keep > 0 {
            term.scroll_up(keep);
            term.grid_mut().cursor.point.line -= keep;
        }
    }
    term.clear_screen(ClearMode::Saved);
    term.selection = None;
}

/// The first screen line to keep: the cursor's, or for an agent the rule topping its prompt box.
fn keep_from(term: &Term<EventProxy>, agent: bool) -> usize {
    let cursor = term.grid().cursor.point.line.0.clamp(0, term.screen_lines() as i32 - 1);
    let rule = agent
        .then(|| (0..=cursor).rev().find(|&line| term.grid()[Line(line)][Column(0)].c == RULE))
        .flatten();
    rule.unwrap_or(cursor) as usize
}

#[cfg(test)]
mod tests {
    use alacritty_terminal::sync::FairMutex;
    use alacritty_terminal::vte::ansi::{Processor, StdSyncHandler};

    use super::*;

    fn term_with(screen: &str) -> FairMutex<Term<EventProxy>> {
        let term = FairMutex::new(EventProxy::test_term(10, 4));
        let mut parser: Processor<StdSyncHandler> = Processor::new();
        parser.advance(&mut *term.lock(), screen.as_bytes());
        term
    }

    #[test]
    fn keeps_the_cursor_line_or_an_agents_prompt_box() {
        let term = term_with("out 1\r\nout 2\r\n──────────\r\n❯ hi\x1b[4;5H");
        assert_eq!(keep_from(&term.lock(), false), 3);
        assert_eq!(keep_from(&term.lock(), true), 2);
    }

    #[test]
    fn moves_the_kept_lines_and_cursor_to_the_top_without_history() {
        let term = term_with("a\r\nb\r\nc\r\nd\r\ne\r\n──────────\r\n❯ hi\x1b[4;5H");
        let mut term = term.lock();
        assert!(term.grid().history_size() > 0);
        clear(&mut term, true);
        assert_eq!(term.grid().history_size(), 0);
        assert_eq!(term.grid()[Line(0)][Column(0)].c, RULE);
        assert_eq!(term.grid()[Line(1)][Column(0)].c, '❯');
        assert_eq!(term.grid()[Line(2)][Column(0)].c, ' ');
        assert_eq!(term.grid().cursor.point, alacritty_terminal::index::Point::new(Line(1), Column(4)));
    }
}
