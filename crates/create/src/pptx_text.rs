//! Text of presentation shapes: the inherited paragraph and run properties (ISO/IEC 29500-1
//! §21.1.2), line breaking, bullets and painting.

use karnoxpdf_fonts::{helvetica_width, win_ansi};
use roxmltree::Node;

use crate::drawingml::{ColorSpec, FillSpec, Rgba, color_in, parse_fill};
use crate::ooxml::{attr, boolean, child, number};
use crate::pptx_paint::{Canvas, Path, Stroke};
use crate::pptx_symbols::{self, Glyph};

// ── properties ──────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum Align {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Spacing {
    /// A fraction of the line's own height (1 = single).
    Percent(f64),
    Points(f64),
}

#[derive(Clone, Debug)]
pub(crate) enum Bullet {
    None,
    Char(String),
    Auto { kind: String, start: u32 },
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RunProps {
    /// The typeface the run names (its `sym` font when it has one, else its `latin` font).
    pub typeface: Option<String>,
    pub size: Option<f64>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strike: Option<bool>,
    pub caps: Option<bool>,
    pub color: Option<ColorSpec>,
    /// Baseline shift as a fraction of the font size (0.3 is a superscript).
    pub baseline: Option<f64>,
}

impl RunProps {
    pub(crate) fn over(&mut self, upper: &RunProps) {
        macro_rules! take {
            ($($f:ident),*) => { $(if upper.$f.is_some() { self.$f = upper.$f.clone(); })* };
        }
        take!(typeface, size, bold, italic, underline, strike, caps, color, baseline);
    }
}

/// Paragraph properties at one list level; everything optional, so levels can be layered.
#[derive(Clone, Debug, Default)]
pub(crate) struct ParaProps {
    pub margin_left: Option<f64>,
    pub margin_right: Option<f64>,
    pub indent: Option<f64>,
    pub align: Option<Align>,
    pub line: Option<Spacing>,
    pub before: Option<Spacing>,
    pub after: Option<Spacing>,
    pub bullet: Option<Bullet>,
    pub bullet_color: Option<ColorSpec>,
    /// Bullet size as a fraction of the text size.
    pub bullet_size: Option<f64>,
    pub bullet_font: Option<String>,
    pub run: RunProps,
}

impl ParaProps {
    pub(crate) fn over(&mut self, upper: &ParaProps) {
        macro_rules! take {
            ($($f:ident),*) => { $(if upper.$f.is_some() { self.$f = upper.$f.clone(); })* };
        }
        take!(margin_left, margin_right, indent, align, line, before, after, bullet, bullet_color, bullet_size, bullet_font);
        self.run.over(&upper.run);
    }
}

fn emu_to_pt(v: f64) -> f64 {
    v / 12_700.0
}

fn spacing(node: Node<'_, '_>, name: &str) -> Option<Spacing> {
    let s = child(node, name)?;
    if let Some(p) = child(s, "spcPct") {
        return number(p, "val").map(|v| Spacing::Percent(v / 100_000.0));
    }
    child(s, "spcPts").and_then(|p| number(p, "val")).map(|v| Spacing::Points(v / 100.0))
}

pub(crate) fn parse_run_props(node: Node<'_, '_>) -> RunProps {
    let color = child_color(node);
    RunProps {
        typeface: child(node, "sym").or_else(|| child(node, "latin")).and_then(|f| attr(f, "typeface")).map(str::to_owned),
        size: number(node, "sz").map(|v| (v / 100.0).clamp(1.0, 1000.0)),
        bold: boolean(node, "b"),
        italic: boolean(node, "i"),
        underline: attr(node, "u").map(|u| u != "none"),
        strike: attr(node, "strike").map(|s| s != "noStrike"),
        caps: attr(node, "cap").map(|c| c == "all" || c == "small"),
        color,
        baseline: number(node, "baseline").map(|v| (v / 100_000.0).clamp(-1.0, 1.0)),
    }
}

/// A run's colour: its solid fill, or the first stop of a gradient fill.
fn child_color(node: Node<'_, '_>) -> Option<ColorSpec> {
    match parse_fill(node)? {
        FillSpec::Solid(c) | FillSpec::Pattern(c) => Some(c),
        FillSpec::Gradient(g) => g.stops.into_iter().next().map(|(_, c)| c),
        FillSpec::None | FillSpec::Picture(_) => None,
    }
}

/// `a:pPr` or one of the `a:lvlNpPr` elements.
pub(crate) fn parse_para_props(node: Node<'_, '_>) -> ParaProps {
    let mut p = ParaProps {
        margin_left: number(node, "marL").map(|v| emu_to_pt(v).clamp(-2000.0, 2000.0)),
        margin_right: number(node, "marR").map(|v| emu_to_pt(v).clamp(-2000.0, 2000.0)),
        indent: number(node, "indent").map(|v| emu_to_pt(v).clamp(-2000.0, 2000.0)),
        align: attr(node, "algn").map(|a| match a {
            "ctr" => Align::Center,
            "r" => Align::Right,
            "just" | "justLow" | "dist" | "thaiDist" => Align::Justify,
            _ => Align::Left,
        }),
        line: spacing(node, "lnSpc"),
        before: spacing(node, "spcBef"),
        after: spacing(node, "spcAft"),
        ..ParaProps::default()
    };
    for item in node.children().filter(|n| n.is_element()) {
        match item.tag_name().name() {
            "buNone" => p.bullet = Some(Bullet::None),
            "buChar" => p.bullet = attr(item, "char").map(|c| Bullet::Char(c.to_owned())),
            // A picture bullet has no text form: the plain bullet stands in for it.
            "buBlip" => p.bullet = Some(Bullet::Char("•".into())),
            "buAutoNum" => {
                p.bullet = Some(Bullet::Auto {
                    kind: attr(item, "type").unwrap_or("arabicPeriod").to_owned(),
                    start: number(item, "startAt").map_or(1, |v| v.clamp(1.0, 32_767.0) as u32),
                })
            }
            "buFont" => p.bullet_font = attr(item, "typeface").map(str::to_owned),
            "buClr" => p.bullet_color = color_in(item),
            "buSzPct" => p.bullet_size = number(item, "val").map(|v| (v / 100_000.0).clamp(0.25, 4.0)),
            "defRPr" => p.run = parse_run_props(item),
            _ => {}
        }
    }
    p
}

/// Layer the `lvl1pPr`…`lvl9pPr` children of a list-style element onto `levels`.
pub(crate) fn merge_list_style(levels: &mut [ParaProps; 9], list: Node<'_, '_>) {
    for (i, level) in levels.iter_mut().enumerate() {
        if let Some(node) = child(list, &format!("lvl{}pPr", i + 1)) {
            level.over(&parse_para_props(node));
        }
    }
}

// ── resolved text ───────────────────────────────────────────────────────────────────────────

/// Which standard PDF font shows a run: Helvetica for text, Symbol or ZapfDingbats for the
/// characters it cannot show (see [`crate::pptx_symbols`]). A run in one of the symbolic faces
/// holds font codes (one char per byte) rather than Unicode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Face {
    #[default]
    Text,
    Symbol,
    Dingbats,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RunStyle {
    pub face: Face,
    pub size: f64,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub color: Rgba,
    pub baseline: f64,
}

impl RunStyle {
    fn font(&self) -> &'static str {
        match self.face {
            Face::Symbol => return "F5",
            Face::Dingbats => return "F6",
            Face::Text => {}
        }
        match (self.bold, self.italic) {
            (false, false) => "F1",
            (true, false) => "F2",
            (false, true) => "F3",
            (true, true) => "F4",
        }
    }
}

/// The text of a run in a form the layout can use; see [`crate::pptx`] for how it is built.
#[derive(Clone, Debug)]
pub(crate) struct Para {
    pub runs: Vec<(String, RunStyle)>,
    pub align: Align,
    pub margin_left: f64,
    pub margin_right: f64,
    pub indent: f64,
    pub line: Spacing,
    pub before: f64,
    pub after: f64,
    pub bullet: Option<BulletMark>,
    /// The size an empty paragraph takes.
    pub end_size: f64,
}

/// A bullet ready to draw.
#[derive(Clone, Debug)]
pub(crate) struct BulletMark {
    pub shape: BulletShape,
    pub size: f64,
    pub color: Rgba,
    pub bold: bool,
}

#[derive(Clone, Debug)]
pub(crate) enum BulletShape {
    Text(String),
    /// A character of Symbol or ZapfDingbats.
    Glyph(Glyph),
    Square,
    Triangle,
}

const LINE: f64 = 1.2;
const DESCENT: f64 = 0.25;
const BOLD_WIDEN: f64 = 1.06;
const SCRIPT_SCALE: f64 = 0.65;

fn text_width(text: &str, style: &RunStyle) -> f64 {
    match style.face {
        Face::Text => helvetica_width(text, style.size) * if style.bold { BOLD_WIDEN } else { 1.0 },
        face => text.chars().map(|c| pptx_symbols::advance(face, c as u32 as u8)).sum::<f64>() * style.size / 1000.0,
    }
}

/// The bytes a run's text is shown with.
fn encode(text: &str, face: Face) -> Vec<u8> {
    match face {
        Face::Text => win_ansi(text),
        _ => text.chars().map(|c| c as u32 as u8).collect(),
    }
}

fn bullet_width(mark: &BulletMark) -> f64 {
    match &mark.shape {
        BulletShape::Text(t) => helvetica_width(t, mark.size) * if mark.bold { BOLD_WIDEN } else { 1.0 },
        BulletShape::Glyph(g) => pptx_symbols::advance(g.face, g.code) * mark.size / 1000.0,
        BulletShape::Square | BulletShape::Triangle => mark.size * 0.5,
    }
}

// ── layout ──────────────────────────────────────────────────────────────────────────────────

struct Item {
    x: f64,
    text: Vec<u8>,
    style: RunStyle,
    width: f64,
    space: bool,
}

struct Line {
    items: Vec<Item>,
    avail: f64,
    width: f64,
    height: f64,
    baseline: f64,
    /// A line that ends its paragraph or a forced break is not stretched when justified.
    last: bool,
}

struct LaidPara {
    lines: Vec<Line>,
    before: f64,
    after: f64,
    bullet: Option<(f64, BulletMark)>,
    align: Align,
}

/// Paragraphs broken into lines for one text area.
pub(crate) struct TextLayout {
    paras: Vec<LaidPara>,
    pub height: f64,
}

enum Piece {
    Word(String, RunStyle, f64),
    Space(String, RunStyle, f64),
    Tab(RunStyle),
    Break,
}

fn scaled(style: &RunStyle, scale: f64) -> RunStyle {
    let mut s = style.clone();
    s.size *= scale * if style.baseline != 0.0 { SCRIPT_SCALE } else { 1.0 };
    s
}

fn pieces(para: &Para, scale: f64) -> Vec<Piece> {
    let mut out = Vec::new();
    for (text, base) in &para.runs {
        let style = scaled(base, scale);
        let mut word = String::new();
        let mut spaces = String::new();
        let flush = |out: &mut Vec<Piece>, word: &mut String, spaces: &mut String, style: &RunStyle| {
            if !word.is_empty() {
                let w = text_width(word, style);
                out.push(Piece::Word(std::mem::take(word), style.clone(), w));
            }
            if !spaces.is_empty() {
                let w = text_width(spaces, style);
                out.push(Piece::Space(std::mem::take(spaces), style.clone(), w));
            }
        };
        for ch in text.chars() {
            match ch {
                ' ' | '\u{a0}' => {
                    if !word.is_empty() {
                        flush(&mut out, &mut word, &mut spaces, &style);
                    }
                    spaces.push(' ');
                }
                '\t' => {
                    flush(&mut out, &mut word, &mut spaces, &style);
                    out.push(Piece::Tab(style.clone()));
                }
                '\n' => {
                    flush(&mut out, &mut word, &mut spaces, &style);
                    out.push(Piece::Break);
                }
                _ => {
                    if !spaces.is_empty() {
                        flush(&mut out, &mut word, &mut spaces, &style);
                    }
                    word.push(ch);
                }
            }
        }
        flush(&mut out, &mut word, &mut spaces, &style);
    }
    out
}

fn line_height(size: f64, spacing: Spacing) -> f64 {
    match spacing {
        Spacing::Percent(p) => size * LINE * p.clamp(0.1, 10.0),
        Spacing::Points(pts) => pts.max(1.0),
    }
}

struct LineBuilder {
    items: Vec<Item>,
    x: f64,
    max_size: f64,
    has_word: bool,
    left: f64,
    avail: f64,
    /// The line continues a wrapped one: spaces at its start are the break, not text.
    skip_spaces: bool,
}

/// Join neighbouring pieces of text in the same style, so a line is shown (and later read back by
/// text extraction) as running text rather than word by word.
fn merge_items(items: Vec<Item>) -> Vec<Item> {
    let mut out: Vec<Item> = Vec::new();
    for item in items {
        if let Some(last) = out.last_mut()
            && !last.text.is_empty()
            && !item.text.is_empty()
            && last.style == item.style
            && (last.x + last.width - item.x).abs() < 1e-6
        {
            last.text.extend(item.text);
            last.width += item.width;
            last.space = false;
            continue;
        }
        out.push(item);
    }
    out
}

/// Break `paras` into lines `width` points wide (unbounded when `wrap` is false). `scale`
/// shrinks every font (normAutofit); `line_cut` is the fraction taken off percentage line
/// spacing.
pub(crate) fn layout(paras: &[Para], width: f64, wrap: bool, scale: f64, line_cut: f64) -> TextLayout {
    let mut out = Vec::new();
    let mut height = 0.0;
    for (index, para) in paras.iter().enumerate() {
        let spacing = match para.line {
            Spacing::Percent(p) => Spacing::Percent((p - line_cut).max(0.5)),
            other => other,
        };
        let bullet = para.bullet.as_ref().map(|b| BulletMark { size: b.size * scale, ..b.clone() });
        let bullet_w = bullet.as_ref().map_or(0.0, bullet_width);
        let first_size = para.runs.iter().map(|(_, s)| s.size).next().unwrap_or(para.end_size) * scale;
        let gap = first_size * 0.15;
        let bullet_x = (para.margin_left + para.indent).max(0.0);
        let first_left = if bullet.is_some() {
            if para.indent < 0.0 { para.margin_left.max(bullet_x + bullet_w + gap) } else { bullet_x + bullet_w + gap }
        } else {
            (para.margin_left + para.indent).max(0.0)
        };
        let rest_left = para.margin_left.max(0.0);
        let right = para.margin_right.max(0.0);
        let area_right = width - right;
        let new_line = |first: bool, skip_spaces: bool| {
            let left = if first { first_left } else { rest_left };
            LineBuilder { items: Vec::new(), x: left, max_size: 0.0, has_word: false, left, avail: (area_right - left).max(1.0), skip_spaces }
        };
        let mut builders: Vec<(LineBuilder, bool)> = Vec::new();
        let mut cur = new_line(true, false);
        // Ends the current line; `soft` is a wrap, as opposed to a forced break or the paragraph's end.
        let finish = |cur: &mut LineBuilder, builders: &mut Vec<(LineBuilder, bool)>, soft: bool| {
            // Trailing spaces hang past the edge: they neither count nor stretch.
            while cur.items.last().is_some_and(|i| i.space) {
                if let Some(i) = cur.items.pop() {
                    cur.x -= i.width;
                }
            }
            let done = std::mem::replace(cur, new_line(false, soft));
            builders.push((done, !soft));
        };
        for piece in pieces(para, scale) {
            match piece {
                Piece::Break => {
                    finish(&mut cur, &mut builders, false);
                }
                Piece::Tab(style) => {
                    // Default tab stops every inch from the text area's left edge.
                    let next = ((cur.x / 72.0).floor() + 1.0) * 72.0;
                    let w = (next - cur.x).max(1.0);
                    cur.max_size = cur.max_size.max(style.size);
                    cur.items.push(Item { x: cur.x, text: Vec::new(), style, width: w, space: true });
                    cur.x += w;
                }
                Piece::Space(text, style, w) => {
                    if cur.skip_spaces && cur.items.is_empty() {
                        continue;
                    }
                    cur.max_size = cur.max_size.max(style.size);
                    cur.items.push(Item { x: cur.x, text: encode(&text, style.face), style, width: w, space: true });
                    cur.x += w;
                }
                Piece::Word(text, style, w) => {
                    if wrap && cur.x + w > cur.left + cur.avail + 0.01 && cur.has_word {
                        finish(&mut cur, &mut builders, true);
                    }
                    if !wrap || cur.x + w <= cur.left + cur.avail + 0.01 {
                        cur.max_size = cur.max_size.max(style.size);
                        cur.items.push(Item { x: cur.x, text: encode(&text, style.face), style, width: w, space: false });
                        cur.x += w;
                        cur.has_word = true;
                    } else {
                        // A word wider than a whole line is broken between letters.
                        let chars: Vec<char> = text.chars().collect();
                        let mut start = 0;
                        while start < chars.len() {
                            let mut end = start;
                            let mut piece_w = 0.0;
                            while let Some(c) = chars.get(end) {
                                let cw = text_width(c.encode_utf8(&mut [0; 4]), &style);
                                if end > start && cur.x + piece_w + cw > cur.left + cur.avail + 0.01 {
                                    break;
                                }
                                piece_w += cw;
                                end += 1;
                            }
                            let piece: String = chars.get(start..end).unwrap_or_default().iter().collect();
                            cur.max_size = cur.max_size.max(style.size);
                            cur.items.push(Item { x: cur.x, text: encode(&piece, style.face), style: style.clone(), width: piece_w, space: false });
                            cur.x += piece_w;
                            cur.has_word = true;
                            start = end;
                            if start < chars.len() {
                                finish(&mut cur, &mut builders, true);
                            }
                        }
                    }
                }
            }
        }
        finish(&mut cur, &mut builders, false);
        let mut lines = Vec::new();
        for (b, last) in builders {
            let size = if b.max_size > 0.0 { b.max_size } else { para.end_size * scale };
            let lh = line_height(size, spacing);
            lines.push(Line { width: b.x - b.left, avail: b.avail, height: lh, baseline: lh - size * DESCENT, items: merge_items(b.items), last });
        }
        let before = if index == 0 { 0.0 } else { para.before * scale.max(0.5) };
        let after = para.after * scale.max(0.5);
        height += before + lines.iter().map(|l| l.height).sum::<f64>() + after;
        out.push(LaidPara { lines, before, after, bullet: bullet.map(|b| (bullet_x, b)), align: para.align });
    }
    TextLayout { paras: out, height }
}

/// Draw a laid-out text area whose top-left corner is `(x, y)`.
pub(crate) fn paint(canvas: &mut Canvas, layout: &TextLayout, x: f64, y: f64) {
    let mut top = y;
    for para in &layout.paras {
        top += para.before;
        let mut line_top = top;
        for (i, line) in para.lines.iter().enumerate() {
            // Centred and right-aligned text that is wider than its area overflows on the side
            // alignment implies, so the slack is signed for them.
            let slack = line.avail - line.width;
            let spaces = line.items.iter().map(|it| it.text.iter().filter(|b| **b == b' ').count()).sum::<usize>();
            let stretch = if para.align == Align::Justify && !line.last && spaces > 0 { slack.max(0.0) / spaces as f64 } else { 0.0 };
            let shift = match para.align {
                Align::Left | Align::Justify => 0.0,
                Align::Center => slack / 2.0,
                Align::Right => slack,
            };
            let baseline = line_top + line.baseline;
            line_top += line.height;
            let mut extra = 0.0;
            for item in &line.items {
                let item_x = x + item.x + shift + extra;
                let style = &item.style;
                let item_spaces = item.text.iter().filter(|b| **b == b' ').count() as f64;
                let item_width = item.width + stretch * item_spaces;
                extra += stretch * item_spaces;
                // A shifted baseline is a fraction of the unscaled font size.
                let rise = if style.baseline != 0.0 { style.baseline * style.size / SCRIPT_SCALE } else { 0.0 };
                let base_y = baseline - rise;
                if !item.text.is_empty() && !item.text.iter().all(|b| *b == b' ') {
                    canvas.text(style.font(), style.size, (item_x, base_y), stretch, style.color, &item.text);
                }
                let rule = |canvas: &mut Canvas, dy: f64| {
                    let mut p = Path::new();
                    p.move_to(item_x, base_y + dy);
                    p.line_to(item_x + item_width, base_y + dy);
                    canvas.stroke(&p, &Stroke { width: (style.size * 0.06).max(0.4), color: style.color, dash: Vec::new() });
                };
                if style.underline && !item.text.is_empty() {
                    rule(canvas, style.size * 0.12);
                }
                if style.strike && !item.text.is_empty() {
                    rule(canvas, -style.size * 0.3);
                }
            }
            if i == 0
                && let Some((bx, mark)) = &para.bullet
            {
                paint_bullet(canvas, mark, x + bx, baseline);
            }
        }
        top += para.lines.iter().map(|l| l.height).sum::<f64>() + para.after;
    }
}

fn paint_bullet(canvas: &mut Canvas, mark: &BulletMark, x: f64, baseline: f64) {
    match &mark.shape {
        BulletShape::Text(text) => {
            let font = if mark.bold { "F2" } else { "F1" };
            canvas.text(font, mark.size, (x, baseline), 0.0, mark.color, &win_ansi(text));
        }
        BulletShape::Glyph(g) => {
            let font = if g.face == Face::Symbol { "F5" } else { "F6" };
            canvas.text(font, mark.size, (x, baseline), 0.0, mark.color, &[g.code]);
        }
        BulletShape::Square => {
            let side = mark.size * 0.3;
            canvas.fill(&Path::rect(x, baseline - mark.size * 0.35 - side / 2.0, side, side), mark.color);
        }
        BulletShape::Triangle => {
            let (h, mid) = (mark.size * 0.4, baseline - mark.size * 0.33);
            let mut p = Path::new();
            p.move_to(x, mid - h / 2.0);
            p.line_to(x + h, mid);
            p.line_to(x, mid + h / 2.0);
            p.close();
            canvas.fill(&p, mark.color);
        }
    }
}

// ── bullets ─────────────────────────────────────────────────────────────────────────────────

fn roman(mut n: u32, upper: bool) -> String {
    const TABLE: [(u32, &str); 13] = [
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ];
    let mut s = String::new();
    for (v, r) in TABLE {
        while n >= v {
            s.push_str(r);
            n -= v;
        }
    }
    if upper { s.to_uppercase() } else { s }
}

fn alpha(n: u32, upper: bool) -> String {
    // a…z, aa…zz, aaa… (ST_TextAutonumberScheme: the letter repeats).
    let n = n.max(1) - 1;
    let letter = char::from(b'a' + (n % 26) as u8);
    let s: String = std::iter::repeat_n(letter, (n / 26 + 1) as usize).collect();
    if upper { s.to_uppercase() } else { s }
}

/// The label of an automatically numbered paragraph (ST_TextAutonumberScheme).
pub(crate) fn auto_number(kind: &str, n: u32) -> String {
    let (body, suffix, prefix) = match kind {
        k if k.starts_with("alphaLc") => (alpha(n, false), k.trim_start_matches("alphaLc"), ""),
        k if k.starts_with("alphaUc") => (alpha(n, true), k.trim_start_matches("alphaUc"), ""),
        k if k.starts_with("romanLc") => (roman(n, false), k.trim_start_matches("romanLc"), ""),
        k if k.starts_with("romanUc") => (roman(n, true), k.trim_start_matches("romanUc"), ""),
        k => (n.to_string(), k.trim_start_matches("arabic"), ""),
    };
    match suffix {
        "ParenR" => format!("{prefix}{body})"),
        "ParenBoth" => format!("({body})"),
        "Plain" => body,
        _ => format!("{body}."),
    }
}

/// How a `buChar` shows in the PDF's own fonts. Symbol-font bullets (Wingdings and the like)
/// are keyed by character code, so they are mapped by what they look like.
pub(crate) fn bullet_shape(ch: &str, font: Option<&str>) -> BulletShape {
    let first = ch.chars().next().unwrap_or('•');
    let name = font.map(str::to_ascii_lowercase).unwrap_or_default();
    // A symbol-font bullet is its font code, either as a private-use character or as Latin-1.
    let code = pptx_symbols::private_use_code(first).or_else(|| u8::try_from(first as u32).ok());
    if name == "symbol"
        && let Some(code) = code
    {
        return BulletShape::Glyph(Glyph { face: Face::Symbol, code });
    }
    if name.starts_with("wingdings") {
        if let Some(glyph) = code.and_then(|c| pptx_symbols::private_use_glyph(c, &name)) {
            return BulletShape::Glyph(glyph);
        }
        return match first {
            '§' | 'n' | 'p' | 'q' | 'u' => BulletShape::Square,
            'Ø' | 'è' | 'ð' => BulletShape::Triangle,
            _ => BulletShape::Text("•".into()),
        };
    }
    if name.contains("webdings") {
        return BulletShape::Text("•".into());
    }
    if win_ansi(&first.to_string()) != b"?" || first == '?' {
        return BulletShape::Text(first.to_string());
    }
    if let Some(glyph) = pptx_symbols::glyph(first) {
        return BulletShape::Glyph(glyph);
    }
    match first {
        '▪' | '▫' | '□' | '◼' | '◾' | '❏' => BulletShape::Square,
        '❯' => BulletShape::Triangle,
        _ => BulletShape::Text("•".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style(size: f64) -> RunStyle {
        RunStyle {
            face: Face::Text,
            size,
            bold: false,
            italic: false,
            underline: false,
            strike: false,
            color: Rgba { rgb: [0.0; 3], alpha: 1.0 },
            baseline: 0.0,
        }
    }

    fn para(text: &str, size: f64) -> Para {
        Para {
            runs: vec![(text.to_owned(), style(size))],
            align: Align::Left,
            margin_left: 0.0,
            margin_right: 0.0,
            indent: 0.0,
            line: Spacing::Percent(1.0),
            before: 0.0,
            after: 0.0,
            bullet: None,
            end_size: size,
        }
    }

    #[test]
    fn numbering_schemes() {
        assert_eq!(auto_number("arabicPeriod", 3), "3.");
        assert_eq!(auto_number("arabicParenR", 3), "3)");
        assert_eq!(auto_number("arabicParenBoth", 3), "(3)");
        assert_eq!(auto_number("alphaLcPeriod", 2), "b.");
        assert_eq!(auto_number("alphaUcParenR", 27), "AA)");
        assert_eq!(auto_number("romanLcPeriod", 14), "xiv.");
        assert_eq!(auto_number("romanUcPeriod", 1994), "MCMXCIV.");
    }

    #[test]
    fn long_text_wraps_inside_the_width() {
        let p = para("the quick brown fox jumps over the lazy dog and keeps running", 12.0);
        let layout = layout(&[p], 120.0, true, 1.0, 0.0);
        let lines = &layout.paras[0].lines;
        assert!(lines.len() >= 3, "{}", lines.len());
        assert!(lines.iter().all(|l| l.width <= 120.5), "no line is wider than the area");
        assert!((layout.height - lines.iter().map(|l| l.height).sum::<f64>()).abs() < 1e-9);
    }

    #[test]
    fn without_wrapping_a_paragraph_is_one_line() {
        let p = para("the quick brown fox jumps over the lazy dog", 12.0);
        assert_eq!(layout(&[p], 50.0, false, 1.0, 0.0).paras[0].lines.len(), 1);
    }

    #[test]
    fn a_word_longer_than_the_line_is_broken_between_letters() {
        let p = para("Supercalifragilisticexpialidocious", 12.0);
        let l = layout(&[p], 60.0, true, 1.0, 0.0);
        assert!(l.paras[0].lines.len() > 1);
        assert!(l.paras[0].lines.iter().all(|l| l.width <= 60.5));
    }

    #[test]
    fn forced_breaks_and_empty_paragraphs_take_a_line() {
        let l = layout(&[para("one\ntwo", 10.0), para("", 10.0)], 200.0, true, 1.0, 0.0);
        assert_eq!(l.paras[0].lines.len(), 2);
        assert_eq!(l.paras[1].lines.len(), 1);
        assert!((l.height - 3.0 * 10.0 * LINE).abs() < 1e-9);
    }

    #[test]
    fn hanging_indent_puts_the_text_after_the_bullet() {
        let mut p = para("item", 20.0);
        p.margin_left = 27.0;
        p.indent = -27.0;
        p.bullet = Some(BulletMark { shape: BulletShape::Text("•".into()), size: 20.0, color: Rgba { rgb: [0.0; 3], alpha: 1.0 }, bold: false });
        let l = layout(&[p], 300.0, true, 1.0, 0.0);
        let line = &l.paras[0].lines[0];
        let text_x = line.items.first().map(|i| i.x);
        assert_eq!(text_x, Some(27.0));
        assert_eq!(l.paras[0].bullet.as_ref().map(|(x, _)| *x), Some(0.0));
    }

    #[test]
    fn scaling_shrinks_the_text() {
        let p = para("some text that is shown smaller", 20.0);
        let full = layout(std::slice::from_ref(&p), 400.0, true, 1.0, 0.0).height;
        let half = layout(&[p], 400.0, true, 0.5, 0.0).height;
        assert!((half - full / 2.0).abs() < 1e-9);
    }

    #[test]
    fn symbol_bullets_map_to_shapes() {
        assert!(matches!(bullet_shape("§", Some("Wingdings")), BulletShape::Square));
        assert!(matches!(bullet_shape("•", Some("Arial")), BulletShape::Text(t) if t == "•"));
        assert!(matches!(bullet_shape("o", Some("Courier New")), BulletShape::Text(t) if t == "o"));
        assert!(matches!(bullet_shape("➢", None), BulletShape::Glyph(g) if g.face == Face::Dingbats && g.code == 0xE2));
        assert!(matches!(bullet_shape("\u{f0b7}", Some("Symbol")), BulletShape::Glyph(g) if g.face == Face::Symbol && g.code == 0xB7));
        assert!(matches!(bullet_shape("▪", None), BulletShape::Square));
    }

    #[test]
    fn parses_levels_and_layers_them() {
        let xml = r#"<a:lstStyle xmlns:a="urn:a"><a:lvl1pPr marL="342900" indent="-342900" algn="ctr"><a:lnSpc><a:spcPct val="90000"/></a:lnSpc><a:spcBef><a:spcPts val="1000"/></a:spcBef><a:buChar char="•"/><a:defRPr sz="2800" b="1"/></a:lvl1pPr></a:lstStyle>"#;
        let doc = roxmltree::Document::parse(xml).unwrap();
        let mut levels: [ParaProps; 9] = Default::default();
        merge_list_style(&mut levels, doc.root_element());
        let l = &levels[0];
        assert_eq!(l.margin_left, Some(27.0));
        assert_eq!(l.indent, Some(-27.0));
        assert_eq!(l.align, Some(Align::Center));
        assert!(matches!(l.line, Some(Spacing::Percent(p)) if (p - 0.9).abs() < 1e-9));
        assert!(matches!(l.before, Some(Spacing::Points(p)) if (p - 10.0).abs() < 1e-9));
        assert_eq!(l.run.size, Some(28.0));
        assert_eq!(l.run.bold, Some(true));
        assert!(levels[1].margin_left.is_none());
    }
}

#[cfg(test)]
mod paint_tests {
    use super::*;

    /// The baseline of every `Tm` in a content stream.
    fn baselines(canvas: Canvas) -> Vec<f64> {
        let (bytes, _) = canvas.finish();
        let text: String = bytes.iter().map(|b| *b as char).collect();
        text.split(" Tm")
            .filter_map(|chunk| chunk.rsplit(' ').next().map(str::to_owned))
            .collect::<Vec<_>>()
            .windows(1)
            .filter_map(|w| w[0].parse::<f64>().ok())
            .collect()
    }

    #[test]
    fn lines_of_a_paragraph_and_paragraphs_stack_downwards() {
        let mut first = Para { ..para_for_paint("alpha beta gamma delta epsilon zeta eta theta") };
        first.after = 6.0;
        let second = para_for_paint("second paragraph");
        let laid = layout(&[first, second], 90.0, true, 1.0, 0.0);
        let lines: Vec<f64> = laid.paras.iter().flat_map(|p| p.lines.iter().map(|l| l.height)).collect();
        assert!(lines.len() >= 3);
        let mut canvas = Canvas::new(500.0);
        paint(&mut canvas, &laid, 0.0, 0.0);
        let ys = baselines(canvas);
        assert!(ys.len() >= 3, "{ys:?}");
        assert!(ys.windows(2).all(|w| w[1] > w[0] + 5.0), "each line sits below the previous one: {ys:?}");
        // One line height (12 pt × 1.2) between the lines of a paragraph.
        assert!((ys[1] - ys[0] - 14.4).abs() < 0.01, "{ys:?}");
        // The paragraph's space after, then the next paragraph's first line.
        let last_of_first = laid.paras[0].lines.len() - 1;
        assert!((ys[last_of_first + 1] - ys[last_of_first] - 14.4 - 6.0).abs() < 0.01, "{ys:?}");
    }

    fn para_for_paint(text: &str) -> Para {
        Para {
            runs: vec![(
                text.to_owned(),
                RunStyle {
                    face: Face::Text,
                    size: 12.0,
                    bold: false,
                    italic: false,
                    underline: false,
                    strike: false,
                    color: Rgba { rgb: [0.0; 3], alpha: 1.0 },
                    baseline: 0.0,
                },
            )],
            align: Align::Left,
            margin_left: 0.0,
            margin_right: 0.0,
            indent: 0.0,
            line: Spacing::Percent(1.0),
            before: 0.0,
            after: 0.0,
            bullet: None,
            end_size: 12.0,
        }
    }
}
