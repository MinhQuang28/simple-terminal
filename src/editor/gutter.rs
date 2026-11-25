//! Line numbers beside the editor's text.

use eframe::egui::text_edit::TextEditOutput;
use eframe::egui::{Align2, Color32, FontId, Galley, Rect, Ui, pos2, vec2};

/// Takes up the column for the numbers of `lines` lines; [`paint`] fills it in once the text is
/// laid out, as its wrapping decides where each line starts.
pub fn reserve(ui: &mut Ui, font: &FontId, lines: usize) -> Rect {
    let digits = lines.to_string().len() as f32;
    let width = ui.fonts_mut(|f| f.glyph_width(font, '0')) * digits;
    ui.allocate_space(vec2(width, 0.0)).1
}

/// Numbers the lines on view, each on the row it starts on; rows a line wraps onto have none.
pub fn paint(ui: &Ui, column: Rect, output: &TextEditOutput, font: &FontId, color: Color32) {
    let clip = ui.clip_rect();
    for (number, top) in line_starts(&output.galley) {
        let top = output.galley_pos.y + top;
        if top > clip.bottom() {
            break;
        }
        if top + font.size * 2.0 >= clip.top() {
            let at = pos2(column.right(), top);
            ui.painter().text(at, Align2::RIGHT_TOP, number, font.clone(), color);
        }
    }
}

/// Each line's one-based number and the top of its first row in the galley.
fn line_starts(galley: &Galley) -> impl Iterator<Item = (usize, f32)> {
    let continued = galley.rows.iter().map(|row| !row.ends_with_newline);
    let wrapped = std::iter::once(false).chain(continued);
    galley
        .rows
        .iter()
        .zip(wrapped)
        .filter(|(_, wrapped)| !wrapped)
        .zip(1..)
        .map(|((row, _), n)| (n, row.pos.y))
}

#[cfg(test)]
mod tests {
    use eframe::egui::text::LayoutJob;
    use eframe::egui::{Context, RawInput};

    use super::*;

    /// The row each line's number is drawn on.
    fn numbered_rows(text: &str, width: f32) -> (usize, Vec<(usize, usize)>) {
        let mut found = None;
        let mut output = Context::default().run_ui(RawInput::default(), |ui| {
            let job = LayoutJob::simple(text.into(), FontId::monospace(13.0), Color32::WHITE, width);
            let galley = ui.fonts_mut(|f| f.layout_job(job));
            let row_at = |top: f32| galley.rows.iter().position(|row| row.pos.y == top).unwrap();
            let starts = line_starts(&galley).map(|(n, top)| (n, row_at(top))).collect();
            found = Some((galley.rows.len(), starts));
        });
        output.textures_delta.clear();
        found.unwrap()
    }

    #[test]
    fn every_row_is_numbered_when_nothing_wraps() {
        let (rows, starts) = numbered_rows("a\n\nb\n", f32::INFINITY);
        assert_eq!((rows, starts), (4, vec![(1, 0), (2, 1), (3, 2), (4, 3)]));
        assert_eq!(numbered_rows("", f32::INFINITY), (1, vec![(1, 0)]));
    }

    #[test]
    fn rows_a_line_wraps_onto_have_no_number() {
        let long = "word ".repeat(30);
        let (rows, starts) = numbered_rows(&format!("one\n{long}\nthree\n"), 120.0);
        assert!(rows > 6, "the second line takes several rows: {rows}");
        assert_eq!(starts, [(1, 0), (2, 1), (3, rows - 2), (4, rows - 1)]);
    }
}
