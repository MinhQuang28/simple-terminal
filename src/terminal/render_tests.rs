use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::CursorShape;
use eframe::egui::{Color32, Context, LayerId, Painter, RawInput, Rect, Shape, pos2, vec2};

use super::*;
use crate::model::Settings;
use crate::terminal::render_cache::PaintCache;
use crate::terminal::snapshot::{Cell, Cursor, Frame};

fn cell(col: usize, c: char, bg: Color32) -> Cell {
    Cell { row: 0, col, c, zerowidth: None, fg: Color32::WHITE, bg, flags: Flags::empty(), width: 1 }
}

/// A highlighted row mixing ASCII and glyphs that break text runs: every glyph must be painted
/// after the row's background rectangle.
#[test]
fn backgrounds_are_painted_before_text() {
    let settings = Settings::default();
    let style = TerminalStyle::new(&settings);
    let bar = Color32::from_rgb(55, 55, 55);
    let cells =
        vec![cell(0, '❯', bar), cell(1, ' ', bar), cell(2, 'a', bar), cell(3, '─', bar), cell(4, 'i', bar)];
    let mut frame = Frame::default();
    frame.cells = cells;
    frame.cursor = Cursor { row: 5, col: 0, shape: CursorShape::Hidden, under: None };

    let ctx = Context::default();
    ctx.begin_pass(RawInput::default());
    let painter = Painter::new(
        ctx.clone(),
        LayerId::background(),
        Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0)),
    );
    let grid = Grid { origin: pos2(0.0, 0.0), cell: vec2(8.0, 16.0), text_offset: 0.0 };
    let mut cache = PaintCache::default();
    paint(&painter, grid, &style, &frame, &mut cache, CursorState { focused: true, visible: true }, None);
    let mut output = ctx.end_pass();
    output.textures_delta.clear();
    let shapes = output.shapes;

    let kinds: Vec<&str> = shapes
        .iter()
        .map(|s| match &s.shape {
            Shape::Rect(r) if r.fill == bar => "bg",
            Shape::Text(_) => "text",
            _ => "other",
        })
        .collect();
    let first_text = kinds.iter().position(|k| *k == "text").expect("text painted");
    let last_bg = kinds.iter().rposition(|k| *k == "bg").expect("background painted");
    assert!(last_bg < first_text, "{kinds:?}");
    assert!(kinds.iter().filter(|k| **k == "text").count() >= 2, "runs were expected to break: {kinds:?}");
}

/// The same frame paints the same shapes from the cache as it did when laid out, moved with
/// the grid, and a new frame generation lays out again.
#[test]
fn a_cached_frame_replays_its_shapes_where_the_grid_moved() {
    let settings = Settings::default();
    let style = TerminalStyle::new(&settings);
    let mut frame = Frame::default();
    frame.cells =
        vec![cell(0, 'a', Color32::BLACK), cell(1, '─', Color32::BLACK), cell(2, '漢', Color32::RED)];
    frame.cells[2].width = 2;
    frame.cursor = Cursor { row: 5, col: 0, shape: CursorShape::Hidden, under: None };
    let ctx = Context::default();
    let shapes = |ctx: &Context, frame: &Frame, cache: &mut PaintCache, y: f32| {
        ctx.begin_pass(RawInput::default());
        let painter = Painter::new(
            ctx.clone(),
            LayerId::background(),
            Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0)),
        );
        let grid = Grid { origin: pos2(0.0, y), cell: vec2(8.0, 16.0), text_offset: 0.0 };
        paint(&painter, grid, &style, frame, cache, CursorState { focused: true, visible: true }, None);
        let mut output = ctx.end_pass();
        output.textures_delta.clear();
        output.shapes.into_iter().map(|s| (s.shape.visual_bounding_rect().min.y, s.shape)).collect::<Vec<_>>()
    };
    let mut cache = PaintCache::default();
    let first = shapes(&ctx, &frame, &mut cache, 0.0);
    assert!(!cache.runs.is_empty() && !cache.items.is_empty(), "text runs and items laid out");
    let laid_out = cache.runs.len() + cache.items.len();

    let moved = shapes(&ctx, &frame, &mut cache, 32.0);
    assert_eq!(cache.runs.len() + cache.items.len(), laid_out, "replayed, not laid out again");
    assert_eq!(first.len(), moved.len());
    for ((y0, _), (y1, _)) in first.iter().zip(&moved) {
        assert!((y1 - y0 - 32.0).abs() < 0.01, "shapes move with the grid: {y0} -> {y1}");
    }

    frame.generation += 1;
    frame.cells.truncate(1);
    shapes(&ctx, &frame, &mut cache, 0.0);
    assert_eq!(cache.runs.len() + cache.items.len(), 1, "a new generation is laid out afresh");
}
