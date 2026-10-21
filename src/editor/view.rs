use std::path::Path;
use std::sync::Arc;

use eframe::egui::text::{CCursor, CCursorRange, LayoutJob};
use eframe::egui::{
    Align, Button, Color32, FontId, Frame, Galley, Id, Label, Layout, Margin, RichText, ScrollArea,
    TextBuffer, TextEdit, TextFormat, Ui,
};
use egui_extras::syntax_highlighting::{CodeTheme, highlight};

use super::buffer::{Buffer, Content, is_markdown};
use super::markdown;
use crate::model::Settings;

/// Highlighting re-runs on every edit, so very large files are shown as plain text.
const MAX_HIGHLIGHT_CHARS: usize = 200_000;

/// Shows the editor for `path`; returns true when the user asked to save. `goto` places the
/// cursor at a one-based line and column and scrolls it into view.
pub fn show(
    ui: &mut Ui,
    buffer: &mut Buffer,
    path: &Path,
    root: &Path,
    settings: &Settings,
    id: Id,
    goto: Option<(usize, usize)>,
) -> bool {
    let mut save = false;
    let muted = ui.visuals().text_color().gamma_multiply(0.55);
    Frame::new().inner_margin(Margin::symmetric(14, 8)).show(ui, |ui| {
        ui.horizontal(|ui| {
            let relative = path.strip_prefix(root).unwrap_or(path);
            ui.label(RichText::new(relative.display().to_string()).color(muted).size(12.5));
            if buffer.is_dirty() {
                ui.label(RichText::new("●").color(settings.theme.ansi[3].color()).size(11.0))
                    .on_hover_text("Unsaved changes");
            }
            if let Some(err) = &buffer.save_error {
                ui.label(RichText::new(err).color(Color32::LIGHT_RED).size(12.0));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let button = Button::new("Save").shortcut_text("⌘S");
                save = ui.add_enabled(buffer.is_dirty(), button).clicked();
                if is_markdown(path) {
                    ui.add_space(8.0);
                    ui.selectable_value(&mut buffer.preview, true, "Preview");
                    ui.selectable_value(&mut buffer.preview, false, "Edit");
                }
            });
        });
    });
    ui.separator();

    let lines = match &buffer.content {
        Content::Text(text) => text.split('\n').count(),
        Content::Unavailable(_) => 0,
    };
    let gutter = buffer.gutter(lines).to_string();
    let Buffer { content, markdown: cache, preview, .. } = buffer;
    let text = match content {
        Content::Text(text) => text,
        Content::Unavailable(reason) => {
            ui.centered_and_justified(|ui| ui.label(RichText::new(reason.as_str()).color(muted)));
            return save;
        }
    };
    if *preview {
        markdown::show(ui, cache, text, id);
        return save;
    }

    let font = FontId::monospace(settings.font_size);
    let language = language_for(path);
    let code_theme =
        if settings.theme.is_light() { CodeTheme::light(font.size) } else { CodeTheme::dark(font.size) }
            .with_font_id(font.clone());
    let mut layouter = |ui: &Ui, buf: &dyn TextBuffer, _wrap: f32| -> Arc<Galley> {
        let code = buf.as_str();
        let mut job = if code.len() <= MAX_HIGHLIGHT_CHARS {
            highlight(ui.ctx(), ui.style(), &code_theme, code, &language)
        } else {
            let color = ui.visuals().text_color();
            LayoutJob::single_section(code.to_string(), TextFormat::simple(font.clone(), color))
        };
        job.wrap.max_width = f32::INFINITY;
        ui.fonts_mut(|f| f.layout_job(job))
    };

    if let Some((line, col)) = goto {
        let index = char_index(text, line, col);
        let mut state = TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
        state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(index))));
        TextEdit::store_state(ui.ctx(), id, state);
        ui.memory_mut(|m| m.request_focus(id));
    }
    let row_height = ui.fonts_mut(|f| f.row_height(&font));
    let mut area = ScrollArea::both().id_salt(id.with("scroll")).auto_shrink(false);
    if let Some((line, _)) = goto {
        area = area.vertical_scroll_offset((line.saturating_sub(4) as f32 * row_height).max(0.0));
    }
    area.show(ui, |ui| {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            ui.add(Label::new(RichText::new(&gutter).font(font.clone()).color(muted)).selectable(false));
            ui.add(
                TextEdit::multiline(text)
                    .id(id)
                    .code_editor()
                    .font(font.clone())
                    .frame(Frame::NONE)
                    .desired_width(f32::INFINITY)
                    .desired_rows(lines.max(1))
                    .lock_focus(true)
                    .layouter(&mut layouter),
            );
        });
    });
    save
}

/// Character offset of a one-based line and column, clamped to the text.
fn char_index(text: &str, line: usize, col: usize) -> usize {
    let mut index = 0;
    for (i, row) in text.split('\n').enumerate() {
        let chars = row.chars().count();
        if i + 1 == line.max(1) {
            return index + col.max(1).min(chars + 1) - 1;
        }
        index += chars + 1;
    }
    text.chars().count()
}

/// syntect accepts either a syntax name or a file extension.
fn language_for(path: &Path) -> String {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    match name {
        "Dockerfile" | "Containerfile" => "Dockerfile".into(),
        "Makefile" | "makefile" | "GNUmakefile" => "Makefile".into(),
        _ => path.extension().and_then(|e| e.to_str()).unwrap_or("txt").to_string(),
    }
}
