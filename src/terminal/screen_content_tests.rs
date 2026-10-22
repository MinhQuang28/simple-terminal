use alacritty_terminal::grid::Scroll;
use alacritty_terminal::vte::ansi::{Processor, StdSyncHandler};

use super::*;

fn feed(term: &mut Term<EventProxy>, bytes: &[u8]) {
    let mut parser: Processor<StdSyncHandler> = Processor::new();
    parser.advance(term, bytes);
}

#[test]
fn runs_through_the_last_used_row_or_the_visible_cursor() {
    let mut term = EventProxy::test_term(10, 6);
    assert_eq!(content_rows(&term), 1, "an empty screen still counts the cursor's row");

    feed(&mut term, b"one\r\ntwo");
    assert_eq!(content_rows(&term), 2);

    // The cursor parked below the text, as Claude's classic renderer leaves it in its prompt box.
    feed(&mut term, b"\x1b[5;1H");
    assert_eq!(content_rows(&term), 5);

    // A hidden cursor paints nothing, so where it is parked doesn't count.
    feed(&mut term, b"\x1b[?25l");
    assert_eq!(content_rows(&term), 2);
    feed(&mut term, b"\x1b[?25h");

    // A coloured background with no glyph on it shows too.
    feed(&mut term, b"\x1b[6;1H\x1b[44m \x1b[0m\x1b[1;1H");
    assert_eq!(content_rows(&term), 6);
}

#[test]
fn decorated_spaces_count_as_content() {
    let mut term = EventProxy::test_term(10, 6);
    feed(&mut term, b"one\r\n\x1b[4m   \x1b[0m\r\n\x1b[9m   \x1b[0m\x1b[1;1H");
    assert_eq!(content_rows(&term), 3);
}

#[test]
fn ignores_the_scroll_position() {
    let mut term = EventProxy::test_term(10, 3);
    feed(&mut term, b"a\r\nb\r\nc\r\nd\r\n\x1b[2J\x1b[1;1Hx");
    assert_eq!(content_rows(&term), 1, "a cleared screen with one line at the top");
    term.scroll_display(Scroll::Delta(1));
    assert_eq!(term.grid().display_offset(), 1);
    assert_eq!(content_rows(&term), 1);
}
