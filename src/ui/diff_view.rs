use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Arc;

use eframe::egui::{
    Align, Align2, Button, Color32, FontId, Frame, Galley, Label, Layout, Margin, Rect, RichText, ScrollArea,
    Ui, pos2,
};
use uuid::Uuid;

use super::Action;
use super::design::TEXT_SECONDARY;
use super::diff_comments::{self, Comment};
use super::diff_rows::{self, DiffRows, RowKind};
use super::widgets::muted;
use crate::editor::{highlight_job, highlight_ready};
use crate::model::Settings;

/// Row height as a multiple of the font size.
const LINE_HEIGHT: f32 = 1.55;

/// A diff laid out for display, kept until the diff or the look it was built for changes.
pub struct DiffLaid {
    key: u64,
    /// The code of every row, syntax-coloured, one line per row.
    galley: Arc<Galley>,
    rows: DiffRows,
}

/// Shows a unified diff with a header offering to open the file or hand the diff to Claude.
/// `text` is `None` while the first diff is still being computed; `laid` caches its layout.
#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut Ui,
    repo: Uuid,
    root: &Path,
    file: &Path,
    text: Option<&Arc<str>>,
    laid: &mut Option<DiffLaid>,
    comments: &mut Vec<Comment>,
    settings: &Settings,
    actions: &mut Vec<Action>,
) {
    let relative = file.strip_prefix(root).unwrap_or(file);
    Frame::new().inner_margin(Margin::symmetric(14, 8)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(relative.display().to_string()).color(muted(ui)).size(TEXT_SECONDARY));
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
        // Laid out plain while the grammars load, then again in colour.
        highlight_ready(),
    )
        .hash(&mut hasher);
    let key = hasher.finish();
    let laid = match laid {
        Some(laid) if laid.key == key => laid,
        slot => {
            let rows = diff_rows::parse(text);
            let language = file.extension().and_then(|e| e.to_str()).unwrap_or("txt");
            let mut job = highlight_job(ui.ctx(), &settings.theme, &font, language, &rows.code);
            // Roomier than the font's own line height, so the bands of changed rows read apart.
            let line_height = (font.size * LINE_HEIGHT).round();
            job.sections.iter_mut().for_each(|s| s.format.line_height = Some(line_height));
            let galley = ui.fonts_mut(|f| f.layout_job(job));
            slot.insert(DiffLaid { key, galley, rows })
        }
    };
    // Comments are kept by row, so they quote the diff from its first row on.
    let header: usize = text.lines().take(laid.rows.skip).map(|l| l.len() + 1).sum();
    let shown = text.get(header..).unwrap_or_default();
    diff_comments::list(ui, repo, relative, shown, comments, actions);
    ScrollArea::both().auto_shrink(false).show(ui, |ui| {
        ui.add_space(4.0);
        let glyph = ui.fonts_mut(|f| f.glyph_width(&font, '0'));
        let digits = laid.rows.rows.iter().filter_map(|r| r.number).max().unwrap_or(0).to_string().len();
        // Left of the code: the comment gutter, the line number, then the +/- column.
        let numbers = glyph * (digits as f32 + 1.0);
        let margin = numbers + glyph * 2.5;
        rows(ui, laid, shown, &font, numbers, settings);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            diff_comments::gutter(ui, &laid.galley, file, comments);
            ui.add_space(margin);
            ui.add(Label::new(laid.galley.clone()).selectable(true));
        });
    });
}

/// Paints what surrounds the code of the rows in view: a band across added, removed and hunk
/// rows, each row's line number and sign, and the text of rows that aren't code.
fn rows(ui: &Ui, laid: &DiffLaid, shown: &str, font: &FontId, numbers: f32, settings: &Settings) {
    let all = &laid.rows.rows;
    let row_height = laid.galley.size().y / all.len().max(1) as f32;
    let (left, top) = (ui.cursor().left() + diff_comments::GUTTER_WIDTH, ui.cursor().top());
    let clip = ui.clip_rect();
    let first = ((clip.top() - top) / row_height).floor().max(0.0) as usize;
    let last = (((clip.bottom() - top) / row_height).ceil().max(0.0) as usize).min(all.len());
    let glyph = ui.fonts_mut(|f| f.glyph_width(font, '0'));
    let code_x = left + numbers + glyph * 2.5;
    let alpha = if settings.theme.is_light() { 60 } else { 46 };
    let added = Color32::from_rgba_unmultiplied(0x3f, 0xb9, 0x50, alpha);
    let removed = Color32::from_rgba_unmultiplied(0xf8, 0x51, 0x49, alpha);
    let hunk = settings.theme.ansi[4].color();
    let painter = ui.painter();
    let lines = shown.lines().skip(first);
    for ((index, row), line) in all.iter().enumerate().take(last).skip(first).zip(lines) {
        let y = top + index as f32 * row_height;
        let band = Rect::from_min_max(pos2(clip.left(), y), pos2(clip.right(), y + row_height));
        let center = y + row_height / 2.0;
        let (fill, sign) = match row.kind {
            RowKind::Added => (Some(added), "+"),
            RowKind::Removed => (Some(removed), "-"),
            RowKind::Hunk => (Some(hunk.gamma_multiply(0.16)), ""),
            RowKind::Context | RowKind::Meta => (None, ""),
        };
        if let Some(fill) = fill {
            painter.rect_filled(band, 0.0, fill);
        }
        if let Some(number) = row.number {
            let at = pos2(left + numbers, center);
            painter.text(at, Align2::RIGHT_CENTER, number, font.clone(), muted(ui));
        }
        painter.text(
            pos2(left + numbers + glyph, center),
            Align2::LEFT_CENTER,
            sign,
            font.clone(),
            muted(ui),
        );
        match row.kind {
            RowKind::Hunk => {
                painter.text(pos2(code_x, center), Align2::LEFT_CENTER, line, font.clone(), hunk);
            }
            RowKind::Meta => {
                painter.text(pos2(code_x, center), Align2::LEFT_CENTER, line, font.clone(), muted(ui));
            }
            _ => {}
        }
    }
}
