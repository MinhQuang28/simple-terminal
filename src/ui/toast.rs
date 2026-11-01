//! A short message that floats above the status bar for a few seconds, for things that
//! succeeded quietly (a commit, a branch switch). Failures go to the inbox instead.

use std::time::Duration;

use eframe::egui::{Align2, Area, Context, Id, Order, RichText, vec2};

use super::design::{SURFACE_PADDING, TEXT_BODY, surface_frame};

const SHOWN_FOR: f64 = 3.0;

fn id() -> Id {
    Id::unique("repodeck-toast")
}

/// Shows `text`, replacing any toast still up.
pub fn post(ctx: &Context, text: impl Into<String>) {
    let until = ctx.input(|i| i.time) + SHOWN_FOR;
    let text = text.into();
    ctx.data_mut(|d| d.insert_temp(id(), (text, until)));
    ctx.request_repaint();
}

/// Draws the current toast, if any; call once a frame.
pub fn show(ctx: &Context) {
    let Some((text, until)) = ctx.data(|d| d.get_temp::<(String, f64)>(id())) else { return };
    let left = until - ctx.input(|i| i.time);
    if left <= 0.0 {
        ctx.data_mut(|d| d.remove::<(String, f64)>(id()));
        return;
    }
    Area::new(id())
        .order(Order::Tooltip)
        .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -44.0))
        .interactable(false)
        .show(ctx, |ui| {
            surface_frame(ui.visuals(), SURFACE_PADDING)
                .show(ui, |ui| ui.label(RichText::new(text).size(TEXT_BODY)));
        });
    ctx.request_repaint_after(Duration::from_secs_f64(left));
}
