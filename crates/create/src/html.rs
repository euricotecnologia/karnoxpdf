//! Native HTML to PDF conversion for KarnoxPDF (L4).
//!
//! Parses HTML documents (or fragments) and renders them into clean, multi-page vector PDFs
//! with headings, formatted text, lists, tables, blockquotes, code blocks, horizontal rules,
//! and page numbers.

use karnoxpdf_cos::{Dict, Document, Object};
use karnoxpdf_fonts::{helvetica_width, literal, win_ansi, wrap};

use crate::{CreateError, add_page, set_title};

/// Supported standard paper formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageFormat {
    A4,
    Letter,
    Legal,
    A3,
}

impl PageFormat {
    pub fn dimensions(self) -> (f64, f64) {
        match self {
            Self::A4 => (595.28, 841.89),
            Self::Letter => (612.0, 792.0),
            Self::Legal => (612.0, 1008.0),
            Self::A3 => (841.89, 1190.55),
        }
    }
}

/// Margin presets for generated pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarginKind {
    Standard, // 20 mm (~56.7 pt)
    Narrow,   // 10 mm (~28.3 pt)
    Wide,     // 30 mm (~85.0 pt)
    None,     // 5 mm (~14.2 pt)
}

impl MarginKind {
    pub fn margins(self) -> [f64; 4] {
        // [top, right, bottom, left] in points
        match self {
            Self::Standard => [56.7, 56.7, 56.7, 56.7],
            Self::Narrow => [28.3, 28.3, 28.3, 28.3],
            Self::Wide => [85.0, 85.0, 85.0, 85.0],
            Self::None => [14.2, 14.2, 14.2, 14.2],
        }
    }
}

/// Options for HTML to PDF rendering.
#[derive(Clone, Debug)]
pub struct HtmlOptions {
    pub title: Option<String>,
    pub format: PageFormat,
    pub landscape: bool,
    pub margins: MarginKind,
    pub base_font_size: f64,
    pub include_header: bool,
    pub include_footer: bool,
}

impl Default for HtmlOptions {
    fn default() -> Self {
        Self {
            title: None,
            format: PageFormat::A4,
            landscape: false,
            margins: MarginKind::Standard,
            base_font_size: 11.0,
            include_header: true,
            include_footer: true,
        }
    }
}

// ---------------------------------------------------------------------------
// HTML Entity Decoding & Parsing Helpers
// ---------------------------------------------------------------------------

fn decode_entities(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '&' {
            let mut entity = String::new();
            while let Some(&c) = chars.peek() {
                if c == ';' {
                    chars.next();
                    break;
                } else if c.is_alphanumeric() || c == '#' {
                    entity.push(chars.next().unwrap_or(' '));
                } else {
                    break;
                }
            }
            match entity.as_str() {
                "nbsp" => out.push(' '),
                "amp" => out.push('&'),
                "lt" => out.push('<'),
                "gt" => out.push('>'),
                "quot" => out.push('"'),
                "apos" | "#39" => out.push('\''),
                "copy" => out.push_str("(c)"),
                "bull" => out.push('•'),
                "mdash" => out.push('—'),
                "ndash" => out.push('–'),
                "euro" => out.push('€'),
                "pound" => out.push('£'),
                "yen" => out.push('¥'),
                "deg" => out.push('°'),
                s if s.starts_with('#') => {
                    if let Some(rest) = s.strip_prefix("#x") {
                        if let Ok(val) = u32::from_str_radix(rest, 16)
                            && let Some(c) = char::from_u32(val)
                        {
                            out.push(c);
                            continue;
                        }
                    } else if let Some(rest) = s.strip_prefix('#')
                        && let Ok(val) = rest.parse::<u32>()
                        && let Some(c) = char::from_u32(val)
                    {
                        out.push(c);
                        continue;
                    }
                    out.push('&');
                    out.push_str(&entity);
                    out.push(';');
                }
                _ => {
                    out.push('&');
                    out.push_str(&entity);
                    out.push(';');
                }
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn parse_color(style: &str) -> Option<[f64; 3]> {
    let lower = style.to_lowercase();
    for part in lower.split(';') {
        let trimmed = part.trim();
        if let Some(val) = trimmed.strip_prefix("color:") {
            let v = val.trim();
            if let Some(hex) = v.strip_prefix('#') {
                if hex.len() == 6 {
                    if let Ok(bytes) = u32::from_str_radix(hex, 16) {
                        return Some([((bytes >> 16) & 255) as f64 / 255.0, ((bytes >> 8) & 255) as f64 / 255.0, (bytes & 255) as f64 / 255.0]);
                    }
                } else if hex.len() == 3 {
                    let mut full = String::new();
                    for c in hex.chars() {
                        full.push(c);
                        full.push(c);
                    }
                    if let Ok(bytes) = u32::from_str_radix(&full, 16) {
                        return Some([((bytes >> 16) & 255) as f64 / 255.0, ((bytes >> 8) & 255) as f64 / 255.0, (bytes & 255) as f64 / 255.0]);
                    }
                }
            } else if v.starts_with("rgb(") && v.ends_with(')') {
                let inner = &v[4..v.len() - 1];
                let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
                if parts.len() == 3
                    && let (Ok(r), Ok(g), Ok(b)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>(), parts[2].parse::<f64>())
                {
                    return Some([(r / 255.0).clamp(0.0, 1.0), (g / 255.0).clamp(0.0, 1.0), (b / 255.0).clamp(0.0, 1.0)]);
                }
            } else {
                match v {
                    "red" => return Some([0.9, 0.1, 0.1]),
                    "blue" => return Some([0.1, 0.3, 0.8]),
                    "green" => return Some([0.1, 0.6, 0.2]),
                    "gray" | "grey" => return Some([0.45, 0.45, 0.45]),
                    "black" => return Some([0.0, 0.0, 0.0]),
                    "orange" => return Some([0.9, 0.5, 0.1]),
                    "purple" => return Some([0.5, 0.2, 0.7]),
                    _ => {}
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Document Intermediate Representation
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct TextRun {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub mono: bool,
    pub underline: bool,
    pub color: [f64; 3],
}

impl TextRun {
    pub fn plain(text: impl Into<String>) -> Self {
        Self { text: text.into(), bold: false, italic: false, mono: false, underline: false, color: [0.1, 0.1, 0.1] }
    }
}

#[derive(Clone, Debug)]
pub struct TableCell {
    pub runs: Vec<TextRun>,
    #[allow(dead_code)]
    pub is_header: bool,
}

#[derive(Clone, Debug)]
pub enum HtmlBlock {
    Heading { level: u8, runs: Vec<TextRun> },
    Paragraph(Vec<TextRun>),
    ListItem { ordered: bool, index: usize, runs: Vec<TextRun> },
    Blockquote(Vec<TextRun>),
    CodeBlock(Vec<String>),
    HorizontalRule,
    Table { headers: Vec<TableCell>, rows: Vec<Vec<TableCell>> },
}

// ---------------------------------------------------------------------------
// HTML Parser Implementation
// ---------------------------------------------------------------------------

pub struct HtmlParser {
    pub title: Option<String>,
    pub blocks: Vec<HtmlBlock>,
}

impl HtmlParser {
    pub fn parse(html: &str) -> Self {
        let mut title = None;
        let mut blocks = Vec::new();

        // 1. Strip script and style blocks
        let clean = Self::strip_tags_content(html, &["script", "style", "noscript"]);

        // 2. Tokenize into tags and text
        let mut i = 0;
        let chars: Vec<char> = clean.chars().collect();
        let len = chars.len();

        let mut current_runs: Vec<TextRun> = Vec::new();
        let mut in_bold: usize = 0;
        let mut in_italic: usize = 0;
        let mut in_underline: usize = 0;
        let mut in_code: usize = 0;
        let mut current_color = [0.1, 0.1, 0.1];
        let mut in_heading: Option<u8> = None;
        let mut in_blockquote = false;
        let mut in_pre = false;
        let mut pre_lines: Vec<String> = Vec::new();
        let mut list_stack: Vec<(bool, usize)> = Vec::new(); // (is_ordered, current_index)
        let mut in_li = false;

        // Table state
        let mut table_headers: Vec<TableCell> = Vec::new();
        let mut table_rows: Vec<Vec<TableCell>> = Vec::new();
        let mut current_row: Vec<TableCell> = Vec::new();
        let mut in_th = false;
        let mut in_td = false;
        let mut cell_runs: Vec<TextRun> = Vec::new();

        while i < len {
            if chars[i] == '<' {
                // Find tag close '>'
                let start = i + 1;
                let mut end = start;
                while end < len && chars[end] != '>' {
                    end += 1;
                }
                if end < len {
                    let tag_content: String = chars[start..end].iter().collect();
                    i = end + 1;

                    let tag_trimmed = tag_content.trim();
                    let is_closing = tag_trimmed.starts_with('/');
                    let tag_str = if is_closing { &tag_trimmed[1..] } else { tag_trimmed };
                    let tag_name = tag_str.split_whitespace().next().unwrap_or("").to_lowercase();

                    if !is_closing {
                        // Check for inline style color
                        if let Some(style_idx) = tag_str.find("style=") {
                            let rest = &tag_str[style_idx + 6..];
                            let quote = rest.chars().next().unwrap_or(' ');
                            if (quote == '"' || quote == '\'')
                                && let Some(close_idx) = rest[1..].find(quote)
                                && let Some(col) = parse_color(&rest[1..1 + close_idx])
                            {
                                current_color = col;
                            }
                        }
                    }

                    match tag_name.as_str() {
                        "title" => {
                            if !is_closing {
                                // Extract title until </title>
                                let mut t_end = i;
                                while t_end < len {
                                    if chars[t_end] == '<' {
                                        let look_ahead: String = chars[t_end..len.min(t_end + 8)].iter().collect();
                                        if look_ahead.to_lowercase().starts_with("</title>") {
                                            break;
                                        }
                                    }
                                    t_end += 1;
                                }
                                let t_text: String = chars[i..t_end].iter().collect();
                                title = Some(decode_entities(t_text.trim()));
                                i = t_end;
                            }
                        }
                        "b" | "strong" => {
                            if is_closing {
                                in_bold = in_bold.saturating_sub(1);
                            } else {
                                in_bold += 1;
                            }
                        }
                        "i" | "em" => {
                            if is_closing {
                                in_italic = in_italic.saturating_sub(1);
                            } else {
                                in_italic += 1;
                            }
                        }
                        "u" => {
                            if is_closing {
                                in_underline = in_underline.saturating_sub(1);
                            } else {
                                in_underline += 1;
                            }
                        }
                        "code" if !in_pre => {
                            if is_closing {
                                in_code = in_code.saturating_sub(1);
                            } else {
                                in_code += 1;
                            }
                        }
                        "pre" => {
                            if is_closing {
                                in_pre = false;
                                if !pre_lines.is_empty() {
                                    blocks.push(HtmlBlock::CodeBlock(std::mem::take(&mut pre_lines)));
                                }
                            } else {
                                in_pre = true;
                                pre_lines.clear();
                            }
                        }
                        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                            let lvl = tag_name.chars().nth(1).and_then(|c| c.to_digit(10)).unwrap_or(1) as u8;
                            if is_closing {
                                if !current_runs.is_empty() {
                                    blocks.push(HtmlBlock::Heading { level: in_heading.unwrap_or(lvl), runs: std::mem::take(&mut current_runs) });
                                }
                                in_heading = None;
                            } else {
                                if !current_runs.is_empty() {
                                    blocks.push(HtmlBlock::Paragraph(std::mem::take(&mut current_runs)));
                                }
                                in_heading = Some(lvl);
                            }
                        }
                        "p" | "div" | "section" | "article" => {
                            if is_closing {
                                if !current_runs.is_empty() {
                                    if in_blockquote {
                                        blocks.push(HtmlBlock::Blockquote(std::mem::take(&mut current_runs)));
                                    } else {
                                        blocks.push(HtmlBlock::Paragraph(std::mem::take(&mut current_runs)));
                                    }
                                }
                            } else if !current_runs.is_empty() {
                                blocks.push(HtmlBlock::Paragraph(std::mem::take(&mut current_runs)));
                            }
                        }
                        "blockquote" => {
                            if is_closing {
                                if !current_runs.is_empty() {
                                    blocks.push(HtmlBlock::Blockquote(std::mem::take(&mut current_runs)));
                                }
                                in_blockquote = false;
                            } else {
                                if !current_runs.is_empty() {
                                    blocks.push(HtmlBlock::Paragraph(std::mem::take(&mut current_runs)));
                                }
                                in_blockquote = true;
                            }
                        }
                        "ul" => {
                            if is_closing {
                                list_stack.pop();
                            } else {
                                list_stack.push((false, 0));
                            }
                        }
                        "ol" => {
                            if is_closing {
                                list_stack.pop();
                            } else {
                                list_stack.push((true, 0));
                            }
                        }
                        "li" => {
                            if is_closing {
                                if in_li && !current_runs.is_empty() {
                                    let (ordered, idx) = list_stack.last().copied().unwrap_or((false, 1));
                                    blocks.push(HtmlBlock::ListItem { ordered, index: idx, runs: std::mem::take(&mut current_runs) });
                                }
                                in_li = false;
                            } else {
                                if in_li && !current_runs.is_empty() {
                                    let (ordered, idx) = list_stack.last().copied().unwrap_or((false, 1));
                                    blocks.push(HtmlBlock::ListItem { ordered, index: idx, runs: std::mem::take(&mut current_runs) });
                                }
                                if let Some((_, count)) = list_stack.last_mut() {
                                    *count += 1;
                                }
                                in_li = true;
                            }
                        }
                        "hr" => {
                            if !current_runs.is_empty() {
                                blocks.push(HtmlBlock::Paragraph(std::mem::take(&mut current_runs)));
                            }
                            blocks.push(HtmlBlock::HorizontalRule);
                        }
                        "br" => {
                            if in_pre {
                                pre_lines.push(String::new());
                            } else {
                                current_runs.push(TextRun::plain("\n"));
                            }
                        }
                        "table" => {
                            if is_closing {
                                if !table_headers.is_empty() || !table_rows.is_empty() {
                                    blocks.push(HtmlBlock::Table {
                                        headers: std::mem::take(&mut table_headers),
                                        rows: std::mem::take(&mut table_rows),
                                    });
                                }
                            } else {
                                if !current_runs.is_empty() {
                                    blocks.push(HtmlBlock::Paragraph(std::mem::take(&mut current_runs)));
                                }
                                table_headers.clear();
                                table_rows.clear();
                            }
                        }
                        "tr" => {
                            if is_closing {
                                if !current_row.is_empty() {
                                    table_rows.push(std::mem::take(&mut current_row));
                                }
                            } else {
                                current_row.clear();
                            }
                        }
                        "th" => {
                            if is_closing {
                                table_headers.push(TableCell { runs: std::mem::take(&mut cell_runs), is_header: true });
                                in_th = false;
                            } else {
                                cell_runs.clear();
                                in_th = true;
                            }
                        }
                        "td" => {
                            if is_closing {
                                current_row.push(TableCell { runs: std::mem::take(&mut cell_runs), is_header: false });
                                in_td = false;
                            } else {
                                cell_runs.clear();
                                in_td = true;
                            }
                        }
                        _ => {}
                    }
                    continue;
                }
            }

            // Normal text extraction
            let start = i;
            while i < len && chars[i] != '<' {
                i += 1;
            }
            let raw_text: String = chars[start..i].iter().collect();
            let decoded = decode_entities(&raw_text);

            if in_pre {
                for line in decoded.lines() {
                    pre_lines.push(line.to_string());
                }
            } else if in_th || in_td {
                let trimmed = decoded.trim();
                if !trimmed.is_empty() {
                    cell_runs.push(TextRun {
                        text: trimmed.to_string(),
                        bold: in_bold > 0 || in_th,
                        italic: in_italic > 0,
                        mono: in_code > 0,
                        underline: in_underline > 0,
                        color: current_color,
                    });
                }
            } else {
                let mut text = decoded;
                if in_heading.is_none() && !in_li && !in_blockquote {
                    // Normalize excessive spaces in body text
                    text = text.replace(['\r', '\n', '\t'], " ");
                }
                if !text.is_empty() {
                    current_runs.push(TextRun {
                        text,
                        bold: in_bold > 0,
                        italic: in_italic > 0,
                        mono: in_code > 0,
                        underline: in_underline > 0,
                        color: current_color,
                    });
                }
            }
        }

        // Flush trailing runs
        if !current_runs.is_empty() {
            if in_blockquote {
                blocks.push(HtmlBlock::Blockquote(current_runs));
            } else {
                blocks.push(HtmlBlock::Paragraph(current_runs));
            }
        }

        Self { title, blocks }
    }

    fn strip_tags_content(html: &str, tag_names: &[&str]) -> String {
        let mut res = html.to_string();
        for &tag in tag_names {
            let open = format!("<{tag}");
            let close = format!("</{tag}>");
            while let Some(start) = res.to_lowercase().find(&open) {
                if let Some(end) = res.to_lowercase().find(&close)
                    && end >= start
                {
                    res.replace_range(start..end + close.len(), " ");
                    continue;
                }
                break;
            }
        }
        res
    }
}

// ---------------------------------------------------------------------------
// PDF Layout & Rendering Engine
// ---------------------------------------------------------------------------

struct PageState {
    stream: Vec<u8>,
}

pub struct HtmlRenderer {
    pages: Vec<PageState>,
    page_w: f64,
    page_h: f64,
    margins: [f64; 4], // top, right, bottom, left
    y: f64,
    base_font_size: f64,
    doc_title: String,
    include_header: bool,
    include_footer: bool,
}

impl HtmlRenderer {
    pub fn new(options: &HtmlOptions, title: &str) -> Self {
        let (mut pw, mut ph) = options.format.dimensions();
        if options.landscape {
            std::mem::swap(&mut pw, &mut ph);
        }
        let margins = options.margins.margins();
        Self {
            pages: vec![PageState { stream: Vec::new() }],
            page_w: pw,
            page_h: ph,
            margins,
            y: margins[0] + if options.include_header { 20.0 } else { 0.0 },
            base_font_size: options.base_font_size.clamp(6.0, 32.0),
            doc_title: title.to_string(),
            include_header: options.include_header,
            include_footer: options.include_footer,
        }
    }

    fn content_w(&self) -> f64 {
        (self.page_w - self.margins[1] - self.margins[3]).max(36.0)
    }

    fn available_bottom(&self) -> f64 {
        self.page_h - self.margins[2] - if self.include_footer { 24.0 } else { 0.0 }
    }

    fn new_page(&mut self) -> Result<(), CreateError> {
        if self.pages.len() >= 1000 {
            return Err(CreateError::Invalid("HTML document exceeded maximum page limit (1000 pages)".into()));
        }
        self.pages.push(PageState { stream: Vec::new() });
        self.y = self.margins[0] + if self.include_header { 20.0 } else { 0.0 };
        Ok(())
    }

    fn ensure_space(&mut self, height: f64) -> Result<(), CreateError> {
        if self.y + height > self.available_bottom() && self.y > self.margins[0] {
            self.new_page()?;
        }
        Ok(())
    }

    fn push_bytes(&mut self, b: &[u8]) {
        if let Some(p) = self.pages.last_mut() {
            p.stream.extend_from_slice(b);
        }
    }

    fn render_runs(
        &mut self,
        runs: &[TextRun],
        size: f64,
        line_height: f64,
        indent_x: f64,
        width: f64,
        bottom_margin: f64,
    ) -> Result<(), CreateError> {
        // Concatenate runs into words and lines
        let full_text: String = runs.iter().map(|r| r.text.as_str()).collect();
        let trimmed = full_text.trim();
        if trimmed.is_empty() {
            return Ok(());
        }

        let lines = wrap(trimmed, size, width);
        let _needed_h = (lines.len() as f64) * line_height + bottom_margin;
        self.ensure_space(line_height)?;

        let is_bold = runs.iter().any(|r| r.bold);
        let is_italic = runs.iter().any(|r| r.italic);
        let is_mono = runs.iter().any(|r| r.mono);
        let color = runs.first().map(|r| r.color).unwrap_or([0.1, 0.1, 0.1]);

        let font_tag = match (is_mono, is_bold, is_italic) {
            (true, true, _) => "F6",       // Courier-Bold
            (true, false, _) => "F5",      // Courier
            (false, true, true) => "F4",   // Helvetica-BoldOblique
            (false, false, true) => "F3",  // Helvetica-Oblique
            (false, true, false) => "F2",  // Helvetica-Bold
            (false, false, false) => "F1", // Helvetica
        };

        for line in lines {
            self.ensure_space(line_height)?;
            let x = self.margins[3] + indent_x;
            let pdf_y = self.page_h - self.y - size;

            let mut cmd =
                format!("BT /{font_tag} {size:.2} Tf {:.3} {:.3} {:.3} rg 1 0 0 1 {x:.2} {pdf_y:.2} Tm ", color[0], color[1], color[2]).into_bytes();
            cmd.extend(literal(&win_ansi(&line)));
            cmd.extend_from_slice(b" Tj ET\n");
            self.push_bytes(&cmd);

            self.y += line_height;
        }

        self.y += bottom_margin;
        Ok(())
    }

    pub fn render_block(&mut self, block: &HtmlBlock) -> Result<(), CreateError> {
        match block {
            HtmlBlock::Heading { level, runs } => {
                let size = match level {
                    1 => (self.base_font_size * 2.0).clamp(16.0, 36.0),
                    2 => (self.base_font_size * 1.5).clamp(14.0, 28.0),
                    3 => (self.base_font_size * 1.25).clamp(12.0, 22.0),
                    _ => (self.base_font_size * 1.1).clamp(11.0, 18.0),
                };
                let line_h = size * 1.3;
                let top_m = size * 0.6;
                let bottom_m = size * 0.4;
                self.ensure_space(top_m + line_h)?;
                self.y += top_m;

                let mut bold_runs = runs.clone();
                for r in &mut bold_runs {
                    r.bold = true;
                }
                let w = self.content_w();
                self.render_runs(&bold_runs, size, line_h, 0.0, w, bottom_m)?;
            }
            HtmlBlock::Paragraph(runs) => {
                let size = self.base_font_size;
                let line_h = size * 1.35;
                let w = self.content_w();
                self.render_runs(runs, size, line_h, 0.0, w, 8.0)?;
            }
            HtmlBlock::ListItem { ordered, index, runs } => {
                let size = self.base_font_size;
                let line_h = size * 1.35;
                let bullet = if *ordered { format!("{index}. ") } else { "•  ".to_string() };
                let bullet_w = helvetica_width(&bullet, size);
                let w = self.content_w() - 20.0;

                self.ensure_space(line_h)?;
                let x_bullet = self.margins[3] + 6.0;
                let pdf_y = self.page_h - self.y - size;

                let mut cmd = format!("BT /F1 {size:.2} Tf 0.2 0.2 0.2 rg 1 0 0 1 {x_bullet:.2} {pdf_y:.2} Tm ").into_bytes();
                cmd.extend(literal(&win_ansi(&bullet)));
                cmd.extend_from_slice(b" Tj ET\n");
                self.push_bytes(&cmd);

                self.render_runs(runs, size, line_h, 6.0 + bullet_w, w, 4.0)?;
            }
            HtmlBlock::Blockquote(runs) => {
                let size = self.base_font_size * 0.95;
                let line_h = size * 1.35;
                let w = self.content_w() - 24.0;
                let start_y = self.y;

                self.render_runs(runs, size, line_h, 20.0, w, 8.0)?;
                let end_y = self.y;
                let bar_h = (end_y - start_y - 4.0).max(12.0);

                // Draw vertical bar on the left
                let bar_x = self.margins[3] + 6.0;
                let bar_pdf_y = self.page_h - start_y - bar_h;
                let bar_cmd = format!("q 0.2 0.45 0.7 rg {bar_x:.2} {bar_pdf_y:.2} 3.0 {bar_h:.2} re f Q\n");
                self.push_bytes(bar_cmd.as_bytes());
            }
            HtmlBlock::CodeBlock(lines) => {
                let size = (self.base_font_size * 0.9).clamp(8.0, 14.0);
                let line_h = size * 1.3;
                let pad = 8.0;
                let block_h = (lines.len() as f64) * line_h + pad * 2.0;

                self.ensure_space(block_h.min(200.0))?;
                let x = self.margins[3];
                let w = self.content_w();

                // Background box
                let bg_y = self.page_h - self.y - block_h;
                let bg_cmd = format!("q 0.95 0.96 0.98 rg 0.85 0.87 0.9 RG 0.5 w {x:.2} {bg_y:.2} {w:.2} {block_h:.2} re B Q\n");
                self.push_bytes(bg_cmd.as_bytes());

                self.y += pad;
                for line in lines {
                    self.ensure_space(line_h)?;
                    let pdf_y = self.page_h - self.y - size;
                    let mut cmd = format!("BT /F5 {size:.2} Tf 0.15 0.15 0.15 rg 1 0 0 1 {:.2} {pdf_y:.2} Tm ", x + pad).into_bytes();
                    cmd.extend(literal(&win_ansi(line)));
                    cmd.extend_from_slice(b" Tj ET\n");
                    self.push_bytes(&cmd);
                    self.y += line_h;
                }
                self.y += pad + 8.0;
            }
            HtmlBlock::HorizontalRule => {
                self.ensure_space(16.0)?;
                let x = self.margins[3];
                let w = self.content_w();
                let pdf_y = self.page_h - self.y - 8.0;
                let cmd = format!("q 0.8 0.8 0.82 RG 0.75 w {x:.2} {pdf_y:.2} m {:.2} {pdf_y:.2} l S Q\n", x + w);
                self.push_bytes(cmd.as_bytes());
                self.y += 16.0;
            }
            HtmlBlock::Table { headers, rows } => {
                let size = (self.base_font_size * 0.92).clamp(8.0, 14.0);
                let row_h = size * 2.0;
                let num_cols = headers.len().max(rows.first().map_or(0, |r| r.len())).max(1);
                let w = self.content_w();
                let col_w = w / (num_cols as f64);

                // Draw header
                if !headers.is_empty() {
                    self.ensure_space(row_h)?;
                    let bg_y = self.page_h - self.y - row_h;
                    let bg_cmd = format!("q 0.92 0.94 0.97 rg 0.7 0.75 0.8 RG 0.5 w {:.2} {bg_y:.2} {w:.2} {row_h:.2} re B Q\n", self.margins[3]);
                    self.push_bytes(bg_cmd.as_bytes());

                    for (col_idx, th) in headers.iter().enumerate() {
                        let cell_x = self.margins[3] + (col_idx as f64) * col_w + 6.0;
                        let pdf_y = self.page_h - self.y - size - 4.0;
                        let text: String = th.runs.iter().map(|r| r.text.as_str()).collect();
                        let mut cmd = format!("BT /F2 {size:.2} Tf 0.1 0.15 0.3 rg 1 0 0 1 {cell_x:.2} {pdf_y:.2} Tm ").into_bytes();
                        cmd.extend(literal(&win_ansi(&text)));
                        cmd.extend_from_slice(b" Tj ET\n");
                        self.push_bytes(&cmd);
                    }
                    self.y += row_h;
                }

                // Draw data rows
                for (r_idx, row) in rows.iter().enumerate() {
                    self.ensure_space(row_h)?;
                    let bg_color = if r_idx % 2 == 0 { "0.99 0.99 1.0" } else { "0.96 0.97 0.99" };
                    let bg_y = self.page_h - self.y - row_h;
                    let bg_cmd = format!("q {bg_color} rg 0.85 0.87 0.9 RG 0.5 w {:.2} {bg_y:.2} {w:.2} {row_h:.2} re B Q\n", self.margins[3]);
                    self.push_bytes(bg_cmd.as_bytes());

                    for (col_idx, td) in row.iter().enumerate() {
                        let cell_x = self.margins[3] + (col_idx as f64) * col_w + 6.0;
                        let pdf_y = self.page_h - self.y - size - 4.0;
                        let text: String = td.runs.iter().map(|r| r.text.as_str()).collect();
                        let mut cmd = format!("BT /F1 {size:.2} Tf 0.1 0.1 0.1 rg 1 0 0 1 {cell_x:.2} {pdf_y:.2} Tm ").into_bytes();
                        cmd.extend(literal(&win_ansi(&text)));
                        cmd.extend_from_slice(b" Tj ET\n");
                        self.push_bytes(&cmd);
                    }
                    self.y += row_h;
                }
                self.y += 12.0;
            }
        }
        Ok(())
    }

    pub fn finish(mut self) -> Result<Document, CreateError> {
        let total_pages = self.pages.len();

        // 1. Draw header & footer on all pages
        for page_idx in 0..total_pages {
            let mut decoration = Vec::new();

            // Header (top)
            if self.include_header && !self.doc_title.is_empty() {
                let h_y = self.page_h - self.margins[0] + 12.0;
                let h_x = self.margins[3];
                let mut cmd = format!("BT /F1 8.5 Tf 0.5 0.5 0.5 rg 1 0 0 1 {h_x:.2} {h_y:.2} Tm ").into_bytes();
                cmd.extend(literal(&win_ansi(&self.doc_title)));
                cmd.extend_from_slice(b" Tj ET\n");
                decoration.extend(cmd);

                // Thin rule under header
                let rule_y = self.page_h - self.margins[0] + 6.0;
                let w = self.content_w();
                let rule = format!("q 0.85 0.85 0.87 RG 0.5 w {h_x:.2} {rule_y:.2} m {:.2} {rule_y:.2} l S Q\n", h_x + w);
                decoration.extend(rule.into_bytes());
            }

            // Footer (bottom)
            if self.include_footer {
                let f_text = format!("{} / {}", page_idx + 1, total_pages);
                let f_w = helvetica_width(&f_text, 9.0);
                let f_x = (self.page_w - f_w) / 2.0;
                let f_y = (self.margins[2] / 2.0).max(14.0);

                let mut cmd = format!("BT /F1 9.0 Tf 0.5 0.5 0.5 rg 1 0 0 1 {f_x:.2} {f_y:.2} Tm ").into_bytes();
                cmd.extend(literal(&win_ansi(&f_text)));
                cmd.extend_from_slice(b" Tj ET\n");
                decoration.extend(cmd);
            }

            if let Some(p) = self.pages.get_mut(page_idx) {
                p.stream.extend(decoration);
            }
        }

        // 2. Build final PDF Document
        let mut doc = Document::new_empty();
        let mut fonts = Dict::new();
        for (name, base) in [
            ("F1", "Helvetica"),
            ("F2", "Helvetica-Bold"),
            ("F3", "Helvetica-Oblique"),
            ("F4", "Helvetica-BoldOblique"),
            ("F5", "Courier"),
            ("F6", "Courier-Bold"),
        ] {
            let mut font = Dict::new();
            font.set(b"Type".to_vec(), Object::name("Font"));
            font.set(b"Subtype".to_vec(), Object::name("Type1"));
            font.set(b"BaseFont".to_vec(), Object::name(base));
            font.set(b"Encoding".to_vec(), Object::name("WinAnsiEncoding"));
            let fr = doc.add(font);
            fonts.set(name.as_bytes().to_vec(), Object::Ref(fr));
        }

        let mut res = Dict::new();
        res.set(b"Font".to_vec(), Object::Dict(fonts));

        for p in self.pages {
            add_page(&mut doc, self.page_w, self.page_h, res.clone(), Some(p.stream))?;
        }

        set_title(&mut doc, &self.doc_title);
        Ok(doc)
    }
}

/// Convert HTML text into a multi-page PDF document.
pub fn from_html(html: &str, options: &HtmlOptions) -> Result<Document, CreateError> {
    let parsed = HtmlParser::parse(html);
    let title = options.title.clone().or(parsed.title).unwrap_or_else(|| "HTML Document".to_string());

    let mut renderer = HtmlRenderer::new(options, &title);

    for block in &parsed.blocks {
        renderer.render_block(block)?;
    }

    renderer.finish()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_html_elements() {
        let sample = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test Document</title></head>
            <body>
                <h1>Main Heading</h1>
                <p>Hello <b>bold</b> and <i>italic</i> world &amp; more.</p>
                <hr>
                <ul>
                    <li>Item 1</li>
                    <li>Item 2</li>
                </ul>
                <blockquote>Important note here</blockquote>
                <table>
                    <tr><th>Name</th><th>Value</th></tr>
                    <tr><td>Alice</td><td>100</td></tr>
                </table>
            </body>
            </html>
        "#;
        let parser = HtmlParser::parse(sample);
        assert_eq!(parser.title.as_deref(), Some("Test Document"));
        assert!(!parser.blocks.is_empty());
    }

    #[test]
    fn render_html_to_valid_pdf() {
        let sample = r#"
            <h1>Sample Report</h1>
            <p>This is a paragraph rendered natively to PDF from HTML content.</p>
            <ul>
                <li>First bullet item</li>
                <li>Second bullet item</li>
            </ul>
            <pre>fn main() { println!("Hello world"); }</pre>
        "#;
        let options = HtmlOptions::default();
        let doc = from_html(sample, &options).expect("failed to convert HTML to PDF");
        let root = doc.root().expect("document has no root");
        let pages_ref = doc.get(root).as_dict().unwrap().reference(b"Pages").unwrap();
        let pages_obj = doc.get(pages_ref);
        let kids = pages_obj.as_dict().unwrap().get(b"Kids").unwrap().as_array().unwrap();
        assert!(!kids.is_empty(), "PDF has at least 1 page");
    }

    #[test]
    fn page_format_dimensions() {
        let (w, h) = PageFormat::A4.dimensions();
        assert!((w - 595.28).abs() < 0.1);
        assert!((h - 841.89).abs() < 0.1);

        let (w, h) = PageFormat::Letter.dimensions();
        assert_eq!((w, h), (612.0, 792.0));
    }
}
