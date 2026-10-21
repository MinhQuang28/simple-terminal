use std::sync::Arc;

use eframe::egui::FontDefinitions;

use crate::font_catalog::{FontFace, load};

/// egui's first emoji font; fallbacks go before it so symbols render as text, not emoji.
const EMOJI_FONT: &str = "NotoEmoji-Regular";

/// System fonts covering symbols the bundled fonts lack, such as the dingbats Claude Code cycles
/// through in its spinner (✢ ✳ ✶ ✻ ✽). Without them each frame falls through to a different font
/// or to nothing, so the spinner flickers.
const FALLBACKS: &[(&str, &str)] = &[
    ("symbols-menlo", "/System/Library/Fonts/Menlo.ttc"),
    ("symbols-apple", "/System/Library/Fonts/Apple Symbols.ttf"),
];

/// Adds the installed fallback fonts to every family, ahead of egui's emoji fonts.
pub fn install(defs: &mut FontDefinitions) {
    let names: Vec<String> = FALLBACKS
        .iter()
        .filter_map(|(name, path)| {
            let data = load(&FontFace { path: path.into(), index: 0 })?;
            defs.font_data.insert((*name).into(), Arc::new(data));
            Some((*name).into())
        })
        .collect();
    for fonts in defs.families.values_mut() {
        let at = fonts.iter().position(|f| f == EMOJI_FONT).unwrap_or(fonts.len());
        fonts.splice(at..at, names.iter().cloned());
    }
}
