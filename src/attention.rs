use std::sync::Once;
use std::thread;

/// Posts a system notification; the app's bundle identity is used when it runs from a bundle.
pub fn notify(title: &str, body: &str) {
    static APP: Once = Once::new();
    APP.call_once(|| {
        let _ = notify_rust::set_application("dev.repodeck.app");
    });
    let (title, body) = (title.to_string(), body.to_string());
    thread::spawn(move || {
        if let Err(err) = notify_rust::Notification::new().summary(&title).body(&body).show() {
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
