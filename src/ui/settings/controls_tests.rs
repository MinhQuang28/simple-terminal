use eframe::egui::{Align, Context, Layout, RawInput};

use super::{number_field, segmented};

/// Row controls are laid out right-to-left; groups must stay content-sized there.
#[test]
fn controls_do_not_grow_to_fill_right_to_left_rows() {
    let ctx = Context::default();
    let mut output = ctx.run_ui(RawInput::default(), |ui| {
        ui.set_width(600.0);
        {
            let mut shape = 0;
            let group = ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                segmented(ui, &mut shape, &[(0, "Block"), (1, "Bar"), (2, "Underline")]);
            });
            assert!(group.response.rect.width() < 260.0, "segmented group filled the row");
            assert!(group.response.rect.height() < 40.0, "segmented group grew vertically");

            let mut size = 13.0_f32;
            let field = ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                number_field(ui, &mut size, 8.0..=32.0, 0.5, " pt")
            });
            assert!(field.response.rect.width() < 200.0, "number field filled the row");
            let row = field.response.rect;
            let value = field.inner.rect;
            assert!(value.left() > row.left() + 20.0 && value.right() < row.right() - 20.0);
        }
    });
    output.textures_delta.clear();
}
