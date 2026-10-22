use std::sync::Once;
use std::thread;

/// The system alert sound, as the macOS notification API names it.
#[cfg(target_os = "macos")]
const SOUND_NAME: &str = "NSUserNotificationDefaultSoundName";
#[cfg(not(target_os = "macos"))]
const SOUND_NAME: &str = "message-new-instant";

/// Posts a system notification, with the system alert sound when `sound`; the app's bundle
/// identity is used when it runs from a bundle.
pub fn notify(title: &str, body: &str, sound: bool) {
    static APP: Once = Once::new();
    APP.call_once(|| {
        let _ = notify_rust::set_application(crate::BUNDLE_ID);
    });
    let (title, body) = (title.to_string(), body.to_string());
    thread::spawn(move || {
        let mut notification = notify_rust::Notification::new();
        notification.summary(&title).body(&body);
        if sound {
            notification.sound_name(SOUND_NAME);
        }
        if let Err(err) = notification.show() {
            log::warn!("Notification failed: {err}");
        }
    });
}

/// Shows `count` on the Dock icon, or clears the badge for zero.
#[cfg(target_os = "macos")]
pub fn set_badge(count: usize) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;
    use objc2_foundation::NSString;

    let Some(mtm) = MainThreadMarker::new() else { return };
    let label = (count > 0).then(|| NSString::from_str(&count.to_string()));
    NSApplication::sharedApplication(mtm).dockTile().setBadgeLabel(label.as_deref());
}

#[cfg(not(target_os = "macos"))]
pub fn set_badge(_count: usize) {}
