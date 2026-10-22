use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use eframe::egui::{self, FontData};
use memmap2::Mmap;
use ttf_parser::{Face, PlatformId, name_id};

/// Family used for bold terminal cells; falls back to the regular monospace stack.
pub const BOLD_FAMILY: &str = "terminal-bold";

/// Font used when none is chosen. egui's bundled Hack lacks Vietnamese letters, which would
/// otherwise render in a proportional fallback font.
pub const SYSTEM_FONT: &str = "SF Mono";
pub const SYSTEM_FONT_PATH: &str = "/System/Library/Fonts/SFNSMono.ttf";

/// Every lowercase Vietnamese letter with a diacritic. Fonts missing any are left out of the
/// catalog, since a fallback glyph mid-word looks broken.
const VIETNAMESE_LETTERS: &str = "ăâđêôơưàáảãạằắẳẵặầấẩẫậèéẻẽẹềếểễệìíỉĩịòóỏõọồốổỗộờớởỡợùúủũụừứửữựỳýỷỹỵ";

#[derive(Clone, Debug, PartialEq)]
pub struct FontFace {
    pub path: PathBuf,
    pub index: u32,
}

#[derive(Clone, Debug)]
pub struct FontEntry {
    pub family: String,
    pub regular: FontFace,
    pub bold: Option<FontFace>,
    /// Fixed-pitch, so usable for the terminal; the interface takes any font.
    pub monospace: bool,
}

/// Fonts installed on the system that cover Vietnamese, discovered on a background thread.
#[derive(Clone, Default)]
pub struct FontCatalog {
    fonts: Arc<Mutex<Option<Vec<FontEntry>>>>,
}

impl FontCatalog {
    pub fn scan(ctx: egui::Context) -> Self {
        let catalog = Self::default();
        let fonts = catalog.fonts.clone();
        let spawned = thread::Builder::new().name("font-scan".into()).spawn(move || {
            let found = scan_system_fonts();
            log::info!("Found {} font families", found.len());
            if let Ok(mut slot) = fonts.lock() {
                *slot = Some(found);
            }
            ctx.request_repaint();
        });
        if let Err(err) = spawned {
            log::error!("Failed to start font scan: {err}");
        }
        catalog
    }

    /// `None` while the scan is still running.
    pub fn fonts(&self) -> Option<Vec<FontEntry>> {
        self.fonts.lock().ok()?.clone()
    }
}

fn font_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = [
        "/System/Library/Fonts",
        "/Library/Fonts",
        "/System/Applications/Utilities/Terminal.app/Contents/Resources/Fonts",
        "/usr/share/fonts",
        "/usr/local/share/fonts",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(Path::new(&home).join("Library/Fonts"));
        dirs.push(Path::new(&home).join(".local/share/fonts"));
    }
    dirs
}

fn collect_files(dir: &Path, depth: u8, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && depth > 0 {
            collect_files(&path, depth - 1, out);
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "ttf" | "otf" | "ttc" | "otc"))
        {
            out.push(path);
        }
    }
}

struct ScannedFace {
    face: FontFace,
    family: String,
    weight: u16,
    italic: bool,
    monospace: bool,
}

fn scan_system_fonts() -> Vec<FontEntry> {
    let mut files = Vec::new();
    for dir in font_dirs() {
        collect_files(&dir, 2, &mut files);
    }
    let mut by_family: BTreeMap<String, Vec<ScannedFace>> = BTreeMap::new();
    for path in files {
        for face in scan_file(&path) {
            by_family.entry(face.family.clone()).or_default().push(face);
        }
    }
    by_family
        .into_iter()
        .filter_map(|(family, faces)| {
            let upright = || faces.iter().filter(|f| !f.italic);
            let regular = upright().min_by_key(|f| f.weight.abs_diff(400))?;
            let bold = upright().filter(|f| f.weight >= 600).min_by_key(|f| f.weight.abs_diff(700));
            Some(FontEntry {
                family,
                regular: regular.face.clone(),
                bold: bold.map(|f| f.face.clone()),
                monospace: regular.monospace,
            })
        })
        .collect()
}

fn scan_file(path: &Path) -> Vec<ScannedFace> {
    let Some(data) = map_file(path) else { return Vec::new() };
    let count = ttf_parser::fonts_in_collection(&data).unwrap_or(1);
    (0..count)
        .filter_map(|index| {
            let face = Face::parse(&data, index).ok()?;
            if !covers_vietnamese(&face) {
                return None;
            }
            let family = family_name(&face)?;
            // Dot-prefixed families are private system UI fonts.
            if family.starts_with('.') {
                return None;
            }
            Some(ScannedFace {
                face: FontFace { path: path.to_path_buf(), index },
                family,
                weight: face.weight().to_number(),
                italic: face.is_italic(),
                monospace: is_monospaced(&face),
            })
        })
        .collect()
}

/// Some fonts (e.g. Monaco) don't set the fixed-pitch flag, so also compare glyph advances.
fn is_monospaced(face: &Face<'_>) -> bool {
    if face.is_monospaced() {
        return true;
    }
    let advance = |c| face.glyph_index(c).and_then(|g| face.glyph_hor_advance(g));
    let widths: Vec<_> = ['i', 'M', 'W', '.'].into_iter().map(advance).collect();
    widths.iter().all(|w| w.is_some() && *w == widths[0])
}

fn covers_vietnamese(face: &Face<'_>) -> bool {
    VIETNAMESE_LETTERS.chars().all(|c| face.glyph_index(c).is_some())
}

fn family_name(face: &Face<'_>) -> Option<String> {
    let names = face.names();
    [name_id::TYPOGRAPHIC_FAMILY, name_id::FAMILY].into_iter().find_map(|id| {
        let records = || names.into_iter().filter(move |n| n.name_id == id);
        records().find_map(|n| n.to_string()).or_else(|| {
            // Older Apple fonts such as Menlo only carry Mac Roman names; their family names are ASCII.
            records()
                .find(|n| n.platform_id == PlatformId::Macintosh && n.name.is_ascii())
                .map(|n| String::from_utf8_lossy(n.name).into_owned())
        })
    })
}

fn map_file(path: &Path) -> Option<Mmap> {
    let file = File::open(path).ok()?;
    // SAFETY: font files are read-only system/user assets; a concurrent truncation would at worst
    // make glyph loading fail, which egui tolerates by falling back to other fonts.
    unsafe { Mmap::map(&file) }.ok()
}

pub(crate) fn load(face: &FontFace) -> Option<FontData> {
    let data = map_file(&face.path)?;
    Face::parse(&data, face.index).ok()?;
    Some(FontData::from_blob(Arc::new(data), face.index))
}
