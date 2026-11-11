//! Characters that Helvetica's WinAnsi encoding cannot show but PDF's other standard fonts can:
//! `Symbol` (Greek letters, mathematical operators, arrows) and `ZapfDingbats` (check marks,
//! stars, geometric bullets, arrows). Their built-in encodings are fixed by ISO 32000-2 Annex D
//! (D.5 and D.6), so a character maps to exactly one byte; no font file is needed.
//!
//! PowerPoint also stores text typed in the Symbol and Wingdings fonts as private-use code points
//! `U+F020..U+F0FF`, the low byte being the font's own character code.

use crate::pptx_text::Face;

/// A character in one of the standard symbolic fonts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Glyph {
    pub face: Face,
    pub code: u8,
}

/// Adobe Symbol encoding: (Unicode, code), for the characters WinAnsi lacks.
const SYMBOL: &[(char, u8)] = &[
    ('∀', 0x22),
    ('∃', 0x24),
    ('∋', 0x27),
    ('∗', 0x2A),
    ('−', 0x2D),
    ('≅', 0x40),
    ('Α', 0x41),
    ('Β', 0x42),
    ('Χ', 0x43),
    ('Δ', 0x44),
    ('Ε', 0x45),
    ('Φ', 0x46),
    ('Γ', 0x47),
    ('Η', 0x48),
    ('Ι', 0x49),
    ('ϑ', 0x4A),
    ('Κ', 0x4B),
    ('Λ', 0x4C),
    ('Μ', 0x4D),
    ('Ν', 0x4E),
    ('Ο', 0x4F),
    ('Π', 0x50),
    ('Θ', 0x51),
    ('Ρ', 0x52),
    ('Σ', 0x53),
    ('Τ', 0x54),
    ('Υ', 0x55),
    ('ς', 0x56),
    ('Ω', 0x57),
    ('Ξ', 0x58),
    ('Ψ', 0x59),
    ('Ζ', 0x5A),
    ('∴', 0x5C),
    ('⊥', 0x5E),
    ('α', 0x61),
    ('β', 0x62),
    ('χ', 0x63),
    ('δ', 0x64),
    ('ε', 0x65),
    ('φ', 0x66),
    ('γ', 0x67),
    ('η', 0x68),
    ('ι', 0x69),
    ('ϕ', 0x6A),
    ('κ', 0x6B),
    ('λ', 0x6C),
    ('μ', 0x6D),
    ('ν', 0x6E),
    ('ο', 0x6F),
    ('π', 0x70),
    ('θ', 0x71),
    ('ρ', 0x72),
    ('σ', 0x73),
    ('τ', 0x74),
    ('υ', 0x75),
    ('ϖ', 0x76),
    ('ω', 0x77),
    ('ξ', 0x78),
    ('ψ', 0x79),
    ('ζ', 0x7A),
    ('∼', 0x7E),
    ('ϒ', 0xA1),
    ('′', 0xA2),
    ('≤', 0xA3),
    ('⁄', 0xA4),
    ('∞', 0xA5),
    ('♣', 0xA7),
    ('♦', 0xA8),
    ('♥', 0xA9),
    ('♠', 0xAA),
    ('↔', 0xAB),
    ('←', 0xAC),
    ('↑', 0xAD),
    ('→', 0xAE),
    ('↓', 0xAF),
    ('″', 0xB2),
    ('≥', 0xB3),
    ('∝', 0xB5),
    ('∂', 0xB6),
    ('≠', 0xB9),
    ('≡', 0xBA),
    ('≈', 0xBB),
    ('↵', 0xBF),
    ('ℵ', 0xC0),
    ('ℑ', 0xC1),
    ('ℜ', 0xC2),
    ('℘', 0xC3),
    ('⊗', 0xC4),
    ('⊕', 0xC5),
    ('∅', 0xC6),
    ('∩', 0xC7),
    ('∪', 0xC8),
    ('⊃', 0xC9),
    ('⊇', 0xCA),
    ('⊄', 0xCB),
    ('⊂', 0xCC),
    ('⊆', 0xCD),
    ('∈', 0xCE),
    ('∉', 0xCF),
    ('∠', 0xD0),
    ('∇', 0xD1),
    ('∏', 0xD5),
    ('√', 0xD6),
    ('⋅', 0xD7),
    ('∧', 0xD9),
    ('∨', 0xDA),
    ('⇔', 0xDB),
    ('⇐', 0xDC),
    ('⇑', 0xDD),
    ('⇒', 0xDE),
    ('⇓', 0xDF),
    ('◊', 0xE0),
    ('∑', 0xE5),
    ('∫', 0xF2),
];

/// ZapfDingbats encoding (Annex D.6): (Unicode, code). Only characters whose code is certain.
const DINGBATS: &[(char, u8)] = &[
    ('✓', 0x33),
    ('✔', 0x34),
    ('✕', 0x35),
    ('✖', 0x36),
    ('✗', 0x37),
    ('✘', 0x38),
    ('★', 0x48),
    ('●', 0x6C),
    ('❍', 0x6D),
    ('■', 0x6E),
    ('❑', 0x71),
    ('▲', 0x73),
    ('▼', 0x74),
    ('◆', 0x75),
    ('❖', 0x76),
    // The Dingbats arrows U+2794..U+27BE follow one another from 0xD4 (0xF0 is unused).
    ('➔', 0xD4),
    ('➘', 0xD8),
    ('➙', 0xD9),
    ('➚', 0xDA),
    ('➛', 0xDB),
    ('➜', 0xDC),
    ('➝', 0xDD),
    ('➞', 0xDE),
    ('➟', 0xDF),
    ('➠', 0xE0),
    ('➡', 0xE1),
    ('➢', 0xE2),
    ('➣', 0xE3),
    ('➤', 0xE4),
    ('➥', 0xE5),
    ('➦', 0xE6),
    ('➧', 0xE7),
    ('➨', 0xE8),
];

/// Where a character PDF's Helvetica cannot show can be shown instead, if anywhere.
pub(crate) fn glyph(c: char) -> Option<Glyph> {
    if let Some((_, code)) = SYMBOL.iter().find(|(u, _)| *u == c) {
        return Some(Glyph { face: Face::Symbol, code: *code });
    }
    if let Some((_, code)) = DINGBATS.iter().find(|(u, _)| *u == c) {
        return Some(Glyph { face: Face::Dingbats, code: *code });
    }
    // Triangular pointers have no glyph of their own; the closest Dingbats arrowhead stands in.
    if matches!(c, '►' | '▶' | '▸' | '‣') {
        return Some(Glyph { face: Face::Dingbats, code: 0xE4 });
    }
    None
}

/// The font code behind a private-use character `U+F020..U+F0FF`.
pub(crate) fn private_use_code(c: char) -> Option<u8> {
    let n = c as u32;
    (0xF020..=0xF0FF).contains(&n).then(|| (n - 0xF000) as u8)
}

/// A private-use character typed in `typeface`, as a glyph of a standard font. Symbol's code
/// is its own; for Wingdings only the commonly used list-marker glyphs have a ZapfDingbats twin.
pub(crate) fn private_use_glyph(code: u8, typeface: &str) -> Option<Glyph> {
    let name = typeface.to_ascii_lowercase();
    if name == "symbol" {
        return Some(Glyph { face: Face::Symbol, code });
    }
    if name.starts_with("wingdings") {
        let zapf = match code {
            0x6C => 0x6C,        // ● black circle
            0x6E => 0x6E,        // ■ black square
            0x71 => 0x71,        // ❑ shadowed white square
            0x76 => 0x76,        // ❖ black diamond minus white X
            0x77 => 0x75,        // ◆ black diamond
            0xD8 => 0xE2,        // ➢ three-D top-lighted rightwards arrowhead
            0xE0 | 0xE8 => 0xD4, // ➔ heavy wide-headed rightwards arrow
            0xFB => 0x38,        // ✘ heavy ballot X
            0xFC => 0x34,        // ✔ heavy check mark
            _ => return None,
        };
        return Some(Glyph { face: Face::Dingbats, code: zapf });
    }
    None
}

/// Approximate advance of a standard-font symbol, in thousandths of an em. The standard fonts'
/// own metrics are not bundled; this only has to be close enough for line breaking.
pub(crate) fn advance(face: Face, code: u8) -> f64 {
    match face {
        Face::Text => 556.0,
        Face::Symbol => match code {
            0xAB..=0xAF | 0xDB..=0xDF => 1000.0, // arrows
            0x20 => 250.0,
            0xB0 => 400.0,
            0xB7 => 460.0,
            0xA5 => 710.0,
            _ => 580.0,
        },
        Face::Dingbats => match code {
            0xD4..=0xFE => 900.0,
            _ => 780.0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_codes_follow_the_standard_encoding() {
        assert_eq!(glyph('μ'), Some(Glyph { face: Face::Symbol, code: 0x6D }));
        assert_eq!(glyph('→'), Some(Glyph { face: Face::Symbol, code: 0xAE }));
        assert_eq!(glyph('≥'), Some(Glyph { face: Face::Symbol, code: 0xB3 }));
        assert_eq!(glyph('√'), Some(Glyph { face: Face::Symbol, code: 0xD6 }));
        assert_eq!(glyph('Δ'), Some(Glyph { face: Face::Symbol, code: 0x44 }));
        assert_eq!(glyph('✓'), Some(Glyph { face: Face::Dingbats, code: 0x33 }));
        assert_eq!(glyph('►'), Some(Glyph { face: Face::Dingbats, code: 0xE4 }));
        assert_eq!(glyph('日'), None);
    }

    #[test]
    fn private_use_symbols_keep_their_font_codes() {
        assert_eq!(private_use_code('\u{f0b0}'), Some(0xB0));
        assert_eq!(private_use_code('a'), None);
        assert_eq!(private_use_glyph(0x6D, "Symbol"), Some(Glyph { face: Face::Symbol, code: 0x6D }));
        assert_eq!(private_use_glyph(0xFC, "Wingdings"), Some(Glyph { face: Face::Dingbats, code: 0x34 }));
        assert_eq!(private_use_glyph(0x01, "Wingdings 2"), None);
        assert_eq!(private_use_glyph(0x41, "Arial"), None);
    }

    #[test]
    fn no_character_maps_twice() {
        let mut seen = std::collections::HashSet::new();
        for (c, _) in SYMBOL.iter().chain(DINGBATS) {
            assert!(seen.insert(*c), "{c:?} is listed twice");
        }
    }
}

/// Latin letters followed by a combining accent, composed into the single WinAnsi character
/// they stand for (`a` + U+0301 → `á`). Text typed on some keyboards and pasted from the web
/// arrives this way.
pub(crate) fn compose(text: &str) -> String {
    // (combining mark, letters, composed letters), the two strings aligned character by character.
    const TABLE: [(char, &str, &str); 7] = [
        ('\u{300}', "AEIOUaeiou", "ÀÈÌÒÙàèìòù"),
        ('\u{301}', "AEIOUYaeiouy", "ÁÉÍÓÚÝáéíóúý"),
        ('\u{302}', "AEIOUaeiou", "ÂÊÎÔÛâêîôû"),
        ('\u{303}', "ANOano", "ÃÑÕãñõ"),
        ('\u{308}', "AEIOUaeiouy", "ÄËÏÖÜäëïöüÿ"),
        ('\u{30a}', "Aa", "Åå"),
        ('\u{327}', "Cc", "Çç"),
    ];
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        let composed = chars.peek().and_then(|mark| {
            let (_, letters, results) = TABLE.iter().find(|(m, _, _)| m == mark)?;
            let at = letters.chars().position(|l| l == c)?;
            results.chars().nth(at)
        });
        match composed {
            Some(done) => {
                out.push(done);
                chars.next();
            }
            None => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod compose_tests {
    use super::compose;

    #[test]
    fn combining_accents_become_latin1_letters() {
        assert_eq!(compose("a\u{301}o\u{303} c\u{327}a\u{303}o"), "áõ ção");
        assert_eq!(compose("E\u{300}"), "È");
        // A mark with no WinAnsi composition is left for the caller to refuse.
        assert_eq!(compose("z\u{301}"), "z\u{301}");
        assert_eq!(compose("plain"), "plain");
    }
}

fn unsupported_char(c: char) -> bool {
    c != '?' && karnoxpdf_fonts::win_ansi(&c.to_string()) == b"?"
}

/// Text as segments, each in the standard PDF font that can show it (Helvetica, Symbol or
/// ZapfDingbats): letters with combining accents are composed, invisible format characters
/// dropped, and symbols Helvetica lacks moved to the symbolic fonts. `typeface` is the font the
/// text names, which decides what private-use characters mean. A character no standard font
/// shows comes back as `Err`, so callers can stop instead of altering the text.
pub(crate) fn segments(text: &str, typeface: Option<&str>) -> Result<Vec<(String, Face)>, char> {
    let text = compose(text);
    let mut out: Vec<(String, Face)> = Vec::new();
    let mut push = |c: char, face: Face| match out.last_mut() {
        Some((s, f)) if *f == face => s.push(c),
        _ => out.push((c.to_string(), face)),
    };
    for c in text.chars() {
        match c {
            '\u{ad}' | '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{200e}' | '\u{200f}' | '\u{feff}' | '\u{2060}' => {}
            '\u{2010}' | '\u{2011}' => push('-', Face::Text),
            '\u{2002}' | '\u{2003}' | '\u{2009}' | '\u{200a}' | '\u{202f}' => push(' ', Face::Text),
            '\u{b}' | '\u{2028}' => push('\n', Face::Text),
            '\t' | '\n' => push(c, Face::Text),
            c if (c as u32) < 0x20 => {}
            c => {
                if c == '\u{f020}' {
                    // The space of a symbol font is a space in every font.
                    push(' ', Face::Text);
                } else if let Some(code) = private_use_code(c) {
                    let glyph = typeface.and_then(|f| private_use_glyph(code, f)).ok_or(c)?;
                    push(char::from(glyph.code), glyph.face);
                } else if unsupported_char(c) {
                    let glyph = glyph(c).ok_or(c)?;
                    push(char::from(glyph.code), glyph.face);
                } else {
                    push(c, Face::Text);
                }
            }
        }
    }
    Ok(out)
}
