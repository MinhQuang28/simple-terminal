//! WCAG contrast maths, for chrome colors that must stay readable on whatever theme is applied.

use eframe::egui::Color32;

/// Contrast WCAG asks of body text.
pub const TEXT_MIN: f32 = 4.5;

/// WCAG relative luminance of an opaque color.
fn luminance(color: Color32) -> f32 {
    let linear = |v: u8| {
        let v = f32::from(v) / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
}

/// WCAG contrast ratio between two opaque colors, from 1 (same) to 21 (black on white).
pub fn ratio(a: Color32, b: Color32) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// `color`, moved toward `toward` only as far as it takes to reach `min` against `background`;
/// `toward` itself when nothing on the way gets there.
pub fn readable(color: Color32, background: Color32, toward: Color32, min: f32) -> Color32 {
    (0..=10)
        .map(|step| color.lerp_to_gamma(toward, step as f32 / 10.0))
        .find(|candidate| ratio(*candidate, background) >= min)
        .unwrap_or(toward)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn black_on_white_is_the_full_ratio() {
        assert!((ratio(Color32::BLACK, Color32::WHITE) - 21.0).abs() < 0.01);
        assert!((ratio(Color32::WHITE, Color32::WHITE) - 1.0).abs() < 0.01);
    }

    #[test]
    fn a_readable_color_is_left_alone_and_a_faint_one_is_pulled_up() {
        let (bg, fg) = (Color32::from_rgb(0x1b, 0x1c, 0x20), Color32::WHITE);
        let clear = Color32::from_rgb(0xe0, 0x6c, 0x75);
        assert_eq!(readable(clear, bg, fg, TEXT_MIN), clear);
        let faint = Color32::from_rgb(0x40, 0x40, 0x48);
        let fixed = readable(faint, bg, fg, TEXT_MIN);
        assert_ne!(fixed, faint);
        assert!(ratio(fixed, bg) >= TEXT_MIN);
    }
}
