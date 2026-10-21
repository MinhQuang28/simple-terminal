use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Instant;

use alacritty_terminal::sync::FairMutex;

use super::*;

#[test]
fn keeps_the_previous_frame_while_the_terminal_is_locked() {
    let term = FairMutex::new(EventProxy::test_term(10, 3));
    let theme = Theme::default();
    let mut frame = Frame::default();
    assert!(frame.refresh(&term, &theme));
    assert_eq!(frame.screen_lines, 3);

    let guard = term.lock_unfair();
    let started = Instant::now();
    assert!(!frame.refresh(&term, &theme));
    assert!(started.elapsed() < MAX_STALE);
    drop(guard);
    assert!(frame.refresh(&term, &theme));
}

#[test]
fn waits_for_the_terminal_once_the_frame_is_too_old() {
    let term = Arc::new(FairMutex::new(EventProxy::test_term(10, 3)));
    let theme = Theme::default();
    let mut frame = Frame::default();
    assert!(frame.refresh(&term, &theme));

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
    assert!(frame.refresh(&term, &theme), "a frame older than MAX_STALE must be recaptured");
    holder.join().unwrap();
}
