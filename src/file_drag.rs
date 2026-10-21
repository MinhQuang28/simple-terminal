use std::path::{Path, PathBuf};
use std::time::Duration;

use eframe::egui::{self, Pos2, Rect};

/// Files being dragged over the window from Finder, or dropped onto it this frame.
#[derive(Default)]
pub struct FileDrag {
    hovering: bool,
    dropped: Vec<PathBuf>,
    pointer: Option<Pos2>,
}

impl FileDrag {
    /// Reads this frame's drag state. The OS owns the pointer during a drag, so egui's pointer
    /// position is stale; the real one is read from the platform.
    pub fn read(ctx: &egui::Context) -> Self {
        let (hovering, dropped) = ctx.input(|i| {
            let dropped: Vec<PathBuf> = i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect();
            (!i.raw.hovered_files.is_empty(), dropped)
        });
        if !hovering && dropped.is_empty() {
            return Self::default();
        }
        if hovering {
            // Nothing wakes egui while the drag moves, so poll the pointer for the highlight.
            ctx.request_repaint_after(Duration::from_millis(33));
        }
        let pointer = platform::pointer_position().or_else(|| ctx.input(|i| i.pointer.latest_pos()));
        Self { hovering, dropped, pointer }
    }

    /// Whether dragged files are currently over `rect`.
    pub fn hovering(&self, rect: Rect) -> bool {
        self.hovering && self.pointer.is_some_and(|p| rect.contains(p))
    }

    /// Paths dropped into `rect` this frame.
    pub fn dropped_in(&self, rect: Rect) -> Option<&[PathBuf]> {
        (!self.dropped.is_empty() && self.pointer.is_some_and(|p| rect.contains(p))).then_some(&self.dropped)
    }

    pub fn pointer(&self) -> Option<Pos2> {
        self.pointer
    }

    /// Draws the drop-target highlight.
    pub fn paint_target(ui: &egui::Ui, rect: Rect) {
        let accent = ui.visuals().hyperlink_color;
        let painter = ui.painter();
        painter.rect_filled(rect, 8.0, accent.gamma_multiply(0.12));
        painter.rect_stroke(rect, 8.0, egui::Stroke::new(2.0, accent), egui::StrokeKind::Inside);
    }
}

/// Paths joined for pasting into a shell, escaped like Finder drops into Terminal.app.
pub fn shell_paths(paths: &[PathBuf]) -> String {
    let mut out = String::new();
    for path in paths {
        out.push_str(&shell_escape(path));
        out.push(' ');
    }
    out
}

fn shell_escape(path: &Path) -> String {
    let text = path.to_string_lossy();
    let safe = |c: char| c.is_ascii_alphanumeric() || "_-./~:@%+=,".contains(c);
    if text.chars().all(safe) {
        return text.into_owned();
    }
    text.chars().fold(String::with_capacity(text.len() + 8), |mut s, c| {
        if !safe(c) {
            s.push('\\');
        }
        s.push(c);
        s
    })
}

#[cfg(target_os = "macos")]
mod platform {
    use eframe::egui::{Pos2, pos2};
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSEvent};

    /// Pointer position in window points, with egui's top-left origin.
    pub fn pointer_position() -> Option<Pos2> {
        let mtm = MainThreadMarker::new()?;
        let window = NSApplication::sharedApplication(mtm).windows().firstObject()?;
        let frame = window.frame();
        let mouse = NSEvent::mouseLocation();
        // Cocoa's origin is bottom-left; the window's titlebar is hidden, so the frame is the
        // content area.
        let x = mouse.x - frame.origin.x;
        let y = frame.origin.y + frame.size.height - mouse.y;
        Some(pos2(x as f32, y as f32))
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    pub fn pointer_position() -> Option<eframe::egui::Pos2> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_shell_specials_only_when_needed() {
        assert_eq!(shell_escape(Path::new("/Users/me/code/repo-1.v2")), "/Users/me/code/repo-1.v2");
        assert_eq!(
            shell_escape(Path::new("/Users/me/My Docs/a (1).png")),
            r"/Users/me/My\ Docs/a\ \(1\).png"
        );
        assert_eq!(shell_escape(Path::new("it's")), r"it\'s");
        assert_eq!(shell_paths(&[PathBuf::from("/a b"), PathBuf::from("/c")]), r"/a\ b /c ");
    }
}
