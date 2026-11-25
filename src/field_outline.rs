use eframe::egui::Color32;
use eframe::egui::theme::{DefaultStyle, StyleProvider};
use eframe::egui::widget_style::{StyleArgs, TextEditStyle};

/// egui's own text field style with the focused outline in a color of its own: egui draws it in
/// the color of selected text, which has to contrast with the selection instead.
pub struct FieldOutline(pub Color32);

impl StyleProvider<TextEditStyle> for FieldOutline {
    fn style(&mut self, args: &StyleArgs<'_>) -> TextEditStyle {
        let mut style: TextEditStyle = DefaultStyle.style(args);
        let stroke = &mut style.atom_layout.frame.stroke;
        if *stroke == args.style.visuals.selection.stroke {
            stroke.color = self.0;
        }
        style
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::text::CCursor;
    use eframe::egui::text_selection::CCursorRange;
    use eframe::egui::{Context, RawInput, Shape, TextEdit};

    use crate::theme::Theme;

    #[test]
    fn a_focused_field_is_outlined_in_the_accent_and_selects_in_readable_text() {
        let theme = Theme::default();
        let ctx = Context::default();
        theme.apply_to(&ctx);
        let mut text = String::from("selected");
        let mut frame = || {
            ctx.run_ui(RawInput { focused: true, ..Default::default() }, |ui| {
                let mut output = TextEdit::singleline(&mut text).show(ui);
                let all = CCursorRange::two(CCursor::new(0), CCursor::new(8));
                output.state.cursor.set_char_range(Some(all));
                output.state.store(ui.ctx(), output.response.id);
                output.response.request_focus();
            })
        };
        // The focus and selection set in one frame show in the next.
        frame().textures_delta.clear();
        frame().textures_delta.clear();
        let mut output = frame();
        output.textures_delta.clear();

        let shapes: Vec<&Shape> = output.shapes.iter().map(|s| &s.shape).collect();
        let outlined = |color| {
            shapes
                .iter()
                .any(|s| matches!(s, Shape::Rect(r) if r.stroke.width > 0.0 && r.stroke.color == color))
        };
        assert!(outlined(theme.accent()), "{shapes:?}");
        let glyphs = |color| {
            shapes.iter().any(|s| {
                matches!(s, Shape::Text(t) if t.galley.rows.iter()
                .any(|r| r.visuals.mesh.vertices.iter().any(|v| v.color == color)))
            })
        };
        assert!(glyphs(theme.selected_text()));
        assert!(!glyphs(theme.accent()));
    }
}
