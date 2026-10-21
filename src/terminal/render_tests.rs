use alacritty_terminal::term::TermMode;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::CursorShape;
use eframe::egui::{Color32, Context, LayerId, Painter, RawInput, Rect, Shape, pos2, vec2};

use super::*;
use crate::model::Settings;
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
    let frame = Frame {
        cells,
        cursor: Cursor { row: 5, col: 0, shape: CursorShape::Hidden, under: None },
        mode: TermMode::empty(),
    };

    let ctx = Context::default();
    ctx.begin_pass(RawInput::default());
    let painter = Painter::new(
        ctx.clone(),
        LayerId::background(),
        Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0)),
    );
    let grid = Grid { origin: pos2(0.0, 0.0), cell: vec2(8.0, 16.0), text_offset: 0.0 };
    paint(&painter, grid, &style, &frame, CursorState { focused: true, visible: true }, None);
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
