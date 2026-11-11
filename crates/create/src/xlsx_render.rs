//! Spreadsheet to PDF: print layout of worksheets the way a spreadsheet prints them. Sheets are
//! cut into pages by the paper size, margins, scale and page breaks they declare; cells are drawn
//! with their fills, borders, alignment and number formats. See `README.md` for what is left out.

use std::collections::{BTreeSet, HashMap, HashSet};

use karnoxpdf_cos::{Dict, Document, Object};
use karnoxpdf_fonts::{helvetica_width, win_ansi};

use crate::CreateError;
use crate::drawingml::{Rgb, Rgba};
use crate::ooxml::{Package, bad};
use crate::pptx::{ConversionNote, NoteKind, standard_fonts};
use crate::pptx_paint::{Canvas, Matrix, Path, Stroke};
use crate::pptx_symbols;
use crate::pptx_text::Face;
use crate::xlsx::{self, Cell, HAlign, LineStyle, Range, Sheet, Skipped, VAlign, Value, Workbook, Xf};
use crate::xlsx_format::{self, Locale};
use crate::{add_page, set_title};

/// The largest workbook accepted.
pub const MAX_XLSX_BYTES: usize = 256 * 1024 * 1024;
const MAX_PAGES: usize = 3000;
const LINE: f64 = 1.3636;
const PAD: f64 = 2.0;

/// How pages are turned: as the workbook says, or forced.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SheetOrientation {
    #[default]
    Auto,
    Portrait,
    Landscape,
}

#[derive(Clone, Debug)]
pub struct XlsxOptions {
    pub orientation: SheetOrientation,
    /// The regional conventions numbers and dates are shown with.
    pub locale: Locale,
    /// Today as a spreadsheet serial number, for the `&D` and `&T` header and footer codes.
    pub today: Option<f64>,
}

impl Default for XlsxOptions {
    fn default() -> Self {
        XlsxOptions { orientation: SheetOrientation::Auto, locale: Locale::english(), today: None }
    }
}

pub struct XlsxConversion {
    pub document: Document,
    /// What the PDF leaves out of the workbook; the numbers are sheet positions (1-based).
    pub notes: Vec<ConversionNote>,
}

/// The paper size ids of `pageSetup/@paperSize` (§18.18.50), in points (portrait).
fn paper(id: u32) -> (f64, f64) {
    let mm = |w: f64, h: f64| (w * 72.0 / 25.4, h * 72.0 / 25.4);
    match id {
        3 | 17 => (792.0, 1224.0),
        4 => (1224.0, 792.0),
        5 => (612.0, 1008.0),
        6 => (396.0, 612.0),
        7 => (522.0, 756.0),
        8 => mm(297.0, 420.0),
        9 | 10 => mm(210.0, 297.0),
        11 => mm(148.0, 210.0),
        12 => mm(257.0, 364.0),
        13 => mm(182.0, 257.0),
        14 => (612.0, 936.0),
        15 => (609.6, 779.5),
        16 => (720.0, 1008.0),
        _ => (612.0, 792.0),
    }
}

// ── text measurement ────────────────────────────────────────────────────────────────────────

fn char_width(c: char, size: f64, bold: bool) -> f64 {
    let mut buf = [0u8; 4];
    let s = c.encode_utf8(&mut buf);
    if win_ansi(s) != b"?" || c == '?' {
        helvetica_width(s, size) * if bold { 1.06 } else { 1.0 }
    } else if let Some(g) = pptx_symbols::glyph(c) {
        pptx_symbols::advance(g.face, g.code) * size / 1000.0
    } else {
        helvetica_width("M", size)
    }
}

fn text_width(text: &str, size: f64, bold: bool) -> f64 {
    text.chars().map(|c| char_width(c, size, bold)).sum()
}

/// Break `text` into lines no wider than `width`; words longer than a line are broken by letter.
fn wrap_lines(text: &str, width: f64, size: f64, bold: bool) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        let mut line_w = 0.0;
        for word in paragraph.split(' ') {
            let w = text_width(word, size, bold);
            let space = if line.is_empty() { 0.0 } else { char_width(' ', size, bold) };
            if !line.is_empty() && line_w + space + w > width {
                lines.push(std::mem::take(&mut line));
                line_w = 0.0;
            }
            if line.is_empty() && w > width {
                for c in word.chars() {
                    let cw = char_width(c, size, bold);
                    if !line.is_empty() && line_w + cw > width {
                        lines.push(std::mem::take(&mut line));
                        line_w = 0.0;
                    }
                    line.push(c);
                    line_w += cw;
                }
                continue;
            }
            if !line.is_empty() {
                line.push(' ');
                line_w += space;
            }
            line.push_str(word);
            line_w += w;
        }
        lines.push(line);
    }
    lines
}

fn encode(text: &str, face: Face) -> Vec<u8> {
    match face {
        Face::Text => win_ansi(text),
        _ => text.chars().map(|c| c as u32 as u8).collect(),
    }
}

fn segment_width(text: &str, face: Face, size: f64, bold: bool) -> f64 {
    match face {
        Face::Text => helvetica_width(text, size) * if bold { 1.06 } else { 1.0 },
        face => text.chars().map(|c| pptx_symbols::advance(face, c as u32 as u8)).sum::<f64>() * size / 1000.0,
    }
}

// ── cells ───────────────────────────────────────────────────────────────────────────────────

fn has_content(cell: &Cell, styles: &xlsx::Styles) -> bool {
    if cell.value.is_some() {
        return true;
    }
    styles.xfs.get(cell.style).is_some_and(|xf| {
        xf.fill.is_some() || xf.border.left.is_some() || xf.border.right.is_some() || xf.border.top.is_some() || xf.border.bottom.is_some()
    })
}

/// What a cell shows.
struct Shown {
    text: String,
    color: Option<Rgb>,
    number: bool,
}

struct Context<'a> {
    workbook: &'a Workbook,
    options: &'a XlsxOptions,
}

impl Context<'_> {
    fn xf(&self, cell: &Cell) -> &Xf {
        self.workbook.styles.xfs.get(cell.style).or_else(|| self.workbook.styles.xfs.first()).unwrap_or_else(|| {
            static DEFAULT: std::sync::OnceLock<Xf> = std::sync::OnceLock::new();
            DEFAULT.get_or_init(Xf::default)
        })
    }

    fn font(&self, xf: &Xf) -> xlsx::Font {
        self.workbook.styles.fonts.get(xf.font).or_else(|| self.workbook.styles.fonts.first()).cloned().unwrap_or_default()
    }

    fn format_code(&self, xf: &Xf) -> String {
        self.workbook
            .styles
            .num_fmts
            .get(&xf.num_fmt)
            .map(String::as_str)
            .or_else(|| xlsx_format::builtin_code(xf.num_fmt))
            .unwrap_or("General")
            .to_owned()
    }

    fn show(&self, cell: &Cell) -> Option<Shown> {
        let xf = self.xf(cell);
        let code = self.format_code(xf);
        let locale = &self.options.locale;
        Some(match cell.value.as_ref()? {
            Value::Number(n) => {
                let f = xlsx_format::format_value(&code, *n, locale, self.workbook.date1904);
                Shown { text: f.text, color: f.color, number: true }
            }
            Value::Text(t) => {
                let f = xlsx_format::format_text(&code, t);
                Shown { text: f.text, color: f.color, number: false }
            }
            Value::Bool(b) => Shown { text: (if *b { locale.booleans.0 } else { locale.booleans.1 }).to_owned(), color: None, number: false },
            Value::Error(e) => Shown { text: e.clone(), color: None, number: false },
        })
    }
}

// ── layout ──────────────────────────────────────────────────────────────────────────────────

struct Geometry {
    /// Visible columns of the area: (column, width).
    cols: Vec<(u32, f64)>,
    /// Visible rows of the area: (row, height).
    rows: Vec<(u32, f64)>,
}

/// Column widths of a sheet, expanded from its `col` ranges as far as `last` needs.
fn column_widths(sheet: &Sheet, last: u32) -> HashMap<u32, (f64, bool)> {
    let mut out = HashMap::new();
    for def in &sheet.columns {
        for c in def.min..=def.max.min(last) {
            out.insert(c, (def.width, def.hidden));
        }
    }
    out
}

fn used_range(ctx: &Context<'_>, sheet: &Sheet) -> Option<Range> {
    let mut range: Option<Range> = None;
    let mut grow = |col: u32, row: u32| {
        range = Some(match range {
            None => Range { col0: col, row0: row, col1: col, row1: row },
            Some(r) => Range { col0: r.col0.min(col), row0: r.row0.min(row), col1: r.col1.max(col), row1: r.row1.max(row) },
        });
    };
    for (row, r) in &sheet.rows {
        for (col, cell) in &r.cells {
            if has_content(cell, &ctx.workbook.styles) {
                grow(*col, *row);
            }
        }
    }
    for m in &sheet.merges {
        grow(m.col0, m.row0);
        grow(m.col1, m.row1);
    }
    range
}

/// The wrapped-text height rows without a stored height need.
fn auto_height(ctx: &Context<'_>, sheet: &Sheet, row: u32, widths: &HashMap<u32, (f64, bool)>, merged: &HashSet<(u32, u32)>) -> f64 {
    let mut height = sheet.default_row_height;
    let Some(r) = sheet.rows.get(&row) else { return height };
    for (col, cell) in &r.cells {
        let xf = ctx.xf(cell);
        if !xf.wrap || merged.contains(&(*col, row)) {
            continue;
        }
        let Some(shown) = ctx.show(cell) else { continue };
        let font = ctx.font(xf);
        let width = widths.get(col).map_or(sheet.default_col_width, |w| w.0) - 2.0 * PAD;
        let lines = wrap_lines(&shown.text, width.max(4.0), font.size, font.bold).len() as f64;
        height = height.max(lines * font.size * LINE + 1.0);
    }
    height
}

fn geometry(ctx: &Context<'_>, sheet: &Sheet, area: Range, widths: &HashMap<u32, (f64, bool)>, merged: &HashSet<(u32, u32)>) -> Geometry {
    let cols = (area.col0..=area.col1)
        .map(|c| (c, widths.get(&c).copied().unwrap_or((sheet.default_col_width, false))))
        .filter(|(_, (w, hidden))| !hidden && *w > 0.0)
        .map(|(c, (w, _))| (c, w))
        .collect();
    let rows = (area.row0..=area.row1)
        .filter_map(|r| {
            let stored = sheet.rows.get(&r);
            if stored.is_some_and(|s| s.hidden) {
                return None;
            }
            let height = match stored.and_then(|s| s.height) {
                Some(h) => h,
                None => auto_height(ctx, sheet, r, widths, merged),
            };
            (height > 0.0).then_some((r, height))
        })
        .collect();
    Geometry { cols, rows }
}

/// Cut `items` (index, size) into runs that fit `avail`, after `lead` points taken by repeated
/// titles on all but the first run, and at the manual breaks (a break after item `id`).
fn strips(items: &[(u32, f64)], avail: f64, lead: f64, breaks: &[u32]) -> Vec<Vec<(u32, f64)>> {
    let mut out: Vec<Vec<(u32, f64)>> = Vec::new();
    let mut current: Vec<(u32, f64)> = Vec::new();
    let mut used = 0.0;
    for (i, (id, size)) in items.iter().enumerate() {
        let limit = if out.is_empty() { avail } else { avail - lead };
        let after_break = i > 0 && breaks.contains(&items[i - 1].0);
        if !current.is_empty() && (used + size > limit + 0.01 || after_break) {
            out.push(std::mem::take(&mut current));
            used = 0.0;
        }
        current.push((*id, *size));
        used += size;
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

// ── drawing ─────────────────────────────────────────────────────────────────────────────────

fn stroke_for(style: LineStyle, color: Rgb) -> Stroke {
    let (width, dash): (f64, Vec<f64>) = match style {
        LineStyle::Hair => (0.25, vec![]),
        LineStyle::Thin => (0.75, vec![]),
        LineStyle::Medium => (1.5, vec![]),
        LineStyle::Thick => (2.25, vec![]),
        LineStyle::Dashed => (0.75, vec![3.0, 2.0]),
        LineStyle::Dotted => (0.75, vec![0.75, 1.5]),
        LineStyle::Double => (0.5, vec![]),
    };
    Stroke { width, color: Rgba { rgb: color, alpha: 1.0 }, dash }
}

struct PageCtx<'a> {
    ctx: &'a Context<'a>,
    sheet: &'a Sheet,
    widths: &'a HashMap<u32, (f64, bool)>,
    heights: &'a HashMap<u32, f64>,
    merge_at: &'a HashMap<(u32, u32), Range>,
    covered: &'a HashSet<(u32, u32)>,
}

impl PageCtx<'_> {
    fn col_width(&self, c: u32) -> f64 {
        match self.widths.get(&c) {
            Some((w, hidden)) => {
                if *hidden {
                    0.0
                } else {
                    *w
                }
            }
            None => self.sheet.default_col_width,
        }
    }

    fn row_height(&self, r: u32) -> f64 {
        self.heights.get(&r).copied().unwrap_or(0.0)
    }

    /// The rectangle a cell occupies: its own, or the whole merged range it starts.
    fn extent(&self, col: u32, row: u32, x: f64, y: f64) -> (f64, f64, f64, f64) {
        match self.merge_at.get(&(col, row)) {
            Some(m) => {
                let w: f64 = (m.col0..=m.col1).map(|c| self.col_width(c)).sum();
                let h: f64 = (m.row0..=m.row1).map(|r| self.row_height(r)).sum();
                (x, y, w, h)
            }
            None => (x, y, self.col_width(col), self.row_height(row)),
        }
    }
}

struct PageData {
    cols: Vec<(u32, f64)>,
    rows: Vec<(u32, f64)>,
}

fn draw_cell_text(
    cv: &mut Canvas,
    pc: &PageCtx<'_>,
    cell: &Cell,
    (x, y, w, h): (f64, f64, f64, f64),
    (free_left, free_right): (f64, f64),
    (place_col, place_row, sheet_position): (u32, u32, usize),
) -> Result<(), CreateError> {
    let Some(shown) = pc.ctx.show(cell) else { return Ok(()) };
    if shown.text.is_empty() {
        return Ok(());
    }
    let xf = pc.ctx.xf(cell);
    let font = pc.ctx.font(xf);
    let size = font.size;
    let color = Rgba { rgb: shown.color.or(font.color).unwrap_or([0.0; 3]), alpha: 1.0 };
    let halign = match xf.halign {
        HAlign::General if shown.number => HAlign::Right,
        HAlign::General => match cell.value {
            Some(Value::Bool(_) | Value::Error(_)) => HAlign::Center,
            _ => HAlign::Left,
        },
        HAlign::Fill | HAlign::Justify | HAlign::Distributed => HAlign::Left,
        other => other,
    };
    let indent = if matches!(halign, HAlign::Left | HAlign::Right) { xf.indent * 9.0 } else { 0.0 };
    let inner_w = (w - 2.0 * PAD - indent).max(1.0);

    let mut lines: Vec<String> = if xf.wrap { wrap_lines(&shown.text, inner_w, size, font.bold) } else { vec![shown.text.replace('\n', " ")] };
    let mut hscale = 1.0;
    if !xf.wrap {
        let natural = text_width(&lines[0], size, font.bold);
        if natural > inner_w && (shown.number || xf.shrink) {
            if natural <= inner_w * 1.25 || xf.shrink {
                hscale = (inner_w / natural).max(0.5);
                if natural * hscale > inner_w + 0.01 && shown.number {
                    lines[0] = "#".repeat(((inner_w / char_width('#', size, font.bold)).floor() as usize).max(1));
                    hscale = 1.0;
                }
            } else {
                // A number that cannot fit is shown as hash marks, as spreadsheets do.
                lines[0] = "#".repeat(((inner_w / char_width('#', size, font.bold)).floor() as usize).max(1));
            }
        }
    }
    let line_h = size * LINE;
    let block_h = line_h * lines.len() as f64;
    let top = match xf.valign {
        VAlign::Top | VAlign::Justify | VAlign::Distributed => y + PAD / 2.0,
        VAlign::Center => y + (h - block_h) / 2.0,
        VAlign::Bottom => y + h - block_h - PAD / 2.0,
    };
    // Text that does not wrap may run into empty neighbours.
    let (left_edge, right_edge) = if xf.wrap || shown.number {
        (x, x + w)
    } else {
        match halign {
            HAlign::Right => (x - free_left, x + w),
            HAlign::Center | HAlign::CenterContinuous => (x - free_left.min(free_right), x + w + free_right.min(free_left)),
            _ => (x, x + w + free_right),
        }
    };
    cv.save();
    cv.clip_rect(left_edge, y, right_edge - left_edge, h.max(0.0));
    for (i, line) in lines.iter().enumerate() {
        if line.is_empty() {
            continue;
        }
        let segments = pptx_symbols::segments(line, Some(&font.name)).map_err(|c| {
            bad(format!(
                "sheet {sheet_position} cell {}{} contains {c:?} (U+{:04X}), which the PDF's built-in fonts cannot show; conversion stopped to avoid changing the text",
                column_name(place_col),
                place_row,
                c as u32
            ))
        })?;
        let width: f64 = segments.iter().map(|(t, f)| segment_width(t, *f, size, font.bold)).sum::<f64>() * hscale;
        let start = match halign {
            HAlign::Right => x + w - PAD - width,
            HAlign::Center | HAlign::CenterContinuous => x + (w - width) / 2.0,
            _ => x + PAD + indent,
        };
        let baseline = top + line_h * i as f64 + line_h - size * 0.3;
        let mut cursor = start;
        for (text, face) in &segments {
            let pdf_font = match face {
                Face::Symbol => "F5",
                Face::Dingbats => "F6",
                Face::Text => match (font.bold, font.italic) {
                    (false, false) => "F1",
                    (true, false) => "F2",
                    (false, true) => "F3",
                    (true, true) => "F4",
                },
            };
            let seg_w = segment_width(text, *face, size, font.bold) * hscale;
            if !text.trim().is_empty() {
                cv.text_scaled(pdf_font, size, (cursor, baseline), hscale, color, &encode(text, *face));
            }
            cursor += seg_w;
        }
        let rule = |cv: &mut Canvas, dy: f64| {
            let mut p = Path::new();
            p.move_to(start, baseline + dy);
            p.line_to(start + width, baseline + dy);
            cv.stroke(&p, &Stroke { width: (size * 0.06).max(0.4), color, dash: Vec::new() });
        };
        if font.underline {
            rule(cv, size * 0.12);
        }
        if font.strike {
            rule(cv, -size * 0.3);
        }
    }
    cv.restore();
    Ok(())
}

pub(crate) fn column_name(col: u32) -> String {
    let mut n = col;
    let mut out = Vec::new();
    while n > 0 {
        n -= 1;
        out.push(b'A' + (n % 26) as u8);
        n /= 26;
    }
    out.reverse();
    String::from_utf8_lossy(&out).into_owned()
}

#[allow(clippy::too_many_arguments)]
fn draw_page(cv: &mut Canvas, pc: &PageCtx<'_>, page: &PageData, position: usize, page_setup_gridlines: bool) -> Result<(), CreateError> {
    let mut col_x: HashMap<u32, f64> = HashMap::new();
    let mut x = 0.0;
    for (c, w) in &page.cols {
        col_x.insert(*c, x);
        x += w;
    }
    let total_w = x;
    let mut row_y: HashMap<u32, f64> = HashMap::new();
    let mut y = 0.0;
    for (r, h) in &page.rows {
        row_y.insert(*r, y);
        y += h;
    }
    let total_h = y;
    cv.save();
    cv.clip_rect(0.0, 0.0, total_w, total_h);
    let visible_cells = |row: u32| -> Vec<(u32, &Cell)> {
        match pc.sheet.rows.get(&row) {
            Some(r) => page.cols.iter().filter_map(|(c, _)| r.cells.get(c).map(|cell| (*c, cell))).collect(),
            None => Vec::new(),
        }
    };
    // Fills first, so that text and borders are never covered.
    for (row, _) in &page.rows {
        for (col, cell) in visible_cells(*row) {
            if pc.covered.contains(&(col, *row)) {
                continue;
            }
            if let (Some(fill), Some(&cx), Some(&cy)) = (pc.ctx.xf(cell).fill, col_x.get(&col), row_y.get(row)) {
                let (ex, ey, ew, eh) = pc.extent(col, *row, cx, cy);
                cv.fill(&Path::rect(ex, ey, ew, eh), Rgba { rgb: fill, alpha: 1.0 });
            }
        }
    }
    if page_setup_gridlines {
        let grid = Stroke { width: 0.5, color: Rgba { rgb: [0.55; 3], alpha: 1.0 }, dash: Vec::new() };
        let mut p = Path::new();
        for (r, _) in &page.rows {
            let ry = row_y.get(r).copied().unwrap_or(0.0);
            p.move_to(0.0, ry);
            p.line_to(total_w, ry);
        }
        p.move_to(0.0, total_h);
        p.line_to(total_w, total_h);
        for (c, _) in &page.cols {
            let cx = col_x.get(c).copied().unwrap_or(0.0);
            p.move_to(cx, 0.0);
            p.line_to(cx, total_h);
        }
        p.move_to(total_w, 0.0);
        p.line_to(total_w, total_h);
        cv.stroke(&p, &grid);
    }
    // Text.
    for (row, _) in &page.rows {
        let Some(&cy) = row_y.get(row) else { continue };
        let Some(stored) = pc.sheet.rows.get(row) else { continue };
        for (col, cell) in visible_cells(*row) {
            if pc.covered.contains(&(col, *row)) || cell.value.is_none() {
                continue;
            }
            let Some(&cx) = col_x.get(&col) else { continue };
            let rect = pc.extent(col, *row, cx, cy);
            // Room to overflow into empty cells on either side.
            let empty = |c: u32| -> bool {
                stored.cells.get(&c).is_none_or(|n| n.value.is_none()) && !pc.covered.contains(&(c, *row)) && !pc.merge_at.contains_key(&(c, *row))
            };
            let mut free_right = 0.0;
            let mut free_left = 0.0;
            let last_col = rect_last_col(pc, col, *row);
            if let Some(i) = page.cols.iter().position(|(c, _)| *c == col) {
                let after = page.cols.iter().skip_while(|(c, _)| *c <= last_col);
                for (c, w) in after {
                    if !empty(*c) {
                        break;
                    }
                    free_right += w;
                }
                for (c, w) in page.cols[..i].iter().rev() {
                    if !empty(*c) {
                        break;
                    }
                    free_left += w;
                }
            }
            draw_cell_text(cv, pc, cell, rect, (free_left, free_right), (col, *row, position))?;
        }
    }
    // Borders.
    for (row, _) in &page.rows {
        let Some(&cy) = row_y.get(row) else { continue };
        for (col, cell) in visible_cells(*row) {
            if pc.covered.contains(&(col, *row)) {
                continue;
            }
            let Some(&cx) = col_x.get(&col) else { continue };
            let xf = pc.ctx.xf(cell);
            let (ex, ey, ew, eh) = pc.extent(col, *row, cx, cy);
            let sides = [
                (xf.border.left, ((ex, ey), (ex, ey + eh))),
                (xf.border.right, ((ex + ew, ey), (ex + ew, ey + eh))),
                (xf.border.top, ((ex, ey), (ex + ew, ey))),
                (xf.border.bottom, ((ex, ey + eh), (ex + ew, ey + eh))),
            ];
            for (edge, ((x0, y0), (x1, y1))) in sides {
                let Some(edge) = edge else { continue };
                let stroke = stroke_for(edge.style, edge.color);
                let mut line = |dx: f64, dy: f64| {
                    let mut p = Path::new();
                    p.move_to(x0 + dx, y0 + dy);
                    p.line_to(x1 + dx, y1 + dy);
                    cv.stroke(&p, &stroke);
                };
                if edge.style == LineStyle::Double {
                    let horizontal = (y0 - y1).abs() < f64::EPSILON;
                    let d = 0.8;
                    if horizontal {
                        line(0.0, -d);
                        line(0.0, d);
                    } else {
                        line(-d, 0.0);
                        line(d, 0.0);
                    }
                } else {
                    line(0.0, 0.0);
                }
            }
        }
    }
    cv.restore();
    Ok(())
}

/// The last column a cell covers (itself, or the end of the merged range it starts).
fn rect_last_col(pc: &PageCtx<'_>, col: u32, row: u32) -> u32 {
    pc.merge_at.get(&(col, row)).map_or(col, |m| m.col1)
}

// ── headers and footers ─────────────────────────────────────────────────────────────────────

struct HeaderPieces {
    left: String,
    center: String,
    right: String,
    size: Option<f64>,
    bold: bool,
    italic: bool,
}

/// Header/footer codes (§18.3.1.36): `&L &C &R` sections, `&P` page, `&N` pages, `&A` sheet name,
/// `&F` file name, `&D` date, `&T` time, `&nn` size, `&B` `&I` toggles, `&&` an ampersand.
fn parse_header(text: &str, page: u32, pages: u32, sheet: &str, file: &str, options: &XlsxOptions) -> HeaderPieces {
    let mut out = HeaderPieces { left: String::new(), center: String::new(), right: String::new(), size: None, bold: false, italic: false };
    let mut target = 'C';
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c != '&' {
            push(&mut out, target, &c.to_string());
            i += 1;
            continue;
        }
        i += 1;
        let Some(&code) = chars.get(i) else { break };
        match code {
            'L' | 'C' | 'R' => target = code,
            'P' => push(&mut out, target, &page.to_string()),
            'N' => push(&mut out, target, &pages.to_string()),
            'A' => push(&mut out, target, sheet),
            'F' => push(&mut out, target, file),
            'D' => {
                if let Some(today) = options.today {
                    push(&mut out, target, &xlsx_format::format_value("<short date>", today.floor(), &options.locale, false).text);
                }
            }
            'T' => {
                if let Some(today) = options.today {
                    push(&mut out, target, &xlsx_format::format_value("h:mm AM/PM", today.fract(), &options.locale, false).text);
                }
            }
            'B' => out.bold = !out.bold,
            'I' => out.italic = !out.italic,
            '&' => push(&mut out, target, "&"),
            '"' => {
                // &"Font,Style": skipped.
                while chars.get(i + 1).is_some_and(|c| *c != '"') {
                    i += 1;
                }
                i += 1;
            }
            d if d.is_ascii_digit() => {
                let mut n = d.to_digit(10).unwrap_or(0) as f64;
                while let Some(next) = chars.get(i + 1).and_then(|c| c.to_digit(10)) {
                    n = n * 10.0 + next as f64;
                    i += 1;
                }
                out.size = Some(n.clamp(1.0, 100.0));
            }
            _ => {}
        }
        i += 1;
    }
    out
}

fn push(pieces: &mut HeaderPieces, target: char, text: &str) {
    match target {
        'L' => pieces.left.push_str(text),
        'R' => pieces.right.push_str(text),
        _ => pieces.center.push_str(text),
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_header_footer(
    cv: &mut Canvas,
    pieces: &HeaderPieces,
    default_size: f64,
    page_w: f64,
    margins: (f64, f64),
    baseline: f64,
    position: usize,
) -> Result<(), CreateError> {
    let size = pieces.size.unwrap_or(default_size);
    let font = match (pieces.bold, pieces.italic) {
        (false, false) => "F1",
        (true, false) => "F2",
        (false, true) => "F3",
        (true, true) => "F4",
    };
    for (text, align) in [(&pieces.left, 'L'), (&pieces.center, 'C'), (&pieces.right, 'R')] {
        if text.is_empty() {
            continue;
        }
        for (n, line) in text.split('\n').enumerate() {
            let segments = pptx_symbols::segments(line, None)
                .map_err(|c| bad(format!("sheet {position} header or footer contains {c:?}, which the PDF's built-in fonts cannot show")))?;
            let width: f64 = segments.iter().map(|(t, f)| segment_width(t, *f, size, pieces.bold)).sum();
            let x = match align {
                'L' => margins.0,
                'R' => page_w - margins.1 - width,
                _ => (page_w - width) / 2.0,
            };
            let mut cursor = x;
            for (t, face) in &segments {
                let pdf_font = match face {
                    Face::Symbol => "F5",
                    Face::Dingbats => "F6",
                    Face::Text => font,
                };
                cv.text(pdf_font, size, (cursor, baseline + n as f64 * size * 1.2), 0.0, Rgba { rgb: [0.0; 3], alpha: 1.0 }, &encode(t, *face));
                cursor += segment_width(t, *face, size, pieces.bold);
            }
        }
    }
    Ok(())
}

// ── conversion ──────────────────────────────────────────────────────────────────────────────

/// Convert a `.xlsx` workbook to a PDF document, printing every visible sheet.
pub fn from_xlsx_layout(title: &str, bytes: &[u8], options: &XlsxOptions) -> Result<XlsxConversion, CreateError> {
    let package = Package::open(bytes, "XLSX", MAX_XLSX_BYTES)?;
    let workbook = xlsx::read(&package)?;
    let ctx = Context { workbook: &workbook, options };
    let mut out = Document::new_empty();
    let fonts = standard_fonts(&mut out);
    let mut notes: std::collections::BTreeMap<NoteKind, BTreeSet<usize>> = std::collections::BTreeMap::new();
    for (position, why) in &workbook.skipped {
        notes
            .entry(match why {
                Skipped::Hidden => NoteKind::HiddenSheet,
                Skipped::ChartSheet => NoteKind::Chart,
            })
            .or_default()
            .insert(*position);
    }
    let default_size = workbook.styles.fonts.first().map_or(11.0, |f| f.size);
    let mut pages_written = 0usize;
    for sheet in &workbook.sheets {
        let position = sheet.position;
        if sheet.has_drawing {
            notes.entry(NoteKind::Drawing).or_default().insert(position);
        }
        if sheet.has_conditional_formatting {
            notes.entry(NoteKind::ConditionalFormatting).or_default().insert(position);
        }
        if sheet.has_table {
            notes.entry(NoteKind::SheetTable).or_default().insert(position);
        }
        let areas: Vec<Range> = if sheet.print_areas.is_empty() { used_range(&ctx, sheet).into_iter().collect() } else { sheet.print_areas.clone() };
        let last_col = areas.iter().map(|a| a.col1).max().unwrap_or(1).max(sheet.title_cols.map_or(1, |t| t.1));
        let widths = column_widths(sheet, last_col);
        let mut merge_at: HashMap<(u32, u32), Range> = HashMap::new();
        let mut covered: HashSet<(u32, u32)> = HashSet::new();
        for m in &sheet.merges {
            if (m.col1 - m.col0) as usize * (m.row1 - m.row0) as usize > 100_000 {
                continue;
            }
            merge_at.insert((m.col0, m.row0), *m);
            for r in m.row0..=m.row1 {
                for c in m.col0..=m.col1 {
                    if (c, r) != (m.col0, m.row0) {
                        covered.insert((c, r));
                    }
                }
            }
        }
        // Page geometry shared by every area of the sheet.
        let (mut page_w, mut page_h) = paper(sheet.page.paper);
        let landscape = match options.orientation {
            SheetOrientation::Auto => sheet.page.landscape,
            SheetOrientation::Portrait => false,
            SheetOrientation::Landscape => true,
        };
        if landscape != (page_w > page_h) {
            std::mem::swap(&mut page_w, &mut page_h);
        }
        let [ml, mr, mt, mb, header_margin, footer_margin] = sheet.page.margins;
        let content_w = (page_w - ml - mr).max(36.0);
        let content_h = (page_h - mt - mb).max(36.0);

        struct Prepared {
            pages: Vec<PageData>,
        }
        let mut prepared: Vec<Prepared> = Vec::new();
        let mut heights: HashMap<u32, f64> = HashMap::new();
        let mut geoms: Vec<Geometry> = Vec::new();
        for area in &areas {
            let geom = geometry(&ctx, sheet, *area, &widths, &covered);
            heights.extend(geom.rows.iter().copied());
            geoms.push(geom);
        }
        let title_rows: Vec<(u32, f64)> = sheet
            .title_rows
            .map(|(a, b)| {
                let g = geometry(&ctx, sheet, Range { col0: 1, row0: a, col1: 1, row1: b }, &widths, &covered);
                g.rows
            })
            .unwrap_or_default();
        heights.extend(title_rows.iter().copied());
        let title_cols: Vec<(u32, f64)> = sheet
            .title_cols
            .map(|(a, b)| geometry(&ctx, sheet, Range { col0: a, row0: 1, col1: b, row1: 1 }, &widths, &covered).cols)
            .unwrap_or_default();

        // Scale: stated, or chosen so the area fits the pages asked for (never enlarging).
        let total_w: f64 = geoms.iter().map(|g| g.cols.iter().map(|c| c.1).sum::<f64>()).fold(0.0, f64::max);
        let total_h: f64 = geoms.iter().map(|g| g.rows.iter().map(|r| r.1).sum::<f64>()).fold(0.0, f64::max);
        let scale = match sheet.page.fit {
            Some((fw, fh)) => {
                let mut s: f64 = 1.0;
                if fw > 0 && total_w > 0.0 {
                    s = s.min(content_w * fw as f64 / total_w);
                }
                if fh > 0 && total_h > 0.0 {
                    s = s.min(content_h * fh as f64 / total_h);
                }
                s.clamp(0.1, 1.0)
            }
            None => sheet.page.scale / 100.0,
        };
        let avail_w = content_w / scale;
        let avail_h = content_h / scale;
        for geom in &geoms {
            let title_w: f64 = title_cols.iter().map(|c| c.1).sum();
            let title_h: f64 = title_rows.iter().map(|r| r.1).sum();
            let col_strips = strips(&geom.cols, avail_w, title_w, &sheet.col_breaks);
            let row_strips = strips(&geom.rows, avail_h, title_h, &sheet.row_breaks);
            let mut pages = Vec::new();
            let mut push_page = |cs: &Vec<(u32, f64)>, rs: &Vec<(u32, f64)>, first_col: bool, first_row: bool| {
                let mut cols = Vec::new();
                if !first_col {
                    cols.extend(title_cols.iter().copied());
                }
                cols.extend(cs.iter().copied());
                let mut rows = Vec::new();
                if !first_row {
                    rows.extend(title_rows.iter().copied());
                }
                rows.extend(rs.iter().copied());
                // A page with nothing to show (no value, fill or border) is not printed.
                let printable = rs.iter().any(|(r, _)| {
                    sheet
                        .rows
                        .get(r)
                        .is_some_and(|row| cs.iter().any(|(c, _)| row.cells.get(c).is_some_and(|cell| has_content(cell, &workbook.styles))))
                }) || sheet.merges.iter().any(|m| rs.iter().any(|(r, _)| *r == m.row0) && cs.iter().any(|(c, _)| *c == m.col0));
                if printable {
                    pages.push(PageData { cols, rows });
                }
            };
            if sheet.page.over_then_down {
                for (ri, rs) in row_strips.iter().enumerate() {
                    for (ci, cs) in col_strips.iter().enumerate() {
                        push_page(cs, rs, ci == 0, ri == 0);
                    }
                }
            } else {
                for (ci, cs) in col_strips.iter().enumerate() {
                    for (ri, rs) in row_strips.iter().enumerate() {
                        push_page(cs, rs, ci == 0, ri == 0);
                    }
                }
            }
            if pages.len() > MAX_PAGES {
                return Err(bad(format!("XLSX would need more than {MAX_PAGES} pages")));
            }
            prepared.push(Prepared { pages });
        }
        let sheet_pages: usize = prepared.iter().map(|p| p.pages.len()).sum();
        let pc = PageCtx { ctx: &ctx, sheet, widths: &widths, heights: &heights, merge_at: &merge_at, covered: &covered };
        let first_number = sheet.page.first_page.unwrap_or(1);
        let mut number = 0u32;
        for area in &prepared {
            for page in &area.pages {
                number += 1;
                pages_written += 1;
                if pages_written > MAX_PAGES {
                    return Err(bad(format!("XLSX would need more than {MAX_PAGES} pages")));
                }
                let mut cv = Canvas::new(page_h);
                let (page_w_used, page_h_used): (f64, f64) = (page.cols.iter().map(|c| c.1).sum(), page.rows.iter().map(|r| r.1).sum());
                let ox = ml + if sheet.page.center_horizontally { ((content_w - page_w_used * scale) / 2.0).max(0.0) } else { 0.0 };
                let oy = mt + if sheet.page.center_vertically { ((content_h - page_h_used * scale) / 2.0).max(0.0) } else { 0.0 };
                cv.save();
                cv.transform(Matrix([scale, 0.0, 0.0, scale, ox, oy]));
                draw_page(&mut cv, &pc, page, position, sheet.page.gridlines)?;
                cv.restore();
                // Header and footer, in the margins.
                let which = if sheet.page.different_first && number == 1 {
                    2
                } else if sheet.page.different_odd_even && (first_number + number - 1).is_multiple_of(2) {
                    1
                } else {
                    0
                };
                let shown_number = first_number + number - 1;
                for (text, baseline) in
                    [(&sheet.page.header[which], header_margin + default_size), (&sheet.page.footer[which], page_h - footer_margin)]
                {
                    if text.is_empty() {
                        continue;
                    }
                    let pieces = parse_header(text, shown_number, sheet_pages as u32, &sheet.name, title, options);
                    draw_header_footer(&mut cv, &pieces, default_size, page_w, (ml, mr), baseline, position)?;
                }
                let (content, used) = cv.finish();
                let mut resources = Dict::new();
                resources.set(b"Font".to_vec(), Object::Dict(fonts.clone()));
                if used.has_alpha() {
                    resources.set(b"ExtGState".to_vec(), Object::Dict(used.extgstate()));
                }
                add_page(&mut out, page_w, page_h, resources, Some(content))?;
            }
        }
    }
    if pages_written == 0 {
        return Err(bad("the workbook has nothing to print"));
    }
    set_title(&mut out, title);
    let notes = notes.into_iter().map(|(kind, items)| ConversionNote { kind, slides: items.into_iter().collect() }).collect();
    Ok(XlsxConversion { document: out, notes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_letters() {
        assert_eq!(column_name(1), "A");
        assert_eq!(column_name(26), "Z");
        assert_eq!(column_name(27), "AA");
        assert_eq!(column_name(16_384), "XFD");
    }

    #[test]
    fn long_text_wraps_inside_the_width() {
        let lines = wrap_lines("one two three four five six seven", 60.0, 11.0, false);
        assert!(lines.len() >= 3 && lines.iter().all(|l| text_width(l, 11.0, false) <= 60.5), "{lines:?}");
        assert_eq!(wrap_lines("a\nb", 100.0, 11.0, false), vec!["a", "b"]);
        let broken = wrap_lines("Supercalifragilisticexpialidocious", 40.0, 11.0, false);
        assert!(broken.len() > 1 && broken.iter().all(|l| text_width(l, 11.0, false) <= 40.5));
    }

    #[test]
    fn strips_respect_widths_and_breaks() {
        let items: Vec<(u32, f64)> = (1..=6).map(|i| (i, 50.0)).collect();
        let s = strips(&items, 120.0, 0.0, &[]);
        assert_eq!(s.iter().map(Vec::len).collect::<Vec<_>>(), vec![2, 2, 2]);
        let s = strips(&items, 1000.0, 0.0, &[2]);
        assert_eq!(s.iter().map(Vec::len).collect::<Vec<_>>(), vec![2, 4], "a manual break after item 2");
        let s = strips(&[(1, 500.0)], 100.0, 0.0, &[]);
        assert_eq!(s.len(), 1, "an item wider than a page still gets one");
        // Repeated titles take room from every strip after the first.
        let s = strips(&items, 120.0, 50.0, &[]);
        assert_eq!(s.iter().map(Vec::len).collect::<Vec<_>>(), vec![2, 1, 1, 1, 1]);
    }

    #[test]
    fn header_codes() {
        let o = XlsxOptions::default();
        let p = parse_header("&LReport &A&CPage &P of &N&R&BBold&B &&", 2, 5, "Sales", "book.xlsx", &o);
        assert_eq!(p.left, "Report Sales");
        assert_eq!(p.center, "Page 2 of 5");
        assert_eq!(p.right, "Bold &");
        let p = parse_header("&14&F", 1, 1, "S", "f.xlsx", &o);
        assert_eq!((p.center.as_str(), p.size), ("f.xlsx", Some(14.0)));
    }

    #[test]
    fn paper_sizes() {
        let (w, h) = paper(9);
        assert!((w - 595.3).abs() < 0.2 && (h - 841.9).abs() < 0.2);
        assert_eq!(paper(1), (612.0, 792.0));
        assert_eq!(paper(999), (612.0, 792.0));
    }
}
