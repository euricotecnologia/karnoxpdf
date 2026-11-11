//! Table styles (`ppt/tableStyles.xml`, ISO/IEC 29500-1 §20.1.4.2): which fill, text and borders
//! a cell takes from the style its table names.

use std::collections::HashMap;

use roxmltree::Node;

use crate::drawingml::{ColorSpec, FillSpec, LineSpec, color_in, parse_fill, parse_line};
use crate::ooxml::{attr, child, children};

/// What one part of a table style (`wholeTbl`, `firstRow`, ...) says about its cells.
#[derive(Clone, Debug, Default)]
pub(crate) struct StylePart {
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub color: Option<ColorSpec>,
    pub fill: Option<FillSpec>,
    /// left, right, top, bottom, insideH, insideV
    pub borders: [Option<LineSpec>; 6],
}

pub(crate) const LEFT: usize = 0;
pub(crate) const RIGHT: usize = 1;
pub(crate) const TOP: usize = 2;
pub(crate) const BOTTOM: usize = 3;
pub(crate) const INSIDE_H: usize = 4;
pub(crate) const INSIDE_V: usize = 5;

impl StylePart {
    fn over(&mut self, upper: &StylePart) {
        if upper.bold.is_some() {
            self.bold = upper.bold;
        }
        if upper.italic.is_some() {
            self.italic = upper.italic;
        }
        if upper.color.is_some() {
            self.color = upper.color.clone();
        }
        if upper.fill.is_some() {
            self.fill = upper.fill.clone();
        }
        for (mine, theirs) in self.borders.iter_mut().zip(&upper.borders) {
            if theirs.is_some() {
                *mine = theirs.clone();
            }
        }
    }
}

/// Which parts of a table the author switched on (`a:tblPr`).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct TableFlags {
    pub first_row: bool,
    pub last_row: bool,
    pub first_col: bool,
    pub last_col: bool,
    pub band_row: bool,
    pub band_col: bool,
}

#[derive(Debug, Default)]
pub(crate) struct TableStyles {
    styles: HashMap<String, HashMap<String, StylePart>>,
}

fn on_off(node: Node<'_, '_>, name: &str) -> Option<bool> {
    attr(node, name).map(|v| v == "on")
}

fn parse_part(node: Node<'_, '_>) -> StylePart {
    let mut part = StylePart::default();
    if let Some(text) = child(node, "tcTxStyle") {
        part.bold = on_off(text, "b");
        part.italic = on_off(text, "i");
        part.color = color_in(text).or_else(|| child(text, "fontRef").and_then(color_in));
    }
    if let Some(cell) = child(node, "tcStyle") {
        if let Some(fill) = child(cell, "fill") {
            part.fill = parse_fill(fill);
        }
        if let Some(borders) = child(cell, "tcBdr") {
            for (slot, name) in ["left", "right", "top", "bottom", "insideH", "insideV"].iter().enumerate() {
                if let Some(ln) = child(borders, name).and_then(|b| child(b, "ln")) {
                    part.borders[slot] = Some(parse_line(ln));
                }
            }
        }
    }
    part
}

impl TableStyles {
    pub(crate) fn parse(doc: &roxmltree::Document<'_>) -> TableStyles {
        let root = doc.root_element();
        let mut out = TableStyles::default();
        for style in children(root, "tblStyle").take(256) {
            let Some(id) = attr(style, "styleId") else { continue };
            let parts = style.children().filter(|n| n.is_element()).map(|p| (p.tag_name().name().to_owned(), parse_part(p))).collect();
            out.styles.insert(id.to_owned(), parts);
        }
        out
    }

    pub(crate) fn knows(&self, id: &str) -> bool {
        self.styles.contains_key(id)
    }

    /// The style a cell at `(row, col)` of a `rows` × `cols` table gets: the parts that apply,
    /// from the whole table up to the header row, each over the one before.
    pub(crate) fn cell(&self, id: &str, flags: TableFlags, (row, col): (usize, usize), (rows, cols): (usize, usize)) -> Option<StylePart> {
        let parts = self.styles.get(id)?;
        let mut applicable: Vec<&str> = vec!["wholeTbl"];
        if flags.band_row {
            let index = row.saturating_sub(usize::from(flags.first_row));
            if !(flags.first_row && row == 0) {
                applicable.push(if index % 2 == 0 { "band1H" } else { "band2H" });
            }
        }
        if flags.band_col {
            let index = col.saturating_sub(usize::from(flags.first_col));
            if !(flags.first_col && col == 0) {
                applicable.push(if index % 2 == 0 { "band1V" } else { "band2V" });
            }
        }
        if flags.last_col && col + 1 == cols {
            applicable.push("lastCol");
        }
        if flags.first_col && col == 0 {
            applicable.push("firstCol");
        }
        if flags.last_row && row + 1 == rows {
            applicable.push("lastRow");
        }
        if flags.first_row && row == 0 {
            applicable.push("firstRow");
        }
        let mut merged = StylePart::default();
        for name in applicable {
            if let Some(part) = parts.get(name) {
                merged.over(part);
            }
        }
        Some(merged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STYLES: &str = r#"<a:tblStyleLst xmlns:a="urn:a" def="{S}"><a:tblStyle styleId="{S}" styleName="Medium 2"><a:wholeTbl><a:tcTxStyle><a:fontRef idx="minor"/><a:schemeClr val="dk1"/></a:tcTxStyle><a:tcStyle><a:tcBdr><a:left><a:ln w="12700"><a:solidFill><a:schemeClr val="lt1"/></a:solidFill></a:ln></a:left></a:tcBdr><a:fill><a:solidFill><a:schemeClr val="accent1"><a:tint val="20000"/></a:schemeClr></a:solidFill></a:fill></a:tcStyle></a:wholeTbl><a:band1H><a:tcStyle><a:tcBdr/><a:fill><a:solidFill><a:schemeClr val="accent1"><a:tint val="40000"/></a:schemeClr></a:solidFill></a:fill></a:tcStyle></a:band1H><a:firstRow><a:tcTxStyle b="on"><a:fontRef idx="minor"/><a:schemeClr val="lt1"/></a:tcTxStyle><a:tcStyle><a:tcBdr/><a:fill><a:solidFill><a:schemeClr val="accent1"/></a:solidFill></a:fill></a:tcStyle></a:firstRow></a:tblStyle></a:tblStyleLst>"#;

    fn styles() -> TableStyles {
        TableStyles::parse(&roxmltree::Document::parse(STYLES).unwrap())
    }

    #[test]
    fn header_row_overrides_banding_and_the_whole_table() {
        let styles = styles();
        let flags = TableFlags { first_row: true, band_row: true, ..TableFlags::default() };
        let header = styles.cell("{S}", flags, (0, 0), (3, 2)).unwrap();
        assert_eq!(header.bold, Some(true));
        let first = styles.cell("{S}", flags, (1, 0), (3, 2)).unwrap();
        let second = styles.cell("{S}", flags, (2, 0), (3, 2)).unwrap();
        assert_eq!(first.bold, None);
        // The first data row is the first band; the second falls back to the whole-table fill.
        let tint = |p: &StylePart| format!("{:?}", p.fill);
        assert_ne!(tint(&first), tint(&second));
        assert!(first.borders[LEFT].is_some(), "whole-table borders are kept");
    }

    #[test]
    fn unknown_styles_are_reported() {
        let styles = styles();
        assert!(styles.knows("{S}"));
        assert!(!styles.knows("{X}"));
        assert!(styles.cell("{X}", TableFlags::default(), (0, 0), (1, 1)).is_none());
    }
}
