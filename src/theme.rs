use eframe::egui::{self, Color32};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub use crate::theme_presets::presets;

/// An sRGB color stored as `#rrggbb` in the state file so it stays hand-editable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Hex(pub [u8; 3]);

impl Hex {
    pub const fn rgb(value: u32) -> Self {
        Self([(value >> 16) as u8, (value >> 8) as u8, value as u8])
    }

    pub fn color(self) -> Color32 {
        let [r, g, b] = self.0;
        Color32::from_rgb(r, g, b)
    }

    fn parse(text: &str) -> Option<Self> {
        let hex = text.strip_prefix('#')?;
        (hex.len() == 6).then_some(())?;
        u32::from_str_radix(hex, 16).ok().map(Self::rgb)
    }
}

impl Serialize for Hex {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let [r, g, b] = self.0;
        s.serialize_str(&format!("#{r:02x}{g:02x}{b:02x}"))
    }
}

impl<'de> Deserialize<'de> for Hex {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Hex::parse(&text).ok_or_else(|| serde::de::Error::custom(format!("invalid color {text:?}")))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub background: Hex,
    pub foreground: Hex,
    pub cursor: Hex,
    pub selection: Hex,
    /// Normal colors 0-7 followed by bright colors 8-15.
    pub ansi: [Hex; 16],
}

impl Default for Theme {
    fn default() -> Self {
        presets().into_iter().next().expect("at least one preset")
    }
}

impl Theme {
    pub fn is_light(&self) -> bool {
        let [r, g, b] = self.background.0;
        0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32 > 140.0
    }

    pub fn accent(&self) -> Color32 {
        self.ansi[4].color()
    }

    /// Color for uncommitted-change counts.
    pub fn modified(&self) -> Color32 {
        self.ansi[3].color()
    }

    /// Chrome behind the rounded content card: a shade darker than the terminal background.
    pub fn window_background(&self) -> Color32 {
        let amount = if self.is_light() { 0.05 } else { 0.25 };
        self.background.color().lerp_to_gamma(Color32::BLACK, amount)
    }

    pub fn border(&self) -> Color32 {
        let target = if self.is_light() { Color32::BLACK } else { Color32::WHITE };
        self.background.color().lerp_to_gamma(target, 0.1)
    }

    /// Restyles the egui chrome (sidebar, buttons, dialogs) to match the terminal palette.
    pub fn apply_to(&self, ctx: &egui::Context) {
        let bg = self.window_background();
        let fg = self.foreground.color();
        let light = self.is_light();
        let mut visuals = if light { egui::Visuals::light() } else { egui::Visuals::dark() };
        let shift = |c: Color32, amount: f32| {
            let target = if light { Color32::BLACK } else { Color32::WHITE };
            c.lerp_to_gamma(target, amount)
        };

        visuals.panel_fill = bg;
        visuals.window_fill = shift(self.background.color(), 0.04);
        visuals.window_corner_radius = egui::CornerRadius::same(12);
        visuals.menu_corner_radius = egui::CornerRadius::same(10);
        let shadow =
            egui::Shadow { offset: [0, 6], blur: 24, spread: 0, color: Color32::from_black_alpha(90) };
        visuals.window_shadow = shadow;
        visuals.popup_shadow = shadow;
        visuals.extreme_bg_color = self.background.color();
        visuals.faint_bg_color = shift(bg, 0.05);
        visuals.override_text_color = Some(fg);
        visuals.hyperlink_color = self.accent();
        visuals.selection.bg_fill = self.selection.color();
        // Focused fields get a thin accent outline rather than a bright full-strength one.
        visuals.selection.stroke = egui::Stroke::new(1.0, self.accent());
        visuals.widgets.noninteractive.bg_fill = visuals.panel_fill;
        visuals.widgets.noninteractive.bg_stroke.color = shift(bg, 0.12);
        visuals.widgets.inactive.weak_bg_fill = shift(bg, 0.08);
        visuals.widgets.inactive.bg_fill = shift(bg, 0.08);
        visuals.widgets.hovered.weak_bg_fill = shift(bg, 0.11);
        visuals.widgets.hovered.bg_fill = shift(bg, 0.11);
        visuals.widgets.active.weak_bg_fill = shift(bg, 0.15);
        visuals.widgets.active.bg_fill = shift(bg, 0.15);
        for widget in [
            &mut visuals.widgets.noninteractive,
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
            &mut visuals.widgets.open,
        ] {
            widget.corner_radius = egui::CornerRadius::same(6);
        }
        ctx.set_visuals(visuals);
        ctx.all_styles_mut(|style| {
            style.spacing.button_padding = egui::vec2(10.0, 4.0);
            style.spacing.menu_margin = egui::Margin::same(6);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips_through_json() {
        let json = serde_json::to_string(&Hex::rgb(0x1a2b3c)).unwrap();
        assert_eq!(json, "\"#1a2b3c\"");
        assert_eq!(serde_json::from_str::<Hex>(&json).unwrap(), Hex::rgb(0x1a2b3c));
        assert!(serde_json::from_str::<Hex>("\"1a2b3c\"").is_err());
    }

    #[test]
    fn presets_have_unique_names() {
        let names: std::collections::HashSet<_> = presets().into_iter().map(|t| t.name).collect();
        assert_eq!(names.len(), presets().len());
    }
}
