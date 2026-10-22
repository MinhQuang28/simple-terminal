use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use alacritty_terminal::event::{Event, EventListener};
use eframe::egui;

/// Output-driven repaints are capped at this rate; the PTY reader already batches bytes, but a
/// streaming program still wakes the UI far more often than a display can show. 120 Hz matches
/// ProMotion displays, where a 60 Hz cap showed every other frame of a scrolling agent pane.
const REPAINT_INTERVAL: Duration = Duration::from_micros(1_000_000 / 120);
/// Panes that aren't on screen only need their sidebar spinner and title refreshed now and then.
const HIDDEN_REPAINT_INTERVAL: Duration = Duration::from_secs(1);

/// Leading-and-trailing-edge repaint throttle shared by every session, so many busy panes
/// together still wake the window at most once per interval: the first wakeup repaints at
/// once, later ones within the interval schedule a single repaint at its end, so the final
/// update is never lost. Hidden panes share a separate, slower lane.
#[derive(Default)]
pub struct RepaintGate {
    visible: Mutex<Option<Instant>>,
    hidden: Mutex<Option<Instant>>,
}

impl RepaintGate {
    fn request(&self, ctx: &egui::Context, visible: bool) {
        let (lane, interval) =
            if visible { (&self.visible, REPAINT_INTERVAL) } else { (&self.hidden, HIDDEN_REPAINT_INTERVAL) };
        let Ok(mut last) = lane.lock() else { return };
        let now = Instant::now();
        match *last {
            // A trailing repaint is already scheduled; it will show this output too.
            Some(at) if at > now => {}
            Some(at) if now - at < interval => {
                ctx.request_repaint_after(at + interval - now);
                *last = Some(at + interval);
            }
            Some(_) | None => {
                *last = Some(now);
                ctx.request_repaint();
            }
        }
    }
}

/// Receives terminal events on the PTY thread and forwards them to the session.
#[derive(Clone)]
pub struct EventProxy {
    pub(super) tx: mpsc::Sender<Event>,
    pub(super) ctx: egui::Context,
    pub(super) repaint: Arc<RepaintGate>,
    pub(super) visible: Arc<AtomicBool>,
}

impl EventListener for EventProxy {
    fn send_event(&self, event: Event) {
        // Replies are written from the UI thread, and programs block on them (Claude Code probes
        // the terminal at startup), so queries skip the throttle; a hidden pane's lane would
        // otherwise hold each reply for up to a second.
        let query = matches!(event, Event::PtyWrite(_) | Event::ColorRequest(..));
        let _ = self.tx.send(event);
        if query {
            self.ctx.request_repaint();
        } else {
            self.repaint.request(&self.ctx, self.visible.load(Ordering::Relaxed));
        }
    }
}
