use std::path::PathBuf;
use std::sync::mpsc::Sender;

use eframe::egui;
use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{AllocAnyThread, DefinedClass, MainThreadMarker, define_class, msg_send};
use objc2_app_kit::{NSApplication, NSPasteboard, NSPasteboardTypeFileURL};
use objc2_foundation::{NSString, NSURL};

use super::FolderRequest;
use crate::model::PaneKind;

struct Ivars {
    tx: Sender<FolderRequest>,
    ctx: egui::Context,
}

// Selectors must match `NSMessage` in `macos/info-plist-services.xml`.
define_class!(
    #[unsafe(super(NSObject))]
    #[name = "RepodeckServiceProvider"]
    #[ivars = Ivars]
    struct ServiceProvider;

    impl ServiceProvider {
        #[unsafe(method(openTerminal:userData:error:))]
        fn open_terminal(&self, pboard: &NSPasteboard, _data: Option<&NSString>, _err: *mut *mut NSString) {
            self.forward(pboard, PaneKind::Shell);
        }

        #[unsafe(method(openAgent:userData:error:))]
        fn open_agent(&self, pboard: &NSPasteboard, _data: Option<&NSString>, _err: *mut *mut NSString) {
            self.forward(pboard, PaneKind::Agent);
        }
    }
);

impl ServiceProvider {
    fn forward(&self, pboard: &NSPasteboard, kind: PaneKind) {
        let Ivars { tx, ctx } = self.ivars();
        for path in file_paths(pboard) {
            let _ = tx.send(FolderRequest { path, kind: kind.clone() });
        }
        ctx.request_repaint();
    }
}

fn file_paths(pboard: &NSPasteboard) -> Vec<PathBuf> {
    let Some(items) = pboard.pasteboardItems() else { return Vec::new() };
    items
        .iter()
        .filter_map(|item| {
            // SAFETY: AppKit's pasteboard type constants are immutable statics.
            let url = item.stringForType(unsafe { NSPasteboardTypeFileURL })?;
            let path = NSURL::URLWithString(&url)?.path()?;
            Some(PathBuf::from(path.to_string()))
        })
        .collect()
}

pub fn register(tx: Sender<FolderRequest>, ctx: egui::Context) {
    let Some(mtm) = MainThreadMarker::new() else {
        log::warn!("Services provider must be registered on the main thread");
        return;
    };
    let provider = ServiceProvider::alloc().set_ivars(Ivars { tx, ctx });
    // SAFETY: plain NSObject initialiser on a freshly allocated instance.
    let provider: Retained<ServiceProvider> = unsafe { msg_send![super(provider), init] };
    // SAFETY: the provider implements the selectors declared in the bundle's NSServices entries.
    unsafe { NSApplication::sharedApplication(mtm).setServicesProvider(Some(&provider)) };
    // NSApp keeps only a weak reference; the provider must live as long as the app.
    std::mem::forget(provider);
}
