use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use eframe::egui::{self, ColorImage};

use super::session::TerminalSession;
use crate::clipboard_image;

/// Input queued behind a pasted image that is still being written to disk.
pub(super) enum Queued {
    Image(Receiver<anyhow::Result<PathBuf>>),
    Input(Vec<u8>),
}

impl TerminalSession {
    /// Saves a pasted image off the UI thread (a Retina screenshot takes up to a second to
    /// encode), then pastes its path; programs such as Claude Code attach image files named in
    /// the prompt.
    pub fn paste_image(&mut self, image: Arc<ColorImage>, ctx: &egui::Context) {
        let (tx, saved) = mpsc::channel();
        let ctx = ctx.clone();
        let spawned = thread::Builder::new().name("paste-image".into()).spawn(move || {
            let _ = tx.send(clipboard_image::save_png(&image));
            ctx.request_repaint();
        });
        match spawned {
            Ok(_) => self.queued.push_back(Queued::Image(saved)),
            Err(err) => log::error!("Failed to start saving the pasted image: {err}"),
        }
    }

    /// Typed or pasted input; while a pasted image is being saved it waits so it still lands
    /// after the image's path.
    pub fn send_input(&mut self, bytes: Vec<u8>) {
        if self.queued.is_empty() {
            self.write(bytes);
        } else {
            self.queued.push_back(Queued::Input(bytes));
        }
    }

    /// Sends queued input in order, up to the first image that isn't saved yet.
    pub(super) fn flush_queued(&mut self) {
        while let Some(front) = self.queued.front() {
            let saved = match front {
                Queued::Input(_) => None,
                Queued::Image(rx) => match rx.try_recv() {
                    Ok(saved) => Some(saved),
                    Err(TryRecvError::Empty) => return,
                    Err(TryRecvError::Disconnected) => Some(Err(anyhow::anyhow!("image writer stopped"))),
                },
            };
            match (self.queued.pop_front(), saved) {
                (Some(Queued::Input(bytes)), _) => self.write(bytes),
                (_, Some(Ok(path))) => self.paste(&format!("{} ", path.display())),
                (_, Some(Err(err))) => log::error!("Failed to save pasted image: {err:#}"),
                _ => {}
            }
        }
    }
}
