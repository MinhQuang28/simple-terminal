//! An installed app's own icon, as the Finder and the Dock show it.

use std::path::Path;

use eframe::egui::{ColorImage, Context, Id, Sense, TextureHandle, TextureOptions, Ui, Vec2};

/// Draws the icon of the app at `bundle`, `size` points square; false when macOS gives none, and
/// nothing is drawn.
pub fn show(ui: &mut Ui, size: f32, bundle: &str) -> bool {
    let pixels = (size * ui.pixels_per_point()).round() as u32;
    let Some(texture) = texture(ui.ctx(), bundle, size, pixels) else { return false };
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let uv = eframe::egui::Rect::from_min_max([0.0, 0.0].into(), [1.0, 1.0].into());
    ui.painter().image(texture.id(), rect, uv, eframe::egui::Color32::WHITE);
    true
}

/// The icon as a texture, read once per app and pixel size.
fn texture(ctx: &Context, bundle: &str, size: f32, pixels: u32) -> Option<TextureHandle> {
    let id = Id::unique(("app-icon", bundle, pixels));
    if let Some(known) = ctx.data(|data| data.get_temp::<Option<TextureHandle>>(id)) {
        return known;
    }
    let texture = platform::png(Path::new(bundle), size)
        .and_then(|png| decode(&png))
        .map(|image| ctx.load_texture(format!("app-icon:{bundle}"), image, TextureOptions::LINEAR));
    ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
    texture
}

fn decode(bytes: &[u8]) -> Option<ColorImage> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8() | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().ok()?;
    let mut pixels = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut pixels).ok()?;
    if info.color_type != png::ColorType::Rgba {
        return None;
    }
    let size = [info.width as usize, info.height as usize];
    Some(ColorImage::from_rgba_unmultiplied(size, &pixels[..info.buffer_size()]))
}

#[cfg(target_os = "macos")]
mod platform {
    use std::path::Path;

    use objc2::AllocAnyThread;
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSWorkspace};
    use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};

    /// The icon of the file at `path` as a PNG, from the representation macOS draws at `size`
    /// points on the main screen.
    pub fn png(path: &Path, size: f32) -> Option<Vec<u8>> {
        let icon = NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(path.to_str()?));
        let side = f64::from(size);
        let mut rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(side, side));
        // SAFETY: `rect` outlives the call, and no hints are passed.
        let image = unsafe { icon.CGImageForProposedRect_context_hints(&mut rect, None, None) }?;
        let bitmap = NSBitmapImageRep::initWithCGImage(NSBitmapImageRep::alloc(), &image);
        // SAFETY: an empty dictionary holds no property of a wrong type.
        let data = unsafe {
            bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
        }?;
        Some(data.to_vec())
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    pub fn png(_path: &std::path::Path, _size: f32) -> Option<Vec<u8>> {
        None
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn reads_the_finders_icon() {
        let png = platform::png(Path::new("/System/Library/CoreServices/Finder.app"), 32.0).unwrap();
        let image = decode(&png).unwrap();
        assert!(image.width() >= 32 && image.width() == image.height());
        assert!(image.pixels.iter().any(|p| p.a() > 0));
    }
}
