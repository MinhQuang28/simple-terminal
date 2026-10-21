use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Arc;

use eframe::egui::text::LayoutJob;
use eframe::egui::{
    Align, Button, Color32, FontId, Frame, Galley, Label, Layout, Margin, RichText, ScrollArea, TextFormat,
    Ui,
};
use uuid::Uuid;

use super::Action;
use super::diff_comments::{self, Comment};
use super::widgets::muted;
use crate::model::Settings;

/// Shows a unified diff with a header offering to open the file or hand the diff to Claude.
/// `text` is `None` while the first diff is still being computed; `galley` caches its layout.
#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut Ui,
    repo: Uuid,
    root: &Path,
    file: &Path,
    text: Option<&Arc<str>>,
    galley: &mut Option<(u64, Arc<Galley>)>,
    comments: &mut Vec<Comment>,
    settings: &Settings,
    actions: &mut Vec<Action>,
) {
    let relative = file.strip_prefix(root).unwrap_or(file);
    Frame::new().inner_margin(Margin::symmetric(14, 8)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(relative.display().to_string()).color(muted(ui)).size(12.5));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_enabled(text.is_some(), Button::new("Send to Claude"))
                    .on_hover_text("Paste the diff into this repo's Claude pane")
                    .clicked()
                    && let Some(text) = text
                {
                    let prompt =
                        format!("Review this diff of {}:\n```diff\n{text}\n```\n", relative.display());
                    actions.push(Action::SendToAgent { repo, text: prompt });
                }
                if file.is_file() && ui.button("Open file").clicked() {
                    actions.push(Action::OpenFile { repo, path: file.to_path_buf() });
                }
            });
        });
    });
    ui.separator();
    let Some(text) = text else {
        ui.centered_and_justified(|ui| ui.label(RichText::new("Computing diff…").color(muted(ui))));
        return;
    };
    if text.trim().is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new("No changes against HEAD").color(muted(ui)));
        });
        return;
    }
    let font = FontId::monospace(settings.font_size);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (
        Arc::as_ptr(text) as *const u8 as usize,
        font.size.to_bits(),
        &settings.theme.name,
        ui.pixels_per_point().to_bits(),
    )
        .hash(&mut hasher);
    let key = hasher.finish();
    let laid_out = match galley {
        Some((k, g)) if *k == key => g.clone(),
        _ => {
            let g = ui.fonts_mut(|f| f.layout_job(layout(ui, text, &font, settings)));
            *galley = Some((key, g.clone()));
            g
        }
    };
    diff_comments::list(ui, repo, relative, text, comments, actions);
    ScrollArea::both().auto_shrink(false).show(ui, |ui| {
        ui.add_space(4.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            diff_comments::gutter(ui, &laid_out, file, comments);
            ui.add(Label::new(laid_out).selectable(true));
        });
    });
}

/// Colors added, removed and hunk-header lines of a unified diff.
fn layout(ui: &Ui, text: &str, font: &FontId, settings: &Settings) -> LayoutJob {
    let base = ui.visuals().text_color();
    let dark = !settings.theme.is_light();
    let (added, removed) = if dark {
        (
            Color32::from_rgba_unmultiplied(0x3f, 0xb9, 0x50, 40),
            Color32::from_rgba_unmultiplied(0xf8, 0x51, 0x49, 40),
        )
    } else {
        (
            Color32::from_rgba_unmultiplied(0x3f, 0xb9, 0x50, 70),
            Color32::from_rgba_unmultiplied(0xf8, 0x51, 0x49, 70),
        )
    };
    let hunk = settings.theme.ansi[4].color();
    let mut job = LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;
    for line in text.lines() {
        let format = match line.as_bytes().first() {
            Some(b'+') if !line.starts_with("+++") => {
                TextFormat { font_id: font.clone(), color: base, background: added, ..Default::default() }
            }
            Some(b'-') if !line.starts_with("---") => {
                TextFormat { font_id: font.clone(), color: base, background: removed, ..Default::default() }
            }
            Some(b'@') => TextFormat { font_id: font.clone(), color: hunk, ..Default::default() },
            Some(b'd' | b'i' | b'n' | b'o' | b'r' | b's') | None => {
                TextFormat { font_id: font.clone(), color: muted(ui), ..Default::default() }
            }
            _ => TextFormat { font_id: font.clone(), color: base, ..Default::default() },
        };
        job.append(line, 0.0, format.clone());
        job.append("\n", 0.0, format);
    }
    job
}
