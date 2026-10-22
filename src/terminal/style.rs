use eframe::egui::{FontFamily, FontId};

use crate::font_catalog::BOLD_FAMILY;
use crate::model::Settings;
use crate::theme::Theme;

/// Appearance settings resolved for painting a terminal pane.
pub struct TerminalStyle<'a> {
    pub theme: &'a Theme,
    pub font: FontId,
    pub bold_font: FontId,
    pub line_height: f32,
    pub letter_spacing: f32,
    pub padding: f32,
    pub cursor_blink: bool,
    pub option_as_meta: bool,
    pub copy_on_select: bool,
    pub agent_prompt_at_bottom: bool,
}

impl<'a> TerminalStyle<'a> {
    pub fn new(settings: &'a Settings) -> Self {
        let size = settings.font_size;
        Self {
            theme: &settings.theme,
            font: FontId::monospace(size),
            bold_font: FontId::new(size, FontFamily::Name(BOLD_FAMILY.into())),
            line_height: settings.line_height,
            letter_spacing: settings.letter_spacing,
            padding: settings.padding,
            cursor_blink: settings.cursor_blink,
            option_as_meta: settings.option_as_meta,
            copy_on_select: settings.copy_on_select,
            agent_prompt_at_bottom: settings.agent_prompt_at_bottom,
        }
    }
}
