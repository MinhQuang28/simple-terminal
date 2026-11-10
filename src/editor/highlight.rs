//! Syntax colouring for the built-in editor, with VS Code's Dark+ / Light+ token colours.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Once};

use super::syntax_colors::RULES;
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

fn syntect_color(hex: Hex) -> Color {
    let [r, g, b] = hex.0;
    Color { r, g, b, a: 255 }
}

/// A syntect theme with VS Code's Dark+ (or Light+) token colours.
fn syntect_theme(theme: &Theme) -> SyntectTheme {
    let light = theme.is_light();
    let scopes = RULES
        .iter()
        .map(|(selector, paint)| ThemeItem {
            scope: selector.parse::<ScopeSelectors>().expect("valid scope selector"),
            style: StyleModifier {
                foreground: Some(syntect_color(paint.color(light, theme.foreground))),
                background: None,
                font_style: None,
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

/// Set once the grammars are built, which takes most of a second.
static READY: AtomicBool = AtomicBool::new(false);

/// Builds the grammars on a background thread, then repaints so files shown plain meanwhile
/// (one restored at startup, say) take their colours.
pub fn warm_up(ctx: &Context) {
    static STARTED: Once = Once::new();
    let ctx = ctx.clone();
    STARTED.call_once(|| {
        std::thread::spawn(move || {
            LazyLock::force(&SYNTAXES);
            READY.store(true, Ordering::Release);
            ctx.request_repaint();
        });
    });
}

/// The coloured layout of `code` in `language` (a file extension or syntax name), memoised per
/// frame so an unchanged file is laid out once. Until the grammars are ready the text is plain,
/// rather than the frame waiting on them.
pub fn layout_job(ctx: &Context, theme: &Theme, font: &FontId, language: &str, code: &str) -> LayoutJob {
    if !READY.load(Ordering::Acquire) {
        warm_up(ctx);
        return LayoutJob::simple(code.into(), font.clone(), theme.foreground.color(), f32::INFINITY);
    }
    type Cache = FrameCache<LayoutJob, Highlighter>;
    ctx.memory_mut(|memory| memory.caches.cache::<Cache>().get((theme, font, language, code)).clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::syntax_colors::{COMMENT, CONTROL, FUNCTION, KEYWORD, Paint, STRING, TAG};

    fn colors(language: &str, code: &str) -> Vec<Color32> {
        let theme = Theme::default();
        let font = FontId::monospace(13.0);
        let job = Highlighter.compute((&theme, &font, language, code));
        let mut colors: Vec<_> = job.sections.iter().map(|s| s.format.color).collect();
        colors.dedup();
        colors
    }

    #[test]
    fn rust_tokens_take_dark_plus_colours() {
        let theme = Theme::default();
        let colors = colors("rs", "fn main() { if true { let s = \"hi\"; } } // done\n");
        let dark = |paint: Paint| paint.color(false, theme.foreground).color();
        assert!(colors.contains(&dark(KEYWORD)), "keyword");
        assert!(colors.contains(&dark(CONTROL)), "control flow");
        assert!(colors.contains(&dark(STRING)), "string");
        assert!(colors.contains(&dark(FUNCTION)), "function");
        assert!(colors.contains(&dark(COMMENT)), "comment");
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
            let tag = TAG.color(theme.is_light(), theme.foreground).color();
            assert_eq!(color_of("div className"), tag, "{language} opening tag");
            assert_eq!(color_of("div>;"), tag, "{language} closing tag");
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
