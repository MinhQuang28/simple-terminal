use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Instant;

use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::vte::ansi::{Processor, StdSyncHandler};

use super::*;

fn feed(term: &FairMutex<Term<EventProxy>>, bytes: &[u8]) {
    let mut parser: Processor<StdSyncHandler> = Processor::new();
    parser.advance(&mut *term.lock(), bytes);
}

/// The column-0 glyph the frame holds at `row`.
fn glyph_at(frame: &Frame, row: i32) -> Option<char> {
    frame.cells.iter().find(|c| c.row == row && c.col == 0).map(|c| c.c)
}

/// The pane row the frame draws the line holding `glyph` on.
fn pane_row(frame: &Frame, glyph: char) -> Option<i32> {
    frame.cells.iter().find(|c| c.c == glyph).map(|c| c.row + frame.band_rows as i32)
}

#[test]
fn keeps_the_previous_frame_while_the_terminal_is_locked() {
    let term = FairMutex::new(EventProxy::test_term(10, 3));
    let theme = Theme::default();
    let mut frame = Frame::default();
    assert!(frame.refresh(&term, &theme, false));
    assert_eq!(frame.screen_lines, 3);

    let guard = term.lock_unfair();
    let started = Instant::now();
    assert!(!frame.refresh(&term, &theme, false));
    assert!(started.elapsed() < MAX_STALE);
    drop(guard);
    assert!(frame.refresh(&term, &theme, false));
}

#[test]
fn waits_for_the_terminal_once_the_frame_is_too_old() {
    let term = Arc::new(FairMutex::new(EventProxy::test_term(10, 3)));
    let theme = Theme::default();
    let mut frame = Frame::default();
    assert!(frame.refresh(&term, &theme, false));

    let (locked_tx, locked) = mpsc::channel();
    let holder = {
        let term = term.clone();
        thread::spawn(move || {
            let _guard = term.lock_unfair();
            locked_tx.send(()).unwrap();
            thread::sleep(MAX_STALE * 2);
        })
    };
    locked.recv().unwrap();
    thread::sleep(MAX_STALE);
    assert!(frame.refresh(&term, &theme, false), "a frame older than MAX_STALE must be recaptured");
    holder.join().unwrap();
}

#[test]
fn anchored_frames_carry_history_above_the_viewport() {
    let theme = Theme::default();
    let term = FairMutex::new(EventProxy::test_term(10, 4));
    // Five lines scroll into history when the screen is cleared; one line is left at the top.
    feed(&term, b"a\r\nb\r\nc\r\nd\r\ne\r\n\x1b[2J\x1b[1;1Hx");
    let mut frame = Frame::default();

    assert!(frame.refresh(&term, &theme, true));
    assert_eq!(frame.band_rows, 3, "three empty screen rows are filled from history");
    assert_eq!(glyph_at(&frame, 0), Some('x'));
    assert_eq!(
        (glyph_at(&frame, -1), glyph_at(&frame, -2), glyph_at(&frame, -3)),
        (Some('e'), Some('d'), Some('c'))
    );
    assert_eq!(glyph_at(&frame, -4), Some('b'), "plus the row above the band, for sub-line scrolling");
    assert_eq!(glyph_at(&frame, -5), None);

    assert!(frame.refresh(&term, &theme, false));
    assert_eq!(frame.band_rows, 0);
    assert_eq!((glyph_at(&frame, -1), glyph_at(&frame, -2)), (Some('e'), None));
}

#[test]
fn keeps_the_view_still_while_output_fills_the_empty_rows() {
    let theme = Theme::default();
    let term = FairMutex::new(EventProxy::test_term(10, 4));
    feed(&term, b"a\r\nb\r\nc\r\nd\r\ne\r\n\x1b[2J\x1b[1;1Hx");
    let mut frame = Frame::default();
    assert!(frame.refresh(&term, &theme, true));

    // Reading history, two lines up.
    term.lock().scroll_display(Scroll::Delta(2));
    assert!(frame.refresh(&term, &theme, true));
    assert_eq!((frame.display_offset, frame.band_rows), (2, 3));
    assert_eq!(pane_row(&frame, 'e'), Some(4));

    // Output fills two empty rows: nothing scrolls off, so the terminal alone would keep the
    // offset at 2 while the shrinking band lifted every line on view by two rows.
    feed(&term, b"\r\ny\r\nz");
    assert!(frame.refresh(&term, &theme, true));
    assert_eq!((frame.display_offset, frame.band_rows), (4, 1));
    assert_eq!(pane_row(&frame, 'e'), Some(4));

    // The last empty row fills, then the screen scrolls; the terminal keeps the offset in step
    // from here on and the view still doesn't move.
    feed(&term, b"\r\n1");
    assert!(frame.refresh(&term, &theme, true));
    assert_eq!((frame.display_offset, frame.band_rows), (5, 0));
    assert_eq!(pane_row(&frame, 'e'), Some(4));
    feed(&term, b"\r\n2");
    assert!(frame.refresh(&term, &theme, true));
    assert_eq!((frame.display_offset, frame.band_rows), (6, 0));
    assert_eq!(pane_row(&frame, 'e'), Some(4));
}

#[test]
fn anchoring_while_scrolled_up_stops_where_the_band_meets_history() {
    let theme = Theme::default();
    let term = FairMutex::new(EventProxy::test_term(10, 4));
    feed(&term, b"a\r\nb\r\nc\r\nd\r\ne\r\n\x1b[2J\x1b[1;1Hx");
    let mut frame = Frame::default();
    assert!(frame.refresh(&term, &theme, false));
    // Scrolled to the first line, then anchored: three of the five history rows join the band.
    term.lock().scroll_display(Scroll::Delta(5));
    assert!(frame.refresh(&term, &theme, true));
    assert_eq!((frame.display_offset, frame.band_rows), (2, 3));
    assert_eq!(glyph_at(&frame, -3), Some('a'), "the band's top row is the first line");
}
