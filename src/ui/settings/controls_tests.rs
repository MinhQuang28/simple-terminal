use eframe::egui::{Align, Context, Layout, RawInput};

use super::super::fields::number_field;
use super::segmented;

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

/// Whatever a row holds, its control ends flush with the others, level with the row's title.
#[test]
fn row_controls_share_a_right_edge() {
    use eframe::egui::{Rect, TextEdit};

    use super::{card, toggle};

    let ctx = Context::default();
    let mut output = ctx.run_ui(RawInput::default(), |ui| {
        ui.set_width(600.0);
        let (mut on, mut shape, mut text) = (true, 0, String::from("claude"));
        let mut rects: Vec<Rect> = Vec::new();
        card(ui, |card| {
            card.row("Field", "A text field, described over a line or two of text.", |ui| {
                rects.push(ui.add(TextEdit::singleline(&mut text).desired_width(200.0)).rect);
            });
            card.row("Switch", "", |ui| rects.push(toggle(ui, &mut on).rect));
            card.row("Choice", "Pick one.", |ui| {
                rects.push(ui.scope(|ui| segmented(ui, &mut shape, &[(0, "A"), (1, "B")])).response.rect);
            });
        });
        let rights: Vec<f32> = rects.iter().map(|r| r.right()).collect();
        assert!(rights.iter().all(|r| (r - rights[0]).abs() < 0.5), "right edges differ: {rights:?}");
        let tops: Vec<f32> = rects.iter().map(|r| r.top()).collect();
        println!("rects: {rects:?}");
        assert!(tops.windows(2).all(|w| w[1] > w[0]), "rows out of order: {tops:?}");
    });
    output.textures_delta.clear();
}

/// The app pads buttons more than egui's default; the steppers must not grow with it, or they
/// spill past the row and every card below comes out wider than the one above.
#[test]
fn number_fields_keep_to_their_row_whatever_the_button_padding() {
    use eframe::egui::{Rect, vec2};

    use super::card;

    let ctx = Context::default();
    ctx.all_styles_mut(|style| style.spacing.button_padding = vec2(10.0, 4.0));
    let mut output = ctx.run_ui(RawInput::default(), |ui| {
        ui.set_max_width(600.0);
        let mut size = 13.0_f32;
        let mut cards: Vec<Rect> = Vec::new();
        let mut groups: Vec<Rect> = Vec::new();
        for _ in 0..3 {
            let shown = ui.scope(|ui| {
                card(ui, |card| {
                    card.row("Size", "Described.", |ui| {
                        number_field(ui, &mut size, 8.0..=32.0, 0.5, " pt");
                        groups.push(ui.min_rect());
                    });
                })
            });
            cards.push(shown.response.rect);
        }
        let widths: Vec<f32> = cards.iter().map(Rect::width).collect();
        assert!(widths.iter().all(|w| (w - widths[0]).abs() < 0.5), "card widths differ: {widths:?}");
        for (group, card) in groups.iter().zip(&cards) {
            assert!(
                group.right() <= card.right() - 19.0,
                "stepper spilled into the card padding: {group:?} in {card:?}"
            );
            assert!(group.height() <= 27.0, "control band grew: {group:?}");
        }
    });
    output.textures_delta.clear();
}
