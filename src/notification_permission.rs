//! Whether macOS lets Repodeck post notifications, read from the UserNotifications framework.
//! The answer arrives on a background queue, so it is polled from the settings page.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui;

/// How often the settings page re-asks macOS, so changes in System Settings show up.
const REFRESH_AFTER: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    /// macOS hasn't asked the user yet; the first notification (or a request) does.
    NotAsked,
    Allowed,
    Denied,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Status {
    pub permission: Permission,
    /// Sounds are allowed for Repodeck in System Settings.
    pub sound_allowed: bool,
}

#[derive(Default)]
struct Shared {
    status: Option<Status>,
    asked_at: Option<Instant>,
}

/// Handle to the latest known status; cheap to clone.
#[derive(Clone)]
pub struct NotificationPermission {
    shared: Arc<Mutex<Shared>>,
    ctx: egui::Context,
}

impl NotificationPermission {
    pub fn new(ctx: egui::Context) -> Self {
        Self { shared: Arc::default(), ctx }
    }

    /// True when the status can be read: the framework only works inside an app bundle.
    pub fn available() -> bool {
        cfg!(target_os = "macos")
            && std::env::current_exe().is_ok_and(|exe| exe.to_string_lossy().contains(".app/Contents/MacOS/"))
    }

    /// The latest status, refreshing it in the background when stale; `None` until the first
    /// answer or when unavailable.
    pub fn status(&self) -> Option<Status> {
        let mut shared = self.shared.lock().ok()?;
        if shared.asked_at.is_none_or(|at| at.elapsed() > REFRESH_AFTER) && Self::available() {
            shared.asked_at = Some(Instant::now());
            self.query();
        }
        shared.status
    }

    /// Shows the system permission prompt when macOS hasn't asked yet.
    pub fn request(&self) {
        if Self::available() {
            let this = self.clone();
            mac::request(move || this.query());
        }
    }

    fn query(&self) {
        let shared = self.shared.clone();
        let ctx = self.ctx.clone();
        mac::query(move |status| {
            if let Ok(mut shared) = shared.lock() {
                shared.status = Some(status);
            }
            ctx.request_repaint();
        });
    }
}

/// Opens System Settings at Repodeck's notification options.
pub fn open_system_settings() {
    let url = format!(
        "x-apple.systempreferences:com.apple.Notifications-Settings.extension?id={}",
        crate::BUNDLE_ID
    );
    if let Err(err) = std::process::Command::new("open").arg(url).spawn() {
        log::warn!("Could not open System Settings: {err}");
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use std::ptr::NonNull;

    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::runtime::Bool;
    use objc2_foundation::NSError;
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNAuthorizationStatus, UNNotificationSetting, UNNotificationSettings,
        UNUserNotificationCenter,
    };

    use super::{Permission, Status};

    fn center() -> Retained<UNUserNotificationCenter> {
        UNUserNotificationCenter::currentNotificationCenter()
    }

    pub fn query(done: impl Fn(Status) + 'static) {
        let handler = RcBlock::new(move |settings: NonNull<UNNotificationSettings>| {
            // SAFETY: the framework hands a live settings object for the handler's duration.
            let settings = unsafe { settings.as_ref() };
            let permission = match settings.authorizationStatus() {
                UNAuthorizationStatus::NotDetermined => Permission::NotAsked,
                UNAuthorizationStatus::Denied => Permission::Denied,
                _ => Permission::Allowed,
            };
            let sound_allowed = settings.soundSetting() == UNNotificationSetting::Enabled;
            done(Status { permission, sound_allowed });
        });
        center().getNotificationSettingsWithCompletionHandler(&handler);
    }

    pub fn request(done: impl Fn() + 'static) {
        let handler = RcBlock::new(move |_granted: Bool, _error: *mut NSError| done());
        let options =
            UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound | UNAuthorizationOptions::Badge;
        center().requestAuthorizationWithOptions_completionHandler(options, &handler);
    }
}

#[cfg(not(target_os = "macos"))]
mod mac {
    use super::Status;
    pub fn query(_done: impl Fn(Status) + 'static) {}
    pub fn request(_done: impl Fn() + 'static) {}
}
