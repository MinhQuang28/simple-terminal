use std::time::Duration;

use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::vte::ansi::{Processor, StdSyncHandler};

use super::*;

/// A 5-row terminal with 21 lines of history: 25 printed lines plus the empty line after them.
fn term_with_history() -> Term<EventProxy> {
    let mut term = EventProxy::test_term(20, 5);
    let mut parser: Processor<StdSyncHandler> = Processor::new();
    for i in 0..25 {
        parser.advance(&mut term, format!("line {i}\r\n").as_bytes());
    }
    assert_eq!(term.grid().history_size(), 21);
    term
}

#[test]
fn whole_lines_move_the_grid_and_fractions_stay_pending() {
    let mut term = term_with_history();
    let mut state = ScrollState::default();
    state.scroll_lines(&mut term, 2.4);
    assert_eq!(term.grid().display_offset(), 2);
    assert!((state.remainder - 0.4).abs() < 1e-5);
    state.scroll_lines(&mut term, 0.7);
    assert_eq!(term.grid().display_offset(), 3);
    assert!((state.remainder - 0.1).abs() < 1e-5);
    assert!((state.pixel_offset(16.0) - 1.6).abs() < 1e-4);
}

#[test]
fn clamps_at_bottom_and_top() {
    let mut term = term_with_history();
    let mut state = ScrollState::default();
    state.scroll_lines(&mut term, -0.6);
    assert_eq!(term.grid().display_offset(), 0);
    assert_eq!(state.remainder, 0.0);

    state.scroll_lines(&mut term, 100.0);
    assert_eq!(term.grid().display_offset(), 21);
    assert_eq!(state.remainder, 0.0);

    state.snap_to_bottom(&mut term);
    assert_eq!(term.grid().display_offset(), 0);
}

#[test]
fn resize_keeps_scroll_state_valid() {
    let mut term = term_with_history();
    let mut state = ScrollState::default();
    state.scroll_lines(&mut term, 5.5);
    term.resize(TermSize::new(20, 30));
    state.scroll_lines(&mut term, 0.0);
    assert_eq!(term.grid().display_offset(), 0);
    assert_eq!(state.remainder, 0.0);
}

#[test]
fn encodes_wheel_reports() {
    assert_eq!(wheel_report(TermMode::SGR_MOUSE, 2, 3, 4), "\x1b[<64;3;4M\x1b[<64;3;4M");
    assert_eq!(wheel_report(TermMode::SGR_MOUSE, -1, 1, 1), "\x1b[<65;1;1M");
    assert_eq!(wheel_report(TermMode::empty(), 1, 1, 2), "\x1b[M\x60\x21\x22");
}

#[test]
fn accumulates_whole_lines() {
    let mut state = ScrollState::default();
    assert_eq!(whole_lines(&mut state, 0.6), 0);
    assert_eq!(whole_lines(&mut state, 0.6), 1);
    assert_eq!(whole_lines(&mut state, -2.5), -2);
    assert!((state.carry + 0.3).abs() < 1e-5);
    assert_eq!(state.remainder, 0.0, "forwarded wheel events must not shift the grid");
}

#[test]
fn paces_alt_screen_arrows_below_a_key_repeat_burst() {
    let mut state = ScrollState::default();
    let t0 = Instant::now();
    assert_eq!(state.take_arrows(2, t0), 2);
    assert_eq!(state.take_arrows(10, t0), 1);
    assert_eq!(state.take_arrows(10, t0 + Duration::from_millis(10)), 0);
    assert_eq!(state.take_arrows(-10, t0 + Duration::from_millis(100)), -3);
    // Worst case over any 100 ms: the burst plus the refill is under Claude Code's eight.
    let mut sent = 0;
    let mut state = ScrollState::default();
    for ms in (0..100).step_by(5) {
        sent += state.take_arrows(10, t0 + Duration::from_millis(ms));
    }
    assert!(sent < 8, "{sent} arrows in 100 ms");
}

#[test]
fn claude_code_fullscreen_enables_mouse_reports() {
    let mut term = EventProxy::test_term(20, 5);
    let mut parser: Processor<StdSyncHandler> = Processor::new();
    parser.advance(&mut term, b"\x1b[?1049h\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1006h");
    let mode = *term.mode();
    assert!(mode.contains(TermMode::ALT_SCREEN));
    assert!(mode.intersects(TermMode::MOUSE_MODE));
    assert!(mode.contains(TermMode::SGR_MOUSE));
}
