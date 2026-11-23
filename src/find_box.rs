//! The find box shared by the terminal and the editor: the query with its match-case, whole-word
//! and regex switches inside the field, the result count, and previous, next and close.

use eframe::egui::{
    Align2, CornerRadius, FontId, Frame, Key, Label, Margin, Modifiers, Response, RichText, Sense, Stroke,
    TextEdit, Ui, vec2,
};

use crate::ui::design::{RADIUS_CHIP, RADIUS_ROW, TEXT_SECONDARY, hairline};
use crate::ui::widgets::{accent, glyph_button, muted};

/// How a query is matched. All off: the text as typed, ignoring case.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FindOptions {
    pub case: bool,
    pub word: bool,
    pub regex: bool,
}

/// What the user did in the box this frame.
pub struct FindBox {
    /// The query field.
    pub edit: Response,
    /// `1` for the next match, `-1` for the previous one.
    pub step: i32,
    pub close: bool,
    /// The query or a switch changed.
    pub changed: bool,
}

/// The regular expression that finds `query` under `options`.
pub fn pattern(query: &str, options: FindOptions) -> String {
    let mut pattern = if options.regex { query.to_string() } else { regex::escape(query) };
    if options.word {
        pattern = format!(r"\b(?:{pattern})\b");
    }
    if !options.case {
        pattern.insert_str(0, "(?i)");
    }
    pattern
}

/// Draws the box in a row; `status` is the result count, e.g. "2 of 5" or "No results".
pub fn show(ui: &mut Ui, query: &mut String, options: &mut FindOptions, status: &str) -> FindBox {
    ui.spacing_mut().item_spacing.x = 6.0;
    let id = ui.make_persistent_id("find-query");
    let focused = ui.memory(|m| m.has_focus(id));
    let (mut step, mut close, mut changed) = (0, false, false);
    if focused {
        // Shift first: the plain pattern would take a shifted Enter too.
        let (back, forward, escape) = ui.input_mut(|i| {
            let back = i.consume_key(Modifiers::SHIFT, Key::Enter);
            (back, i.consume_key(Modifiers::NONE, Key::Enter), i.key_pressed(Key::Escape))
        });
        step = if back { -1 } else { i32::from(forward) };
        close = escape;
    }
    let border = if focused { accent(ui) } else { hairline(ui) };
    let field = Frame::new()
        .fill(ui.visuals().extreme_bg_color)
        .stroke(Stroke::new(1.0, border))
        .corner_radius(CornerRadius::same(RADIUS_ROW))
        .inner_margin(Margin::symmetric(6, 3));
    let edit = field
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let edit =
                TextEdit::singleline(query).id(id).hint_text("Find").frame(Frame::NONE).desired_width(150.0);
            let edit = ui.add(edit);
            changed |= switch(ui, "Aa", &mut options.case, "Match case");
            changed |= switch(ui, "ab", &mut options.word, "Match whole word");
            changed |= switch(ui, ".*", &mut options.regex, "Use regular expression");
            edit
        })
        .inner;
    changed |= edit.changed();
    let count = RichText::new(status).size(TEXT_SECONDARY).color(muted(ui));
    ui.add_sized(vec2(64.0, 20.0), Label::new(count).selectable(false));
    if glyph_button(ui, "↑").on_hover_text("Previous match (⇧Enter)").clicked() {
        step = -1;
    }
    if glyph_button(ui, "↓").on_hover_text("Next match (Enter)").clicked() {
        step = 1;
    }
    close |= glyph_button(ui, "×").on_hover_text("Close (Esc)").clicked();
    FindBox { edit, step, close, changed }
}

/// A small switch inside the field, lit while on; returns whether it was flipped.
fn switch(ui: &mut Ui, glyph: &str, on: &mut bool, hint: &str) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(22.0, 20.0), Sense::click());
    let radius = CornerRadius::same(RADIUS_CHIP);
    if *on {
        ui.painter().rect_filled(rect, radius, accent(ui).gamma_multiply(0.3));
    } else if response.hovered() {
        ui.painter().rect_filled(rect, radius, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let color = if *on { ui.visuals().strong_text_color() } else { muted(ui) };
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        glyph,
        FontId::proportional(TEXT_SECONDARY),
        color,
    );
    let clicked = response.on_hover_text(hint).clicked();
    *on ^= clicked;
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_the_pattern_from_the_switches() {
        let options = |case, word, regex| FindOptions { case, word, regex };
        assert_eq!(pattern("a.b", options(false, false, false)), r"(?i)a\.b");
        assert_eq!(pattern("a.b", options(true, false, true)), "a.b");
        assert_eq!(pattern("a|b", options(true, true, true)), r"\b(?:a|b)\b");
    }
}
