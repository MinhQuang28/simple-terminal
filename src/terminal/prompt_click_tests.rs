use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::vte::ansi::{Processor, StdSyncHandler};

use super::*;
use crate::terminal::session::EventProxy;
use crate::theme::Theme;

/// Claude Code's prompt box as its classic renderer draws it, caret after "mu" on row 3.
fn prompt_frame() -> Frame {
    let screen = "chat above\r\n\
        ────────────────────\r\n\
        ❯ alpha beta gamma\r\n  \
          delta 中文 mu\r\n\
        ────────────────────\r\n  \
          ⏵⏵ auto mode on\x1b[4;16H";
    let term = FairMutex::new(EventProxy::test_term(20, 6));
    let mut parser: Processor<StdSyncHandler> = Processor::new();
    parser.advance(&mut *term.lock(), screen.as_bytes());
    let mut frame = Frame::default();
    frame.refresh(&term, &Theme::default(), false);
    frame
}

fn move_to(row: i32, col: usize) -> Phase {
    Phase::Move { row, col, rows_sent: false, then_erase: None }
}

#[test]
fn only_rows_inside_the_box_holding_the_cursor_count() {
    let frame = prompt_frame();
    assert_eq!((frame.cursor.row, frame.cursor.col), (3, 15));
    assert!(in_prompt_box(&frame, 2));
    assert!(in_prompt_box(&frame, 3));
    assert!(!in_prompt_box(&frame, 1), "a rule row");
    assert!(!in_prompt_box(&frame, 0), "the conversation above");
    assert!(!in_prompt_box(&frame, 5), "the status below");
}

#[test]
fn moves_rows_first_then_waits_for_the_caret() {
    let frame = prompt_frame();
    let sent = Phase::Move { row: 2, col: 4, rows_sent: true, then_erase: None };
    assert_eq!(next_step(&frame, move_to(2, 4)), Step::Send(Key::ArrowUp, 1, Some(sent)));
    assert_eq!(next_step(&frame, sent), Step::Wait);
}

#[test]
fn counts_characters_not_cells_along_the_row() {
    let frame = prompt_frame();
    // "中文 mu" spans cols 8..15 in five characters.
    assert_eq!(next_step(&frame, move_to(3, 8)), Step::Send(Key::ArrowLeft, 5, None));
    assert_eq!(next_step(&frame, move_to(3, 2)), Step::Send(Key::ArrowLeft, 11, None));
    assert_eq!(next_step(&frame, move_to(3, 15)), Step::Send(Key::ArrowRight, 0, None));
}

#[test]
fn erasing_reaches_the_start_a_row_at_a_time() {
    let frame = prompt_frame();
    let erase = Phase::Erase { row: 2, col: 8 };
    // The caret's row first, keeping the phase to read the caret again afterwards.
    assert_eq!(next_step(&frame, erase), Step::Send(Key::Backspace, 11, Some(erase)));
    let same_row = Phase::Erase { row: 3, col: 8 };
    assert_eq!(next_step(&frame, same_row), Step::Send(Key::Backspace, 5, None));
    let behind = Phase::Erase { row: 3, col: 15 };
    assert_eq!(next_step(&frame, behind), Step::Send(Key::Backspace, 0, None));
}

#[test]
fn clamps_to_the_text_on_the_row() {
    let frame = prompt_frame();
    assert_eq!(clamp_col(&frame, 2, 0), TEXT_START);
    assert_eq!(clamp_col(&frame, 2, 19), 18);
    assert_eq!(clamp_col(&frame, 3, 9), 9);
}
