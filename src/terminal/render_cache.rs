use std::cell::Cell as StdCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use alacritty_terminal::term::cell::Flags;
use eframe::egui::{Color32, Galley, Painter, Rect, Vec2, vec2};

use super::render::Grid;
use super::snapshot::Cell;
use super::style::TerminalStyle;
use super::{box_drawing, decorations};

thread_local! {
    /// Font atlas fill ratio and scale seen by the last paint, and a counter bumped whenever
    /// egui may have rebuilt the atlas since: kept galleys point into the old one then.
    static ATLAS: StdCell<(f32, f32, u64)> = const { StdCell::new((0.0, 0.0, 0)) };
}

/// What the cells of each row laid out to, kept while the frame and the style stay the same,
/// and row by row across frames: a terminal idles for far more frames than it changes, and when
/// it changes (an agent's spinner, a line of output) most rows read the same as before, often
/// just moved up a line. Rows are keyed by their content rather than their position, so a
/// scrolled row is reused too, and hold finished galleys so a replay neither copies nor hashes
/// a layout job.
#[derive(Default)]
pub struct PaintCache {
    key: Option<(u64, u64)>,
    /// The frame's rows, top to bottom: their row index and layout.
    rows: Vec<(i32, Arc<RowLayout>)>,
    /// The same layouts by content hash, to pick up again in the next frame.
    by_hash: HashMap<u64, Arc<RowLayout>>,
    /// Rows laid out (rather than reused) by the last rebuild.
    pub(super) laid_out: usize,
}

/// One row's shapes, placed relative to the row's top-left corner.
#[derive(Default)]
pub(super) struct RowLayout {
    pub(super) backgrounds: Vec<(Rect, Color32)>,
    /// Text runs: where they start, their galley, and the colour of their last section.
    pub(super) runs: Vec<(Vec2, Arc<Galley>, Color32)>,
    pub(super) items: Vec<Item>,
}

/// A cell painted on its own rather than as part of a text run.
pub(super) enum Item {
    Decoration {
        rect: Rect,
        flags: Flags,
        color: Color32,
    },
    Box {
        rect: Rect,
        c: char,
        color: Color32,
    },
    /// A glyph placed on its own cell: wide, or with an advance that doesn't match the cell.
    Glyph {
        pos: Vec2,
        galley: Arc<Galley>,
        color: Color32,
    },
}

impl PaintCache {
    /// Whether the cache holds frame `generation` laid out for `style`; otherwise the caller
    /// rebuilds it.
    pub(super) fn reuse(
        &mut self,
        generation: u64,
        style: &TerminalStyle<'_>,
        grid: Grid,
        painter: &Painter,
    ) -> bool {
        let style = style_key(style, grid, atlas_epoch(painter));
        if self.key.is_some_and(|(_, s)| s != style) {
            // Laid out for another font, size or atlas: nothing kept is any good.
            self.by_hash.clear();
        }
        let key = (generation, style);
        if self.key == Some(key) {
            return true;
        }
        self.key = Some(key);
        false
    }

    /// Rebuilds the row list from `cells` (ordered by row), reusing the layout of every row whose
    /// content was laid out last frame and calling `layout` for the rest.
    pub(super) fn rebuild(&mut self, cells: &[Cell], mut layout: impl FnMut(&[Cell], &mut RowLayout)) {
        let mut previous = std::mem::take(&mut self.by_hash);
        self.rows.clear();
        self.laid_out = 0;
        for row in cells.chunk_by(|a, b| a.row == b.row) {
            let hash = row_hash(row);
            let shapes = match self.by_hash.get(&hash).cloned().or_else(|| previous.remove(&hash)) {
                Some(shapes) => shapes,
                None => {
                    let mut shapes = RowLayout::default();
                    layout(row, &mut shapes);
                    self.laid_out += 1;
                    Arc::new(shapes)
                }
            };
            self.by_hash.insert(hash, shapes.clone());
            self.rows.push((row[0].row, shapes));
        }
    }

    /// Paints the cached rows at `grid`'s origin: every background first, so a row's rect never
    /// covers glyphs of the row above that reach into it, then decorations and glyphs.
    pub(super) fn replay(&self, painter: &Painter, grid: Grid) {
        let origins: Vec<Vec2> = self
            .rows
            .iter()
            .map(|(row, _)| grid.origin.to_vec2() + vec2(0.0, *row as f32 * grid.cell.y))
            .collect();
        for ((_, shapes), origin) in self.rows.iter().zip(&origins) {
            for (rect, color) in &shapes.backgrounds {
                painter.rect_filled(rect.translate(*origin), 0.0, *color);
            }
        }
        for ((_, shapes), origin) in self.rows.iter().zip(&origins) {
            for item in &shapes.items {
                match item {
                    Item::Decoration { rect, flags, color } => {
                        decorations::paint(painter, rect.translate(*origin), *flags, *color);
                    }
                    Item::Box { rect, c, color } => {
                        box_drawing::paint(painter, rect.translate(*origin), *c, *color);
                    }
                    Item::Glyph { pos, galley, color } => {
                        painter.galley((*origin + *pos).to_pos2(), galley.clone(), *color);
                    }
                }
            }
        }
        for ((_, shapes), origin) in self.rows.iter().zip(&origins) {
            for (pos, galley, color) in &shapes.runs {
                painter.galley((*origin + *pos).to_pos2(), galley.clone(), *color);
            }
        }
    }

    #[cfg(test)]
    pub(super) fn shape_count(&self) -> usize {
        self.rows.iter().map(|(_, s)| s.runs.len() + s.items.len()).sum()
    }
}

/// Drops every kept layout, in every pane: called when fonts are replaced (`ctx.set_fonts`),
/// which builds a new atlas that the fill ratio alone can't be relied on to reveal.
pub fn fonts_replaced() {
    ATLAS.with(|atlas| {
        let (_, scale, epoch) = atlas.get();
        atlas.set((0.0, scale, epoch + 1));
    });
}

/// Counts possible rebuilds of egui's font atlas: it starts over when it fills up or the text
/// options (the scale) change, and galleys laid out before then point at glyphs no longer there.
/// Both show as a drop in the fill ratio or a new scale by the next frame.
fn atlas_epoch(painter: &Painter) -> u64 {
    let fill = painter.fonts_mut(|f| f.font_atlas_fill_ratio());
    let scale = painter.pixels_per_point();
    ATLAS.with(|atlas| {
        let (last_fill, last_scale, mut epoch) = atlas.get();
        if fill < last_fill || scale != last_scale {
            epoch += 1;
        }
        atlas.set((fill, scale, epoch));
        epoch
    })
}

/// A row's content as the layout sees it, without its position.
fn row_hash(cells: &[Cell]) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();
    for cell in cells {
        (cell.col, cell.c, &cell.zerowidth, cell.fg, cell.bg, cell.flags.bits(), cell.width)
            .hash(&mut hasher);
    }
    hasher.finish()
}

/// Everything besides the frame that the layout depends on.
fn style_key(style: &TerminalStyle<'_>, grid: Grid, epoch: u64) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();
    style.font.hash(&mut hasher);
    style.bold_font.hash(&mut hasher);
    style.letter_spacing.to_bits().hash(&mut hasher);
    grid.cell.x.to_bits().hash(&mut hasher);
    grid.cell.y.to_bits().hash(&mut hasher);
    grid.text_offset.to_bits().hash(&mut hasher);
    style.theme.background.color().hash(&mut hasher);
    epoch.hash(&mut hasher);
    hasher.finish()
}
