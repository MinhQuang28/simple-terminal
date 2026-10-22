use std::hash::{Hash, Hasher};

use alacritty_terminal::term::cell::Flags;
use eframe::egui::text::LayoutJob;
use eframe::egui::{Color32, Painter, Rect, Vec2};

use super::render::Grid;
use super::style::TerminalStyle;
use super::{box_drawing, decorations};

/// What one frame's cells laid out to, relative to the grid origin, replayed while the frame
/// and the style stay the same: a terminal idles for far more frames than it changes, and
/// laying out every cell again each frame was the view's main steady-state cost.
#[derive(Default)]
pub struct PaintCache {
    key: Option<(u64, u64)>,
    pub(super) backgrounds: Vec<(Rect, Color32)>,
    /// Text runs: where they start, their layout, and the colour of their last section.
    pub(super) runs: Vec<(Vec2, LayoutJob, Color32)>,
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
        job: LayoutJob,
        color: Color32,
    },
}

impl PaintCache {
    /// Whether the cache holds frame `generation` laid out for `style`; otherwise clears it and
    /// returns false so the caller fills it.
    pub(super) fn reuse(&mut self, generation: u64, style: &TerminalStyle<'_>, grid: Grid) -> bool {
        let key = (generation, style_key(style, grid));
        if self.key == Some(key) {
            return true;
        }
        self.key = Some(key);
        self.backgrounds.clear();
        self.runs.clear();
        self.items.clear();
        false
    }

    /// Paints the cached layout at `grid`'s origin. Galleys come from egui's own layout cache,
    /// which keeps a job's galley while it is laid out every frame, so a replay hashes each run
    /// rather than shaping it.
    pub(super) fn replay(&self, painter: &Painter, grid: Grid) {
        let origin = grid.origin.to_vec2();
        for (rect, color) in &self.backgrounds {
            painter.rect_filled(rect.translate(origin), 0.0, *color);
        }
        for item in &self.items {
            match item {
                Item::Decoration { rect, flags, color } => {
                    decorations::paint(painter, rect.translate(origin), *flags, *color);
                }
                Item::Box { rect, c, color } => {
                    box_drawing::paint(painter, rect.translate(origin), *c, *color);
                }
                Item::Glyph { pos, job, color } => {
                    painter.galley(grid.origin + *pos, painter.layout_job(job.clone()), *color);
                }
            }
        }
        for (pos, job, color) in &self.runs {
            painter.galley(grid.origin + *pos, painter.layout_job(job.clone()), *color);
        }
    }
}

/// Everything besides the frame that the layout depends on.
fn style_key(style: &TerminalStyle<'_>, grid: Grid) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();
    style.font.hash(&mut hasher);
    style.bold_font.hash(&mut hasher);
    style.letter_spacing.to_bits().hash(&mut hasher);
    grid.cell.x.to_bits().hash(&mut hasher);
    grid.cell.y.to_bits().hash(&mut hasher);
    grid.text_offset.to_bits().hash(&mut hasher);
    style.theme.background.color().hash(&mut hasher);
    hasher.finish()
}
