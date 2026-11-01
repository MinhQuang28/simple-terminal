use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use alacritty_terminal::event::{Event, EventListener};
use eframe::egui;

use crate::tab_title::{WORKING_GLYPHS, is_working};

/// Output-driven repaints of the pane the user is typing in are capped at this rate; the PTY
/// reader already batches bytes, but a streaming program still wakes the UI far more often than
/// a display can show. 120 Hz matches ProMotion displays, where a 60 Hz cap showed every other
/// frame of a scrolling agent pane.
const FOCUSED_REPAINT_INTERVAL: Duration = Duration::from_micros(1_000_000 / 120);
/// Other panes on screen (splits, or every pane while the window is in the background) are
/// watched less closely; each frame repaints the whole window, so they settle for half the rate.
const VISIBLE_REPAINT_INTERVAL: Duration = Duration::from_micros(1_000_000 / 60);
/// Panes that aren't on screen only need their sidebar spinner and title refreshed now and then.
const HIDDEN_REPAINT_INTERVAL: Duration = Duration::from_secs(1);

/// How closely the user is watching a pane, which decides how eagerly its output repaints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Presence {
    Hidden = 0,
    Visible = 1,
    /// On screen with keyboard focus in a focused window.
    Focused = 2,
}

impl Presence {
    pub(super) fn load(cell: &AtomicU8) -> Self {
        match cell.load(Ordering::Relaxed) {
            0 => Self::Hidden,
            1 => Self::Visible,
            _ => Self::Focused,
        }
    }
}

/// Leading-and-trailing-edge repaint throttle shared by every session, so many busy panes
/// together still wake the window at most once per interval: the first wakeup repaints at
/// once, later ones within the interval schedule a single repaint at its end, so the final
/// update is never lost. Hidden panes share a separate, slower lane.
#[derive(Default)]
pub struct RepaintGate {
    focused: Mutex<Option<Instant>>,
    visible: Mutex<Option<Instant>>,
    hidden: Mutex<Option<Instant>>,
}

impl RepaintGate {
    fn request(&self, ctx: &egui::Context, presence: Presence) {
        let (lane, interval) = match presence {
            Presence::Focused => (&self.focused, FOCUSED_REPAINT_INTERVAL),
            Presence::Visible => (&self.visible, VISIBLE_REPAINT_INTERVAL),
            Presence::Hidden => (&self.hidden, HIDDEN_REPAINT_INTERVAL),
        };
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
    pub(super) presence: Arc<AtomicU8>,
    /// The last title as it shows, so spinner frames alone don't repaint the window.
    pub(super) shown_title: Arc<Mutex<Option<String>>>,
}

impl EventProxy {
    /// Whether `title` would show differently from the last one: Claude Code steps a spinner
    /// glyph through its title several times a second, which changes neither the tab's text nor
    /// whether it reads as working.
    fn title_changed(&self, title: Option<&str>) -> bool {
        let shown = title.map(|t| {
            let working = if is_working(t) { "w" } else { "" };
            format!(
                "{working}{}",
                t.trim_start_matches(|c: char| WORKING_GLYPHS.contains(&c) || c.is_whitespace())
            )
        });
        let Ok(mut last) = self.shown_title.lock() else { return true };
        if *last == shown {
            return false;
        }
        *last = shown;
        true
    }
}

impl EventListener for EventProxy {
    fn send_event(&self, event: Event) {
        // Replies are written from the UI thread, and programs block on them (Claude Code probes
        // the terminal at startup), so queries skip the throttle; a hidden pane's lane would
        // otherwise hold each reply for up to a second.
        let query = matches!(event, Event::PtyWrite(_) | Event::ColorRequest(..));
        let quiet = match &event {
            Event::Title(title) => !self.title_changed(Some(title)),
            Event::ResetTitle => !self.title_changed(None),
            _ => false,
        };
        let _ = self.tx.send(event);
        if query {
            self.ctx.request_repaint();
        } else if !quiet {
            self.repaint.request(&self.ctx, Presence::load(&self.presence));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spinner_frames_in_the_title_are_not_a_change() {
        let (tx, _rx) = mpsc::channel();
        let proxy = EventProxy {
            tx,
            ctx: egui::Context::default(),
            repaint: Arc::default(),
            presence: Arc::default(),
            shown_title: Arc::default(),
        };
        assert!(proxy.title_changed(Some("◐ Fix login")));
        assert!(!proxy.title_changed(Some("◑ Fix login")), "only the spinner glyph stepped");
        assert!(proxy.title_changed(Some("✳ Fix login")), "working → idle shows differently");
        assert!(proxy.title_changed(Some("✳ Fix logout")));
        assert!(proxy.title_changed(None));
        assert!(!proxy.title_changed(None));
    }
}
