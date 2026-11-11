//! DrawingML colours, fills, lines and themes (ISO/IEC 29500-1 §20.1), as far as presentation
//! conversion needs them. Parsing keeps colours symbolic ([`ColorSpec`]) because theme colours
//! only get a value once the master that owns them is known ([`Palette`]).

use std::collections::HashMap;

use roxmltree::Node;

use crate::ooxml::{attr, child, children, number, rel_attr};

pub(crate) type Rgb = [f64; 3];

/// A resolved colour with its opacity (1 = opaque).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Rgba {
    pub rgb: Rgb,
    pub alpha: f64,
}

#[derive(Clone, Debug)]
enum Base {
    Srgb(Rgb),
    Scheme(String),
    /// `phClr`: whatever colour the referencing shape style supplies.
    Placeholder,
}

/// A colour element with its transforms (`lumMod`, `tint`, ...), not yet tied to a theme.
#[derive(Clone, Debug)]
pub(crate) struct ColorSpec {
    base: Base,
    /// (transform name, value as a fraction: 100000 = 1.0)
    mods: Vec<(String, f64)>,
}

fn hex(value: &str) -> Option<Rgb> {
    if value.len() != 6 || !value.is_ascii() {
        return None;
    }
    let n = u32::from_str_radix(value, 16).ok()?;
    Some([((n >> 16) & 255) as f64 / 255.0, ((n >> 8) & 255) as f64 / 255.0, (n & 255) as f64 / 255.0])
}

fn preset_color(name: &str) -> Rgb {
    let n = |v: u8| v as f64 / 255.0;
    match name {
        "white" => [1.0; 3],
        "red" => [1.0, 0.0, 0.0],
        "green" => [0.0, n(128), 0.0],
        "lime" => [0.0, 1.0, 0.0],
        "blue" => [0.0, 0.0, 1.0],
        "yellow" => [1.0, 1.0, 0.0],
        "cyan" | "aqua" => [0.0, 1.0, 1.0],
        "magenta" | "fuchsia" => [1.0, 0.0, 1.0],
        "gray" | "grey" => [n(128); 3],
        "dkGray" | "darkGray" => [n(169); 3],
        "ltGray" | "lightGray" => [n(211); 3],
        "orange" => [1.0, n(165), 0.0],
        _ => [0.0; 3],
    }
}

/// A theme colour by its scheme name (`hlink`, `accent1`, ...).
pub(crate) fn scheme_color(name: &str) -> ColorSpec {
    ColorSpec { base: Base::Scheme(name.to_owned()), mods: Vec::new() }
}

/// The colour element among `parent`'s children, if any.
pub(crate) fn color_in(parent: Node<'_, '_>) -> Option<ColorSpec> {
    parent.children().filter(|n| n.is_element()).find_map(parse_color)
}

/// A single colour element (`srgbClr`, `schemeClr`, `sysClr`, `prstClr`).
pub(crate) fn parse_color(node: Node<'_, '_>) -> Option<ColorSpec> {
    let base = match node.tag_name().name() {
        "srgbClr" => Base::Srgb(hex(attr(node, "val")?)?),
        "sysClr" => Base::Srgb(attr(node, "lastClr").and_then(hex).unwrap_or(if attr(node, "val") == Some("window") { [1.0; 3] } else { [0.0; 3] })),
        "prstClr" => Base::Srgb(preset_color(attr(node, "val")?)),
        "schemeClr" if attr(node, "val") == Some("phClr") => Base::Placeholder,
        "schemeClr" => Base::Scheme(attr(node, "val")?.to_owned()),
        _ => return None,
    };
    let mods = node
        .children()
        .filter(|n| n.is_element())
        .filter_map(|m| Some((m.tag_name().name().to_owned(), number(m, "val")? / 100_000.0)))
        .take(16)
        .collect();
    Some(ColorSpec { base, mods })
}

// ── colour maths ────────────────────────────────────────────────────────────────────────────

fn to_hsl([r, g, b]: Rgb) -> [f64; 3] {
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) / 2.0;
    if (max - min).abs() < 1e-12 {
        return [0.0, 0.0, l];
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if (max - r).abs() < 1e-12 {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if (max - g).abs() < 1e-12 {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    [h / 6.0, s, l]
}

fn from_hsl([h, s, l]: [f64; 3]) -> Rgb {
    if s <= 0.0 {
        return [l; 3];
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let channel = |mut t: f64| {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    [channel(h + 1.0 / 3.0), channel(h), channel(h - 1.0 / 3.0)]
}

fn to_linear(c: f64) -> f64 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn from_linear(c: f64) -> f64 {
    if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

fn apply_mods(mut rgb: Rgb, mut alpha: f64, mods: &[(String, f64)]) -> Rgba {
    for (name, v) in mods {
        match name.as_str() {
            "alpha" => alpha = *v,
            "alphaMod" => alpha *= v,
            "alphaOff" => alpha += v,
            "lumMod" | "lumOff" | "lum" | "satMod" | "satOff" | "sat" => {
                let mut hsl = to_hsl(rgb);
                match name.as_str() {
                    "lumMod" => hsl[2] *= v,
                    "lumOff" => hsl[2] += v,
                    "lum" => hsl[2] = *v,
                    "satMod" => hsl[1] *= v,
                    "satOff" => hsl[1] += v,
                    _ => hsl[1] = *v,
                }
                hsl[1] = hsl[1].clamp(0.0, 1.0);
                hsl[2] = hsl[2].clamp(0.0, 1.0);
                rgb = from_hsl(hsl);
            }
            // §20.1.2.3.34: a tint mixes the colour with white, a shade with black, in linear RGB.
            "tint" => rgb = rgb.map(|c| from_linear(to_linear(c) * v + (1.0 - v))),
            "shade" => rgb = rgb.map(|c| from_linear(to_linear(c) * v)),
            "comp" => {
                let mut hsl = to_hsl(rgb);
                hsl[0] = (hsl[0] + 0.5).fract();
                rgb = from_hsl(hsl);
            }
            "inv" => rgb = rgb.map(|c| 1.0 - c),
            "gray" => {
                let y = 0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2];
                rgb = [y; 3];
            }
            _ => {}
        }
        rgb = rgb.map(|c| c.clamp(0.0, 1.0));
    }
    Rgba { rgb, alpha: alpha.clamp(0.0, 1.0) }
}

// ── theme ───────────────────────────────────────────────────────────────────────────────────

/// What a theme part provides: its colour scheme and the fill and line styles that shapes
/// refer to by index.
#[derive(Debug, Default)]
pub(crate) struct Theme {
    colors: HashMap<String, Rgb>,
    pub fills: Vec<FillSpec>,
    pub background_fills: Vec<FillSpec>,
    pub lines: Vec<LineSpec>,
}

impl Theme {
    pub(crate) fn parse(doc: &roxmltree::Document<'_>) -> Theme {
        let mut theme = Theme::default();
        let elements = doc.root_element().descendants().filter(|n| n.is_element());
        for node in elements {
            match node.tag_name().name() {
                "clrScheme" => {
                    for slot in node.children().filter(|n| n.is_element()) {
                        if let Some(Base::Srgb(rgb)) = color_in(slot).map(|c| c.base) {
                            theme.colors.insert(slot.tag_name().name().to_owned(), rgb);
                        }
                    }
                }
                "fillStyleLst" => theme.fills = node.children().filter(|n| n.is_element()).filter_map(parse_fill_element).take(8).collect(),
                "bgFillStyleLst" => {
                    theme.background_fills = node.children().filter(|n| n.is_element()).filter_map(parse_fill_element).take(8).collect()
                }
                "lnStyleLst" => theme.lines = children(node, "ln").map(parse_line).take(8).collect(),
                _ => {}
            }
        }
        theme
    }
}

/// `p:clrMap`: which theme colour each of the master's colour names stands for.
pub(crate) fn parse_color_map(node: Option<Node<'_, '_>>) -> HashMap<String, String> {
    let mut map: HashMap<String, String> =
        [("bg1", "lt1"), ("tx1", "dk1"), ("bg2", "lt2"), ("tx2", "dk2")].into_iter().map(|(a, b)| (a.to_owned(), b.to_owned())).collect();
    if let Some(node) = node {
        for a in node.attributes().take(32) {
            map.insert(a.name().to_owned(), a.value().to_owned());
        }
    }
    map
}

/// Turns [`ColorSpec`]s into colours for one master.
pub(crate) struct Palette<'a> {
    pub theme: &'a Theme,
    pub map: &'a HashMap<String, String>,
}

impl Palette<'_> {
    /// `placeholder` is the colour `phClr` stands for, where a shape style supplies one.
    pub(crate) fn resolve(&self, spec: &ColorSpec, placeholder: Option<Rgba>) -> Rgba {
        let (rgb, alpha) = match &spec.base {
            Base::Srgb(rgb) => (*rgb, 1.0),
            Base::Placeholder => placeholder.map_or(([0.0; 3], 1.0), |p| (p.rgb, p.alpha)),
            Base::Scheme(name) => {
                let mapped = self.map.get(name).map_or(name.as_str(), String::as_str);
                (self.theme.colors.get(mapped).copied().unwrap_or([0.0; 3]), 1.0)
            }
        };
        apply_mods(rgb, alpha, &spec.mods)
    }
}

// ── fills and lines ─────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub(crate) struct GradientSpec {
    /// (position 0..=1, colour)
    pub stops: Vec<(f64, ColorSpec)>,
    /// Direction in degrees clockwise from left-to-right, for linear gradients.
    pub angle: f64,
    pub radial: bool,
}

#[derive(Clone, Debug)]
pub(crate) enum FillSpec {
    None,
    Solid(ColorSpec),
    Gradient(GradientSpec),
    /// A picture fill, with the relationship id of its image.
    Picture(Option<String>),
    /// A pattern fill, drawn as its foreground colour.
    Pattern(ColorSpec),
}

/// The fill element among `parent`'s children, if any. `grpFill` (take the group's fill) is not
/// a fill of its own.
pub(crate) fn parse_fill(parent: Node<'_, '_>) -> Option<FillSpec> {
    parent.children().filter(|n| n.is_element()).find_map(parse_fill_element)
}

pub(crate) fn parse_fill_element(node: Node<'_, '_>) -> Option<FillSpec> {
    match node.tag_name().name() {
        "noFill" => Some(FillSpec::None),
        "solidFill" => Some(color_in(node).map_or(FillSpec::None, FillSpec::Solid)),
        "gradFill" => {
            let mut stops: Vec<(f64, ColorSpec)> = child(node, "gsLst")
                .into_iter()
                .flat_map(|list| children(list, "gs"))
                .filter_map(|gs| Some((number(gs, "pos")? / 100_000.0, color_in(gs)?)))
                .take(32)
                .collect();
            stops.sort_by(|a, b| a.0.total_cmp(&b.0));
            let angle = child(node, "lin").and_then(|lin| number(lin, "ang")).map_or(0.0, |a| a / 60_000.0);
            let radial = child(node, "path").is_some();
            match stops.len() {
                0 => Some(FillSpec::None),
                1 => stops.into_iter().next().map(|(_, c)| FillSpec::Solid(c)),
                _ => Some(FillSpec::Gradient(GradientSpec { stops, angle, radial })),
            }
        }
        "blipFill" => Some(FillSpec::Picture(child(node, "blip").and_then(|b| rel_attr(b, "embed")).map(str::to_owned))),
        "pattFill" => child(node, "fgClr").and_then(color_in).map(FillSpec::Pattern).or(Some(FillSpec::None)),
        _ => None,
    }
}

/// `a:ln`. Every part is optional: an absent part is inherited from the style behind it.
#[derive(Clone, Debug, Default)]
pub(crate) struct LineSpec {
    /// Width in points.
    pub width: Option<f64>,
    pub fill: Option<FillSpec>,
    /// Dash pattern in multiples of the line width; empty is solid.
    pub dash: Option<Vec<f64>>,
}

impl LineSpec {
    /// This line, with anything it leaves open taken from `base`.
    pub(crate) fn over(&self, base: &LineSpec) -> LineSpec {
        LineSpec {
            width: self.width.or(base.width),
            fill: self.fill.clone().or_else(|| base.fill.clone()),
            dash: self.dash.clone().or_else(|| base.dash.clone()),
        }
    }
}

fn dash_pattern(name: &str) -> Vec<f64> {
    // §20.1.10.48 ST_PresetLineDashVal: dash and gap lengths in line widths.
    match name {
        "dot" => vec![1.0, 3.0],
        "dash" => vec![4.0, 3.0],
        "lgDash" => vec![8.0, 3.0],
        "dashDot" => vec![4.0, 3.0, 1.0, 3.0],
        "lgDashDot" => vec![8.0, 3.0, 1.0, 3.0],
        "lgDashDotDot" => vec![8.0, 3.0, 1.0, 3.0, 1.0, 3.0],
        "sysDash" => vec![3.0, 1.0],
        "sysDot" => vec![1.0, 1.0],
        "sysDashDot" => vec![3.0, 1.0, 1.0, 1.0],
        "sysDashDotDot" => vec![3.0, 1.0, 1.0, 1.0, 1.0, 1.0],
        _ => Vec::new(),
    }
}

pub(crate) fn parse_line(node: Node<'_, '_>) -> LineSpec {
    LineSpec {
        width: number(node, "w").map(|w| (w / 12_700.0).clamp(0.0, 1584.0)),
        fill: parse_fill(node),
        dash: child(node, "prstDash").and_then(|d| attr(d, "val")).map(dash_pattern),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn color(xml: &str) -> ColorSpec {
        let wrapped = format!(r#"<a:solidFill xmlns:a="urn:a">{xml}</a:solidFill>"#);
        let doc = roxmltree::Document::parse(&wrapped).unwrap();
        color_in(doc.root_element()).unwrap()
    }

    fn palette_parts() -> (Theme, HashMap<String, String>) {
        let xml = r#"<a:theme xmlns:a="urn:a"><a:themeElements><a:clrScheme name="x"><a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1><a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="44546A"/></a:dk2><a:lt2><a:srgbClr val="E7E6E6"/></a:lt2><a:accent1><a:srgbClr val="4472C4"/></a:accent1></a:clrScheme></a:themeElements></a:theme>"#;
        let doc = roxmltree::Document::parse(xml).unwrap();
        (Theme::parse(&doc), parse_color_map(None))
    }

    fn byte(v: f64) -> u8 {
        (v * 255.0).round() as u8
    }

    #[test]
    fn scheme_colors_go_through_the_color_map() {
        let (theme, map) = palette_parts();
        let palette = Palette { theme: &theme, map: &map };
        let tx1 = palette.resolve(&color(r#"<a:schemeClr val="tx1"/>"#), None);
        let bg1 = palette.resolve(&color(r#"<a:schemeClr val="bg1"/>"#), None);
        assert_eq!(tx1.rgb, [0.0; 3]);
        assert_eq!(bg1.rgb, [1.0; 3]);
        let accent = palette.resolve(&color(r#"<a:schemeClr val="accent1"/>"#), None);
        assert_eq!([byte(accent.rgb[0]), byte(accent.rgb[1]), byte(accent.rgb[2])], [0x44, 0x72, 0xC4]);
    }

    #[test]
    fn luminance_modifiers_follow_hsl() {
        let (theme, map) = palette_parts();
        let palette = Palette { theme: &theme, map: &map };
        // Accent 1 with lumMod 60% lumOff 40% is Office's "Lighter 40%": 8FAADC.
        let c = palette.resolve(&color(r#"<a:schemeClr val="accent1"><a:lumMod val="60000"/><a:lumOff val="40000"/></a:schemeClr>"#), None);
        let got = [byte(c.rgb[0]), byte(c.rgb[1]), byte(c.rgb[2])];
        for (g, want) in got.iter().zip([0x8F_u8, 0xAA, 0xDC]) {
            assert!((*g as i32 - want as i32).abs() <= 1, "{got:?}");
        }
        // lumMod 50% darkens: Office's "Darker 50%" of accent 1 is 203864.
        let d = palette.resolve(&color(r#"<a:schemeClr val="accent1"><a:lumMod val="50000"/></a:schemeClr>"#), None);
        let got = [byte(d.rgb[0]), byte(d.rgb[1]), byte(d.rgb[2])];
        for (g, want) in got.iter().zip([0x20_u8, 0x38, 0x64]) {
            assert!((*g as i32 - want as i32).abs() <= 1, "{got:?}");
        }
    }

    #[test]
    fn placeholder_colors_and_alpha() {
        let (theme, map) = palette_parts();
        let palette = Palette { theme: &theme, map: &map };
        let phr = Rgba { rgb: [1.0, 0.0, 0.0], alpha: 1.0 };
        let c = palette.resolve(&color(r#"<a:schemeClr val="phClr"><a:alpha val="50000"/></a:schemeClr>"#), Some(phr));
        assert_eq!(c.rgb, [1.0, 0.0, 0.0]);
        assert!((c.alpha - 0.5).abs() < 1e-9);
    }

    #[test]
    fn line_specs_inherit_unset_parts() {
        let doc = roxmltree::Document::parse(r#"<a:ln xmlns:a="urn:a" w="25400"><a:prstDash val="dash"/></a:ln>"#).unwrap();
        let line = parse_line(doc.root_element());
        assert_eq!(line.width, Some(2.0));
        assert_eq!(line.dash, Some(vec![4.0, 3.0]));
        assert!(line.fill.is_none());
        let base = LineSpec { width: Some(9.0), fill: Some(FillSpec::None), dash: None };
        let merged = line.over(&base);
        assert_eq!(merged.width, Some(2.0));
        assert!(matches!(merged.fill, Some(FillSpec::None)));
    }
}
