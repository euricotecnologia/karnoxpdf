//! Editable spreadsheet export: detected tables and remaining text, one worksheet per page.

use std::fmt::Write;

use crate::{Page, Zip, esc, tables_with_2col_min_rows};

const MAX_PAGES: usize = 1024;
const MAX_ROWS: usize = 1_048_576;
const MAX_COLS: usize = 16_384;
const MAX_INPUT_BYTES: usize = 256 * 1024 * 1024;

/// Export extracted PDF tables and text to an OOXML workbook. Page artwork and exact PDF
/// positioning cannot be represented as spreadsheet cells.
pub fn xlsx(pages: &[Page]) -> Result<Vec<u8>, String> {
    if pages.is_empty() {
        return Err("The PDF has no pages to export.".into());
    }
    if pages.iter().all(|p| p.blocks.iter().all(|b| b.text.trim().is_empty())) {
        return Err("The PDF has no extractable text. Run OCR before exporting to Excel.".into());
    }
    if pages.len() > MAX_PAGES {
        return Err(format!("The PDF has too many pages for spreadsheet export (maximum {MAX_PAGES})."));
    }
    let input_bytes = pages
        .iter()
        .flat_map(|p| &p.blocks)
        .map(|b| b.text.len())
        .try_fold(0usize, |n, len| n.checked_add(len))
        .ok_or("The extracted text is too large to export.")?;
    if input_bytes > MAX_INPUT_BYTES {
        return Err("The extracted text is too large to export (maximum 256 MB).".into());
    }

    let mut zip = Zip::default();
    zip.add("[Content_Types].xml", format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/><Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/>{}</Types>",
        (1..=pages.len()).map(|i| format!("<Override PartName=\"/xl/worksheets/sheet{i}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>")).collect::<String>()
    ).as_bytes(), true);
    zip.add("_rels/.rels", b"<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>", true);
    let sheets = (1..=pages.len()).map(|i| format!("<sheet name=\"Page {i}\" sheetId=\"{i}\" r:id=\"rId{i}\"/>")).collect::<String>();
    zip.add("xl/workbook.xml", format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheets>{sheets}</sheets></workbook>").as_bytes(), true);
    let rels = (1..=pages.len()).map(|i| format!("<Relationship Id=\"rId{i}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet{i}.xml\"/>")).collect::<String>();
    zip.add("xl/_rels/workbook.xml.rels", format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{rels}<Relationship Id=\"rIdStyles\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/></Relationships>").as_bytes(), true);
    zip.add("xl/styles.xml", b"<?xml version=\"1.0\" encoding=\"UTF-8\"?><styleSheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><fonts count=\"2\"><font><sz val=\"11\"/><name val=\"Liberation Sans\"/></font><font><b/><sz val=\"11\"/><name val=\"Liberation Sans\"/></font></fonts><fills count=\"2\"><fill><patternFill patternType=\"none\"/></fill><fill><patternFill patternType=\"gray125\"/></fill></fills><borders count=\"1\"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs><cellXfs count=\"2\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/><xf numFmtId=\"0\" fontId=\"1\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/></cellXfs><cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles></styleSheet>", true);
    for (index, page) in pages.iter().enumerate() {
        let xml = worksheet(page)?;
        zip.add(&format!("xl/worksheets/sheet{}.xml", index + 1), xml.as_bytes(), true);
    }
    Ok(zip.finish())
}

fn worksheet(page: &Page) -> Result<String, String> {
    let (found, consumed) = tables_with_2col_min_rows(&page.blocks, 2);
    let mut rows: Vec<Vec<(String, bool)>> = Vec::new();
    // Detected table order follows the source page; each table gets a blank separator row.
    for table in &found {
        for cells in &table.rows {
            if rows.len() >= MAX_ROWS {
                return Err("The extracted content exceeds Excel's row limit.".into());
            }
            let mut row = Vec::new();
            for cell in cells {
                if row.len() >= MAX_COLS {
                    return Err("A table exceeds Excel's column limit.".into());
                }
                row.push((cell.text.clone(), cell.bold));
                for _ in 1..cell.span.min(MAX_COLS - row.len() + 1) {
                    row.push((String::new(), false));
                }
            }
            rows.push(row);
        }
        if rows.len() >= MAX_ROWS {
            return Err("The extracted content exceeds Excel's row limit.".into());
        }
        rows.push(Vec::new());
    }
    let mut remaining: Vec<_> = page
        .blocks
        .iter()
        .enumerate()
        .filter(|(i, b)| !consumed.get(*i).copied().unwrap_or(false) && !b.text.trim().is_empty())
        .map(|(_, b)| b)
        .collect();
    remaining.sort_by(|a, b| b.rect[3].total_cmp(&a.rect[3]).then_with(|| a.rect[0].total_cmp(&b.rect[0])));
    for block in remaining {
        for line in block.text.lines() {
            if rows.len() >= MAX_ROWS {
                return Err("The extracted content exceeds Excel's row limit.".into());
            }
            rows.push(vec![(line.to_owned(), block.bold)]);
        }
    }
    if rows.len() > MAX_ROWS {
        return Err("The extracted content exceeds Excel's row limit.".into());
    }
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetViews><sheetView workbookViewId=\"0\"/></sheetViews><sheetFormatPr defaultRowHeight=\"15\"/><cols><col min=\"1\" max=\"64\" width=\"22\" customWidth=\"1\"/></cols><sheetData>",
    );
    for (r, cells) in rows.iter().enumerate() {
        let _ = write!(xml, "<row r=\"{}\">", r + 1);
        for (c, (value, bold)) in cells.iter().enumerate() {
            if c >= MAX_COLS {
                return Err("A table exceeds Excel's column limit.".into());
            }
            if value.is_empty() {
                continue;
            }
            if value.chars().count() > 32_767 {
                return Err("A text cell exceeds Excel's 32,767-character limit.".into());
            }
            let reference = format!("{}{}", column(c), r + 1);
            let style = if *bold { " s=\"1\"" } else { "" };
            if let Some(n) = numeric(value) {
                let _ = write!(xml, "<c r=\"{reference}\"{style}><v>{n}</v></c>");
            } else {
                let _ = write!(xml, "<c r=\"{reference}\"{style} t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>", esc(value));
            }
        }
        xml.push_str("</row>");
    }
    xml.push_str("</sheetData></worksheet>");
    Ok(xml)
}

fn numeric(value: &str) -> Option<String> {
    let s = value.trim();
    // Excel stores numeric values with about 15 significant decimal digits. Keep long account
    // numbers, identifiers and high-precision amounts as text rather than silently rounding.
    if s.chars().filter(|c| c.is_ascii_digit()).count() > 15 {
        return None;
    }
    if s.is_empty() || s.starts_with('0') && s.len() > 1 && !s.starts_with("0.") || s.starts_with("-0") && s.len() > 2 && !s.starts_with("-0.") {
        return None;
    }
    if !s.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | '-')) {
        return None;
    }
    let n: f64 = s.parse().ok()?;
    if n.is_finite() { Some(n.to_string()) } else { None }
}

fn column(mut n: usize) -> String {
    let mut s = String::new();
    loop {
        s.insert(0, char::from(b'A' + (n % 26) as u8));
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exports_editable_cells_and_preserves_leading_zeroes() {
        let page = Page {
            blocks: vec![crate::Block { text: "Invoice\n00123\n42.5".into(), rect: [0.0, 0.0, 100.0, 40.0], size: 12.0, bold: true, italic: false }],
            ..Page::default()
        };
        let xml = worksheet(&page).unwrap();
        assert!(xml.contains("<v>42.5</v>"));
        assert!(xml.contains(">00123</t>"));
        assert!(xlsx(&[page]).unwrap().starts_with(b"PK"));
    }

    #[test]
    fn scanned_page_requires_ocr() {
        assert!(xlsx(&[Page { width: 612.0, height: 792.0, ..Page::default() }]).unwrap_err().contains("OCR"));
    }

    #[test]
    fn detected_table_populates_multiple_columns() {
        let cell =
            |text: &str, x: f64, y: f64| crate::Block { text: text.into(), rect: [x, y, x + 80.0, y + 12.0], size: 11.0, bold: false, italic: false };
        let page = Page {
            width: 612.0,
            height: 792.0,
            blocks: vec![cell("Item", 72.0, 620.0), cell("Price", 232.0, 620.0), cell("A", 72.0, 590.0), cell("42.5", 232.0, 590.0)],
            images: Vec::new(),
        };
        let xml = worksheet(&page).unwrap();
        assert!(xml.contains("r=\"A1\""), "{xml}");
        assert!(xml.contains("r=\"B1\""), "{xml}");
        assert!(xml.contains("r=\"B2\"><v>42.5</v>"), "{xml}");
    }

    #[test]
    fn long_identifiers_remain_exact_text() {
        assert_eq!(numeric("1234567890123456"), None);
        assert_eq!(numeric("123456789012345"), Some("123456789012345".into()));
    }
}
