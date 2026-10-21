use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};

use eframe::egui;

use crate::model::PaneKind;

#[cfg(target_os = "macos")]
mod macos;

/// A folder sent from Finder's Services menu ("New Repodeck Terminal at Folder").
pub struct FolderRequest {
    pub path: PathBuf,
    pub kind: PaneKind,
}

/// Starts receiving Services menu requests; the receiver stays empty on other platforms.
pub fn register(ctx: &egui::Context) -> Receiver<FolderRequest> {
    let (tx, rx) = mpsc::channel();
    #[cfg(target_os = "macos")]
    macos::register(tx, ctx.clone());
    #[cfg(not(target_os = "macos"))]
    let _ = (tx, ctx);
    rx
}
