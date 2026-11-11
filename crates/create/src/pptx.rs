//! Native, bounded PowerPoint (PresentationML) to PDF conversion.
//!
//! One slide becomes one page of the slide's size. The package is read as ISO/IEC 29500 says:
//! slides name their layout, layouts their master, masters their theme, and a shape that is a
//! placeholder takes whatever it leaves open from the placeholder behind it. Text, shapes, lines,
//! pictures and tables are drawn. Content that is not drawn yet (charts, SmartArt, embedded
//! objects, ...) is listed in [`PptxConversion::notes`] instead of being dropped silently, and
//! text that the PDF's own fonts cannot show stops the conversion rather than being altered.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use karnoxpdf_cos::{Dict, Document, ObjRef, Object};
use roxmltree::Node;

use crate::drawingml::{ColorSpec, FillSpec, LineSpec, Palette, Rgba, Theme, color_in, parse_color_map, parse_fill, parse_line, scheme_color};
use crate::ooxml::{Package, Rel, attr, bad, boolean, child, children, number, parse_xml, rel_attr, relationships};
use crate::pptx_paint::{Canvas, GradientKind, Matrix, Path, Stroke};
use crate::pptx_symbols;
use crate::pptx_table::{self as table, StylePart, TableFlags, TableStyles};
use crate::pptx_text::{
    Align, Bullet, BulletMark, Face, Para, ParaProps, RunProps, RunStyle, Spacing, TextLayout, auto_number, bullet_shape, layout, merge_list_style,
    paint, parse_para_props, parse_run_props,
};
use crate::{CreateError, add_page, image_xobject, set_title};

const EMU: f64 = 12_700.0;
const MAX_XML: usize = 16 * 1024 * 1024;
const MAX_SLIDES: usize = 1000;
const MAX_IMAGE: usize = 48 * 1024 * 1024;
const MAX_IMAGES: usize = 4000;
const MAX_SHAPES: usize = 20_000;
const MAX_DEPTH: usize = 16;
const MAX_PARAGRAPHS: usize = 4000;
const MAX_TEXT_CHARS: usize = 400_000;

/// The largest presentation accepted. Presentations carry video and large pictures, which are
/// never read, so this is well above the limit for Word documents.
pub const MAX_PPTX_BYTES: usize = 512 * 1024 * 1024;

type N<'a> = Node<'a, 'a>;

/// Content the converter could not draw, so the PDF differs from the presentation there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NoteKind {
    /// A hidden slide, left out as PowerPoint's own PDF export does by default.
    HiddenSlide,
    Chart,
    /// SmartArt.
    Diagram,
    /// An embedded or linked object (OLE).
    EmbeddedObject,
    /// A picture in a format that is not supported (EMF, WMF, SVG, ...), or unreadable.
    Picture,
    /// A preset shape the converter has no outline for; drawn as its bounding rectangle.
    ShapeGeometry,
    /// Text laid out vertically; drawn horizontally.
    VerticalText,
    /// A table style that the presentation does not define; its fill and borders are not drawn.
    TableStyle,
    /// A sheet that is hidden in its workbook (numbers in notes are sheet positions).
    HiddenSheet,
    /// Charts, pictures and shapes drawn on a worksheet.
    Drawing,
    /// Conditional formatting rules (colour scales, data bars, highlighted cells).
    ConditionalFormatting,
    /// Spreadsheet table styles (banded rows and the like).
    SheetTable,
    /// Anything else the converter does not understand.
    Other,
}

impl NoteKind {
    /// A stable identifier for scripts.
    pub fn id(self) -> &'static str {
        match self {
            NoteKind::HiddenSlide => "hidden_slide",
            NoteKind::Chart => "chart",
            NoteKind::Diagram => "diagram",
            NoteKind::EmbeddedObject => "embedded_object",
            NoteKind::Picture => "picture",
            NoteKind::ShapeGeometry => "shape_geometry",
            NoteKind::VerticalText => "vertical_text",
            NoteKind::TableStyle => "table_style",
            NoteKind::HiddenSheet => "hidden_sheet",
            NoteKind::Drawing => "drawing",
            NoteKind::ConditionalFormatting => "conditional_formatting",
            NoteKind::SheetTable => "sheet_table",
            NoteKind::Other => "other",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            NoteKind::HiddenSlide => "hidden slides were left out",
            NoteKind::Chart => "charts were not drawn",
            NoteKind::Diagram => "SmartArt graphics were not drawn",
            NoteKind::EmbeddedObject => "embedded objects were not drawn",
            NoteKind::Picture => "pictures in an unsupported format were not drawn",
            NoteKind::ShapeGeometry => "some shapes were drawn as rectangles",
            NoteKind::VerticalText => "vertical text was drawn horizontally",
            NoteKind::TableStyle => "a table style is missing from the presentation, so its colours were not applied",
            NoteKind::HiddenSheet => "hidden sheets were left out",
            NoteKind::Drawing => "charts, pictures and shapes on sheets were not drawn",
            NoteKind::ConditionalFormatting => "conditional formatting was not applied",
            NoteKind::SheetTable => "table styles were not applied",
            NoteKind::Other => "some content was not understood and not drawn",
        }
    }
}

/// One kind of difference between the presentation and the PDF, and the slides it concerns
/// (1-based).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversionNote {
    pub kind: NoteKind,
    pub slides: Vec<usize>,
}

pub struct PptxConversion {
    pub document: Document,
    pub notes: Vec<ConversionNote>,
}

// ── geometry helpers ────────────────────────────────────────────────────────────────────────

/// `a:xfrm` / `p:xfrm`, in points.
#[derive(Clone, Copy, Debug)]
struct Xfrm {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    rot: f64,
    flip_h: bool,
    flip_v: bool,
    /// A group's child coordinate space: x, y, width, height.
    child: Option<(f64, f64, f64, f64)>,
}

fn pair(node: Node<'_, '_>, name: &str, a: &str, b: &str) -> Option<(f64, f64)> {
    let n = child(node, name)?;
    Some((number(n, a)? / EMU, number(n, b)? / EMU))
}

fn parse_xfrm(node: Node<'_, '_>) -> Option<Xfrm> {
    let (x, y) = pair(node, "off", "x", "y")?;
    let (w, h) = pair(node, "ext", "cx", "cy")?;
    let child_space = match (pair(node, "chOff", "x", "y"), pair(node, "chExt", "cx", "cy")) {
        (Some((cx, cy)), Some((cw, ch))) => Some((cx, cy, cw, ch)),
        _ => None,
    };
    Some(Xfrm {
        x,
        y,
        w: w.max(0.0),
        h: h.max(0.0),
        rot: number(node, "rot").map_or(0.0, |r| r / 60_000.0),
        flip_h: boolean(node, "flipH").unwrap_or(false),
        flip_v: boolean(node, "flipV").unwrap_or(false),
        child: child_space,
    })
}

/// Where a shape ends up on the slide once its groups are applied.
#[derive(Clone, Copy, Debug)]
struct Frame {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    rot: f64,
    flip_h: bool,
    flip_v: bool,
}

fn resolve_frame(own: Xfrm, groups: &[Xfrm]) -> Frame {
    let mut f = Frame { x: own.x, y: own.y, w: own.w, h: own.h, rot: own.rot, flip_h: own.flip_h, flip_v: own.flip_v };
    for g in groups.iter().rev() {
        let (cx0, cy0, cw, ch) = g.child.unwrap_or((g.x, g.y, g.w, g.h));
        let sx = if cw.abs() > 1e-9 { g.w / cw } else { 1.0 };
        let sy = if ch.abs() > 1e-9 { g.h / ch } else { 1.0 };
        f.x = g.x + (f.x - cx0) * sx;
        f.y = g.y + (f.y - cy0) * sy;
        f.w *= sx;
        f.h *= sy;
        if g.flip_h {
            f.x = g.x + g.w - (f.x - g.x) - f.w;
            f.flip_h = !f.flip_h;
            f.rot = -f.rot;
        }
        if g.flip_v {
            f.y = g.y + g.h - (f.y - g.y) - f.h;
            f.flip_v = !f.flip_v;
            f.rot = -f.rot;
        }
        if g.rot != 0.0 {
            let (gx, gy) = (g.x + g.w / 2.0, g.y + g.h / 2.0);
            let (dx, dy) = (f.x + f.w / 2.0 - gx, f.y + f.h / 2.0 - gy);
            let (s, c) = g.rot.to_radians().sin_cos();
            let (ncx, ncy) = (gx + dx * c - dy * s, gy + dx * s + dy * c);
            f.x = ncx - f.w / 2.0;
            f.y = ncy - f.h / 2.0;
            f.rot += g.rot;
        }
    }
    f
}

// ── placeholders ────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct Ph {
    ty: String,
    idx: u32,
    has_idx: bool,
}

fn ph_of(node: Node<'_, '_>) -> Option<Ph> {
    let nv = node.children().filter(|n| n.is_element()).find(|n| n.tag_name().name().starts_with("nv"))?;
    let ph = child(child(nv, "nvPr")?, "ph")?;
    Some(Ph {
        ty: attr(ph, "type").unwrap_or("obj").to_owned(),
        idx: number(ph, "idx").map_or(0, |v| v.clamp(0.0, 1.0e6) as u32),
        has_idx: attr(ph, "idx").is_some(),
    })
}

/// The kind of placeholder on a master that a placeholder of type `ty` takes its format from.
fn master_type(ty: &str) -> &str {
    match ty {
        "ctrTitle" | "title" => "title",
        "dt" | "ftr" | "sldNum" | "hdr" => ty,
        _ => "body",
    }
}

fn shapes_with_ph<'a>(tree: N<'a>) -> impl Iterator<Item = (N<'a>, Ph)> {
    tree.children().filter(|n| n.is_element() && n.tag_name().name() == "sp").filter_map(|n| ph_of(n).map(|p| (n, p)))
}

/// The layout's placeholder a slide's placeholder inherits from (§19.3.1.36: by index, then type).
fn layout_placeholder<'a>(tree: N<'a>, ph: &Ph) -> Option<N<'a>> {
    let all: Vec<_> = shapes_with_ph(tree).collect();
    all.iter()
        .find(|(_, p)| p.idx == ph.idx && p.ty == ph.ty)
        .or_else(|| all.iter().find(|(_, p)| ph.has_idx && p.has_idx && p.idx == ph.idx))
        .or_else(|| all.iter().find(|(_, p)| p.ty == ph.ty))
        .map(|(n, _)| *n)
}

fn master_placeholder<'a>(tree: N<'a>, ph: &Ph) -> Option<N<'a>> {
    let want = master_type(&ph.ty);
    shapes_with_ph(tree).find(|(_, p)| master_type(&p.ty) == want).map(|(n, _)| n)
}

// ── shared state ────────────────────────────────────────────────────────────────────────────

struct Shared<'p> {
    package: &'p Package<'p>,
    out: Document,
    images: Vec<ObjRef>,
    image_cache: HashMap<String, Option<usize>>,
    notes: BTreeMap<NoteKind, BTreeSet<usize>>,
}

impl Shared<'_> {
    fn note(&mut self, kind: NoteKind, slide: usize) {
        self.notes.entry(kind).or_default().insert(slide);
    }

    /// The index of the image stored in the part `name`, embedding it on first use.
    fn image(&mut self, name: &str, slide: usize) -> Option<usize> {
        let found = match self.image_cache.get(name) {
            Some(cached) => *cached,
            None => {
                let loaded = self.load_image(name);
                self.image_cache.insert(name.to_owned(), loaded);
                loaded
            }
        };
        if found.is_none() {
            self.note(NoteKind::Picture, slide);
        }
        found
    }

    fn load_image(&mut self, name: &str) -> Option<usize> {
        if self.images.len() >= MAX_IMAGES {
            return None;
        }
        let bytes = self.package.read(name, MAX_IMAGE).ok()??;
        let (r, _) = image_xobject(&mut self.out, name, &bytes).ok()?;
        self.images.push(r);
        Some(self.images.len() - 1)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Layer {
    Master,
    Layout,
    Slide,
}

/// A fill or stroke colour, ready for the canvas.
enum Paint {
    None,
    Solid(Rgba),
    Gradient(Vec<(f64, Rgba)>, GradientKind),
}

#[derive(Clone, Debug, Default)]
struct BodyProps {
    left: Option<f64>,
    top: Option<f64>,
    right: Option<f64>,
    bottom: Option<f64>,
    anchor: Option<String>,
    wrap: Option<bool>,
    vertical: bool,
    autofit: Option<Autofit>,
}

#[derive(Clone, Copy, Debug)]
enum Autofit {
    Off,
    /// Shrink on overflow (`normAutofit`), starting from what PowerPoint stored.
    Shrink {
        scale: f64,
        line_cut: f64,
    },
}

impl BodyProps {
    fn parse(node: Node<'_, '_>) -> BodyProps {
        let inset = |name: &str| number(node, name).map(|v| (v / EMU).clamp(0.0, 2000.0));
        let autofit = if child(node, "noAutofit").is_some() || child(node, "spAutoFit").is_some() {
            Some(Autofit::Off)
        } else {
            child(node, "normAutofit").map(|n| Autofit::Shrink {
                scale: number(n, "fontScale").map_or(1.0, |v| (v / 100_000.0).clamp(0.1, 1.0)),
                line_cut: number(n, "lnSpcReduction").map_or(0.0, |v| (v / 100_000.0).clamp(0.0, 0.5)),
            })
        };
        BodyProps {
            left: inset("lIns"),
            top: inset("tIns"),
            right: inset("rIns"),
            bottom: inset("bIns"),
            anchor: attr(node, "anchor").map(str::to_owned),
            wrap: attr(node, "wrap").map(|w| w != "none"),
            vertical: attr(node, "vert").is_some_and(|v| v != "horz"),
            autofit,
        }
    }

    fn over(&mut self, upper: &BodyProps) {
        macro_rules! take {
            ($($f:ident),*) => { $(if upper.$f.is_some() { self.$f = upper.$f.clone(); })* };
        }
        take!(left, top, right, bottom, anchor, wrap, autofit);
        self.vertical |= upper.vertical;
    }
}

/// What the converter needs from the shape tree it is walking.
struct Drawer<'d, 'p> {
    shared: &'d mut Shared<'p>,
    canvas: Canvas,
    slide_no: usize,
    palette: &'d Palette<'d>,
    theme: &'d Theme,
    default_text: Option<N<'d>>,
    tx_styles: Option<N<'d>>,
    layout_tree: Option<N<'d>>,
    master_tree: Option<N<'d>>,
    table_styles: &'d TableStyles,
    shapes_left: usize,
}

impl<'d, 'p> Drawer<'d, 'p> {
    fn note(&mut self, kind: NoteKind) {
        let slide = self.slide_no;
        self.shared.note(kind, slide);
    }

    // ── colours and fills ───────────────────────────────────────────────────────────────────

    fn color(&self, spec: &ColorSpec, placeholder: Option<Rgba>) -> Rgba {
        self.palette.resolve(spec, placeholder)
    }

    fn paint(&self, spec: &FillSpec, placeholder: Option<Rgba>) -> Paint {
        match spec {
            FillSpec::None | FillSpec::Picture(_) => Paint::None,
            FillSpec::Solid(c) | FillSpec::Pattern(c) => Paint::Solid(self.color(c, placeholder)),
            FillSpec::Gradient(g) => {
                let stops = g.stops.iter().map(|(p, c)| (p.clamp(0.0, 1.0), self.color(c, placeholder))).collect();
                Paint::Gradient(stops, if g.radial { GradientKind::Radial } else { GradientKind::Linear(g.angle) })
            }
        }
    }

    /// A theme fill style (`fillRef`) or line style (`lnRef`) with the colour its reference gives.
    fn style_ref(&self, style: Option<N<'d>>, name: &str) -> Option<(u32, Option<Rgba>)> {
        let r = child(style?, name)?;
        let idx = number(r, "idx")?.clamp(0.0, 4_000_000.0) as u32;
        Some((idx, color_in(r).map(|c| self.color(&c, None))))
    }

    fn theme_fill(&self, idx: u32) -> Option<&'d FillSpec> {
        match idx {
            0 => None,
            1..=999 => self.theme.fills.get(idx as usize - 1),
            _ => self.theme.background_fills.get((idx - 1001) as usize),
        }
    }

    // ── text ────────────────────────────────────────────────────────────────────────────────

    /// The paragraph properties of each list level for one text body: the presentation's defaults,
    /// the master's style for this kind of text, then the placeholder chain and the shape itself.
    fn levels(&self, ph: Option<&Ph>, layers: &[N<'d>], own_list: Option<N<'d>>, base: Option<&ParaProps>) -> [ParaProps; 9] {
        let mut levels: [ParaProps; 9] = Default::default();
        if let Some(default) = self.default_text {
            merge_list_style(&mut levels, default);
        }
        if let Some(styles) = self.tx_styles {
            let kind = match ph.map(|p| p.ty.as_str()) {
                Some("title" | "ctrTitle") => "titleStyle",
                Some("dt" | "ftr" | "sldNum" | "hdr") | None => "otherStyle",
                Some(_) => "bodyStyle",
            };
            if let Some(style) = child(styles, kind) {
                merge_list_style(&mut levels, style);
            }
        }
        if let Some(base) = base {
            for level in &mut levels {
                level.over(base);
            }
        }
        for layer in layers.iter().rev() {
            if let Some(list) = child(*layer, "txBody").and_then(|b| child(b, "lstStyle")) {
                merge_list_style(&mut levels, list);
            }
        }
        if let Some(list) = own_list {
            merge_list_style(&mut levels, list);
        }
        levels
    }

    /// The text of a run as segments, each in the standard PDF font that can show it. A character
    /// no standard font shows stops the conversion, rather than being changed.
    fn segments(&self, text: &str, typeface: Option<&str>) -> Result<Vec<(String, Face)>, CreateError> {
        pptx_symbols::segments(text, typeface).map_err(|c| {
            let font = typeface.map_or(String::new(), |f| format!(" in the font {f}"));
            bad(format!(
                "slide {} contains {c:?} (U+{:04X}){font}, which the PDF's built-in fonts cannot show; conversion stopped to avoid changing the text",
                self.slide_no, c as u32
            ))
        })
    }

    /// A run's style, and whether its text is shown in capitals.
    fn run_style(&self, props: &RunProps, own: Option<N<'d>>) -> (RunStyle, bool, Option<String>) {
        let mut p = props.clone();
        if let Some(own) = own {
            p.over(&parse_run_props(own));
            // A hyperlink run takes the theme's hyperlink colour and an underline.
            if child(own, "hlinkClick").is_some() {
                p.color = Some(scheme_color("hlink"));
                p.underline = Some(true);
            }
        }
        let style = RunStyle {
            face: Face::Text,
            size: p.size.unwrap_or(18.0),
            bold: p.bold.unwrap_or(false),
            italic: p.italic.unwrap_or(false),
            underline: p.underline.unwrap_or(false),
            strike: p.strike.unwrap_or(false),
            color: p.color.as_ref().map_or(Rgba { rgb: [0.0; 3], alpha: 1.0 }, |c| self.color(c, None)),
            baseline: p.baseline.unwrap_or(0.0),
        };
        (style, p.caps == Some(true), p.typeface.clone())
    }

    fn paragraphs(&mut self, body: N<'d>, levels: &[ParaProps; 9]) -> Result<Vec<Para>, CreateError> {
        let mut counters: [Option<u32>; 9] = [None; 9];
        let mut chars_left = MAX_TEXT_CHARS;
        let mut out = Vec::new();
        for p in children(body, "p").take(MAX_PARAGRAPHS) {
            let ppr = child(p, "pPr");
            let level = ppr.and_then(|n| number(n, "lvl")).map_or(0, |v| v.clamp(0.0, 8.0) as usize);
            let mut props = levels.get(level).cloned().unwrap_or_default();
            if let Some(n) = ppr {
                props.over(&parse_para_props(n));
            }
            let mut runs: Vec<(String, RunStyle)> = Vec::new();
            for item in p.children().filter(|n| n.is_element()) {
                match item.tag_name().name() {
                    "r" | "fld" => {
                        let raw = if item.tag_name().name() == "fld" && attr(item, "type").is_some_and(|t| t.starts_with("slidenum")) {
                            self.slide_no.to_string()
                        } else {
                            child(item, "t").and_then(|t| t.text()).unwrap_or_default().to_owned()
                        };
                        let (style, caps, typeface) = self.run_style(&props.run, child(item, "rPr"));
                        chars_left = chars_left
                            .checked_sub(raw.len())
                            .ok_or_else(|| bad(format!("slide {} has a text box with too much text", self.slide_no)))?;
                        for (text, face) in self.segments(&if caps { raw.to_uppercase() } else { raw }, typeface.as_deref())? {
                            runs.push((text, RunStyle { face, ..style.clone() }));
                        }
                    }
                    "br" => runs.push(("\n".into(), self.run_style(&props.run, child(item, "rPr")).0)),
                    "AlternateContent" => self.note(NoteKind::Other),
                    _ => {}
                }
            }
            let end_style = self.run_style(&props.run, child(p, "endParaRPr")).0;
            let first_style = runs.iter().find(|(t, _)| t != "\n").map(|(_, s)| s.clone()).unwrap_or_else(|| end_style.clone());
            let size = runs.iter().map(|(_, s)| s.size).fold(0.0_f64, f64::max).max(if runs.is_empty() { end_style.size } else { 0.0 });
            let has_text = runs.iter().any(|(t, _)| !t.trim().is_empty());
            let bullet = if has_text {
                let color = props.bullet_color.as_ref().map_or(first_style.color, |c| self.color(c, None));
                let bsize = first_style.size * props.bullet_size.unwrap_or(1.0);
                match &props.bullet {
                    Some(Bullet::Char(ch)) => {
                        counters.iter_mut().skip(level).for_each(|c| *c = None);
                        Some(BulletMark { shape: bullet_shape(ch, props.bullet_font.as_deref()), size: bsize, color, bold: first_style.bold })
                    }
                    Some(Bullet::Auto { kind, start }) => {
                        let next = counters.get(level).copied().flatten().map_or(*start, |n| n.saturating_add(1));
                        counters.iter_mut().skip(level + 1).for_each(|c| *c = None);
                        if let Some(c) = counters.get_mut(level) {
                            *c = Some(next);
                        }
                        Some(BulletMark {
                            shape: crate::pptx_text::BulletShape::Text(auto_number(kind, next)),
                            size: bsize,
                            color,
                            bold: first_style.bold,
                        })
                    }
                    _ => {
                        counters.iter_mut().skip(level).for_each(|c| *c = None);
                        None
                    }
                }
            } else {
                None
            };
            let points = |s: Option<Spacing>| match s {
                Some(Spacing::Points(v)) => v.max(0.0),
                Some(Spacing::Percent(v)) => v.max(0.0) * size,
                None => 0.0,
            };
            out.push(Para {
                runs,
                align: props.align.unwrap_or(Align::Left),
                margin_left: props.margin_left.unwrap_or(0.0),
                margin_right: props.margin_right.unwrap_or(0.0),
                indent: props.indent.unwrap_or(0.0),
                line: props.line.unwrap_or(Spacing::Percent(1.0)),
                before: points(props.before),
                after: points(props.after),
                bullet,
                end_size: end_style.size,
            });
        }
        Ok(out)
    }

    /// Lay `paras` out in a `w` × `h` area, shrinking them when the body asks for that and they
    /// do not fit.
    fn fit(&self, paras: &[Para], body: &BodyProps, w: f64, h: f64) -> TextLayout {
        let wrap = body.wrap.unwrap_or(true);
        match body.autofit {
            Some(Autofit::Shrink { scale, line_cut }) => {
                let mut scale = scale;
                let mut laid = layout(paras, w, wrap, scale, line_cut);
                let mut steps = 0;
                while laid.height > h && scale > 0.25 && steps < 24 {
                    scale *= 0.94;
                    steps += 1;
                    laid = layout(paras, w, wrap, scale, line_cut.max(0.1));
                }
                laid
            }
            _ => layout(paras, w, wrap, 1.0, 0.0),
        }
    }

    fn draw_body(&mut self, paras: &[Para], body: &BodyProps, w: f64, h: f64) {
        if paras.iter().all(|p| p.runs.is_empty()) {
            return;
        }
        let (l, t, r, b) = (body.left.unwrap_or(7.2), body.top.unwrap_or(3.6), body.right.unwrap_or(7.2), body.bottom.unwrap_or(3.6));
        let (area_w, area_h) = ((w - l - r).max(1.0), (h - t - b).max(1.0));
        let laid = self.fit(paras, body, area_w, area_h);
        let top = match body.anchor.as_deref() {
            Some("ctr") => t + (area_h - laid.height) / 2.0,
            Some("b") => t + area_h - laid.height,
            _ => t,
        };
        paint(&mut self.canvas, &laid, l, top);
    }

    // ── shapes ──────────────────────────────────────────────────────────────────────────────

    fn draw_tree(&mut self, tree: N<'d>, layer: Layer, rels: &[Rel], groups: &mut Vec<Xfrm>, depth: usize) -> Result<(), CreateError> {
        if depth > MAX_DEPTH {
            self.note(NoteKind::Other);
            return Ok(());
        }
        for node in tree.children().filter(|n| n.is_element()) {
            let name = node.tag_name().name();
            if matches!(name, "sp" | "cxnSp" | "pic" | "grpSp" | "graphicFrame") {
                self.shapes_left = self.shapes_left.checked_sub(1).ok_or_else(|| bad("a slide has too many shapes"))?;
            }
            match name {
                "sp" | "cxnSp" => self.draw_shape(node, layer, rels, groups)?,
                "pic" => self.draw_picture(node, layer, rels, groups)?,
                "graphicFrame" => self.draw_frame(node, rels, groups)?,
                "grpSp" => {
                    if is_hidden(node) {
                        continue;
                    }
                    let Some(xfrm) = child(node, "grpSpPr").and_then(|p| child(p, "xfrm")).and_then(parse_xfrm) else { continue };
                    groups.push(xfrm);
                    let result = self.draw_tree(node, layer, rels, groups, depth + 1);
                    groups.pop();
                    result?;
                }
                // Markup-compatibility wrapper: a reader that does not know the newer features a
                // `Choice` requires takes the `Fallback` (ISO/IEC 29500-3 §10.2.3).
                "AlternateContent" => match child(node, "Fallback") {
                    Some(fallback) => self.draw_tree(fallback, layer, rels, groups, depth + 1)?,
                    None => self.note(NoteKind::Other),
                },
                // Structure that carries no drawing of its own.
                "nvGrpSpPr" | "grpSpPr" | "extLst" => {}
                // Anything else on a slide (ink, content parts, ...) is not drawn: say so.
                _ => self.note(NoteKind::Other),
            }
        }
        Ok(())
    }

    fn line_stroke(&self, line: &LineSpec, placeholder: Option<Rgba>) -> Option<Stroke> {
        let width = line.width.unwrap_or(0.0).max(0.25);
        let color = match self.paint(line.fill.as_ref()?, placeholder) {
            Paint::Solid(c) => c,
            Paint::Gradient(stops, _) => stops.first()?.1,
            Paint::None => return None,
        };
        let dash = line.dash.as_ref().map(|d| d.iter().map(|m| m * width.max(0.5)).collect()).unwrap_or_default();
        Some(Stroke { width, color, dash })
    }

    /// Line properties for a shape: its own `a:ln` over the placeholder chain over the theme's
    /// line style named by its `lnRef`.
    fn shape_line(&self, sp_pr: Option<N<'d>>, layers: &[N<'d>], style: Option<N<'d>>) -> Option<Stroke> {
        let mut line = LineSpec::default();
        let mut placeholder = None;
        if let Some((idx, color)) = self.style_ref(style, "lnRef") {
            placeholder = color;
            if idx > 0
                && let Some(theme_line) = self.theme.lines.get(idx as usize - 1)
            {
                line = theme_line.clone();
            }
        }
        if let Some(inherited) = layers.iter().find_map(|l| child(*l, "spPr").and_then(|p| child(p, "ln"))) {
            line = parse_line(inherited).over(&line);
        }
        if let Some(own) = sp_pr.and_then(|p| child(p, "ln")) {
            line = parse_line(own).over(&line);
        }
        self.line_stroke(&line, placeholder)
    }

    fn shape_fill(&self, sp_pr: Option<N<'d>>, layers: &[N<'d>], style: Option<N<'d>>) -> (FillSpec, Option<Rgba>) {
        let own = sp_pr.and_then(parse_fill).or_else(|| layers.iter().find_map(|l| child(*l, "spPr").and_then(parse_fill)));
        if let Some(own) = own {
            return (own, None);
        }
        match self.style_ref(style, "fillRef") {
            Some((idx, color)) => (self.theme_fill(idx).cloned().unwrap_or(FillSpec::None), color),
            None => (FillSpec::None, None),
        }
    }

    fn draw_shape(&mut self, node: N<'d>, layer: Layer, rels: &[Rel], groups: &[Xfrm]) -> Result<(), CreateError> {
        if is_hidden(node) {
            return Ok(());
        }
        let ph = ph_of(node);
        if ph.is_some() && layer != Layer::Slide {
            return Ok(());
        }
        let mut layers: Vec<N<'d>> = Vec::new();
        if let Some(ph) = &ph {
            let from_layout = self.layout_tree.and_then(|t| layout_placeholder(t, ph));
            layers.extend(from_layout);
            // The master's placeholder is the one the *layout's* placeholder names; fall back to the
            // slide's own type when the layout does not have one.
            let via = from_layout.and_then(|l| ph_of(l)).unwrap_or_else(|| ph.clone());
            layers.extend(self.master_tree.and_then(|t| master_placeholder(t, &via)));
        }
        let sp_pr = child(node, "spPr");
        let xfrm = sp_pr
            .and_then(|p| child(p, "xfrm"))
            .and_then(parse_xfrm)
            .or_else(|| layers.iter().find_map(|l| child(*l, "spPr").and_then(|p| child(p, "xfrm")).and_then(parse_xfrm)));
        let Some(xfrm) = xfrm else { return Ok(()) };
        let frame = resolve_frame(xfrm, groups);
        if frame.w <= 0.0 && frame.h <= 0.0 {
            return Ok(());
        }
        let style = child(node, "style");
        let (fill_spec, fill_ph) = self.shape_fill(sp_pr, &layers, style);
        let stroke = self.shape_line(sp_pr, &layers, style);

        let geometry = sp_pr.and_then(geometry_node).or_else(|| layers.iter().find_map(|l| child(*l, "spPr").and_then(geometry_node)));
        let mut approximated = false;
        let shapes = match geometry {
            Some(g) => build_geometry(g, frame.w, frame.h).unwrap_or_else(|| {
                approximated = true;
                vec![ShapePath { path: Path::rect(0.0, 0.0, frame.w, frame.h), fill: true, stroke: true }]
            }),
            None => Vec::new(),
        };

        // Text, resolved before anything is drawn so that an unencodable character stops cleanly.
        let tx_body = child(node, "txBody");
        let mut laid: Option<(Vec<Para>, BodyProps)> = None;
        if let Some(body) = tx_body {
            let font_color = style.and_then(|s| child(s, "fontRef")).and_then(color_in);
            let base = font_color.map(|c| {
                let mut p = ParaProps::default();
                p.run.color = Some(c);
                p
            });
            let levels = self.levels(ph.as_ref(), &layers, child(body, "lstStyle"), base.as_ref());
            let paras = self.paragraphs(body, &levels)?;
            let mut props = BodyProps::default();
            for layer_sp in layers.iter().rev() {
                if let Some(b) = child(*layer_sp, "txBody").and_then(|b| child(b, "bodyPr")) {
                    props.over(&BodyProps::parse(b));
                }
            }
            if let Some(b) = child(body, "bodyPr") {
                props.over(&BodyProps::parse(b));
            }
            if paras.iter().any(|p| !p.runs.is_empty()) {
                laid = Some((paras, props));
            }
        }

        let (cx, cy) = (frame.x + frame.w / 2.0, frame.y + frame.h / 2.0);
        self.canvas.save();
        if frame.rot.rem_euclid(360.0) != 0.0 {
            self.canvas.transform(Matrix::rotate_about(frame.rot, cx, cy));
        }
        self.canvas.transform(Matrix::translate(frame.x, frame.y));
        if !shapes.is_empty() {
            self.canvas.save();
            if frame.flip_h || frame.flip_v {
                self.canvas.transform(Matrix::flip(frame.flip_h, frame.flip_v, frame.w, frame.h));
            }
            let visible = !matches!(fill_spec, FillSpec::None) || stroke.is_some();
            if visible && approximated {
                self.note(NoteKind::ShapeGeometry);
            }
            let fill_paint = self.paint(&fill_spec, fill_ph);
            let picture = match &fill_spec {
                FillSpec::Picture(Some(id)) => rels.iter().find(|r| &r.id == id).and_then(|r| r.target.clone()),
                _ => None,
            };
            for s in &shapes {
                if s.fill {
                    match &fill_paint {
                        Paint::Solid(c) => self.canvas.fill(&s.path, *c),
                        Paint::Gradient(stops, kind) => self.canvas.gradient(&s.path, stops, kind, frame.w, frame.h),
                        Paint::None => {}
                    }
                    if let Some(part) = &picture {
                        let slide = self.slide_no;
                        if let Some(index) = self.shared.image(part, slide) {
                            self.canvas.save();
                            self.canvas.clip(&s.path);
                            self.canvas.image(index, (0.0, 0.0, frame.w, frame.h), [0.0; 4], 1.0);
                            self.canvas.restore();
                        }
                    } else if matches!(fill_spec, FillSpec::Picture(_)) {
                        self.note(NoteKind::Picture);
                    }
                }
                if s.stroke
                    && let Some(stroke) = &stroke
                {
                    self.canvas.stroke(&s.path, stroke);
                }
            }
            self.canvas.restore();
        }
        if let Some((paras, body)) = laid {
            if body.vertical {
                self.note(NoteKind::VerticalText);
            }
            self.draw_body(&paras, &body, frame.w, frame.h);
        }
        self.canvas.restore();
        Ok(())
    }

    fn draw_picture(&mut self, node: N<'d>, layer: Layer, rels: &[Rel], groups: &[Xfrm]) -> Result<(), CreateError> {
        if is_hidden(node) {
            return Ok(());
        }
        let ph = ph_of(node);
        if ph.is_some() && layer != Layer::Slide {
            return Ok(());
        }
        let mut layers: Vec<N<'d>> = Vec::new();
        if let Some(ph) = &ph {
            let from_layout = self.layout_tree.and_then(|t| layout_placeholder(t, ph));
            layers.extend(from_layout);
            layers.extend(self.master_tree.and_then(|t| master_placeholder(t, ph)));
        }
        let sp_pr = child(node, "spPr");
        let xfrm = sp_pr
            .and_then(|p| child(p, "xfrm"))
            .and_then(parse_xfrm)
            .or_else(|| layers.iter().find_map(|l| child(*l, "spPr").and_then(|p| child(p, "xfrm")).and_then(parse_xfrm)));
        let Some(xfrm) = xfrm else { return Ok(()) };
        let frame = resolve_frame(xfrm, groups);
        let blip = child(node, "blipFill").and_then(|b| child(b, "blip"));
        let Some(embed) = blip.and_then(|b| rel_attr(b, "embed")) else { return Ok(()) };
        let slide = self.slide_no;
        let Some(part) = rels.iter().find(|r| r.id == embed).and_then(|r| r.target.clone()) else {
            self.note(NoteKind::Picture);
            return Ok(());
        };
        let Some(index) = self.shared.image(&part, slide) else { return Ok(()) };
        let crop = child(node, "blipFill")
            .and_then(|b| child(b, "srcRect"))
            .map_or([0.0; 4], |r| ["l", "t", "r", "b"].map(|side| number(r, side).map_or(0.0, |v| (v / 100_000.0).clamp(-1.0, 0.95))));
        let alpha = blip.and_then(|b| child(b, "alphaModFix")).and_then(|a| number(a, "amt")).map_or(1.0, |v| (v / 100_000.0).clamp(0.0, 1.0));
        let style = child(node, "style");
        let stroke = self.shape_line(sp_pr, &layers, style).filter(|_| sp_pr.and_then(|p| child(p, "ln")).is_some());
        let geometry = sp_pr.and_then(geometry_node);
        let clip = geometry.and_then(|g| build_geometry(g, frame.w, frame.h)).and_then(|mut s| if s.len() == 1 { s.pop() } else { None });
        if geometry.is_some_and(|g| g.tag_name().name() == "prstGeom" && !is_known_preset(g)) {
            self.note(NoteKind::ShapeGeometry);
        }
        let (cx, cy) = (frame.x + frame.w / 2.0, frame.y + frame.h / 2.0);
        self.canvas.save();
        if frame.rot.rem_euclid(360.0) != 0.0 {
            self.canvas.transform(Matrix::rotate_about(frame.rot, cx, cy));
        }
        self.canvas.transform(Matrix::translate(frame.x, frame.y));
        self.canvas.save();
        if frame.flip_h || frame.flip_v {
            self.canvas.transform(Matrix::flip(frame.flip_h, frame.flip_v, frame.w, frame.h));
        }
        if let Some(c) = &clip {
            self.canvas.clip(&c.path);
        }
        self.canvas.image(index, (0.0, 0.0, frame.w, frame.h), crop, alpha);
        self.canvas.restore();
        if let Some(stroke) = stroke {
            let outline = clip.map_or_else(|| Path::rect(0.0, 0.0, frame.w, frame.h), |c| c.path);
            self.canvas.stroke(&outline, &stroke);
        }
        self.canvas.restore();
        Ok(())
    }

    fn draw_frame(&mut self, node: N<'d>, rels: &[Rel], groups: &[Xfrm]) -> Result<(), CreateError> {
        if is_hidden(node) {
            return Ok(());
        }
        let Some(xfrm) = child(node, "xfrm").and_then(parse_xfrm) else { return Ok(()) };
        let frame = resolve_frame(xfrm, groups);
        let data = node.descendants().find(|n| n.is_element() && n.tag_name().name() == "graphicData");
        let uri = data.and_then(|d| attr(d, "uri")).unwrap_or("");
        match uri.rsplit('/').next().unwrap_or("") {
            "table" => {
                if let Some(tbl) = data.and_then(|d| child(d, "tbl")) {
                    self.draw_table(tbl, frame, rels)?;
                }
            }
            "chart" => self.note(NoteKind::Chart),
            "diagram" => self.note(NoteKind::Diagram),
            "ole" => self.note(NoteKind::EmbeddedObject),
            _ if uri.contains("chart") => self.note(NoteKind::Chart),
            _ => self.note(NoteKind::Other),
        }
        Ok(())
    }

    // ── tables ──────────────────────────────────────────────────────────────────────────────

    fn draw_table(&mut self, tbl: N<'d>, frame: Frame, _rels: &[Rel]) -> Result<(), CreateError> {
        let props = child(tbl, "tblPr");
        let flags = TableFlags {
            first_row: props.and_then(|p| boolean(p, "firstRow")).unwrap_or(false),
            last_row: props.and_then(|p| boolean(p, "lastRow")).unwrap_or(false),
            first_col: props.and_then(|p| boolean(p, "firstCol")).unwrap_or(false),
            last_col: props.and_then(|p| boolean(p, "lastCol")).unwrap_or(false),
            band_row: props.and_then(|p| boolean(p, "bandRow")).unwrap_or(false),
            band_col: props.and_then(|p| boolean(p, "bandCol")).unwrap_or(false),
        };
        let style_id = props.and_then(|p| child(p, "tableStyleId")).and_then(|n| n.text()).map(str::trim).filter(|s| !s.is_empty());
        if let Some(id) = style_id
            && !self.table_styles.knows(id)
        {
            self.note(NoteKind::TableStyle);
        }
        let columns: Vec<f64> =
            child(tbl, "tblGrid").into_iter().flat_map(|g| children(g, "gridCol")).filter_map(|c| number(c, "w")).map(|w| w / EMU).take(64).collect();
        let rows: Vec<N<'d>> = children(tbl, "tr").take(2000).collect();
        if columns.is_empty() || rows.is_empty() {
            return Ok(());
        }
        let (n_rows, n_cols) = (rows.len(), columns.len());

        struct Cell<'a> {
            node: N<'a>,
            row: usize,
            col: usize,
            col_span: usize,
            row_span: usize,
        }
        let mut cells: Vec<Cell<'d>> = Vec::new();
        for (r, row) in rows.iter().enumerate() {
            for (c, tc) in children(*row, "tc").take(n_cols).enumerate() {
                if boolean(tc, "hMerge").unwrap_or(false) || boolean(tc, "vMerge").unwrap_or(false) {
                    continue;
                }
                let span = |name: &str| number(tc, name).map_or(1, |v| v.clamp(1.0, 64.0) as usize);
                cells.push(Cell { node: tc, row: r, col: c, col_span: span("gridSpan").min(n_cols - c), row_span: span("rowSpan").min(n_rows - r) });
            }
        }

        // Text and style of each cell.
        struct Prepared {
            paras: Vec<Para>,
            width: f64,
            margins: [f64; 4],
            anchor: Option<String>,
            fill: Option<Paint>,
            borders: [Option<Stroke>; 4],
        }
        let mut prepared: Vec<Prepared> = Vec::new();
        for cell in &cells {
            let tc_pr = child(cell.node, "tcPr");
            let part: StylePart =
                style_id.and_then(|id| self.table_styles.cell(id, flags, (cell.row, cell.col), (n_rows, n_cols))).unwrap_or_default();
            let mut base = ParaProps::default();
            base.run.bold = part.bold;
            base.run.italic = part.italic;
            base.run.color = part.color.clone();
            let own_list = child(cell.node, "txBody").and_then(|b| child(b, "lstStyle"));
            let levels = self.levels(None, &[], own_list, Some(&base));
            let paras = match child(cell.node, "txBody") {
                Some(body) => self.paragraphs(body, &levels)?,
                None => Vec::new(),
            };
            let width: f64 = columns.iter().skip(cell.col).take(cell.col_span).sum();
            let margin = |name: &str, default: f64| tc_pr.and_then(|p| number(p, name)).map_or(default, |v| (v / EMU).clamp(0.0, 500.0));
            let margins = [margin("marL", 7.2), margin("marT", 3.6), margin("marR", 7.2), margin("marB", 3.6)];
            let own_fill = tc_pr.and_then(parse_fill);
            let fill = own_fill.or(part.fill.clone()).map(|f| self.paint(&f, None));
            let edge = |own: &str, styled: usize| -> Option<Stroke> {
                let line = tc_pr.and_then(|p| child(p, own)).map(parse_line).or_else(|| part.borders.get(styled).cloned().flatten())?;
                self.line_stroke(&line, None)
            };
            let last_col = cell.col + cell.col_span >= n_cols;
            let last_row = cell.row + cell.row_span >= n_rows;
            let borders = [
                edge("lnL", if cell.col == 0 { table::LEFT } else { table::INSIDE_V }),
                edge("lnR", if last_col { table::RIGHT } else { table::INSIDE_V }),
                edge("lnT", if cell.row == 0 { table::TOP } else { table::INSIDE_H }),
                edge("lnB", if last_row { table::BOTTOM } else { table::INSIDE_H }),
            ];
            prepared.push(Prepared { paras, width, margins, anchor: tc_pr.and_then(|p| attr(p, "anchor")).map(str::to_owned), fill, borders });
        }

        // Row heights grow to fit their text, as in PowerPoint.
        let mut heights: Vec<f64> = rows.iter().map(|r| number(*r, "h").map_or(0.0, |h| (h / EMU).max(0.0))).collect();
        let wrap_body = BodyProps::default();
        for (cell, p) in cells.iter().zip(&prepared) {
            if cell.row_span == 1 {
                let inner = (p.width - p.margins[0] - p.margins[2]).max(1.0);
                let laid = self.fit(&p.paras, &wrap_body, inner, f64::INFINITY);
                if let Some(h) = heights.get_mut(cell.row) {
                    *h = h.max(laid.height + p.margins[1] + p.margins[3]);
                }
            }
        }
        let tops: Vec<f64> = heights
            .iter()
            .scan(0.0, |acc, h| {
                let t = *acc;
                *acc += h;
                Some(t)
            })
            .collect();
        let lefts: Vec<f64> = columns
            .iter()
            .scan(0.0, |acc, w| {
                let l = *acc;
                *acc += w;
                Some(l)
            })
            .collect();

        let (cx, cy) = (frame.x + frame.w / 2.0, frame.y + frame.h / 2.0);
        self.canvas.save();
        if frame.rot.rem_euclid(360.0) != 0.0 {
            self.canvas.transform(Matrix::rotate_about(frame.rot, cx, cy));
        }
        self.canvas.transform(Matrix::translate(frame.x, frame.y));
        for (cell, p) in cells.iter().zip(&prepared) {
            let (Some(&x), Some(&y)) = (lefts.get(cell.col), tops.get(cell.row)) else { continue };
            let h: f64 = heights.iter().skip(cell.row).take(cell.row_span).sum();
            let rect = Path::rect(x, y, p.width, h);
            match &p.fill {
                Some(Paint::Solid(c)) => self.canvas.fill(&rect, *c),
                Some(Paint::Gradient(stops, kind)) => {
                    self.canvas.save();
                    self.canvas.transform(Matrix::translate(x, y));
                    self.canvas.gradient(&Path::rect(0.0, 0.0, p.width, h), stops, kind, p.width, h);
                    self.canvas.restore();
                }
                _ => {}
            }
        }
        for (cell, p) in cells.iter().zip(&prepared) {
            let (Some(&x), Some(&y)) = (lefts.get(cell.col), tops.get(cell.row)) else { continue };
            let h: f64 = heights.iter().skip(cell.row).take(cell.row_span).sum();
            if !p.paras.iter().all(|q| q.runs.is_empty()) {
                let inner_w = (p.width - p.margins[0] - p.margins[2]).max(1.0);
                let inner_h = (h - p.margins[1] - p.margins[3]).max(1.0);
                let laid = self.fit(&p.paras, &wrap_body, inner_w, inner_h);
                let top = match p.anchor.as_deref() {
                    Some("ctr") => (inner_h - laid.height) / 2.0,
                    Some("b") => inner_h - laid.height,
                    _ => 0.0,
                };
                paint(&mut self.canvas, &laid, x + p.margins[0], y + p.margins[1] + top);
            }
            let edges =
                [((x, y), (x, y + h)), ((x + p.width, y), (x + p.width, y + h)), ((x, y), (x + p.width, y)), ((x, y + h), (x + p.width, y + h))];
            for (stroke, ((x0, y0), (x1, y1))) in p.borders.iter().zip(edges) {
                if let Some(stroke) = stroke {
                    let mut line = Path::new();
                    line.move_to(x0, y0);
                    line.line_to(x1, y1);
                    self.canvas.stroke(&line, stroke);
                }
            }
        }
        self.canvas.restore();
        Ok(())
    }

    // ── slide ───────────────────────────────────────────────────────────────────────────────

    fn background(&mut self, bg: N<'d>, rels: &[Rel], (w, h): (f64, f64)) {
        let page = Path::rect(0.0, 0.0, w, h);
        if let Some(props) = child(bg, "bgPr") {
            match parse_fill(props) {
                Some(FillSpec::Picture(Some(id))) => {
                    let slide = self.slide_no;
                    if let Some(index) = rels.iter().find(|r| r.id == id).and_then(|r| r.target.clone()).and_then(|p| self.shared.image(&p, slide)) {
                        self.canvas.image(index, (0.0, 0.0, w, h), [0.0; 4], 1.0);
                    }
                }
                Some(FillSpec::Picture(None)) => self.note(NoteKind::Picture),
                Some(spec) => self.fill_page(&page, &spec, None, w, h),
                None => {}
            }
        } else if let Some((idx, color)) = self.style_ref(Some(bg), "bgRef")
            && let Some(spec) = self.theme_fill(idx).cloned()
        {
            self.fill_page(&page, &spec, color, w, h);
        }
    }

    fn fill_page(&mut self, page: &Path, spec: &FillSpec, placeholder: Option<Rgba>, w: f64, h: f64) {
        match self.paint(spec, placeholder) {
            Paint::Solid(c) => self.canvas.fill(page, c),
            Paint::Gradient(stops, kind) => self.canvas.gradient(page, &stops, &kind, w, h),
            Paint::None => {}
        }
    }
}

fn is_hidden(node: Node<'_, '_>) -> bool {
    node.children()
        .filter(|n| n.is_element())
        .find(|n| n.tag_name().name().starts_with("nv"))
        .and_then(|nv| child(nv, "cNvPr"))
        .is_some_and(|c| boolean(c, "hidden").unwrap_or(false))
}

fn geometry_node<'a>(sp_pr: N<'a>) -> Option<N<'a>> {
    child(sp_pr, "prstGeom").or_else(|| child(sp_pr, "custGeom"))
}

// ── geometry ────────────────────────────────────────────────────────────────────────────────

struct ShapePath {
    path: Path,
    fill: bool,
    stroke: bool,
}

fn is_known_preset(node: Node<'_, '_>) -> bool {
    attr(node, "prst").is_some_and(|p| preset(p, &HashMap::new(), 10.0, 10.0).is_some())
}

fn adjustments(node: Node<'_, '_>) -> HashMap<String, f64> {
    child(node, "avLst")
        .into_iter()
        .flat_map(|l| children(l, "gd"))
        .filter_map(|gd| Some((attr(gd, "name")?.to_owned(), attr(gd, "fmla")?.strip_prefix("val ")?.trim().parse::<f64>().ok()?)))
        .collect()
}

/// The outline of a preset shape (§20.1.9 presetShapeDefinitions) and whether it is closed.
fn preset(name: &str, adj: &HashMap<String, f64>, w: f64, h: f64) -> Option<(Path, bool)> {
    let ss = w.min(h);
    let value = |key: &str, default: f64| adj.get(key).copied().unwrap_or(default);
    let mut p = Path::new();
    match name {
        "rect" | "flowChartProcess" => return Some((Path::rect(0.0, 0.0, w, h), true)),
        "roundRect" | "flowChartAlternateProcess" => {
            let a = value("adj", 16_667.0).clamp(0.0, 50_000.0);
            return Some((Path::round_rect(0.0, 0.0, w, h, ss * a / 100_000.0), true));
        }
        "ellipse" | "flowChartConnector" => return Some((Path::ellipse(0.0, 0.0, w, h), true)),
        "triangle" => {
            let apex = w * value("adj", 50_000.0).clamp(0.0, 100_000.0) / 100_000.0;
            p.move_to(0.0, h);
            p.line_to(apex, 0.0);
            p.line_to(w, h);
        }
        "rtTriangle" => {
            p.move_to(0.0, 0.0);
            p.line_to(0.0, h);
            p.line_to(w, h);
        }
        "diamond" | "flowChartDecision" => {
            p.move_to(0.0, h / 2.0);
            p.line_to(w / 2.0, 0.0);
            p.line_to(w, h / 2.0);
            p.line_to(w / 2.0, h);
        }
        "parallelogram" => {
            let max = if ss > 0.0 { 100_000.0 * w / ss } else { 100_000.0 };
            let inset = ss * value("adj", 25_000.0).clamp(0.0, max) / 100_000.0;
            p.move_to(0.0, h);
            p.line_to(inset, 0.0);
            p.line_to(w, 0.0);
            p.line_to(w - inset, h);
        }
        "chevron" => {
            let max = if ss > 0.0 { 100_000.0 * w / ss } else { 100_000.0 };
            let inset = ss * value("adj", 50_000.0).clamp(0.0, max) / 100_000.0;
            p.move_to(0.0, 0.0);
            p.line_to(w - inset, 0.0);
            p.line_to(w, h / 2.0);
            p.line_to(w - inset, h);
            p.line_to(0.0, h);
            p.line_to(inset, h / 2.0);
        }
        "homePlate" => {
            let max = if ss > 0.0 { 100_000.0 * w / ss } else { 100_000.0 };
            let x = w - ss * value("adj", 50_000.0).clamp(0.0, max) / 100_000.0;
            p.move_to(0.0, 0.0);
            p.line_to(x, 0.0);
            p.line_to(w, h / 2.0);
            p.line_to(x, h);
            p.line_to(0.0, h);
        }
        "line" | "straightConnector1" => {
            p.move_to(0.0, 0.0);
            p.line_to(w, h);
            return Some((p, false));
        }
        _ => return None,
    }
    p.close();
    Some((p, true))
}

fn guide_value(node: Node<'_, '_>, name: &str) -> Option<f64> {
    attr(node, name)?.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// `a:custGeom` paths with literal coordinates (§20.1.9.8). A path that refers to shape guides
/// returns `None`.
fn custom_geometry(node: Node<'_, '_>, w: f64, h: f64) -> Option<Vec<ShapePath>> {
    let list = child(node, "pathLst")?;
    let mut out = Vec::new();
    for path in children(list, "path").take(64) {
        let (pw, ph) = (guide_value(path, "w").unwrap_or(0.0), guide_value(path, "h").unwrap_or(0.0));
        // A path without its own size is in EMU, the shape's own unit.
        let (sx, sy) = (if pw > 0.0 { w / pw } else { 1.0 / EMU }, if ph > 0.0 { h / ph } else { 1.0 / EMU });
        let mut p = Path::new();
        let point = |n: Node<'_, '_>| -> Option<(f64, f64)> { Some((guide_value(n, "x")? * sx, guide_value(n, "y")? * sy)) };
        let mut count = 0usize;
        for cmd in path.children().filter(|n| n.is_element()) {
            count += 1;
            if count > 20_000 {
                return None;
            }
            let pts: Vec<(f64, f64)> = children(cmd, "pt").map(point).collect::<Option<_>>()?;
            match (cmd.tag_name().name(), pts.as_slice()) {
                ("moveTo", [a]) => p.move_to(a.0, a.1),
                ("lnTo", [a]) => p.line_to(a.0, a.1),
                ("cubicBezTo", [a, b, c]) => p.curve_to(a.0, a.1, b.0, b.1, c.0, c.1),
                ("quadBezTo", [a, b]) => p.quad_to(a.0, a.1, b.0, b.1),
                ("arcTo", []) => {
                    let (wr, hr) = (guide_value(cmd, "wR")? * sx, guide_value(cmd, "hR")? * sy);
                    p.arc_to(wr, hr, guide_value(cmd, "stAng")? / 60_000.0, guide_value(cmd, "swAng")? / 60_000.0);
                }
                ("close", []) => p.close(),
                _ => return None,
            }
        }
        let filled = attr(path, "fill") != Some("none");
        out.push(ShapePath { path: p, fill: filled, stroke: attr(path, "stroke") != Some("0") && attr(path, "stroke") != Some("false") });
    }
    if out.is_empty() { None } else { Some(out) }
}

fn build_geometry(node: Node<'_, '_>, w: f64, h: f64) -> Option<Vec<ShapePath>> {
    if node.tag_name().name() == "custGeom" {
        return custom_geometry(node, w, h);
    }
    let (path, closed) = preset(attr(node, "prst")?, &adjustments(node), w, h)?;
    Some(vec![ShapePath { path, fill: closed, stroke: true }])
}

// ── conversion ──────────────────────────────────────────────────────────────────────────────

struct Library<'t> {
    docs: HashMap<String, roxmltree::Document<'t>>,
    rels: HashMap<String, Vec<Rel>>,
    themes: HashMap<String, Theme>,
}

/// The colour mapping a layout or slide overrides its master's with, if it does.
fn color_map_override(root: Option<N<'_>>) -> Option<HashMap<String, String>> {
    let ovr = child(child(root?, "clrMapOvr")?, "overrideClrMapping")?;
    Some(parse_color_map(Some(ovr)))
}

fn target_of(rels: &[Rel], kind: &str) -> Option<String> {
    rels.iter().find(|r| r.kind == kind).and_then(|r| r.target.clone())
}

fn root_tree<'a>(doc: &'a roxmltree::Document<'a>) -> Option<N<'a>> {
    child(child(doc.root_element(), "cSld")?, "spTree")
}

/// The six standard fonts slides and sheets draw with, as the page font resources `F1`..`F6`.
pub(crate) fn standard_fonts(doc: &mut Document) -> Dict {
    let mut fonts = Dict::new();
    for (key, face) in [
        ("F1", "Helvetica"),
        ("F2", "Helvetica-Bold"),
        ("F3", "Helvetica-Oblique"),
        ("F4", "Helvetica-BoldOblique"),
        ("F5", "Symbol"),
        ("F6", "ZapfDingbats"),
    ] {
        let mut font = Dict::new();
        font.set(b"Type".to_vec(), Object::name("Font"));
        font.set(b"Subtype".to_vec(), Object::name("Type1"));
        font.set(b"BaseFont".to_vec(), Object::name(face));
        // Symbol and ZapfDingbats use their own built-in encodings.
        if !matches!(key, "F5" | "F6") {
            font.set(b"Encoding".to_vec(), Object::name("WinAnsiEncoding"));
        }
        let reference = doc.add(font);
        fonts.set(key.as_bytes().to_vec(), Object::Ref(reference));
    }
    fonts
}

/// Convert a `.pptx` presentation to a PDF document, one page per slide.
pub fn from_pptx_layout(title: &str, bytes: &[u8]) -> Result<PptxConversion, CreateError> {
    let package = Package::open(bytes, "PPTX", MAX_PPTX_BYTES)?;
    let main = relationships(&package, "")?
        .iter()
        .find(|r| r.kind == "officeDocument")
        .and_then(|r| r.target.clone())
        .unwrap_or_else(|| "ppt/presentation.xml".to_owned());
    let presentation_xml = package.read_xml(&main, MAX_XML)?.ok_or_else(|| bad("PPTX has no presentation part"))?;
    let presentation = parse_xml(&presentation_xml)?;
    let root = presentation.root_element();
    if root.tag_name().name() != "presentation" {
        return Err(bad("the file is not a PowerPoint presentation"));
    }
    let size = child(root, "sldSz").ok_or_else(|| bad("PPTX has no slide size"))?;
    let page_w = number(size, "cx").map(|v| v / EMU).ok_or_else(|| bad("PPTX has an invalid slide size"))?;
    let page_h = number(size, "cy").map(|v| v / EMU).ok_or_else(|| bad("PPTX has an invalid slide size"))?;
    if !(72.0..=14_400.0).contains(&page_w) || !(72.0..=14_400.0).contains(&page_h) {
        return Err(bad("PPTX slide size is outside PDF limits"));
    }
    let presentation_rels = relationships(&package, &main)?;
    let slide_parts: Vec<String> = child(root, "sldIdLst")
        .into_iter()
        .flat_map(|l| children(l, "sldId"))
        .filter_map(|s| rel_attr(s, "id"))
        .filter_map(|id| presentation_rels.iter().find(|r| r.id == id).and_then(|r| r.target.clone()))
        .collect();
    if slide_parts.is_empty() {
        return Err(bad("PPTX has no slides"));
    }
    if slide_parts.len() > MAX_SLIDES {
        return Err(bad(format!("PPTX has more than {MAX_SLIDES} slides")));
    }

    // Everything the slides draw on: layouts, masters and themes, read once.
    let mut slide_rels: Vec<Vec<Rel>> = Vec::new();
    let mut needed: BTreeSet<String> = BTreeSet::new();
    let mut rels_by_part: HashMap<String, Vec<Rel>> = HashMap::new();
    for part in &slide_parts {
        let rels = relationships(&package, part)?;
        if let Some(layout) = target_of(&rels, "slideLayout") {
            needed.insert(layout);
        }
        slide_rels.push(rels);
    }
    let layouts: Vec<String> = needed.iter().cloned().collect();
    for layout in &layouts {
        let rels = relationships(&package, layout)?;
        if let Some(master) = target_of(&rels, "slideMaster") {
            needed.insert(master);
        }
        rels_by_part.insert(layout.clone(), rels);
    }
    let masters: Vec<String> = needed.iter().filter(|p| !layouts.contains(p)).cloned().collect();
    let mut theme_parts: BTreeSet<String> = BTreeSet::new();
    for master in &masters {
        let rels = relationships(&package, master)?;
        if let Some(theme) = target_of(&rels, "theme") {
            theme_parts.insert(theme);
        }
        rels_by_part.insert(master.clone(), rels);
    }
    let mut texts: HashMap<String, String> = HashMap::new();
    for part in needed.iter().chain(theme_parts.iter()) {
        if let Some(xml) = package.read_xml(part, MAX_XML)? {
            texts.insert(part.clone(), xml);
        }
    }
    let table_styles_text = match target_of(&presentation_rels, "tableStyles") {
        Some(part) => package.read_xml(&part, MAX_XML)?,
        None => None,
    };
    let mut library = Library { docs: HashMap::new(), rels: rels_by_part, themes: HashMap::new() };
    for (name, text) in &texts {
        let doc = parse_xml(text)?;
        if theme_parts.contains(name) {
            library.themes.insert(name.clone(), Theme::parse(&doc));
        } else {
            library.docs.insert(name.clone(), doc);
        }
    }
    let table_styles = match table_styles_text.as_deref() {
        Some(text) => TableStyles::parse(&parse_xml(text)?),
        None => TableStyles::default(),
    };

    let mut shared =
        Shared { package: &package, out: Document::new_empty(), images: Vec::new(), image_cache: HashMap::new(), notes: BTreeMap::new() };
    let fonts = standard_fonts(&mut shared.out);
    let empty_theme = Theme::default();
    let mut pages = 0usize;

    for (index, (slide_part, rels)) in slide_parts.iter().zip(&slide_rels).enumerate() {
        let slide_no = index + 1;
        let xml = package.read_xml(slide_part, MAX_XML)?.ok_or_else(|| bad(format!("slide {slide_no} is missing from the package")))?;
        let slide_doc = parse_xml(&xml)?;
        let slide_root = slide_doc.root_element();
        if attr(slide_root, "show").is_some_and(|v| v == "0" || v == "false") {
            shared.note(NoteKind::HiddenSlide, slide_no);
            continue;
        }
        let layout_part = target_of(rels, "slideLayout");
        let layout_doc = layout_part.as_ref().and_then(|p| library.docs.get(p));
        let layout_rels: &[Rel] = layout_part.as_ref().and_then(|p| library.rels.get(p)).map_or(&[], Vec::as_slice);
        let master_part = target_of(layout_rels, "slideMaster");
        let master_doc = master_part.as_ref().and_then(|p| library.docs.get(p));
        let master_rels: &[Rel] = master_part.as_ref().and_then(|p| library.rels.get(p)).map_or(&[], Vec::as_slice);
        let theme = target_of(master_rels, "theme").and_then(|p| library.themes.get(&p)).unwrap_or(&empty_theme);
        let layout_root = layout_doc.map(|d| d.root_element());
        let master_root = master_doc.map(|d| d.root_element());
        // Colour names (`bg1`, `tx1`, ...) map to theme colours through the master's `clrMap`, which a
        // layout or a slide can override (`clrMapOvr`, §19.3.1.7): dark layouts do. The mapping a slide
        // ends up with applies to everything drawn on it, inherited backgrounds and shapes included.
        let master_map = parse_color_map(master_root.and_then(|m| child(m, "clrMap")));
        let layout_map = color_map_override(layout_root).unwrap_or(master_map);
        let slide_map = color_map_override(Some(slide_root)).unwrap_or(layout_map);
        let slide_palette = Palette { theme, map: &slide_map };

        let mut drawer = Drawer {
            shared: &mut shared,
            canvas: Canvas::new(page_h),
            slide_no,
            palette: &slide_palette,
            theme,
            default_text: child(root, "defaultTextStyle"),
            tx_styles: master_root.and_then(|m| child(m, "txStyles")),
            layout_tree: layout_doc.and_then(root_tree),
            master_tree: master_doc.and_then(root_tree),
            table_styles: &table_styles,
            shapes_left: MAX_SHAPES,
        };

        // Background: the slide's own, else its layout's, else its master's.
        let own_bg = child(slide_root, "cSld").and_then(|c| child(c, "bg"));
        let layout_bg = layout_root.and_then(|l| child(l, "cSld")).and_then(|c| child(c, "bg"));
        let master_bg = master_root.and_then(|m| child(m, "cSld")).and_then(|c| child(c, "bg"));
        if let Some((bg, bg_rels)) =
            own_bg.map(|b| (b, rels.as_slice())).or(layout_bg.map(|b| (b, layout_rels))).or(master_bg.map(|b| (b, master_rels)))
        {
            drawer.background(bg, bg_rels, (page_w, page_h));
        }
        // Shapes on the master and layout sit under the slide's, unless it hides them
        // (`showMasterSp`, §19.3.1.38).
        let show = |node: Option<N<'_>>| node.and_then(|n| boolean(n, "showMasterSp")).unwrap_or(true);
        let slide_shows = show(Some(slide_root));
        let layout_shows = show(layout_root);
        let mut groups = Vec::new();
        if slide_shows
            && layout_shows
            && let Some(tree) = drawer.master_tree
        {
            drawer.draw_tree(tree, Layer::Master, master_rels, &mut groups, 0)?;
        }
        if slide_shows && let Some(tree) = drawer.layout_tree {
            drawer.draw_tree(tree, Layer::Layout, layout_rels, &mut groups, 0)?;
        }
        if let Some(tree) = root_tree(&slide_doc) {
            drawer.draw_tree(tree, Layer::Slide, rels, &mut groups, 0)?;
        }

        let (content, used) = drawer.canvas.finish();
        let mut resources = Dict::new();
        resources.set(b"Font".to_vec(), Object::Dict(fonts.clone()));
        if !used.images.is_empty() {
            let mut xobjects = Dict::new();
            for i in &used.images {
                if let Some(r) = shared.images.get(*i) {
                    xobjects.set(format!("Im{i}").into_bytes(), Object::Ref(*r));
                }
            }
            resources.set(b"XObject".to_vec(), Object::Dict(xobjects));
        }
        if used.has_alpha() {
            resources.set(b"ExtGState".to_vec(), Object::Dict(used.extgstate()));
        }
        if used.has_shading() {
            resources.set(b"Shading".to_vec(), Object::Dict(used.shading()));
        }
        add_page(&mut shared.out, page_w, page_h, resources, Some(content))?;
        pages += 1;
    }
    if pages == 0 {
        return Err(bad("PPTX has no visible slides"));
    }
    set_title(&mut shared.out, title);
    let notes = shared.notes.iter().map(|(kind, slides)| ConversionNote { kind: *kind, slides: slides.iter().copied().collect() }).collect();
    Ok(PptxConversion { document: shared.out, notes })
}
