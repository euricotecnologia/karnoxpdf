//! Native, bounded WordprocessingML layout for common DOCX documents.
//! This deliberately rejects unsupported drawings instead of silently flattening them.

use std::collections::HashMap;

use karnoxpdf_cos::{Dict, Document, Object};
use karnoxpdf_fonts::{helvetica_width, literal, win_ansi};
use roxmltree::Node;

use crate::{CreateError, add_page, docx::part, set_title};

const MAX_XML: usize = 16 * 1024 * 1024;
const MAX_BLOCKS: usize = 20_000;
const MAX_PAGES: usize = 500;

fn bad(message: impl Into<String>) -> CreateError {
    CreateError::Invalid(message.into())
}
fn child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    node.children().find(|n| n.is_element() && n.tag_name().name() == name)
}
fn attr<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<&'a str> {
    node.attributes().find(|a| a.name() == name).map(|a| a.value())
}
fn value<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<&'a str> {
    child(node, name).and_then(|n| attr(n, "val"))
}
fn twips(value: Option<&str>) -> Option<f64> {
    value?.parse::<f64>().ok().filter(|n| n.is_finite()).map(|n| n / 20.0)
}
fn points(value: Option<&str>) -> Option<f64> {
    value?.parse::<f64>().ok().filter(|n| n.is_finite()).map(|n| n / 2.0)
}
fn flag(node: Node<'_, '_>, name: &str) -> Option<bool> {
    child(node, name).map(|n| !matches!(attr(n, "val"), Some("0" | "false" | "off")))
}
fn rgb(value: &str) -> Option<[f64; 3]> {
    if value.len() != 6 {
        return None;
    }
    let bytes = u32::from_str_radix(value, 16).ok()?;
    Some([((bytes >> 16) & 255) as f64 / 255.0, ((bytes >> 8) & 255) as f64 / 255.0, (bytes & 255) as f64 / 255.0])
}

#[derive(Clone, Copy, PartialEq)]
struct TextStyle {
    size: f64,
    bold: bool,
    italic: bool,
    color: [f64; 3],
}
impl Default for TextStyle {
    fn default() -> Self {
        Self { size: 11.0, bold: false, italic: false, color: [0.0; 3] }
    }
}
impl TextStyle {
    fn apply(&mut self, props: Node<'_, '_>) {
        if let Some(size) = points(value(props, "sz")).filter(|n| (4.0..=72.0).contains(n)) {
            self.size = size;
        }
        if let Some(bold) = flag(props, "b") {
            self.bold = bold;
        }
        if let Some(italic) = flag(props, "i") {
            self.italic = italic;
        }
        if let Some(color) = value(props, "color").and_then(rgb) {
            self.color = color;
        }
    }
    fn font(self) -> &'static str {
        match (self.bold, self.italic) {
            (false, false) => "F1",
            (true, false) => "F2",
            (false, true) => "F3",
            (true, true) => "F4",
        }
    }
}

#[derive(Clone, Copy, Default)]
struct ParaStyle {
    before: f64,
    after: f64,
    left: f64,
    right: f64,
    align: Alignment,
    page_break: bool,
}
#[derive(Clone, Copy, Default)]
enum Alignment {
    #[default]
    Left,
    Center,
    Right,
}
impl ParaStyle {
    fn apply(&mut self, props: Node<'_, '_>) {
        if let Some(spacing) = child(props, "spacing") {
            if let Some(v) = twips(attr(spacing, "before")) {
                self.before = v.clamp(0.0, 144.0);
            }
            if let Some(v) = twips(attr(spacing, "after")) {
                self.after = v.clamp(0.0, 144.0);
            }
        }
        if let Some(indent) = child(props, "ind") {
            if let Some(v) = twips(attr(indent, "left")) {
                self.left = v.clamp(0.0, 400.0);
            }
            if let Some(v) = twips(attr(indent, "right")) {
                self.right = v.clamp(0.0, 400.0);
            }
        }
        if let Some(justify) = value(props, "jc") {
            self.align = match justify {
                "center" => Alignment::Center,
                "right" | "end" => Alignment::Right,
                _ => Alignment::Left,
            };
        }
        if flag(props, "pageBreakBefore") == Some(true) {
            self.page_break = true;
        }
    }
}
#[derive(Clone)]
struct StyleDef {
    based_on: Option<String>,
    text: Option<roxmltree::NodeId>,
    para: Option<roxmltree::NodeId>,
}

#[derive(Clone)]
struct Run {
    text: String,
    style: TextStyle,
}
struct Shape {
    x: f64,
    y: f64,
    x_from_page: bool,
    y_from_page: bool,
    color: [f64; 3],
    filled: bool,
    points: Vec<(f64, f64)>,
}
struct Paragraph {
    runs: Vec<Run>,
    para: ParaStyle,
    page_break: bool,
    shapes: Vec<Shape>,
}
struct Line {
    runs: Vec<Run>,
    width: f64,
    size: f64,
}
impl Line {
    fn new() -> Self {
        Self { runs: Vec::new(), width: 0.0, size: 11.0 }
    }
}

fn layout(p: &Paragraph, max_width: f64) -> Vec<Line> {
    let mut lines = vec![Line::new()];
    let width = max_width.max(20.0);
    for run in &p.runs {
        for token in run.text.split_inclusive([' ', '\n']) {
            let word = token.trim_end_matches('\n');
            if !word.is_empty() {
                let advance = helvetica_width(word, run.style.size);
                let full = lines.last().is_some_and(|line| line.width + advance > width && !line.runs.is_empty());
                if full {
                    lines.push(Line::new());
                }
                if let Some(line) = lines.last_mut() {
                    line.width += advance;
                    line.size = line.size.max(run.style.size);
                    if let Some(previous) = line.runs.last_mut().filter(|previous| previous.style == run.style) {
                        previous.text.push_str(word);
                    } else {
                        line.runs.push(Run { text: word.to_owned(), style: run.style });
                    }
                }
            }
            if token.ends_with('\n') {
                lines.push(Line::new());
            }
        }
    }
    lines
}

fn number(value: Option<&str>) -> Option<f64> {
    value?.parse::<f64>().ok().filter(|n| n.is_finite() && n.abs() <= 200_000_000.0)
}

fn drawing_shape(drawing: Node<'_, '_>) -> Result<Shape, CreateError> {
    let unsupported = || CreateError::UnsupportedDocxDrawing;
    let anchor = child(drawing, "anchor").ok_or_else(unsupported)?;
    let horizontal = child(anchor, "positionH").ok_or_else(unsupported)?;
    let vertical = child(anchor, "positionV").ok_or_else(unsupported)?;
    let x_from_page = match attr(horizontal, "relativeFrom") {
        Some("page") => true,
        Some("margin") => false,
        _ => return Err(unsupported()),
    };
    let y_from_page = match attr(vertical, "relativeFrom") {
        Some("page") => true,
        Some("paragraph") => false,
        _ => return Err(unsupported()),
    };
    let offset = |node: Node<'_, '_>| child(node, "posOffset").and_then(|n| number(n.text())).map(|n| n / 12_700.0);
    let x = offset(horizontal).ok_or_else(unsupported)?;
    let y = offset(vertical).ok_or_else(unsupported)?;
    let extent = child(anchor, "extent").ok_or_else(unsupported)?;
    let width = number(attr(extent, "cx")).map(|n| n / 12_700.0).filter(|n| *n > 0.0).ok_or_else(unsupported)?;
    let height = number(attr(extent, "cy")).map(|n| n / 12_700.0).filter(|n| *n > 0.0).ok_or_else(unsupported)?;
    let graphic_data = anchor.descendants().find(|n| n.is_element() && n.tag_name().name() == "graphicData").ok_or_else(unsupported)?;
    if attr(graphic_data, "uri") != Some("http://schemas.microsoft.com/office/word/2010/wordprocessingShape") {
        return Err(unsupported());
    }
    let wsp = child(graphic_data, "wsp").ok_or_else(unsupported)?;
    if wsp.descendants().any(|n| n.is_element() && matches!(n.tag_name().name(), "txbx" | "txbxContent" | "blip" | "imagedata")) {
        return Err(unsupported());
    }
    let props = child(wsp, "spPr").ok_or_else(unsupported)?;
    if child(props, "xfrm").and_then(|n| attr(n, "rot")).is_some_and(|v| v != "0") {
        return Err(unsupported());
    }
    let fill = child(props, "solidFill");
    let stroke = child(props, "ln").and_then(|n| child(n, "solidFill"));
    let filled = fill.is_some();
    let color = fill.or(stroke).and_then(|n| child(n, "srgbClr")).and_then(|n| attr(n, "val")).and_then(rgb).ok_or_else(unsupported)?;
    let path = child(props, "custGeom").and_then(|n| child(n, "pathLst")).and_then(|n| child(n, "path")).ok_or_else(unsupported)?;
    let path_w = number(attr(path, "w")).filter(|n| *n > 0.0).ok_or_else(unsupported)?;
    let path_h = number(attr(path, "h")).filter(|n| *n > 0.0).ok_or_else(unsupported)?;
    let mut points = Vec::new();
    for command in path.children().filter(|n| n.is_element()) {
        if !matches!(command.tag_name().name(), "moveTo" | "lnTo" | "close") || points.len() >= 256 {
            return Err(unsupported());
        }
        if command.tag_name().name() == "close" {
            continue;
        }
        let point = child(command, "pt").ok_or_else(unsupported)?;
        let px = number(attr(point, "x")).ok_or_else(unsupported)? / path_w * width;
        let py = number(attr(point, "y")).ok_or_else(unsupported)? / path_h * height;
        if !px.is_finite() || !py.is_finite() {
            return Err(unsupported());
        }
        points.push((px, py));
    }
    if points.len() < 2 {
        return Err(unsupported());
    }
    Ok(Shape { x, y, x_from_page, y_from_page, color, filled, points })
}

struct Painter {
    pages: Vec<Vec<u8>>,
    page_w: f64,
    page_h: f64,
    margins: [f64; 4], // top, right, bottom, left
    y: f64,
}
impl Painter {
    fn new(page_w: f64, page_h: f64, margins: [f64; 4]) -> Self {
        Self { pages: vec![Vec::new()], page_w, page_h, margins, y: margins[0] }
    }
    fn page(&mut self) -> Result<(), CreateError> {
        if self.pages.len() >= MAX_PAGES {
            return Err(bad("DOCX exceeds the 500-page conversion limit"));
        }
        self.pages.push(Vec::new());
        self.y = self.margins[0];
        Ok(())
    }
    fn ensure(&mut self, height: f64) -> Result<(), CreateError> {
        if self.y + height > self.page_h - self.margins[2] && self.y > self.margins[0] {
            self.page()?;
        }
        Ok(())
    }
    fn push(&mut self, bytes: &[u8]) {
        if let Some(page) = self.pages.last_mut() {
            page.extend_from_slice(bytes);
        }
    }
    fn draw_lines(&mut self, lines: &[Line], para: ParaStyle, x: f64, width: f64, top: f64) -> Result<f64, CreateError> {
        let mut y = top;
        for line in lines {
            let h = (line.size * 1.22).max(8.0);
            let indent = match para.align {
                Alignment::Left => 0.0,
                Alignment::Center => (width - line.width).max(0.0) / 2.0,
                Alignment::Right => (width - line.width).max(0.0),
            };
            let mut xx = x + indent;
            for run in &line.runs {
                if run.text.chars().any(|ch| ch != '?' && win_ansi(&ch.to_string()) == b"?") {
                    return Err(bad("DOCX contains a character unavailable in the native PDF font; conversion stopped to avoid changing the text"));
                }
                let style = run.style;
                let mut command = format!(
                    "BT /{} {:.2} Tf {:.3} {:.3} {:.3} rg 1 0 0 1 {:.3} {:.3} Tm ",
                    style.font(),
                    style.size,
                    style.color[0],
                    style.color[1],
                    style.color[2],
                    xx,
                    self.page_h - y - style.size
                )
                .into_bytes();
                command.extend(literal(&win_ansi(&run.text)));
                command.extend_from_slice(b" Tj ET\n");
                self.push(&command);
                xx += helvetica_width(&run.text, style.size);
            }
            y += h;
        }
        Ok(y - top)
    }
    fn paragraph(&mut self, paragraph: &Paragraph) -> Result<(), CreateError> {
        if paragraph.page_break || paragraph.para.page_break {
            self.page()?;
        }
        let x = self.margins[3] + paragraph.para.left;
        let width = (self.page_w - self.margins[1] - x - paragraph.para.right).max(24.0);
        let lines = layout(paragraph, width);
        let height = lines.iter().map(|l| (l.size * 1.22).max(8.0)).sum::<f64>();
        self.ensure(paragraph.para.before + height + paragraph.para.after)?;
        self.y += paragraph.para.before;
        for shape in &paragraph.shapes {
            self.draw_shape(shape);
        }
        self.draw_lines(&lines, paragraph.para, x, width, self.y)?;
        self.y += height + paragraph.para.after;
        Ok(())
    }
    fn draw_shape(&mut self, shape: &Shape) {
        let x = shape.x + if shape.x_from_page { 0.0 } else { self.margins[3] };
        let y = shape.y + if shape.y_from_page { 0.0 } else { self.y };
        let mut command = format!("q {:.3} {:.3} {:.3} {} ", shape.color[0], shape.color[1], shape.color[2], if shape.filled { "rg" } else { "RG" });
        for (index, (px, py)) in shape.points.iter().enumerate() {
            command.push_str(&format!("{:.3} {:.3} {} ", x + px, self.page_h - y - py, if index == 0 { "m" } else { "l" }));
        }
        command.push_str(if shape.filled { "h f Q\n" } else { "0.75 w S Q\n" });
        self.push(command.as_bytes());
    }
    fn line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) {
        self.push(format!("0.65 G 0.55 w {:.3} {:.3} m {:.3} {:.3} l S\n", x0, self.page_h - y0, x1, self.page_h - y1).as_bytes());
    }
    fn finish(self, title: &str) -> Result<Document, CreateError> {
        let mut doc = Document::new_empty();
        let mut fonts = Dict::new();
        for (key, face) in [("F1", "Helvetica"), ("F2", "Helvetica-Bold"), ("F3", "Helvetica-Oblique"), ("F4", "Helvetica-BoldOblique")] {
            let mut font = Dict::new();
            font.set(b"Type".to_vec(), Object::name("Font"));
            font.set(b"Subtype".to_vec(), Object::name("Type1"));
            font.set(b"BaseFont".to_vec(), Object::name(face));
            font.set(b"Encoding".to_vec(), Object::name("WinAnsiEncoding"));
            let reference = doc.add(font);
            fonts.set(key.as_bytes().to_vec(), Object::Ref(reference));
        }
        for content in self.pages {
            let mut resources = Dict::new();
            resources.set(b"Font".to_vec(), Object::Dict(fonts.clone()));
            add_page(&mut doc, self.page_w, self.page_h, resources, Some(content))?;
        }
        set_title(&mut doc, title);
        Ok(doc)
    }
}

fn paragraph(
    node: Node<'_, '_>,
    styles: Option<&roxmltree::Document<'_>>,
    defs: &HashMap<String, StyleDef>,
    default: TextStyle,
) -> Result<Paragraph, CreateError> {
    let props = child(node, "pPr");
    let mut text_style = default;
    let mut para = ParaStyle::default();
    if let (Some(style_doc), Some(id)) = (styles, props.and_then(|p| value(p, "pStyle"))) {
        let mut chain = Vec::new();
        let mut next = Some(id.to_owned());
        for _ in 0..8 {
            let Some(current) = next.take() else { break };
            if chain.contains(&current) {
                break;
            }
            next = defs.get(&current).and_then(|d| d.based_on.clone());
            chain.push(current);
        }
        for name in chain.iter().rev() {
            if let Some(def) = defs.get(name) {
                if let Some(p) = def.para.and_then(|id| style_doc.get_node(id)) {
                    para.apply(p);
                }
                if let Some(r) = def.text.and_then(|id| style_doc.get_node(id)) {
                    text_style.apply(r);
                }
            }
        }
    }
    if let Some(p) = props {
        para.apply(p);
    }
    let mut runs = Vec::new();
    let mut page_break = false;
    for run in node.descendants().filter(|n| n.is_element() && n.tag_name().name() == "r") {
        let mut style = text_style;
        if let Some(props) = child(run, "rPr") {
            style.apply(props);
        }
        let mut text = String::new();
        for item in run.descendants().filter(|n| n.is_element()) {
            match item.tag_name().name() {
                "t" => text.push_str(item.text().unwrap_or_default()),
                "tab" => text.push_str("    "),
                "br" if attr(item, "type") == Some("page") => page_break = true,
                "br" | "cr" => text.push('\n'),
                _ => {}
            }
        }
        if !text.is_empty() {
            runs.push(Run { text, style });
        }
    }
    let shapes =
        node.descendants().filter(|n| n.is_element() && n.tag_name().name() == "drawing").map(drawing_shape).collect::<Result<Vec<_>, _>>()?;
    Ok(Paragraph { runs, para, page_break, shapes })
}

fn table(
    painter: &mut Painter,
    node: Node<'_, '_>,
    styles: Option<&roxmltree::Document<'_>>,
    defs: &HashMap<String, StyleDef>,
    default: TextStyle,
) -> Result<(), CreateError> {
    let rows: Vec<_> = node.children().filter(|n| n.is_element() && n.tag_name().name() == "tr").collect();
    if rows.len() > 10_000 {
        return Err(bad("DOCX table has too many rows"));
    }
    let grid: Vec<f64> = child(node, "tblGrid")
        .into_iter()
        .flat_map(|g| g.children())
        .filter(|n| n.is_element() && n.tag_name().name() == "gridCol")
        .filter_map(|n| twips(attr(n, "w")))
        .collect();
    let columns =
        grid.len().max(rows.iter().map(|r| r.children().filter(|c| c.is_element() && c.tag_name().name() == "tc").count()).max().unwrap_or(1));
    if columns > 64 {
        return Err(bad("DOCX table has too many columns"));
    }
    let content_width = painter.page_w - painter.margins[1] - painter.margins[3];
    let natural = grid.iter().sum::<f64>();
    let widths: Vec<f64> = if grid.len() == columns && natural > 1.0 {
        grid.iter().map(|w| w / natural * content_width).collect()
    } else {
        vec![content_width / columns as f64; columns]
    };
    let bordered = child(node, "tblPr").and_then(|p| child(p, "tblBorders")).is_some();
    let x0 = painter.margins[3];
    for row in rows {
        let cells: Vec<_> = row.children().filter(|n| n.is_element() && n.tag_name().name() == "tc").collect();
        let mut contents = Vec::new();
        let mut index = 0usize;
        let mut height: f64 = 18.0;
        for cell in cells {
            if index >= widths.len() {
                break;
            }
            let span = child(cell, "tcPr")
                .and_then(|p| value(p, "gridSpan"))
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(1)
                .clamp(1, widths.len() - index);
            let cell_w: f64 = widths.iter().skip(index).take(span).sum();
            let ps: Vec<_> = cell
                .children()
                .filter(|p| p.is_element() && p.tag_name().name() == "p")
                .map(|p| paragraph(p, styles, defs, default))
                .collect::<Result<Vec<_>, _>>()?;
            if ps.iter().any(|p| !p.shapes.is_empty()) {
                return Err(bad("DOCX drawings inside table cells are not supported yet"));
            }
            let needed: f64 =
                ps.iter().map(|p| layout(p, cell_w - 8.0).iter().map(|l| (l.size * 1.22).max(8.0)).sum::<f64>() + p.para.before + p.para.after).sum();
            height = height.max(needed + 6.0);
            contents.push((index, span, cell_w, ps));
            index += span;
        }
        if let Some(min) = child(row, "trPr").and_then(|p| child(p, "trHeight")).and_then(|n| twips(attr(n, "val"))) {
            height = height.max(min);
        }
        height = height.min((painter.page_h - painter.margins[0] - painter.margins[2]).max(20.0));
        painter.ensure(height)?;
        let y = painter.y;
        if bordered {
            painter.line(x0, y, x0 + content_width, y);
            painter.line(x0, y + height, x0 + content_width, y + height);
            painter.line(x0, y, x0, y + height);
            painter.line(x0 + content_width, y, x0 + content_width, y + height);
        }
        for (start, span, cell_w, ps) in contents {
            let x = x0 + widths.iter().take(start).sum::<f64>();
            if bordered && start > 0 {
                painter.line(x, y, x, y + height);
            }
            let mut top = y + 3.0;
            for p in ps {
                top += p.para.before;
                let lines = layout(&p, cell_w - 8.0);
                top += painter.draw_lines(&lines, p.para, x + 4.0, cell_w - 8.0, top)? + p.para.after;
            }
            let _ = span;
        }
        painter.y += height;
    }
    Ok(())
}

/// Convert common DOCX paragraphs and tables to a PDF document entirely in Rust.
pub fn from_docx_layout(title: &str, bytes: &[u8]) -> Result<Document, CreateError> {
    let xml = part(bytes, b"word/document.xml", MAX_XML)?.ok_or_else(|| bad("DOCX has no word/document.xml part"))?;
    let xml = std::str::from_utf8(&xml).map_err(|_| bad("DOCX document XML is not UTF-8"))?;
    let document = roxmltree::Document::parse_with_options(xml, roxmltree::ParsingOptions { allow_dtd: false, ..Default::default() })
        .map_err(|e| bad(format!("invalid DOCX XML: {e}")))?;
    for node in document.descendants().filter(|n| n.is_element()) {
        match node.tag_name().name() {
            "drawing" => {
                let _ = drawing_shape(node)?;
            }
            "pict" => {
                let has_drawing_choice = node
                    .ancestors()
                    .find(|ancestor| ancestor.is_element() && ancestor.tag_name().name() == "Fallback")
                    .and_then(|fallback| fallback.parent())
                    .filter(|parent| parent.is_element() && parent.tag_name().name() == "AlternateContent")
                    .is_some_and(|alternate| {
                        alternate.children().any(|choice| {
                            choice.is_element()
                                && choice.tag_name().name() == "Choice"
                                && choice.descendants().any(|descendant| descendant.is_element() && descendant.tag_name().name() == "drawing")
                        })
                    });
                if !has_drawing_choice {
                    return Err(CreateError::UnsupportedDocxDrawing);
                }
            }
            "object" => return Err(bad("DOCX embedded objects are not supported yet")),
            _ => {}
        }
    }
    let style_bytes = part(bytes, b"word/styles.xml", MAX_XML)?;
    let style_text = style_bytes.as_deref().map(std::str::from_utf8).transpose().map_err(|_| bad("DOCX styles XML is not UTF-8"))?;
    let styles = style_text
        .map(|s| roxmltree::Document::parse_with_options(s, roxmltree::ParsingOptions { allow_dtd: false, ..Default::default() }))
        .transpose()
        .map_err(|e| bad(format!("invalid DOCX styles XML: {e}")))?;
    let mut defs = HashMap::new();
    let mut default = TextStyle::default();
    if let Some(doc) = &styles {
        if let Some(rpr) = doc.descendants().find(|n| n.is_element() && n.tag_name().name() == "rPrDefault").and_then(|n| child(n, "rPr")) {
            default.apply(rpr);
        }
        for style in doc.descendants().filter(|n| n.is_element() && n.tag_name().name() == "style").take(1024) {
            if let Some(id) = attr(style, "styleId") {
                defs.insert(
                    id.to_owned(),
                    StyleDef {
                        based_on: value(style, "basedOn").map(str::to_owned),
                        text: child(style, "rPr").map(|n| n.id()),
                        para: child(style, "pPr").map(|n| n.id()),
                    },
                );
            }
        }
    }
    let body = document.descendants().find(|n| n.is_element() && n.tag_name().name() == "body").ok_or_else(|| bad("DOCX has no document body"))?;
    let section = body.descendants().find(|n| n.is_element() && n.tag_name().name() == "sectPr");
    let size = section.and_then(|s| child(s, "pgSz"));
    let page_w = size.and_then(|n| twips(attr(n, "w"))).unwrap_or(612.0);
    let page_h = size.and_then(|n| twips(attr(n, "h"))).unwrap_or(792.0);
    if !(72.0..=14_400.0).contains(&page_w) || !(72.0..=14_400.0).contains(&page_h) {
        return Err(bad("DOCX page size is outside PDF limits"));
    }
    let margin = section.and_then(|s| child(s, "pgMar"));
    let margins = ["top", "right", "bottom", "left"].map(|side| margin.and_then(|n| twips(attr(n, side))).unwrap_or(72.0).clamp(0.0, 400.0));
    if margins[1] + margins[3] >= page_w - 20.0 || margins[0] + margins[2] >= page_h - 20.0 {
        return Err(bad("DOCX page margins leave no printable area"));
    }
    let mut painter = Painter::new(page_w, page_h, margins);
    let mut count = 0usize;
    for node in body.children().filter(|n| n.is_element()) {
        match node.tag_name().name() {
            "p" => {
                painter.paragraph(&paragraph(node, styles.as_ref(), &defs, default)?)?;
                count += 1;
            }
            "tbl" => {
                table(&mut painter, node, styles.as_ref(), &defs, default)?;
                count += 1;
            }
            _ => {}
        }
        if count > MAX_BLOCKS {
            return Err(bad("DOCX has too many document blocks"));
        }
    }
    if count == 0 {
        return Err(bad("DOCX has no supported paragraphs or tables"));
    }
    painter.finish(title)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(xml: &str) -> Vec<u8> {
        let mut zip = karnoxpdf_export::Zip::default();
        zip.add("word/document.xml", xml.as_bytes(), true);
        zip.finish()
    }

    #[test]
    fn unsupported_drawing_fails_without_dropping_content() {
        let bytes = package(r#"<w:document xmlns:w="urn:word"><w:body><w:p><w:r><w:t>Invoice</w:t><w:drawing/></w:r></w:p></w:body></w:document>"#);
        assert_eq!(from_docx_layout("Invoice", &bytes).err(), Some(CreateError::UnsupportedDocxDrawing));
    }

    #[test]
    fn alternate_content_shape_is_drawn_once() {
        let xml = r##"<w:document xmlns:w="urn:word" xmlns:wp="urn:position" xmlns:a="urn:drawing" xmlns:wps="urn:shape" xmlns:mc="urn:compat"><w:body><w:p><w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:anchor><wp:positionH relativeFrom="page"><wp:posOffset>685800</wp:posOffset></wp:positionH><wp:positionV relativeFrom="paragraph"><wp:posOffset>428625</wp:posOffset></wp:positionV><wp:extent cx="6032500" cy="6350"/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp><wps:spPr><a:custGeom><a:pathLst><a:path w="6032500" h="6350"><a:moveTo><a:pt x="0" y="0"/></a:moveTo><a:lnTo><a:pt x="6032500" y="0"/></a:lnTo><a:lnTo><a:pt x="6032500" y="6350"/></a:lnTo><a:lnTo><a:pt x="0" y="6350"/></a:lnTo><a:close/></a:path></a:pathLst></a:custGeom><a:solidFill><a:srgbClr val="566D80"/></a:solidFill></wps:spPr></wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></mc:Choice><mc:Fallback><w:pict><v:shape xmlns:v="urn:vml"/></w:pict></mc:Fallback></mc:AlternateContent></w:r></w:p></w:body></w:document>"##;
        let doc = from_docx_layout("line", &package(xml)).unwrap();
        let root = doc.root().unwrap();
        let pages = doc.get(root).as_dict().unwrap().reference(b"Pages").unwrap();
        let first = doc.get(pages).as_dict().unwrap().get(b"Kids").unwrap().as_array().unwrap()[0].as_ref().unwrap();
        let content = doc.get(first).as_dict().unwrap().reference(b"Contents").unwrap();
        let object = doc.get(content);
        let Object::Stream(stream) = &*object else { panic!("expected a PDF content stream") };
        let source = String::from_utf8(stream.decoded().unwrap()).unwrap();
        assert_eq!(source.matches("h f Q").count(), 1, "the VML fallback must not render twice");
        assert!(source.contains("0.337 0.427 0.502 rg"), "{source}");
    }

    #[test]
    fn malformed_package_and_page_size_are_errors() {
        assert!(from_docx_layout("bad", b"not a docx").is_err());
        let bytes = package(
            r#"<w:document xmlns:w="urn:word"><w:body><w:p><w:r><w:t>Invoice</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="0" w:h="0"/></w:sectPr></w:body></w:document>"#,
        );
        assert!(from_docx_layout("bad", &bytes).is_err());
    }
}
