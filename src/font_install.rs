use std::sync::Arc;

use eframe::egui::{self, FontDefinitions, FontFamily, FontTweak};

use crate::font_catalog::{BOLD_FAMILY, FontEntry, FontFace, SYSTEM_FONT, SYSTEM_FONT_PATH, load};
use crate::symbol_fallback_fonts;

/// egui's bundled interface font, used when no interface font is chosen.
const DEFAULT_UI_FONT: &str = "Ubuntu-Light";
/// Interface text size the labels were designed at; other sizes scale the font from it.
pub const UI_BASE_FONT_SIZE: f32 = 13.0;

/// Font scale that shows interface text at `size` points.
pub fn ui_scale(size: f32) -> f32 {
    size / UI_BASE_FONT_SIZE
}

/// Fonts to install: the terminal's (also used by the editor) and the interface's.
pub struct FontChoice<'a> {
    /// `None` uses [`SYSTEM_FONT`].
    pub terminal: Option<&'a FontEntry>,
    /// `None` keeps egui's bundled interface font.
    pub ui: Option<&'a FontEntry>,
    /// Multiplier on every interface text size, so labels drawn at fixed sizes scale too.
    pub ui_scale: f32,
}

/// Installs the chosen fonts. The terminal font heads the monospace family and egui's fonts stay
/// behind it as fallbacks for glyphs it lacks; the interface font heads the proportional family.
pub fn apply(ctx: &egui::Context, choice: FontChoice<'_>) {
    let mut defs = FontDefinitions::default();
    install_terminal(&mut defs, choice.terminal.unwrap_or(&system_font()));
    install_ui(&mut defs, choice.ui, choice.ui_scale);
    symbol_fallback_fonts::install(&mut defs);
    ctx.set_fonts(defs);
}

fn install_terminal(defs: &mut FontDefinitions, entry: &FontEntry) {
    let mut mono = defs.families.get(&FontFamily::Monospace).cloned().unwrap_or_default();
    if let Some(data) = load(&entry.regular) {
        defs.font_data.insert("terminal-regular".into(), Arc::new(data));
        mono.insert(0, "terminal-regular".into());
    }
    let mut bold = mono.clone();
    if let Some(data) = entry.bold.as_ref().and_then(load) {
        defs.font_data.insert("terminal-bold".into(), Arc::new(data));
        bold.insert(0, "terminal-bold".into());
    }
    defs.families.insert(FontFamily::Monospace, mono);
    defs.families.insert(FontFamily::Name(BOLD_FAMILY.into()), bold);
}

fn install_ui(defs: &mut FontDefinitions, entry: Option<&FontEntry>, scale: f32) {
    let tweak = FontTweak { scale, ..Default::default() };
    match entry.and_then(|e| load(&e.regular)) {
        Some(data) => {
            defs.font_data.insert("ui-regular".into(), Arc::new(data.tweak(tweak)));
            defs.families.entry(FontFamily::Proportional).or_default().insert(0, "ui-regular".into());
        }
        None => {
            // Only the size changes, so egui's own font takes the tweak instead.
            if let Some(data) = defs.font_data.get(DEFAULT_UI_FONT).filter(|_| scale != 1.0) {
                let scaled = (**data).clone().tweak(tweak);
                defs.font_data.insert(DEFAULT_UI_FONT.into(), Arc::new(scaled));
            }
        }
    }
}

fn system_font() -> FontEntry {
    let regular = FontFace { path: SYSTEM_FONT_PATH.into(), index: 0 };
    FontEntry { family: SYSTEM_FONT.into(), regular, bold: None, monospace: true }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_font_heads_monospace_and_ui_font_heads_proportional() {
        let mut defs = FontDefinitions::default();
        install_terminal(&mut defs, &system_font());
        install_ui(&mut defs, Some(&system_font()), 1.2);
        assert_eq!(defs.families[&FontFamily::Monospace][0], "terminal-regular");
        assert_eq!(defs.families[&FontFamily::Proportional][0], "ui-regular");
        assert!(
            defs.families[&FontFamily::Name(BOLD_FAMILY.into())].contains(&"terminal-regular".to_string())
        );
        assert_eq!(defs.font_data["ui-regular"].tweak.scale, 1.2);
        assert_eq!(defs.font_data["terminal-regular"].tweak.scale, 1.0, "the terminal keeps its own size");
    }

    #[test]
    fn scaling_without_an_interface_font_tweaks_eguis_font() {
        let mut defs = FontDefinitions::default();
        install_ui(&mut defs, None, 1.1);
        assert_eq!(defs.families[&FontFamily::Proportional][0], DEFAULT_UI_FONT);
        assert_eq!(defs.font_data[DEFAULT_UI_FONT].tweak.scale, 1.1);
    }
}
