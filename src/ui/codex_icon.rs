//! The Codex (OpenAI) knot, from a mask bundled at 64 px and tinted at draw time.

use std::io::Cursor;
use std::sync::LazyLock;

use eframe::egui::{
    Color32, ColorImage, Context, Id, Painter, Rect, TextureFilter, TextureHandle, TextureOptions, pos2,
};

const MASK_PNG: &[u8] = include_bytes!("codex-icon.png");

/// White pixels whose alpha traces the knot.
static MASK: LazyLock<ColorImage> = LazyLock::new(|| {
    let decoder = png::Decoder::new(Cursor::new(MASK_PNG));
    let mut reader = decoder.read_info().expect("bundled icon decodes");
    let mut pixels = vec![0; reader.output_buffer_size().expect("bundled icon has a size")];
    let info = reader.next_frame(&mut pixels).expect("bundled icon decodes");
    ColorImage::from_rgba_unmultiplied(
        [info.width as usize, info.height as usize],
        &pixels[..info.buffer_size()],
    )
});

fn texture(ctx: &Context) -> TextureHandle {
    let id = Id::unique("codex-icon");
    ctx.data_mut(|data| {
        data.get_temp_mut_or_insert_with(id, || {
            ctx.load_texture(
                "codex-icon",
                MASK.clone(),
                TextureOptions { mipmap_mode: Some(TextureFilter::Linear), ..TextureOptions::LINEAR },
            )
        })
        .clone()
    })
}

/// Paints the knot filling `rect` in `color`.
pub fn paint(painter: &Painter, rect: Rect, color: Color32) {
    let texture = texture(painter.ctx());
    let side = rect.width().min(rect.height());
    let square = Rect::from_center_size(rect.center(), eframe::egui::vec2(side, side));
    painter.image(texture.id(), square, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_mask_is_square_and_traces_a_shape() {
        let [w, h] = MASK.size;
        assert_eq!(w, h);
        let opaque = MASK.pixels.iter().filter(|p| p.a() > 128).count();
        assert!(opaque > w * h / 10 && opaque < w * h / 2, "{opaque} of {}", w * h);
        assert!(MASK.pixels.iter().all(|p| p.r() == p.a()), "white, premultiplied");
    }
}
