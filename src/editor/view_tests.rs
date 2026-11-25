use eframe::egui::{Context, RawInput, Shape, vec2};

use super::super::Buffers;
use super::*;

/// Opens a file of long lines at `line` and returns how far down the pane that line's number is
/// drawn, in rows, or `None` when it isn't on view.
fn row_shown_at(line: usize, wrap: bool) -> Option<f32> {
    let path = std::env::temp_dir().join(format!("repodeck-view-{}.txt", uuid::Uuid::new_v4()));
    let text: String = (1..=200).map(|n| format!("{n} {}\n", "word ".repeat(40))).collect();
    std::fs::write(&path, text).unwrap();
    let settings = Settings { editor_word_wrap: wrap, ..Settings::default() };
    let mut buffers = Buffers::default();
    let ctx = Context::default();
    let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(500.0, 400.0));
    let input = || RawInput { screen_rect: Some(screen), focused: true, ..Default::default() };

    let mut shown = None;
    for frame in 0..4 {
        let goto = (frame == 0).then_some((line, 1));
        let id = Id::unique("editor");
        let mut output = ctx.run_ui(input(), |ui| {
            show(ui, buffers.get(&path), &path, &path, &settings, id, goto, &mut Vec::new());
        });
        output.textures_delta.clear();
        let row_height = ctx.fonts_mut(|f| f.row_height(&FontId::monospace(settings.font_size)));
        shown = output.shapes.iter().find_map(|s| match &s.shape {
            Shape::Text(t) if t.galley.text() == line.to_string() => Some(t.pos.y / row_height),
            _ => None,
        });
    }
    std::fs::remove_file(path).unwrap();
    shown
}

#[test]
fn going_to_a_line_shows_it_near_the_top() {
    for wrap in [false, true] {
        let row = row_shown_at(150, wrap);
        // Four rows of context above it, below the editor's header.
        assert!(row.is_some_and(|row| (4.0..10.0).contains(&row)), "wrap {wrap}: {row:?}");
    }
}
