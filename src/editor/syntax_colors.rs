//! Token colours of VS Code's Dark+ theme, or Light+ on a light terminal theme, mapped onto the
//! TextMate scopes syntect's grammars emit. Operators and punctuation keep the theme's text colour.

use crate::theme::Hex;

/// How a token is painted: a Dark+/Light+ pair, or the theme's own text colour.
#[derive(Clone, Copy)]
pub enum Paint {
    Plain,
    Pair(Hex, Hex),
}

const fn pair(dark: u32, light: u32) -> Paint {
    Paint::Pair(Hex::rgb(dark), Hex::rgb(light))
}

pub const COMMENT: Paint = pair(0x6A9955, 0x008000);
pub const STRING: Paint = pair(0xCE9178, 0xA31515);
pub const KEYWORD: Paint = pair(0x569CD6, 0x0000FF);
pub const CONTROL: Paint = pair(0xC586C0, 0xAF00DB);
pub const FUNCTION: Paint = pair(0xDCDCAA, 0x795E26);
pub const TYPE: Paint = pair(0x4EC9B0, 0x267F99);
pub const VARIABLE: Paint = pair(0x9CDCFE, 0x001080);
pub const TAG: Paint = pair(0x569CD6, 0x800000);

/// Scope selectors and their paint; syntect picks the most specific selector that matches.
pub const RULES: &[(&str, Paint)] = &[
    ("comment, punctuation.definition.comment, markup.quote", COMMENT),
    (
        "string, punctuation.definition.string, markup.raw, markup.inline.raw, string.other.link, \
         meta.link.inline.description",
        STRING,
    ),
    ("string.regexp, string.regexp punctuation.definition.string", pair(0xD16969, 0x811F3F)),
    (
        "keyword.operator.quantifier.regexp, keyword.operator.or.regexp, keyword.control.anchor.regexp",
        pair(0xD7BA7D, 0xEE0000),
    ),
    ("constant.character.escape, constant.other.placeholder", pair(0xD7BA7D, 0xEE0000)),
    ("constant.numeric, markup.inserted", pair(0xB5CEA8, 0x098658)),
    (
        "keyword, storage, constant.language, variable.language, keyword.operator.new, \
         keyword.operator.expression, keyword.operator.word, keyword.operator.logical.python, \
         keyword.operator.cast, keyword.operator.sizeof, keyword.operator.instanceof, \
         punctuation.definition.template-expression, punctuation.section.embedded, \
         punctuation.section.interpolation, \
         entity.name.function.preprocessor, meta.diff.header, markup.bold",
        KEYWORD,
    ),
    ("keyword.control, keyword.other.import, keyword.other.use", CONTROL),
    ("keyword.operator, punctuation.separator, punctuation.terminator", Paint::Plain),
    (
        "entity.name.function, support.function, variable.function, entity.name.function.macro, \
         support.macro, entity.name.function.decorator, variable.annotation",
        FUNCTION,
    ),
    (
        "entity.name.type, entity.name.class, entity.name.struct, entity.name.enum, \
         entity.name.union, entity.name.trait, entity.name.interface, entity.name.namespace, \
         entity.name.impl, support.type, support.class, entity.other.inherited-class, \
         storage.type.primitive, support.type.primitive",
        TYPE,
    ),
    (
        "variable, variable.parameter, variable.other.property, variable.other.object.property, \
         meta.definition.variable.name, support.variable, entity.name.variable, \
         meta.object-literal.key, meta.mapping.key string",
        VARIABLE,
    ),
    (
        "variable.other.constant, variable.other.enummember, constant.other, support.constant",
        pair(0x4FC1FF, 0x0070C1),
    ),
    ("entity.other.attribute-name, support.type.property-name", pair(0x9CDCFE, 0xE50000)),
    (
        "support.type.property-name.json, meta.mapping.key.json string, \
         meta.structure.dictionary.key.json string",
        pair(0x9CDCFE, 0x0451A5),
    ),
    ("support.constant.property-value, meta.property-value support.constant", pair(0xCE9178, 0x0451A5)),
    (
        "entity.other.attribute-name.class.css, entity.other.attribute-name.id.css, \
         entity.other.pseudo-class, entity.other.pseudo-element",
        pair(0xD7BA7D, 0x800000),
    ),
    ("entity.name.tag, markup.heading", TAG),
    ("markup.deleted", STRING),
    ("punctuation.definition.tag", pair(0x808080, 0x800000)),
    ("invalid, invalid.illegal, message.error", pair(0xF44747, 0xCD3131)),
];

impl Paint {
    /// The colour on a theme whose text colour is `foreground`.
    pub fn color(self, light: bool, foreground: Hex) -> Hex {
        match self {
            Self::Plain => foreground,
            Self::Pair(dark, light_color) => {
                if light {
                    light_color
                } else {
                    dark
                }
            }
        }
    }
}
