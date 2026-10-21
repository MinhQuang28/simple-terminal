use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{Color, NamedColor, Rgb};
use eframe::egui::Color32;

use crate::theme::Theme;

const DIM: f32 = 0.66;

fn rgb(c: Rgb) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}

/// xterm 256-color palette: 16 themed ANSI colors, 6x6x6 cube, 24 grays.
fn indexed(i: u8, theme: &Theme) -> Color32 {
    match i {
        0..=15 => theme.ansi[i as usize].color(),
        16..=231 => {
            let i = i - 16;
            let level = |v: u8| if v == 0 { 0 } else { v * 40 + 55 };
            Color32::from_rgb(level(i / 36), level((i / 6) % 6), level(i % 6))
        }
        232..=255 => {
            let v = (i - 232) * 10 + 8;
            Color32::from_rgb(v, v, v)
        }
    }
}

fn named(color: NamedColor, theme: &Theme) -> Color32 {
    match color {
        NamedColor::Foreground | NamedColor::BrightForeground => theme.foreground.color(),
        NamedColor::Background => theme.background.color(),
        NamedColor::Cursor => theme.cursor.color(),
        NamedColor::DimForeground => theme.foreground.color().gamma_multiply(DIM),
        NamedColor::DimBlack
        | NamedColor::DimRed
        | NamedColor::DimGreen
        | NamedColor::DimYellow
        | NamedColor::DimBlue
        | NamedColor::DimMagenta
        | NamedColor::DimCyan
        | NamedColor::DimWhite => theme.ansi[color.to_bright() as usize].color().gamma_multiply(DIM),
        other => theme.ansi[(other as usize).min(15)].color(),
    }
}

/// Theme color for a palette slot as numbered in color queries: 0-255 indexed, then
/// foreground, background and cursor.
pub fn for_index(index: usize, theme: &Theme) -> Option<Rgb> {
    let color = match index {
        0..=255 => indexed(index as u8, theme),
        256 => theme.foreground.color(),
        257 => theme.background.color(),
        258 => theme.cursor.color(),
        _ => return None,
    };
    Some(Rgb { r: color.r(), g: color.g(), b: color.b() })
}

/// Resolves a cell color, honoring palette overrides set by the program via OSC 4/10/11.
pub fn resolve(color: Color, overrides: &Colors, theme: &Theme) -> Color32 {
    match color {
        Color::Spec(c) => rgb(c),
        Color::Indexed(i) => overrides[i as usize].map(rgb).unwrap_or_else(|| indexed(i, theme)),
        Color::Named(n) => overrides[n].map(rgb).unwrap_or_else(|| named(n, theme)),
    }
}
