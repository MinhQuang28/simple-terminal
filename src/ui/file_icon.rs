//! vscode-icons file and folder icons, drawn in the explorer, the Changes list and file tabs.

use eframe::egui::load::TexturePoll;
use eframe::egui::{Color32, Image, Painter, Rect, Sense, Ui, Vec2, pos2};

use crate::file_icons;

/// Paints icon `name` (see `file_icons`) filling `rect`.
pub fn paint(painter: &Painter, rect: Rect, name: &str) {
    let Some((name, svg)) = file_icons::svg(name) else { return };
    let image = Image::from_bytes(format!("bytes://vscode-icons/{name}.svg"), svg);
    // Rasterised at the pixel size it shows at, so small icons stay crisp.
    let size = rect.size() * painter.pixels_per_point();
    let Ok(TexturePoll::Ready { texture }) = image.load_for_size(painter.ctx(), size) else { return };
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    painter.image(texture.id, rect, uv, Color32::WHITE);
}

/// Allocates `size` points square and paints the icon of a file or folder named `name`.
pub fn show(ui: &mut Ui, size: f32, name: &str, is_dir: bool, open: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    if ui.is_rect_visible(rect) {
        let icon = if is_dir { file_icons::for_folder(name, open) } else { file_icons::for_file(name) };
        paint(ui.painter(), rect, icon);
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Context, RawInput};

    use super::*;
    use crate::file_icon_files::FILES;

    /// Every bundled icon rasterises: an SVG resvg can't read would leave a blank space.
    #[test]
    fn every_icon_renders() {
        let ctx = Context::default();
        egui_extras::install_image_loaders(&ctx);
        ctx.begin_pass(RawInput::default());
        for (name, svg) in FILES {
            let image = Image::from_bytes(format!("bytes://vscode-icons/{name}.svg"), *svg);
            let ready = match image.load_for_size(&ctx, Vec2::splat(32.0)) {
                Ok(TexturePoll::Ready { .. }) => Ok(()),
                Ok(TexturePoll::Pending { .. }) => Err("pending".to_string()),
                Err(err) => Err(err.to_string()),
            };
            assert_eq!(ready, Ok(()), "{name}");
        }
        let mut output = ctx.end_pass();
        output.textures_delta.clear();
    }
}
