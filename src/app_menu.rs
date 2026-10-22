//! The macOS application menu. winit builds one named after the process ("repodeck"); this
//! renames its items to the app and adds "Settings…" (⌘,) where macOS apps keep it.

use std::sync::mpsc::Sender;

use eframe::egui;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSApplication, NSMenuItem};
use objc2_foundation::{NSString, ns_string};

use crate::app::Job;

/// What the Settings item needs to reach the app: its job queue and a way to wake the frame.
struct Ivars {
    jobs: Sender<Job>,
    ctx: egui::Context,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and MenuTarget doesn't implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "RepodeckMenuTarget"]
    #[ivars = Ivars]
    struct MenuTarget;

    impl MenuTarget {
        #[unsafe(method(openSettings:))]
        fn open_settings(&self, _sender: Option<&AnyObject>) {
            let Ivars { jobs, ctx } = self.ivars();
            let _ = jobs.send(Box::new(|app| app.settings_open = true));
            ctx.request_repaint();
        }
    }

    unsafe impl NSObjectProtocol for MenuTarget {}
);

impl MenuTarget {
    fn new(mtm: MainThreadMarker, jobs: Sender<Job>, ctx: egui::Context) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Ivars { jobs, ctx });
        unsafe { msg_send![super(this), init] }
    }
}

/// Renames the menu items after `app_name` and inserts "Settings…" after "About". Call once
/// from the main thread after the window exists.
pub fn install(app_name: &str, jobs: Sender<Job>, ctx: egui::Context) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let app = NSApplication::sharedApplication(mtm);
    let Some(app_menu) = app.mainMenu().and_then(|m| m.itemAtIndex(0)).and_then(|i| i.submenu()) else {
        log::warn!("No application menu to add Settings to");
        return;
    };
    let process = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default();
    if !process.is_empty() && process != app_name {
        for item in app_menu.itemArray().iter() {
            let title = item.title().to_string();
            if title.contains(&process) {
                item.setTitle(&NSString::from_str(&title.replace(&process, app_name)));
            }
        }
    }

    let target = MenuTarget::new(mtm, jobs, ctx);
    // SAFETY: the selector is implemented on MenuTarget, which lives for the whole process.
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            ns_string!("Settings…"),
            Some(sel!(openSettings:)),
            ns_string!(","),
        )
    };
    let any: &AnyObject = &target;
    unsafe { item.setTarget(Some(any)) };
    // About, separator, Settings…, separator, Services… as in every macOS app menu.
    app_menu.insertItem_atIndex(&item, 2);
    app_menu.insertItem_atIndex(&NSMenuItem::separatorItem(mtm), 3);
    // The menu item only holds a weak reference to its target.
    std::mem::forget(target);
}
