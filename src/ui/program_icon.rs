//! A program's icon on its coloured tile, as Herdr draws them.

use eframe::egui::load::TexturePoll;
use eframe::egui::{Color32, Image, Painter, Rect, Sense, Ui, Vec2, pos2};

use crate::programs::Program;

/// Paints `program`'s tile and icon filling `rect`.
pub fn paint(painter: &Painter, rect: Rect, program: &Program) {
    painter.rect_filled(rect, rect.height() * 0.24, program.tile);
    let Some(svg) = program.icon else { return };
    let inner = rect.shrink(rect.height() * 0.17);
    let image = Image::from_bytes(program.uri.clone(), svg);
    // Rasterised at the pixel size it shows at, so small icons stay crisp.
    let size = inner.size() * painter.pixels_per_point();
    let Ok(TexturePoll::Ready { texture }) = image.load_for_size(painter.ctx(), size) else { return };
    let tint = if program.tint { program.color } else { Color32::WHITE };
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    painter.image(texture.id, inner, uv, tint);
}

/// Allocates and paints the icon inline, `size` points square.
pub fn icon(ui: &mut Ui, size: f32, program: &Program) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    if ui.is_rect_visible(rect) {
        paint(ui.painter(), rect, program);
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Context, RawInput, Vec2};

    use super::*;
    use crate::programs;

    /// Every bundled icon rasterises: an SVG resvg can't read would leave its tile empty.
    #[test]
    fn every_icon_renders() {
        let ctx = Context::default();
        egui_extras::install_image_loaders(&ctx);
        ctx.begin_pass(RawInput::default());
        for (path, _) in crate::program_icon_files::FILES {
            let key = path.trim_start_matches("icons/").trim_end_matches(".svg");
            let Some(program) = programs::get(key).filter(|p| p.icon.is_some()) else { continue };
            let image = Image::from_bytes(program.uri.clone(), program.icon.unwrap());
            let poll = image.load_for_size(&ctx, Vec2::splat(32.0));
            let ready = match &poll {
                Ok(TexturePoll::Ready { .. }) => Ok(()),
                Ok(TexturePoll::Pending { .. }) => Err("pending".to_string()),
                Err(err) => Err(err.to_string()),
            };
            assert_eq!(ready, Ok(()), "{path}");
        }
        let mut output = ctx.end_pass();
        output.textures_delta.clear();
    }
}
