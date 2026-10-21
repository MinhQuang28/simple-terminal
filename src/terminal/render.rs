use std::cell::RefCell;
use std::collections::HashMap;

use alacritty_terminal::term::TermMode;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::CursorShape;
use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{Color32, FontFamily, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2, vec2};

use super::snapshot::{Cell, Frame};
use super::style::TerminalStyle;
use super::{box_drawing, decorations};

thread_local! {
    /// Whether a glyph's advance matches the cell width, so it can share a text run with ASCII
    /// instead of being placed on its own cell. Keyed by font since fallbacks differ per font.
    static FITS_CELL: RefCell<HashMap<(char, FontFamily, u32), bool>> = RefCell::new(HashMap::new());
}

/// Pixel geometry of the terminal grid.
#[derive(Clone, Copy)]
pub struct Grid {
    pub origin: Pos2,
    pub cell: Vec2,
    /// Vertical offset that centers a font row inside a (possibly taller) cell.
    pub text_offset: f32,
}

impl Grid {
    pub fn cell_rect(&self, row: i32, col: usize, width: usize) -> Rect {
        let min = self.origin + vec2(col as f32 * self.cell.x, row as f32 * self.cell.y);
        Rect::from_min_size(min, vec2(self.cell.x * width as f32, self.cell.y))
    }
}

#[derive(Clone, Copy)]
pub struct CursorState {
    pub focused: bool,
    /// False during the "off" phase of a blinking cursor.
    pub visible: bool,
}

/// Consecutive cells on one row with the same weight, drawn as one text shape; color changes
/// only add a section.
struct TextRun {
    row: i32,
    col: usize,
    len: usize,
    bold: bool,
    job: LayoutJob,
    pending: String,
    color: Color32,
}

/// Consecutive cells on one row with the same background, drawn as one rect.
struct BgRun {
    row: i32,
    col: usize,
    end: usize,
    color: Color32,
}

struct Painting<'a> {
    painter: &'a Painter,
    grid: Grid,
    style: &'a TerminalStyle<'a>,
}

impl Painting<'_> {
    fn format(&self, bold: bool, color: Color32) -> TextFormat {
        let font = if bold { &self.style.bold_font } else { &self.style.font };
        TextFormat {
            font_id: font.clone(),
            color,
            extra_letter_spacing: self.style.letter_spacing,
            ..Default::default()
        }
    }

    fn text(&self, cell: Rect, text: String, bold: bool, color: Color32) {
        let galley = self.painter.layout_job(LayoutJob::single_section(text, self.format(bold, color)));
        self.painter.galley(cell.min + vec2(0.0, self.grid.text_offset), galley, color);
    }

    fn flush_text(&self, run: &mut Option<TextRun>) {
        let Some(mut r) = run.take() else { return };
        r.job.append(&r.pending, 0.0, self.format(r.bold, r.color));
        let galley = self.painter.layout_job(r.job);
        let pos = self.grid.cell_rect(r.row, r.col, 1).min + vec2(0.0, self.grid.text_offset);
        self.painter.galley(pos, galley, r.color);
    }

    fn flush_bg(&self, run: &mut Option<BgRun>) {
        if let Some(r) = run.take() {
            self.painter.rect_filled(self.grid.cell_rect(r.row, r.col, r.end - r.col), 0.0, r.color);
        }
    }

    fn fits_cell(&self, c: char, bold: bool) -> bool {
        if c.is_ascii() {
            return true;
        }
        let font = if bold { &self.style.bold_font } else { &self.style.font };
        FITS_CELL.with_borrow_mut(|cache| {
            *cache.entry((c, font.family.clone(), font.size.to_bits())).or_insert_with(|| {
                let (width, reference) =
                    self.painter.fonts_mut(|f| (f.glyph_width(font, c), f.glyph_width(font, 'M')));
                (width - reference).abs() < 0.5
            })
        })
    }
}

/// Backgrounds go down before any text so a row's rect never covers glyphs painted earlier.
fn paint_backgrounds(p: &Painting<'_>, frame: &Frame) {
    let background = p.style.theme.background.color();
    let mut bg_run: Option<BgRun> = None;
    for cell in &frame.cells {
        let Cell { row, col, bg, width, .. } = *cell;
        let continues = bg_run.as_ref().is_some_and(|r| r.row == row && r.color == bg && r.end == col);
        match bg_run.as_mut() {
            Some(r) if continues => r.end = col + width,
            _ => {
                p.flush_bg(&mut bg_run);
                if bg != background {
                    bg_run = Some(BgRun { row, col, end: col + width, color: bg });
                }
            }
        }
    }
    p.flush_bg(&mut bg_run);
}

/// Paints the captured frame and cursor; returns the cursor rect for IME placement.
pub fn paint(
    painter: &Painter,
    grid: Grid,
    style: &TerminalStyle<'_>,
    frame: &Frame,
    cursor_state: CursorState,
    preedit: Option<&str>,
) -> Rect {
    let p = Painting { painter, grid, style };
    paint_backgrounds(&p, frame);
    let mut run: Option<TextRun> = None;

    for cell in &frame.cells {
        let Cell { row, col, c, fg, flags, width, .. } = *cell;
        let bold = flags.contains(Flags::BOLD);
        let rect = grid.cell_rect(row, col, width);
        decorations::paint(painter, rect, flags, fg);
        if c == ' ' && cell.zerowidth.is_none() {
            // Spaces only extend an open run; one that ends in spaces is simply shorter.
            if let Some(r) = run.as_mut().filter(|r| r.row == row && r.col + r.len == col) {
                r.pending.push(' ');
                r.len += 1;
            }
            continue;
        }
        if box_drawing::paint(painter, rect, c, fg) {
            p.flush_text(&mut run);
            continue;
        }

        let continues_run =
            run.as_ref().is_some_and(|r| r.row == row && r.bold == bold && r.col + r.len == col);
        if width == 1 && cell.zerowidth.is_none() && p.fits_cell(c, bold) {
            if !continues_run {
                p.flush_text(&mut run);
                run = Some(TextRun {
                    row,
                    col,
                    len: 0,
                    bold,
                    job: LayoutJob::default(),
                    pending: String::new(),
                    color: fg,
                });
            }
            if let Some(r) = run.as_mut() {
                if r.color != fg {
                    r.job.append(&r.pending, 0.0, p.format(bold, r.color));
                    r.pending.clear();
                    r.color = fg;
                }
                r.pending.push(c);
                r.len += 1;
            }
        } else {
            // Wide glyphs and ones with other advances are placed on their own cell.
            p.flush_text(&mut run);
            let mut text = c.to_string();
            text.extend(cell.zerowidth.iter().flat_map(|z| z.iter()));
            p.text(rect, text, bold, fg);
        }
    }
    p.flush_text(&mut run);

    let cursor = &frame.cursor;
    let cursor_rect = grid.cell_rect(cursor.row, cursor.col, 1);
    if let Some(text) = preedit.filter(|t| !t.is_empty()) {
        paint_preedit(&p, cursor_rect, text);
    } else if cursor.shape != CursorShape::Hidden
        && !frame.mode.contains(TermMode::VI)
        && cursor_state.visible
    {
        paint_cursor(&p, cursor_rect, cursor.shape, cursor_state.focused, cursor.under);
    }
    cursor_rect
}

fn paint_cursor(
    p: &Painting<'_>,
    rect: Rect,
    shape: CursorShape,
    focused: bool,
    under: Option<(char, Color32, bool)>,
) {
    let theme = p.style.theme;
    let color = theme.cursor.color();
    match shape {
        CursorShape::Block if focused => {
            p.painter.rect_filled(rect, 0.0, color);
            if let Some((c, bg, bold)) = under.filter(|(c, _, _)| !c.is_whitespace()) {
                let text_color = if bg == theme.selection.color() { theme.background.color() } else { bg };
                if !box_drawing::paint(p.painter, rect, c, text_color) {
                    p.text(rect, c.to_string(), bold, text_color);
                }
            }
        }
        CursorShape::Beam => {
            p.painter.vline(rect.left() + 1.0, rect.y_range(), Stroke::new(2.0, color));
        }
        CursorShape::Underline => {
            p.painter.hline(rect.x_range(), rect.bottom() - 1.0, Stroke::new(2.0, color));
        }
        CursorShape::Block | CursorShape::HollowBlock => {
            p.painter.rect_stroke(rect, 0.0, Stroke::new(1.0, color), StrokeKind::Inside);
        }
        CursorShape::Hidden => {}
    }
}

fn paint_preedit(p: &Painting<'_>, cursor: Rect, text: &str) {
    let theme = p.style.theme;
    let width = p.grid.cell.x * text.chars().count() as f32;
    let rect = Rect::from_min_size(cursor.min, vec2(width, p.grid.cell.y));
    p.painter.rect_filled(rect, 0.0, theme.background.color());
    p.text(rect, text.to_string(), false, theme.foreground.color());
    p.painter.hline(rect.x_range(), rect.bottom() - 1.0, Stroke::new(1.0, theme.cursor.color()));
}

/// Cell size and text offset for the given style.
pub fn metrics(painter: &Painter, style: &TerminalStyle<'_>) -> (Vec2, f32) {
    let font: &FontId = &style.font;
    let (glyph_width, row_height) = painter.fonts_mut(|f| (f.glyph_width(font, 'M'), f.row_height(font)));
    let cell_height = (font.size * style.line_height).round();
    let cell = vec2(glyph_width + style.letter_spacing, cell_height);
    (cell, ((cell_height - row_height) / 2.0).floor())
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
