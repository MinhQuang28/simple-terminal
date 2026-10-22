use std::path::Path;
use std::sync::Arc;

use super::buffer::{Buffer, Content, is_markdown};
use super::find_bar::{self, BarKind, EditorBar, Found, Jump, char_index, line_of};
use super::{highlight, markdown};
use crate::model::Settings;
use eframe::egui::text::{CCursor, CCursorRange, LayoutJob};
use eframe::egui::text_edit::TextEditOutput;
use eframe::egui::{
    Align, Button, Color32, FontId, Frame, Galley, Id, Key, Label, Layout, Margin, Modifiers, Rect, RichText,
    ScrollArea, TextBuffer, TextEdit, TextFormat, Ui, pos2,
};

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
    let Buffer { content, markdown: cache, preview, bar, .. } = buffer;
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
    let wrap = settings.editor_word_wrap;
    let mut layouter = |ui: &Ui, buf: &dyn TextBuffer, width: f32| -> Arc<Galley> {
        let code = buf.as_str();
        let mut job = if code.len() <= MAX_HIGHLIGHT_CHARS {
            highlight::layout_job(ui.ctx(), &settings.theme, &font, &language, code)
        } else {
            let color = ui.visuals().text_color();
            LayoutJob::single_section(code.to_string(), TextFormat::simple(font.clone(), color))
        };
        job.wrap.max_width = if wrap { width } else { f32::INFINITY };
        ui.fonts_mut(|f| f.layout_job(job))
    };

    // ⌘F and ⌘L open the find and go-to-line bars while a file shows.
    let (find, go_to) = ui.input_mut(|i| {
        (i.consume_key(Modifiers::COMMAND, Key::F), i.consume_key(Modifiers::COMMAND, Key::L))
    });
    if find {
        *bar = Some(EditorBar::new(BarKind::Find));
    } else if go_to {
        *bar = Some(EditorBar::new(BarKind::GoToLine));
    }
    let mut jump = goto.map(|(line, col)| {
        let index = char_index(text, line, col);
        Jump { start: index, end: index }
    });
    let mut focus_text = goto.is_some();
    if let Some(open) = bar {
        let (bar_jump, close) = Frame::new()
            .inner_margin(Margin::symmetric(14, 6))
            .show(ui, |ui| find_bar::show(ui, open, text))
            .inner;
        if bar_jump.is_some() {
            jump = bar_jump;
            focus_text |= close;
        }
        if close {
            *bar = None;
            focus_text = true;
        }
    }
    if let Some(Jump { start, end }) = jump {
        let mut state = TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
        state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(start), CCursor::new(end))));
        TextEdit::store_state(ui.ctx(), id, state);
    }
    if focus_text {
        ui.memory_mut(|m| m.request_focus(id));
    }
    let row_height = ui.fonts_mut(|f| f.row_height(&font));
    let mut area = ScrollArea::both().id_salt(id.with("scroll")).auto_shrink(false);
    if let Some(Jump { start, .. }) = jump {
        let line = line_of(text, start);
        area = area.vertical_scroll_offset((line.saturating_sub(4) as f32 * row_height).max(0.0));
    }
    let matches = bar.as_ref().filter(|b| b.kind == BarKind::Find).map(|b| b.matches());
    let accent = settings.theme.accent();
    area.show(ui, |ui| {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            ui.add(Label::new(RichText::new(&gutter).font(font.clone()).color(muted)).selectable(false));
            let output = TextEdit::multiline(text)
                .id(id)
                .code_editor()
                .font(font.clone())
                .frame(Frame::NONE)
                .desired_width(f32::INFINITY)
                .desired_rows(lines.max(1))
                .lock_focus(true)
                .layouter(&mut layouter)
                .show(ui);
            if let Some((matches, current)) = matches {
                paint_matches(ui, &output, matches, current, row_height, wrap, accent);
            }
        });
    });
    save
}

/// Highlights the matches on view. Measuring a match walks the galley's rows, so only matches
/// on lines that can be on screen are measured: with wrapping off a line is one row, with it on
/// a line's row is at or past its number, so the lower bound is dropped.
fn paint_matches(
    ui: &Ui,
    output: &TextEditOutput,
    matches: &[Found],
    current: usize,
    row_height: f32,
    wrap: bool,
    accent: Color32,
) {
    let clip = output.text_clip_rect;
    let top = clip.top() - output.galley_pos.y;
    let first = if wrap { 0 } else { (top / row_height).floor().max(0.0) as usize };
    let last = ((clip.bottom() - output.galley_pos.y) / row_height).ceil().max(0.0) as usize;
    let from = matches.partition_point(|m| m.line < first);
    let painter = ui.painter().with_clip_rect(clip);
    for (i, m) in matches.iter().enumerate().skip(from).take_while(|(_, m)| m.line <= last).take(2000) {
        let start = output.galley.pos_from_cursor(CCursor::new(m.start));
        let end = output.galley.pos_from_cursor(CCursor::new(m.end));
        let rect = Rect::from_min_max(start.min, pos2(end.min.x.max(start.min.x), start.max.y))
            .translate(output.galley_pos.to_vec2());
        if rect.intersects(clip) {
            let alpha = if i == current { 0.55 } else { 0.25 };
            painter.rect_filled(rect, 2.0, accent.gamma_multiply(alpha));
        }
    }
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
