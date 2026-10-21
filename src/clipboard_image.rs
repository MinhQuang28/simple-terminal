use std::fs::{self, File};
use std::io::BufWriter;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use eframe::egui::ColorImage;

/// Pasted images older than this are removed; programs read them right after the paste.
const KEEP_FOR: Duration = Duration::from_secs(24 * 60 * 60);

fn paste_dir() -> PathBuf {
    std::env::temp_dir().join("repodeck-pasted-images")
}

/// Writes a clipboard image to a PNG file whose path can be pasted into a terminal, e.g. for
/// Claude Code, which attaches image files named in a prompt.
pub fn save_png(image: &ColorImage) -> Result<PathBuf> {
    let dir = paste_dir();
    fs::create_dir_all(&dir)?;
    prune(&dir);
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let short = &uuid::Uuid::new_v4().simple().to_string()[..6];
    let path = dir.join(format!("paste-{secs}-{short}.png"));
    write_png(image, &path).with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}

fn write_png(image: &ColorImage, path: &std::path::Path) -> Result<()> {
    let [width, height] = image.size;
    let mut encoder = png::Encoder::new(BufWriter::new(File::create(path)?), width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
    writer.write_image_data(&rgba)?;
    writer.finish()?;
    Ok(())
}

fn prune(dir: &std::path::Path) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > KEEP_FOR);
        if old {
            let _ = fs::remove_file(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::Color32;

    use super::*;

    #[test]
    fn writes_a_decodable_png() {
        let image = ColorImage::filled([3, 2], Color32::from_rgba_unmultiplied(10, 20, 30, 128));
        let path = save_png(&image).unwrap();
        let decoder = png::Decoder::new(std::io::BufReader::new(File::open(&path).unwrap()));
        let mut reader = decoder.read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buf).unwrap();
        assert_eq!((info.width, info.height), (3, 2));
        assert_eq!(&buf[..4], &[10, 20, 30, 128]);
        fs::remove_file(path).unwrap();
    }
}
