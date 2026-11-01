//! Whether the window is in front, shared with background pollers so they can rest while the
//! user is in another app and catch up as soon as they come back.

use std::sync::atomic::{AtomicBool, Ordering};

static ACTIVE: AtomicBool = AtomicBool::new(true);

pub fn is_active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// Records whether the window is focused and visible; true when it just came back to the front.
pub fn set_active(active: bool) -> bool {
    let was = ACTIVE.swap(active, Ordering::Relaxed);
    active && !was
}
