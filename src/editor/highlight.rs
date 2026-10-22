//! Syntax colouring for the built-in editor, in the terminal theme's palette so code looks the
//! same in the editor as in the shell.

use std::sync::LazyLock;

use eframe::egui::cache::{ComputerMut, FrameCache};
use eframe::egui::text::{ByteIndex, LayoutJob, LayoutSection};
use eframe::egui::{Color32, Context, FontId, Stroke, TextFormat};
use syntect::easy::HighlightLines;
use syntect::highlighting::{
    Color, FontStyle, ScopeSelectors, StyleModifier, Theme as SyntectTheme, ThemeItem, ThemeSettings,
};
use syntect::parsing::{SyntaxDefinition, SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

use crate::theme::{Hex, Theme};

/// Grammars layered over syntect's bundle; later ones win for a shared extension.
const GRAMMARS: &[&str] = &[
    include_str!("grammars/toml.sublime-syntax"),
    include_str!("grammars/typescript.sublime-syntax"),
    include_str!("grammars/typescript-react.sublime-syntax"),
];

static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(|| {
    let mut builder = SyntaxSet::load_defaults_newlines().into_builder();
    for grammar in GRAMMARS {
        builder.add(SyntaxDefinition::load_from_str(grammar, true, None).expect("bundled grammar parses"));
    }
    builder.build()
});

/// Languages no bundled grammar covers, shown with the closest one.
const ALIASES: &[(&str, &str)] = &[
    // syntect's own JavaScript grammar predates JSX, classes and template literals.
    ("js", "ts"),
    ("mjs", "ts"),
    ("cjs", "ts"),
    ("jsx", "tsx"),
    ("vue", "html"),
    ("svelte", "html"),
    ("env", "sh"),
    ("zsh", "sh"),
    ("fish", "sh"),
    ("kt", "java"),
    ("kts", "java"),
    ("swift", "rs"),
    ("jsonc", "json"),
    ("json5", "json"),
    ("mdx", "md"),
];

/// Which palette entry each kind of token takes; later rows win for more specific scopes.
const SCOPES: &[(&str, Slot)] = &[
    ("comment, punctuation.definition.comment", Slot::Comment),
    ("keyword, storage, keyword.operator", Slot::Keyword),
    ("keyword.operator", Slot::Plain),
    ("string, punctuation.definition.string", Slot::String),
    ("constant.numeric, constant.language, constant.character, constant.other", Slot::Constant),
    ("entity.name.function, support.function, variable.function, meta.function-call", Slot::Function),
    (
        "entity.name.type, entity.name.class, entity.name.struct, entity.name.enum, entity.name.trait, \
         entity.name.namespace, support.type, support.class, entity.other.inherited-class, \
         storage.type.primitive, entity.name.interface",
        Slot::Type,
    ),
    ("variable.language, variable.parameter, entity.name.tag, markup.heading", Slot::Tag),
    ("entity.other.attribute-name, meta.attribute, support.constant, markup.bold", Slot::Attribute),
    ("markup.raw, markup.inline.raw, markup.underline.link", Slot::String),
    ("invalid, invalid.illegal, message.error", Slot::Error),
];

#[derive(Clone, Copy)]
enum Slot {
    Plain,
    Comment,
    Keyword,
    String,
    Constant,
    Function,
    Type,
    Tag,
    Attribute,
    Error,
}

impl Slot {
    fn color(self, theme: &Theme) -> Hex {
        match self {
            Self::Plain => theme.foreground,
            Self::Comment => blend(theme.foreground, theme.background, 0.5),
            Self::Keyword => theme.ansi[5],
            Self::String => theme.ansi[2],
            Self::Constant => theme.ansi[6],
            Self::Function => theme.ansi[4],
            Self::Type => theme.ansi[3],
            Self::Tag => theme.ansi[1],
            Self::Attribute => theme.ansi[11],
            Self::Error => theme.ansi[9],
        }
    }

    fn font_style(self) -> Option<FontStyle> {
        match self {
            Self::Comment => Some(FontStyle::ITALIC),
            Self::Tag => Some(FontStyle::BOLD),
            _ => None,
        }
    }
}

fn blend(a: Hex, b: Hex, t: f32) -> Hex {
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Hex([mix(a.0[0], b.0[0]), mix(a.0[1], b.0[1]), mix(a.0[2], b.0[2])])
}

fn syntect_color(hex: Hex) -> Color {
    let [r, g, b] = hex.0;
    Color { r, g, b, a: 255 }
}

/// A syntect theme that paints tokens with the terminal palette.
fn syntect_theme(theme: &Theme) -> SyntectTheme {
    let scopes = SCOPES
        .iter()
        .map(|(selector, slot)| ThemeItem {
            scope: selector.parse::<ScopeSelectors>().expect("valid scope selector"),
            style: StyleModifier {
                foreground: Some(syntect_color(slot.color(theme))),
                background: None,
                font_style: slot.font_style(),
            },
        })
        .collect();
    SyntectTheme {
        settings: ThemeSettings {
            foreground: Some(syntect_color(theme.foreground)),
            background: Some(syntect_color(theme.background)),
            ..Default::default()
        },
        scopes,
        ..Default::default()
    }
}

fn syntax_for(language: &str) -> Option<&'static SyntaxReference> {
    let ext = ALIASES.iter().find(|(from, _)| *from == language).map_or(language, |(_, to)| to);
    SYNTAXES.find_syntax_by_extension(ext).or_else(|| SYNTAXES.find_syntax_by_name(language))
}

#[derive(Default)]
struct Highlighter;

impl ComputerMut<(&Theme, &FontId, &str, &str), LayoutJob> for Highlighter {
    fn compute(&mut self, (theme, font, language, code): (&Theme, &FontId, &str, &str)) -> LayoutJob {
        let plain = || LayoutJob::simple(code.into(), font.clone(), theme.foreground.color(), f32::INFINITY);
        let Some(syntax) = syntax_for(language) else { return plain() };
        let syntect_theme = syntect_theme(theme);
        let mut lines = HighlightLines::new(syntax, &syntect_theme);
        let mut job = LayoutJob { text: code.into(), ..Default::default() };
        let mut offset = 0;
        for line in LinesWithEndings::from(code) {
            let Ok(pieces) = lines.highlight_line(line, &SYNTAXES) else { return plain() };
            for (style, piece) in pieces {
                let fg = style.foreground;
                let color = Color32::from_rgb(fg.r, fg.g, fg.b);
                let underline = style.font_style.contains(FontStyle::UNDERLINE);
                job.sections.push(LayoutSection {
                    leading_space: 0.0,
                    byte_range: ByteIndex(offset)..ByteIndex(offset + piece.len()),
                    format: TextFormat {
                        font_id: font.clone(),
                        color,
                        italics: style.font_style.contains(FontStyle::ITALIC),
                        underline: if underline { Stroke::new(1.0, color) } else { Stroke::NONE },
                        ..Default::default()
                    },
                });
                offset += piece.len();
            }
        }
        job
    }
}

/// Loads the grammars on a background thread so the first file opens without a pause.
pub fn warm_up() {
    std::thread::spawn(|| LazyLock::force(&SYNTAXES));
}

/// The coloured layout of `code` in `language` (a file extension or syntax name), memoised per
/// frame so an unchanged file is laid out once.
pub fn layout_job(ctx: &Context, theme: &Theme, font: &FontId, language: &str, code: &str) -> LayoutJob {
    type Cache = FrameCache<LayoutJob, Highlighter>;
    ctx.memory_mut(|memory| memory.caches.cache::<Cache>().get((theme, font, language, code)).clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colors(language: &str, code: &str) -> Vec<Color32> {
        let theme = Theme::default();
        let font = FontId::monospace(13.0);
        let job = Highlighter.compute((&theme, &font, language, code));
        let mut colors: Vec<_> = job.sections.iter().map(|s| s.format.color).collect();
        colors.dedup();
        colors
    }

    #[test]
    fn rust_keywords_strings_and_comments_take_their_palette_entries() {
        let theme = Theme::default();
        let colors = colors("rs", "fn main() { let s = \"hi\"; } // done\n");
        assert!(colors.contains(&theme.ansi[5].color()), "keyword");
        assert!(colors.contains(&theme.ansi[2].color()), "string");
        assert!(colors.contains(&theme.ansi[4].color()), "function");
        assert!(colors.len() > 3, "{colors:?}");
    }

    #[test]
    fn bundled_grammars_cover_typescript_jsx_and_toml() {
        let theme = Theme::default();
        assert!(colors("ts", "const x: number = 1; // c\n").len() > 2);
        assert!(colors("toml", "[package]\nname = \"x\"\n").len() > 1);
        for language in ["tsx", "jsx"] {
            let job = Highlighter.compute((
                &theme,
                &FontId::monospace(13.0),
                language,
                "const A = () => <div className=\"x\">hi</div>;\n",
            ));
            let color_of = |needle: &str| {
                let at = ByteIndex(job.text.find(needle).unwrap());
                job.sections.iter().find(|s| s.byte_range.contains(&at)).unwrap().format.color
            };
            assert_eq!(color_of("div className"), theme.ansi[1].color(), "{language} opening tag");
            assert_eq!(color_of("div>;"), theme.ansi[1].color(), "{language} closing tag");
            assert_eq!(color_of("hi<"), theme.foreground.color(), "{language} text child");
        }
    }

    #[test]
    fn unknown_languages_use_the_plain_foreground() {
        let theme = Theme::default();
        assert_eq!(colors("zzz", "whatever\n"), [theme.foreground.color()]);
    }

    #[test]
    fn sections_cover_the_text_in_order() {
        let job =
            Highlighter.compute((&Theme::default(), &FontId::monospace(13.0), "py", "x = 1\nprint(x)\n"));
        let mut end = ByteIndex(0);
        for section in &job.sections {
            assert_eq!(section.byte_range.start, end);
            end = section.byte_range.end;
        }
        assert_eq!(end, ByteIndex(job.text.len()));
    }
}
