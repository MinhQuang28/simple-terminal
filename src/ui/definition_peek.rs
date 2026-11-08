//! Peek Definition (⌥F12, or F12 with several matches): the declarations of a name with a few
//! lines of each, over the editor. Clicking one opens it there; `Esc` or a click outside closes.

use eframe::egui::{
    Context, FontId, Id, Label, Modal, RichText, ScrollArea, Sense, TextFormat, Ui, text::LayoutJob,
};

use super::Action;
use super::design;
use super::file_icon;
use super::widgets::{accent, muted, row_sized};
use crate::definitions::{Definition, Peek};

pub fn show(ctx: &Context, peek: &Peek, actions: &mut Vec<Action>) {
    let frame = design::surface_frame(&ctx.global_style().visuals, 12);
    let width = (ctx.content_rect().width() * 0.6).max(420.0);
    let modal = Modal::new(Id::unique("definition-peek")).frame(frame).show(ctx, |ui| {
        ui.set_width(width);
        let count = match peek.found.len() {
            0 => "no definition found".to_string(),
            1 => "1 definition".to_string(),
            n => format!("{n} definitions"),
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(&peek.word).monospace().strong());
            ui.label(RichText::new(count).color(muted(ui)).size(12.0));
        });
        ui.add_space(8.0);
        ScrollArea::vertical().max_height(ctx.content_rect().height() * 0.6).show(ui, |ui| {
            for definition in &peek.found {
                entry(ui, peek, definition, actions);
                ui.add_space(6.0);
            }
        });
    });
    if modal.should_close() {
        actions.push(Action::ClosePeekDefinition);
    }
}

/// `file.rs  src/ui  :42`, then the lines from the declaration down; clicking opens it.
fn entry(ui: &mut Ui, peek: &Peek, definition: &Definition, actions: &mut Vec<Action>) {
    let name = definition.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let folder = definition.path.parent().and_then(|p| p.strip_prefix(&peek.root).ok());
    let (header, ()) = row_sized(ui, false, 0.0, 24.0, |ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        file_icon::show(ui, 15.0, &name, false, false);
        ui.label(RichText::new(&name).size(12.5));
        if let Some(folder) = folder.filter(|f| !f.as_os_str().is_empty()) {
            ui.label(RichText::new(folder.display().to_string()).size(11.0).color(muted(ui)));
        }
        ui.label(RichText::new(format!(":{}", definition.line)).size(11.0).color(muted(ui)));
    });
    let font = FontId::monospace(12.0);
    let mut job = LayoutJob::default();
    for (i, line) in definition.excerpt.iter().enumerate() {
        let color = if i == 0 { ui.visuals().strong_text_color() } else { muted(ui) };
        let mut format = TextFormat::simple(font.clone(), color);
        if i == 0 {
            format.background = accent(ui).gamma_multiply(0.15);
        }
        job.append(line, 0.0, format);
        job.append("\n", 0.0, TextFormat::simple(font.clone(), color));
    }
    let code = ui.add(Label::new(job).selectable(false).sense(Sense::click()));
    if header.clicked() || code.clicked() {
        let (line, col) = (Some(definition.line as u32), Some(definition.col as u32));
        actions.push(Action::OpenFileAt { repo: peek.repo, path: definition.path.clone(), line, col });
        actions.push(Action::ClosePeekDefinition);
    }
}
