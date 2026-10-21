use eframe::egui::{Frame, Id, Margin, ScrollArea, Ui};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

/// Rendered Markdown preview of `text`, scrollable, with reading-width margins.
pub fn show(ui: &mut Ui, cache: &mut CommonMarkCache, text: &str, id: Id) {
    ScrollArea::vertical().id_salt(id.with("markdown")).auto_shrink(false).show(ui, |ui| {
        Frame::new().inner_margin(Margin::symmetric(24, 16)).show(ui, |ui| {
            ui.set_max_width(860.0);
            ui.spacing_mut().item_spacing.y = 8.0;
            CommonMarkViewer::new().show(ui, cache, text);
        });
    });
}
