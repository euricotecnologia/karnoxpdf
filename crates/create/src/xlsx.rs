//! Spreadsheet (SpreadsheetML, ISO/IEC 29500-1 §18) workbooks as a model: sheets with their
//! cells, merged ranges, column widths and row heights, page setup, and the styles that cells
//! name. Reading follows the package relationships; nothing is trusted and everything is capped.

use std::collections::{BTreeMap, HashMap};

use roxmltree::Node;

use crate::CreateError;
use crate::drawingml::{Palette, Rgb, Theme, parse_color_map, scheme_color};
use crate::ooxml::{Package, Rel, attr, bad, boolean, child, children, number, parse_xml, rel_attr, relationships};

const MAX_XML: usize = 96 * 1024 * 1024;
const MAX_SHEETS: usize = 256;
const MAX_CELLS: usize = 2_000_000;
pub(crate) const MAX_ROW: u32 = 1_048_576;
pub(crate) const MAX_COL: u32 = 16_384;

// ── styles ──────────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default)]
pub(crate) struct Font {
    pub size: f64,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub color: Option<Rgb>,
    pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LineStyle {
    Hair,
    Thin,
    Medium,
    Thick,
    Dashed,
    Dotted,
    Double,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Edge {
    pub style: LineStyle,
    pub color: Rgb,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Border {
    pub left: Option<Edge>,
    pub right: Option<Edge>,
    pub top: Option<Edge>,
    pub bottom: Option<Edge>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum HAlign {
    #[default]
    General,
    Left,
    Center,
    Right,
    Fill,
    Justify,
    CenterContinuous,
    Distributed,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum VAlign {
    Top,
    Center,
    #[default]
    Bottom,
    Justify,
    Distributed,
}

/// One cell format (`cellXfs/xf`).
#[derive(Clone, Debug, Default)]
pub(crate) struct Xf {
    pub num_fmt: u32,
    pub font: usize,
    pub fill: Option<Rgb>,
    pub border: Border,
    pub halign: HAlign,
    pub valign: VAlign,
    pub wrap: bool,
    pub shrink: bool,
    pub indent: f64,
}

#[derive(Debug, Default)]
pub(crate) struct Styles {
    pub num_fmts: HashMap<u32, String>,
    pub fonts: Vec<Font>,
    pub xfs: Vec<Xf>,
}

/// The legacy indexed palette (§18.8.27), used by files that name colours by index.
const INDEXED: [u32; 66] = [
    0x000000, 0xFFFFFF, 0xFF0000, 0x00FF00, 0x0000FF, 0xFFFF00, 0xFF00FF, 0x00FFFF, 0x000000, 0xFFFFFF, 0xFF0000, 0x00FF00, 0x0000FF, 0xFFFF00,
    0xFF00FF, 0x00FFFF, 0x800000, 0x008000, 0x000080, 0x808000, 0x800080, 0x008080, 0xC0C0C0, 0x808080, 0x9999FF, 0x993366, 0xFFFFCC, 0xCCFFFF,
    0x660066, 0xFF8080, 0x0066CC, 0xCCCCFF, 0x000080, 0xFF00FF, 0xFFFF00, 0x00FFFF, 0x800080, 0x800000, 0x008080, 0x0000FF, 0x00CCFF, 0xCCFFFF,
    0xCCFFCC, 0xFFFF99, 0x99CCFF, 0xFF99CC, 0xCC99FF, 0xFFCC99, 0x3366FF, 0x33CCCC, 0x99CC00, 0xFFCC00, 0xFF9900, 0xFF6600, 0x666699, 0x969696,
    0x003366, 0x339966, 0x003300, 0x333300, 0x993300, 0x993366, 0x333399, 0x333333, 0x000000, 0xFFFFFF,
];

fn rgb_of(v: u32) -> Rgb {
    [((v >> 16) & 255) as f64 / 255.0, ((v >> 8) & 255) as f64 / 255.0, (v & 255) as f64 / 255.0]
}

/// Resolves the colour elements of a workbook (`rgb`, `theme` + `tint`, `indexed`).
pub(crate) struct Colors<'a> {
    theme: &'a Theme,
    indexed: Vec<Rgb>,
}

impl Colors<'_> {
    fn indexed(&self, i: usize) -> Option<Rgb> {
        self.indexed.get(i).copied()
    }

    /// A colour element's value; `None` for automatic colours.
    pub(crate) fn resolve(&self, node: Node<'_, '_>) -> Option<Rgb> {
        let tint = number(node, "tint").unwrap_or(0.0).clamp(-1.0, 1.0);
        let base = if let Some(rgb) = attr(node, "rgb") {
            // ARGB; the alpha byte is ignored.
            let hex = rgb.get(rgb.len().saturating_sub(6)..)?;
            u32::from_str_radix(hex, 16).ok().map(rgb_of)?
        } else if let Some(index) = number(node, "theme") {
            // §18.8.3: the theme indices put the light colours first.
            let name = ["lt1", "dk1", "lt2", "dk2", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6", "hlink", "folHlink"]
                .get(index as usize)
                .copied()?;
            let map = parse_color_map(None);
            Palette { theme: self.theme, map: &map }.resolve(&scheme_color(name), None).rgb
        } else {
            self.indexed(number(node, "indexed")? as usize)?
        };
        Some(if tint == 0.0 { base } else { apply_tint(base, tint) })
    }
}

/// §18.3.1.15: a tint lightens (positive) or darkens (negative) the luminance.
fn apply_tint(rgb: Rgb, tint: f64) -> Rgb {
    let (r, g, b) = (rgb[0], rgb[1], rgb[2]);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) / 2.0;
    let d = max - min;
    let (h, s) = if d < 1e-12 {
        (0.0, 0.0)
    } else {
        let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
        let h = if (max - r).abs() < 1e-12 {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if (max - g).abs() < 1e-12 {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        (h / 6.0, s)
    };
    let l = if tint < 0.0 { l * (1.0 + tint) } else { l * (1.0 - tint) + tint };
    let l = l.clamp(0.0, 1.0);
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

fn edge(node: Option<Node<'_, '_>>, colors: &Colors<'_>) -> Option<Edge> {
    let node = node?;
    let style = match attr(node, "style")? {
        "hair" => LineStyle::Hair,
        "thin" => LineStyle::Thin,
        "medium" | "mediumDashed" | "mediumDashDot" | "mediumDashDotDot" | "slantDashDot" => LineStyle::Medium,
        "thick" => LineStyle::Thick,
        "dashed" | "dashDot" | "dashDotDot" => LineStyle::Dashed,
        "dotted" => LineStyle::Dotted,
        "double" => LineStyle::Double,
        _ => return None,
    };
    Some(Edge { style, color: child(node, "color").and_then(|c| colors.resolve(c)).unwrap_or([0.0; 3]) })
}

/// A fill's colour. Solid fills are their foreground colour; the patterns are drawn as a blend of
/// foreground and background by how much of the cell the pattern covers.
fn fill_color(fill: Node<'_, '_>, colors: &Colors<'_>) -> Option<Rgb> {
    let pattern = child(fill, "patternFill")?;
    let kind = attr(pattern, "patternType").unwrap_or("none");
    let fg = child(pattern, "fgColor").and_then(|c| colors.resolve(c));
    let bg = child(pattern, "bgColor").and_then(|c| colors.resolve(c)).unwrap_or([1.0; 3]);
    let density = match kind {
        "none" | "gray125" => return None,
        "solid" => return fg.or(Some([0.0; 3])),
        "darkGray" => 0.75,
        "mediumGray" => 0.5,
        "lightGray" => 0.25,
        "gray0625" => 0.0625,
        _ => 0.5,
    };
    let fg = fg.unwrap_or([0.0; 3]);
    Some([0, 1, 2].map(|i| bg[i] + (fg[i] - bg[i]) * density))
}

fn parse_styles(doc: &roxmltree::Document<'_>, colors: &Colors<'_>) -> Styles {
    let root = doc.root_element();
    let mut styles = Styles::default();
    if let Some(list) = child(root, "numFmts") {
        for f in children(list, "numFmt").take(1000) {
            if let (Some(id), Some(code)) = (number(f, "numFmtId"), attr(f, "formatCode")) {
                styles.num_fmts.insert(id as u32, code.to_owned());
            }
        }
    }
    if let Some(list) = child(root, "fonts") {
        for f in children(list, "font").take(4000) {
            let val = |name: &str| child(f, name).and_then(|n| number(n, "val"));
            let on = |name: &str| child(f, name).is_some_and(|n| boolean(n, "val").unwrap_or(true));
            styles.fonts.push(Font {
                size: val("sz").map_or(11.0, |s| s.clamp(1.0, 409.0)),
                bold: on("b"),
                italic: on("i"),
                underline: child(f, "u").is_some_and(|n| attr(n, "val") != Some("none")),
                strike: on("strike"),
                color: child(f, "color").and_then(|c| colors.resolve(c)),
                name: child(f, "name").and_then(|n| attr(n, "val")).unwrap_or("").to_owned(),
            });
        }
    }
    let fills: Vec<Option<Rgb>> =
        child(root, "fills").into_iter().flat_map(|l| children(l, "fill")).take(4000).map(|f| fill_color(f, colors)).collect();
    let borders: Vec<Border> = child(root, "borders")
        .into_iter()
        .flat_map(|l| children(l, "border"))
        .take(4000)
        .map(|b| Border {
            left: edge(child(b, "left").or_else(|| child(b, "start")), colors),
            right: edge(child(b, "right").or_else(|| child(b, "end")), colors),
            top: edge(child(b, "top"), colors),
            bottom: edge(child(b, "bottom"), colors),
        })
        .collect();
    if let Some(list) = child(root, "cellXfs") {
        for xf in children(list, "xf").take(65_536) {
            let index = |name: &str| number(xf, name).map_or(0, |v| v.max(0.0) as usize);
            let align = child(xf, "alignment");
            let a = |name: &str| align.and_then(|n| attr(n, name));
            styles.xfs.push(Xf {
                num_fmt: number(xf, "numFmtId").map_or(0, |v| v as u32),
                font: index("fontId"),
                fill: fills.get(index("fillId")).copied().flatten(),
                border: borders.get(index("borderId")).copied().unwrap_or_default(),
                halign: match a("horizontal") {
                    Some("left") => HAlign::Left,
                    Some("center") => HAlign::Center,
                    Some("right") => HAlign::Right,
                    Some("fill") => HAlign::Fill,
                    Some("justify") => HAlign::Justify,
                    Some("centerContinuous") => HAlign::CenterContinuous,
                    Some("distributed") => HAlign::Distributed,
                    _ => HAlign::General,
                },
                valign: match a("vertical") {
                    Some("top") => VAlign::Top,
                    Some("center") => VAlign::Center,
                    Some("justify") => VAlign::Justify,
                    Some("distributed") => VAlign::Distributed,
                    _ => VAlign::Bottom,
                },
                wrap: matches!(a("wrapText"), Some("1" | "true")),
                shrink: matches!(a("shrinkToFit"), Some("1" | "true")),
                indent: align.and_then(|n| number(n, "indent")).unwrap_or(0.0).clamp(0.0, 250.0),
            });
        }
    }
    if styles.fonts.is_empty() {
        styles.fonts.push(Font { size: 11.0, ..Font::default() });
    }
    if styles.xfs.is_empty() {
        styles.xfs.push(Xf::default());
    }
    styles
}

// ── sheets ──────────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Value {
    Number(f64),
    Text(String),
    Bool(bool),
    Error(String),
}

#[derive(Clone, Debug)]
pub(crate) struct Cell {
    pub value: Option<Value>,
    pub style: usize,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Row {
    /// In points; `None` when the row has the default height.
    pub height: Option<f64>,
    pub hidden: bool,
    pub cells: BTreeMap<u32, Cell>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ColumnDef {
    pub min: u32,
    pub max: u32,
    /// Width in points.
    pub width: f64,
    pub hidden: bool,
}

/// A rectangle of cells, 1-based and inclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Range {
    pub col0: u32,
    pub row0: u32,
    pub col1: u32,
    pub row1: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct PageSetup {
    pub paper: u32,
    pub landscape: bool,
    /// Percent; `None` when the sheet fits itself to pages instead.
    pub scale: f64,
    pub fit: Option<(u32, u32)>,
    /// Page margins in points: left, right, top, bottom, header, footer.
    pub margins: [f64; 6],
    pub gridlines: bool,
    pub headings: bool,
    pub center_horizontally: bool,
    pub center_vertically: bool,
    pub over_then_down: bool,
    pub first_page: Option<u32>,
    /// Headers and footers: [odd, even, first] header, then footer, as written.
    pub header: [String; 3],
    pub footer: [String; 3],
    pub different_odd_even: bool,
    pub different_first: bool,
}

impl Default for PageSetup {
    fn default() -> Self {
        PageSetup {
            paper: 1,
            landscape: false,
            scale: 100.0,
            fit: None,
            margins: [0.7 * 72.0, 0.7 * 72.0, 0.75 * 72.0, 0.75 * 72.0, 0.3 * 72.0, 0.3 * 72.0],
            gridlines: false,
            headings: false,
            center_horizontally: false,
            center_vertically: false,
            over_then_down: false,
            first_page: None,
            header: Default::default(),
            footer: Default::default(),
            different_odd_even: false,
            different_first: false,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Sheet {
    pub name: String,
    /// Position in the workbook, 1-based, hidden sheets counted.
    pub position: usize,
    pub rows: BTreeMap<u32, Row>,
    pub columns: Vec<ColumnDef>,
    pub merges: Vec<Range>,
    pub default_col_width: f64,
    pub default_row_height: f64,
    pub page: PageSetup,
    pub print_areas: Vec<Range>,
    pub title_rows: Option<(u32, u32)>,
    pub title_cols: Option<(u32, u32)>,
    pub row_breaks: Vec<u32>,
    pub col_breaks: Vec<u32>,
    pub has_drawing: bool,
    pub has_conditional_formatting: bool,
    pub has_table: bool,
}

pub(crate) struct Workbook {
    pub sheets: Vec<Sheet>,
    pub styles: Styles,
    pub date1904: bool,
    /// Sheets the workbook has that are not printed: (1-based position, why).
    pub skipped: Vec<(usize, Skipped)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Skipped {
    Hidden,
    ChartSheet,
}

/// A cell reference such as `B12` or `$B$12` → (column, row), 1-based.
pub(crate) fn parse_cell_ref(text: &str) -> Option<(u32, u32)> {
    let text = text.trim().replace('$', "");
    let letters = text.chars().take_while(char::is_ascii_alphabetic).count();
    if letters == 0 || letters > 3 || letters == text.len() {
        return None;
    }
    let col = text[..letters].bytes().fold(0u32, |c, b| c * 26 + (b.to_ascii_uppercase() - b'A') as u32 + 1);
    let row: u32 = text[letters..].parse().ok()?;
    ((1..=MAX_COL).contains(&col) && (1..=MAX_ROW).contains(&row)).then_some((col, row))
}

pub(crate) fn parse_range(text: &str) -> Option<Range> {
    let (a, b) = text.split_once(':').unwrap_or((text, text));
    let (c0, r0) = parse_cell_ref(a)?;
    let (c1, r1) = parse_cell_ref(b)?;
    Some(Range { col0: c0.min(c1), row0: r0.min(r1), col1: c0.max(c1), row1: r0.max(r1) })
}

/// The part of a defined name's reference that names cells on `sheet`: `Sheet1!$A$1:$C$9,...`.
fn sheet_refs<'a>(reference: &'a str, sheet: &str) -> Vec<&'a str> {
    let quoted = sheet.replace('\'', "''");
    reference
        .split(',')
        .filter_map(|part| {
            let (name, cells) = part.rsplit_once('!')?;
            let name = name.trim().trim_matches('\'').replace("''", "'");
            (name == sheet || name == quoted).then_some(cells)
        })
        .collect()
}

fn parse_title(cells: &str, rows: bool) -> Option<(u32, u32)> {
    let (a, b) = cells.split_once(':').unwrap_or((cells, cells));
    let a = a.replace('$', "");
    let b = b.replace('$', "");
    if rows {
        let (x, y): (u32, u32) = (a.parse().ok()?, b.parse().ok()?);
        Some((x.min(y), x.max(y)))
    } else {
        let col = |s: &str| s.bytes().try_fold(0u32, |c, b| b.is_ascii_alphabetic().then(|| c * 26 + (b.to_ascii_uppercase() - b'A') as u32 + 1));
        let (x, y) = (col(&a)?, col(&b)?);
        Some((x.min(y), x.max(y)))
    }
}

fn parse_sheet(doc: &roxmltree::Document<'_>, name: String, shared: &[String], date1904: bool, budget: &mut usize) -> Result<Sheet, CreateError> {
    let root = doc.root_element();
    let mut sheet = Sheet { name, default_col_width: 48.0, default_row_height: 15.0, ..Sheet::default() };
    let fmt = child(root, "sheetFormatPr");
    if let Some(f) = fmt {
        if let Some(h) = number(f, "defaultRowHeight") {
            sheet.default_row_height = h.clamp(1.0, 409.0);
        }
        // §18.3.1.81: the default width in characters, or the base width plus padding, rounded up
        // to a multiple of 8 pixels. A character is 7 pixels, a pixel 0.75 points.
        if let Some(w) = number(f, "defaultColWidth") {
            sheet.default_col_width = (w * 7.0).round() * 0.75;
        } else if let Some(base) = number(f, "baseColWidth") {
            sheet.default_col_width = (((base * 7.0 + 5.0) / 8.0).ceil() * 8.0) * 0.75;
        }
    }
    if let Some(cols) = child(root, "cols") {
        for c in children(cols, "col").take(16_384) {
            let (Some(min), Some(max)) = (number(c, "min"), number(c, "max")) else { continue };
            let width = number(c, "width").map_or(sheet.default_col_width, |w| (w * 7.0).round() * 0.75);
            sheet.columns.push(ColumnDef {
                min: min.clamp(1.0, MAX_COL as f64) as u32,
                max: max.clamp(1.0, MAX_COL as f64) as u32,
                width: width.clamp(0.0, 1500.0),
                hidden: boolean(c, "hidden").unwrap_or(false),
            });
        }
    }
    if let Some(data) = child(root, "sheetData") {
        let mut next_row = 1u32;
        for r in children(data, "row") {
            let index = number(r, "r").map_or(next_row, |v| v.clamp(1.0, MAX_ROW as f64) as u32);
            next_row = index.saturating_add(1);
            let mut row =
                Row { height: number(r, "ht").map(|h| h.clamp(0.0, 409.5)), hidden: boolean(r, "hidden").unwrap_or(false), cells: BTreeMap::new() };
            let mut next_col = 1u32;
            for c in children(r, "c") {
                let (col, _) = match attr(c, "r").and_then(parse_cell_ref) {
                    Some(p) => p,
                    None => (next_col, index),
                };
                next_col = col.saturating_add(1);
                *budget = budget.checked_sub(1).ok_or_else(|| bad("the workbook has too many cells"))?;
                let style = number(c, "s").map_or(0, |v| v.max(0.0) as usize);
                let text_of = |name: &str| child(c, name).and_then(|n| n.text()).map(str::to_owned);
                let value = match attr(c, "t").unwrap_or("n") {
                    "s" => text_of("v").and_then(|v| v.trim().parse::<usize>().ok()).and_then(|i| shared.get(i)).map(|s| Value::Text(s.clone())),
                    "str" => text_of("v").map(Value::Text),
                    "inlineStr" => child(c, "is").map(|is| Value::Text(rich_text(is))),
                    "b" => text_of("v").map(|v| Value::Bool(v.trim() == "1" || v.trim().eq_ignore_ascii_case("true"))),
                    "e" => text_of("v").map(Value::Error),
                    "d" => text_of("v").and_then(|v| iso_to_serial(v.trim(), date1904)).map(Value::Number),
                    _ => text_of("v").and_then(|v| v.trim().parse::<f64>().ok()).filter(|v| v.is_finite()).map(Value::Number),
                };
                row.cells.insert(col, Cell { value, style });
            }
            sheet.rows.insert(index, row);
        }
    }
    if let Some(merges) = child(root, "mergeCells") {
        sheet.merges = children(merges, "mergeCell").take(100_000).filter_map(|m| attr(m, "ref").and_then(parse_range)).collect();
    }
    sheet.has_drawing = child(root, "drawing").is_some() || child(root, "legacyDrawingHF").is_some();
    sheet.has_conditional_formatting = child(root, "conditionalFormatting").is_some();
    sheet.has_table = child(root, "tableParts").is_some();
    let page = &mut sheet.page;
    if let Some(setup) = child(root, "pageSetup") {
        page.paper = number(setup, "paperSize").map_or(1, |v| v as u32);
        page.landscape = attr(setup, "orientation") == Some("landscape");
        page.scale = number(setup, "scale").map_or(100.0, |s| s.clamp(10.0, 400.0));
        page.over_then_down = attr(setup, "pageOrder") == Some("overThenDown");
        if boolean(setup, "useFirstPageNumber").unwrap_or(false) {
            page.first_page = number(setup, "firstPageNumber").map(|n| n.max(0.0) as u32);
        }
        if child(root, "sheetPr").and_then(|s| child(s, "pageSetUpPr")).is_some_and(|p| boolean(p, "fitToPage").unwrap_or(false)) {
            let w = number(setup, "fitToWidth").map_or(1, |v| v.max(0.0) as u32);
            let h = number(setup, "fitToHeight").map_or(1, |v| v.max(0.0) as u32);
            page.fit = Some((w, h));
        }
    } else if child(root, "sheetPr").and_then(|s| child(s, "pageSetUpPr")).is_some_and(|p| boolean(p, "fitToPage").unwrap_or(false)) {
        page.fit = Some((1, 1));
    }
    if let Some(m) = child(root, "pageMargins") {
        for (i, name) in ["left", "right", "top", "bottom", "header", "footer"].iter().enumerate() {
            if let Some(v) = number(m, name) {
                page.margins[i] = (v * 72.0).clamp(0.0, 400.0);
            }
        }
    }
    if let Some(o) = child(root, "printOptions") {
        page.gridlines = boolean(o, "gridLines").unwrap_or(false);
        page.headings = boolean(o, "headings").unwrap_or(false);
        page.center_horizontally = boolean(o, "horizontalCentered").unwrap_or(false);
        page.center_vertically = boolean(o, "verticalCentered").unwrap_or(false);
    }
    if let Some(hf) = child(root, "headerFooter") {
        page.different_odd_even = boolean(hf, "differentOddEven").unwrap_or(false);
        page.different_first = boolean(hf, "differentFirst").unwrap_or(false);
        let text = |name: &str| child(hf, name).and_then(|n| n.text()).unwrap_or("").to_owned();
        page.header = [text("oddHeader"), text("evenHeader"), text("firstHeader")];
        page.footer = [text("oddFooter"), text("evenFooter"), text("firstFooter")];
    }
    let breaks = |name: &str| -> Vec<u32> {
        child(root, name)
            .into_iter()
            .flat_map(|b| children(b, "brk"))
            .take(10_000)
            .filter_map(|b| number(b, "id"))
            .map(|v| v.max(0.0) as u32)
            .collect()
    };
    sheet.row_breaks = breaks("rowBreaks");
    sheet.col_breaks = breaks("colBreaks");
    Ok(sheet)
}

/// A shared string item's text: its plain text, or its rich-text runs joined (phonetic hints left out).
fn rich_text(si: Node<'_, '_>) -> String {
    if let Some(t) = child(si, "t") {
        return t.text().unwrap_or("").to_owned();
    }
    children(si, "r").filter_map(|r| child(r, "t")).filter_map(|t| t.text()).collect()
}

/// `2023-03-15` or `2023-03-15T12:30:00` → a serial number.
fn iso_to_serial(text: &str, date1904: bool) -> Option<f64> {
    let (date, time) = text.split_once('T').unwrap_or((text, ""));
    let mut parts = date.split('-');
    let (y, m, d): (i64, u32, u32) = (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?, parts.next()?.parse().ok()?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let days = crate::xlsx_format::days_from_civil(y, m, d);
    let base = if date1904 { crate::xlsx_format::days_from_civil(1904, 1, 1) } else { crate::xlsx_format::days_from_civil(1899, 12, 30) };
    let mut serial = (days - base) as f64;
    if !date1904 && serial < 61.0 {
        serial -= 1.0; // before 1900-03-01 the invented leap day shifts the numbering
    }
    let mut seconds = 0.0;
    for (i, part) in time.trim_end_matches('Z').split(':').filter(|p| !p.is_empty()).take(3).enumerate() {
        seconds += part.parse::<f64>().ok()? * [3600.0, 60.0, 1.0][i];
    }
    Some(serial + seconds / 86_400.0)
}

/// Read a whole workbook.
pub(crate) fn read(package: &Package<'_>) -> Result<Workbook, CreateError> {
    let main = relationships(package, "")?
        .iter()
        .find(|r| r.kind == "officeDocument")
        .and_then(|r| r.target.clone())
        .unwrap_or_else(|| "xl/workbook.xml".to_owned());
    let workbook_xml = package.read_xml(&main, MAX_XML)?.ok_or_else(|| bad("XLSX has no workbook part"))?;
    let workbook_doc = parse_xml(&workbook_xml)?;
    let root = workbook_doc.root_element();
    if root.tag_name().name() != "workbook" {
        return Err(bad("the file is not an Excel workbook"));
    }
    let rels: Vec<Rel> = relationships(package, &main)?;
    let date1904 = child(root, "workbookPr").is_some_and(|p| boolean(p, "date1904").unwrap_or(false));

    // Theme colours, then styles.
    let theme_xml = match rels.iter().find(|r| r.kind == "theme").and_then(|r| r.target.clone()) {
        Some(part) => package.read_xml(&part, MAX_XML)?,
        None => None,
    };
    let theme = match &theme_xml {
        Some(xml) => Theme::parse(&parse_xml(xml)?),
        None => Theme::default(),
    };
    let styles_xml = match rels.iter().find(|r| r.kind == "styles").and_then(|r| r.target.clone()) {
        Some(part) => package.read_xml(&part, MAX_XML)?,
        None => None,
    };
    let styles_doc = styles_xml.as_deref().map(parse_xml).transpose()?;
    let mut indexed: Vec<Rgb> = INDEXED.iter().map(|v| rgb_of(*v)).collect();
    if let Some(doc) = &styles_doc
        && let Some(list) = child(doc.root_element(), "colors").and_then(|c| child(c, "indexedColors"))
    {
        for (i, c) in children(list, "rgbColor").take(64).enumerate() {
            if let Some(rgb) = attr(c, "rgb").and_then(|v| v.get(v.len().saturating_sub(6)..)).and_then(|h| u32::from_str_radix(h, 16).ok())
                && let Some(slot) = indexed.get_mut(i)
            {
                *slot = rgb_of(rgb);
            }
        }
    }
    let colors = Colors { theme: &theme, indexed };
    let styles = match &styles_doc {
        Some(doc) => parse_styles(doc, &colors),
        None => Styles { fonts: vec![Font { size: 11.0, ..Font::default() }], xfs: vec![Xf::default()], ..Styles::default() },
    };

    // Shared strings.
    let shared_xml = match rels.iter().find(|r| r.kind == "sharedStrings").and_then(|r| r.target.clone()) {
        Some(part) => package.read_xml(&part, MAX_XML)?,
        None => None,
    };
    let shared: Vec<String> = match &shared_xml {
        Some(xml) => {
            let doc = parse_xml(xml)?;
            children(doc.root_element(), "si").take(4_000_000).map(rich_text).collect()
        }
        None => Vec::new(),
    };

    // Defined names: print areas and titles.
    let names: Vec<(String, Option<usize>, String)> = child(root, "definedNames")
        .into_iter()
        .flat_map(|l| children(l, "definedName"))
        .take(10_000)
        .filter_map(|n| Some((attr(n, "name")?.to_owned(), number(n, "localSheetId").map(|v| v as usize), n.text()?.to_owned())))
        .collect();

    let mut sheets = Vec::new();
    let mut skipped = Vec::new();
    let mut budget = MAX_CELLS;
    let listed: Vec<_> = child(root, "sheets").into_iter().flat_map(|l| children(l, "sheet")).collect();
    if listed.len() > MAX_SHEETS {
        return Err(bad(format!("XLSX has more than {MAX_SHEETS} sheets")));
    }
    for (position, entry) in listed.iter().enumerate() {
        let number_of = position + 1;
        let name = attr(*entry, "name").unwrap_or("Sheet").to_owned();
        let rel = rel_attr(*entry, "id").and_then(|id| rels.iter().find(|r| r.id == id));
        if matches!(attr(*entry, "state"), Some("hidden" | "veryHidden")) {
            skipped.push((number_of, Skipped::Hidden));
            continue;
        }
        let Some(rel) = rel else { continue };
        let Some(part) = rel.target.clone() else { continue };
        if rel.kind != "worksheet" {
            skipped.push((number_of, Skipped::ChartSheet));
            continue;
        }
        let Some(xml) = package.read_xml(&part, MAX_XML)? else { continue };
        let doc = parse_xml(&xml)?;
        let mut sheet = parse_sheet(&doc, name.clone(), &shared, date1904, &mut budget)?;
        sheet.position = number_of;
        for (defined, local, text) in &names {
            if local.is_some_and(|l| l != position) {
                continue;
            }
            match defined.as_str() {
                "_xlnm.Print_Area" => sheet.print_areas.extend(sheet_refs(text, &name).into_iter().filter_map(parse_range)),
                "_xlnm.Print_Titles" => {
                    for cells in sheet_refs(text, &name) {
                        if cells.chars().any(|c| c.is_ascii_digit()) && !cells.contains(|c: char| c.is_ascii_alphabetic()) {
                            sheet.title_rows = parse_title(cells, true);
                        } else if !cells.chars().any(|c| c.is_ascii_digit()) {
                            sheet.title_cols = parse_title(cells, false);
                        }
                    }
                }
                _ => {}
            }
        }
        sheets.push(sheet);
    }
    if sheets.is_empty() && skipped.is_empty() {
        return Err(bad("XLSX has no worksheets"));
    }
    Ok(Workbook { sheets, styles, date1904, skipped })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_references_and_ranges() {
        assert_eq!(parse_cell_ref("A1"), Some((1, 1)));
        assert_eq!(parse_cell_ref("$AB$12"), Some((28, 12)));
        assert_eq!(parse_cell_ref("XFD1048576"), Some((16_384, 1_048_576)));
        assert_eq!(parse_cell_ref("XFE1"), None);
        assert_eq!(parse_cell_ref("12"), None);
        assert_eq!(parse_range("B2:A1"), Some(Range { col0: 1, row0: 1, col1: 2, row1: 2 }));
        assert_eq!(parse_range("C3"), Some(Range { col0: 3, row0: 3, col1: 3, row1: 3 }));
    }

    #[test]
    fn defined_names_pick_the_sheets_own_references() {
        assert_eq!(sheet_refs("Sheet1!$A$1:$C$9,'My Sheet'!$A:$B", "Sheet1"), vec!["$A$1:$C$9"]);
        assert_eq!(sheet_refs("'It''s'!$1:$2", "It's"), vec!["$1:$2"]);
        assert_eq!(parse_title("$1:$2", true), Some((1, 2)));
        assert_eq!(parse_title("$A:$C", false), Some((1, 3)));
    }

    #[test]
    fn tints_lighten_and_darken() {
        // 50% lighter than the Office blue 4472C4 is B4C7E7 in Excel's own palette (tint 0.6 → B4C7E7).
        let lighter = apply_tint(rgb_of(0x4472C4), 0.6);
        let byte = |v: f64| (v * 255.0).round() as i32;
        for (got, want) in lighter.iter().zip([0xB4, 0xC7, 0xE7]) {
            assert!((byte(*got) - want).abs() <= 1, "{lighter:?}");
        }
        let darker = apply_tint(rgb_of(0x4472C4), -0.5);
        for (got, want) in darker.iter().zip([0x20, 0x38, 0x64]) {
            assert!((byte(*got) - want).abs() <= 1, "{darker:?}");
        }
    }

    #[test]
    fn iso_dates_become_serials() {
        assert_eq!(iso_to_serial("2023-03-15", false), Some(45000.0));
        assert_eq!(iso_to_serial("2023-03-15T12:00:00", false), Some(45000.5));
        assert_eq!(iso_to_serial("1900-01-01", false), Some(1.0));
        assert_eq!(iso_to_serial("1904-01-02", true), Some(1.0));
        assert_eq!(iso_to_serial("nonsense", false), None);
    }
}
