//! Spreadsheet conversion tests on synthetic workbooks.

use karnoxpdf_cos::{Document, Object};

use crate::{CreateError, Locale, NoteKind, SheetOrientation, XlsxConversion, XlsxOptions, from_xlsx_layout};

const NS: &str = r#"xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#;

/// Fonts: 0 normal, 1 bold, 2 red italic underlined. Fills: 2 solid yellow. Borders: 1 thin box,
/// 2 double bottom. Cell formats (`s=`): 1 `0.00`, 2 date (14), 3 bold, 4 yellow fill, 5 box,
/// 6 wrapped, 7 centred, 8 `#,##0`, 9 right-aligned text, 10 red italic underlined, 11 percent,
/// 12 currency with a double blue bottom border.
const STYLES: &str = r#"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><numFmts count="1"><numFmt numFmtId="164" formatCode="&quot;R$&quot; #,##0.00"/></numFmts><fonts count="3"><font><sz val="11"/><name val="Calibri"/></font><font><b/><sz val="11"/><name val="Calibri"/></font><font><i/><u/><sz val="12"/><color rgb="FFFF0000"/><name val="Calibri"/></font></fonts><fills count="3"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill><fill><patternFill patternType="solid"><fgColor rgb="FFFFFF00"/></patternFill></fill></fills><borders count="3"><border><left/><right/><top/><bottom/></border><border><left style="thin"><color rgb="FF000000"/></left><right style="thin"><color rgb="FF000000"/></right><top style="thin"><color rgb="FF000000"/></top><bottom style="thin"><color rgb="FF000000"/></bottom></border><border><left/><right/><top/><bottom style="double"><color rgb="FF0000FF"/></bottom></border></borders><cellXfs count="13"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/><xf numFmtId="2" fontId="0" fillId="0" borderId="0"/><xf numFmtId="14" fontId="0" fillId="0" borderId="0"/><xf numFmtId="0" fontId="1" fillId="0" borderId="0"/><xf numFmtId="0" fontId="0" fillId="2" borderId="0"/><xf numFmtId="0" fontId="0" fillId="0" borderId="1"/><xf numFmtId="0" fontId="0" fillId="0" borderId="0"><alignment wrapText="1" vertical="top"/></xf><xf numFmtId="0" fontId="0" fillId="0" borderId="0"><alignment horizontal="center"/></xf><xf numFmtId="3" fontId="0" fillId="0" borderId="0"/><xf numFmtId="0" fontId="0" fillId="0" borderId="0"><alignment horizontal="right"/></xf><xf numFmtId="0" fontId="2" fillId="0" borderId="0"/><xf numFmtId="9" fontId="0" fillId="0" borderId="0"/><xf numFmtId="164" fontId="0" fillId="0" borderId="2"/></cellXfs></styleSheet>"#;

struct Book {
    sheets: Vec<(String, String, String)>, // name, state, xml
    shared: Vec<String>,
    date1904: bool,
    defined_names: String,
}

fn sheet_xml(extra_head: &str, rows: &str, tail: &str) -> String {
    format!(r#"<worksheet {NS}>{extra_head}<sheetData>{rows}</sheetData>{tail}</worksheet>"#)
}

fn book(sheets: &[String]) -> Book {
    Book {
        sheets: sheets.iter().enumerate().map(|(i, x)| (format!("Sheet{}", i + 1), String::new(), x.clone())).collect(),
        shared: Vec::new(),
        date1904: false,
        defined_names: String::new(),
    }
}

impl Book {
    fn build_with(&self, mutate: &mut dyn FnMut(&str, &str) -> String) -> Vec<u8> {
        let mut zip = karnoxpdf_export::Zip::default();
        let mut add = |zip: &mut karnoxpdf_export::Zip, name: &str, text: &str| zip.add(name, mutate(name, text).as_bytes(), true);
        add(
            &mut zip,
            "_rels/.rels",
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#,
        );
        let mut rels = String::from(r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#);
        let mut listed = String::new();
        for (i, (name, state, _)) in self.sheets.iter().enumerate() {
            rels.push_str(&format!(
                r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{}.xml"/>"#,
                i + 1,
                i + 1
            ));
            let state = if state.is_empty() { String::new() } else { format!(r#" state="{state}""#) };
            listed.push_str(&format!(r#"<sheet name="{name}" sheetId="{}"{state} r:id="rId{}"/>"#, i + 1, i + 1));
        }
        rels.push_str(r#"<Relationship Id="rIdS" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="rIdSS" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/></Relationships>"#);
        add(&mut zip, "xl/_rels/workbook.xml.rels", &rels);
        let pr = if self.date1904 { r#"<workbookPr date1904="1"/>"# } else { "" };
        add(&mut zip, "xl/workbook.xml", &format!(r#"<workbook {NS}>{pr}<sheets>{listed}</sheets>{}</workbook>"#, self.defined_names));
        add(&mut zip, "xl/styles.xml", STYLES);
        let strings: String = self.shared.iter().map(|s| format!("<si><t>{s}</t></si>")).collect();
        add(&mut zip, "xl/sharedStrings.xml", &format!(r#"<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">{strings}</sst>"#));
        for (i, (_, _, xml)) in self.sheets.iter().enumerate() {
            add(&mut zip, &format!("xl/worksheets/sheet{}.xml", i + 1), xml);
        }
        zip.finish()
    }

    fn build(&self) -> Vec<u8> {
        self.build_with(&mut |_, t| t.to_owned())
    }

    fn convert_with(&self, options: &XlsxOptions) -> Result<XlsxConversion, CreateError> {
        from_xlsx_layout("Book", &self.build(), options)
    }

    fn convert(&self) -> Result<XlsxConversion, CreateError> {
        self.convert_with(&XlsxOptions::default())
    }
}

fn pages(doc: &Document) -> Vec<karnoxpdf_cos::ObjRef> {
    let root = doc.root().unwrap();
    let tree = doc.get(root).as_dict().unwrap().reference(b"Pages").unwrap();
    doc.get(tree).as_dict().unwrap().get(b"Kids").unwrap().as_array().unwrap().iter().filter_map(|k| k.as_ref()).collect()
}

fn content(doc: &Document, index: usize) -> String {
    let page = pages(doc)[index];
    let stream_ref = doc.get(page).as_dict().unwrap().reference(b"Contents").unwrap();
    let object = doc.get(stream_ref);
    let Object::Stream(stream) = &*object else { panic!("expected a content stream") };
    stream.decoded().unwrap().iter().map(|b| *b as char).collect()
}

fn media_box(doc: &Document, index: usize) -> Vec<f64> {
    let page = pages(doc)[index];
    let dict = doc.get(page);
    dict.as_dict().unwrap().get(b"MediaBox").unwrap().as_array().unwrap().iter().map(|o| o.as_f64().unwrap()).collect()
}

fn n(r: u32, c: &str, style: u32, v: &str) -> String {
    format!(r#"<c r="{c}{r}" s="{style}"><v>{v}</v></c>"#)
}

fn s(r: u32, c: &str, style: u32, idx: usize) -> String {
    format!(r#"<c r="{c}{r}" s="{style}" t="s"><v>{idx}</v></c>"#)
}

fn row(r: u32, cells: &str) -> String {
    format!(r#"<row r="{r}">{cells}</row>"#)
}

#[test]
fn values_are_shown_with_their_number_formats() {
    let rows = [
        row(1, &format!("{}{}", s(1, "A", 3, 0), s(1, "B", 3, 1))),
        row(2, &format!("{}{}", s(2, "A", 0, 2), n(2, "B", 1, "1234.5"))),
        row(3, &format!("{}{}", s(3, "A", 0, 3), n(3, "B", 8, "1234567"))),
        row(4, &format!("{}{}", s(4, "A", 0, 4), n(4, "B", 11, "0.256"))),
        row(5, &format!("{}{}", s(5, "A", 0, 5), n(5, "B", 2, "45000"))),
        row(6, &format!("{}{}", s(6, "A", 0, 6), n(6, "B", 12, "99.9"))),
        row(7, &format!("{}{}", s(7, "A", 0, 7), n(7, "B", 0, "0.5"))),
    ]
    .concat();
    let mut b = book(&[sheet_xml(r#"<cols><col min="1" max="2" width="14" customWidth="1"/></cols>"#, &rows, "")]);
    b.shared = ["Item", "Value", "Fixed", "Thousands", "Percent", "Date", "Currency", "General"].map(String::from).to_vec();
    let c = content(&b.convert().unwrap().document, 0);
    for expected in ["(Item) Tj", "(Value) Tj", "(1234.50) Tj", "(1,234,567) Tj", "(26%) Tj", "(3/15/2023) Tj", "(R$ 99.90) Tj", "(0.5) Tj"] {
        assert!(c.contains(expected), "{expected} in {c}");
    }
    assert!(c.contains("/F2 11 Tf"), "bold header: {c}");
    // The same workbook for a Brazilian reader: dd/mm/yyyy and decimal commas.
    let pt = b.convert_with(&XlsxOptions { locale: Locale::from_language("pt-br"), ..XlsxOptions::default() }).unwrap();
    let c = content(&pt.document, 0);
    assert!(c.contains("(15/03/2023) Tj") && c.contains("(1.234.567) Tj") && c.contains("(1234,50) Tj") && c.contains("(0,5) Tj"), "{c}");
}

#[test]
fn text_overflows_into_empty_cells_and_numbers_stay_put() {
    // A long text in A1 runs over the empty B1 but stops at the filled C1.
    let rows = row(1, &format!("{}{}", s(1, "A", 0, 0), s(1, "C", 0, 1))) + &row(2, &format!("{}{}", n(2, "A", 0, "42"), s(2, "B", 0, 1)));
    let mut b = book(&[sheet_xml("<cols><col min=\"1\" max=\"3\" width=\"10\" customWidth=\"1\"/></cols>", &rows, "")]);
    b.shared = vec!["A fairly long caption".into(), "x".into()];
    let c = content(&b.convert().unwrap().document, 0);
    assert!(c.contains("(42) Tj"), "{c}");
    // Columns are 10 characters = 70 px = 52.5 pt wide: the caption's clip spans A and B.
    assert!(c.contains("0 0 105 "), "the caption may use the empty cell next to it: {c}");
}

#[test]
fn fills_borders_and_fonts_are_drawn() {
    let rows = row(1, &format!("{}{}{}", s(1, "A", 4, 0), s(1, "B", 5, 0), s(1, "C", 10, 0))) + &row(2, &s(2, "A", 12, 0));
    let mut b = book(&[sheet_xml("", &rows, "")]);
    b.shared = vec!["x".into()];
    let c = content(&b.convert().unwrap().document, 0);
    assert!(c.contains("1 1 0 rg"), "yellow fill: {c}");
    assert!(c.contains("0.75 w"), "thin border: {c}");
    assert!(c.contains("1 0 0 rg") && c.contains("/F3 12 Tf"), "red italic 12 pt: {c}");
    assert!(c.contains("0 0 1 RG"), "double bottom border in blue: {c}");
}

#[test]
fn merged_cells_span_their_range_and_center() {
    let rows = row(1, &s(1, "A", 7, 0)) + &row(2, &n(2, "A", 0, "1"));
    let mut b = book(&[sheet_xml(
        "<cols><col min=\"1\" max=\"4\" width=\"10\" customWidth=\"1\"/></cols>",
        &rows,
        r#"<mergeCells count="1"><mergeCell ref="A1:D1"/></mergeCells>"#,
    )]);
    b.shared = vec!["Title".into()];
    let c = content(&b.convert().unwrap().document, 0);
    // Four 52.5 pt columns = 210 pt: "Title" is centred in that, not in one column.
    let tm = c.split(" Tm").next().unwrap().rsplit("1 0 0 -1 ").next().unwrap().to_owned();
    let x: f64 = tm.split(' ').next().unwrap().parse().unwrap();
    assert!(x > 80.0 && x < 130.0, "centred across the merge: x = {x}");
}

#[test]
fn hidden_rows_columns_and_sheets_are_left_out() {
    let rows = row(1, &format!("{}{}", s(1, "A", 0, 0), s(1, "B", 0, 1))) + r#"<row r="2" hidden="1"><c r="A2" t="s"><v>2</v></c></row>"#;
    let mut b =
        book(&[sheet_xml(r#"<cols><col min="2" max="2" width="10" hidden="1"/></cols>"#, &rows, ""), sheet_xml("", &row(1, &s(1, "A", 0, 3)), "")]);
    b.sheets[1].1 = "hidden".into();
    b.shared = ["shown", "hidden column", "hidden row", "hidden sheet"].map(String::from).to_vec();
    let out = b.convert().unwrap();
    let c = content(&out.document, 0);
    assert!(c.contains("(shown) Tj") && !c.contains("hidden column") && !c.contains("hidden row"), "{c}");
    assert_eq!(pages(&out.document).len(), 1);
    assert_eq!(out.notes, vec![crate::ConversionNote { kind: NoteKind::HiddenSheet, slides: vec![2] }]);
}

#[test]
fn page_setup_sets_paper_orientation_and_margins() {
    let tail = r#"<pageMargins left="1" right="1" top="1" bottom="1" header="0.5" footer="0.5"/><pageSetup paperSize="9" orientation="landscape"/>"#;
    let b = book(&[sheet_xml("", &row(1, &n(1, "A", 0, "1")), tail)]);
    let doc = b.convert().unwrap().document;
    let m = media_box(&doc, 0);
    assert!((m[2] - 841.9).abs() < 0.2 && (m[3] - 595.3).abs() < 0.2, "A4 landscape: {m:?}");
    let forced = b.convert_with(&XlsxOptions { orientation: SheetOrientation::Portrait, ..XlsxOptions::default() }).unwrap().document;
    let m = media_box(&forced, 0);
    assert!(m[2] < m[3], "portrait forced: {m:?}");
    // The cells start at the 1 inch margin.
    assert!(content(&doc, 0).contains("1 0 0 1 72 72 cm"), "{}", content(&doc, 0));
}

#[test]
fn long_sheets_break_into_pages_and_repeat_titles() {
    let mut rows = row(1, &s(1, "A", 3, 0));
    for r in 2..=120 {
        rows.push_str(&row(r, &n(r, "A", 0, &r.to_string())));
    }
    let mut b = book(&[sheet_xml("", &rows, "")]);
    b.shared = vec!["HEADER".into()];
    b.defined_names = r#"<definedNames><definedName name="_xlnm.Print_Titles" localSheetId="0">Sheet1!$1:$1</definedName></definedNames>"#.into();
    let out = b.convert().unwrap();
    let count = pages(&out.document).len();
    assert!(count >= 3, "120 rows of 15 pt on Letter need several pages: {count}");
    for i in 0..count {
        assert!(content(&out.document, i).contains("(HEADER) Tj"), "page {i} repeats the title row");
    }
    // Without titles the header shows once.
    b.defined_names.clear();
    let out = b.convert().unwrap();
    let with: usize = (0..pages(&out.document).len()).filter(|i| content(&out.document, *i).contains("(HEADER) Tj")).count();
    assert_eq!(with, 1);
}

#[test]
fn manual_breaks_and_fit_to_page() {
    let mut rows = String::new();
    for r in 1..=10 {
        rows.push_str(&row(r, &n(r, "A", 0, &r.to_string())));
    }
    let tail = r#"<rowBreaks count="1" manualBreakCount="1"><brk id="4" max="16383" man="1"/></rowBreaks>"#;
    let out = book(&[sheet_xml("", &rows, tail)]).convert().unwrap();
    assert_eq!(pages(&out.document).len(), 2, "a break after row 4");
    assert!(content(&out.document, 0).contains("(4) Tj") && !content(&out.document, 0).contains("(5) Tj"));
    // 150 rows fit on one page when the sheet asks to be fitted: scaled down.
    let mut rows = String::new();
    for r in 1..=150 {
        rows.push_str(&row(r, &n(r, "A", 0, "1")));
    }
    let head = r#"<sheetPr><pageSetUpPr fitToPage="1"/></sheetPr>"#;
    let fitted = book(&[sheet_xml(head, &rows, r#"<pageSetup fitToWidth="1" fitToHeight="1"/>"#)]).convert().unwrap();
    assert_eq!(pages(&fitted.document).len(), 1);
    let c = content(&fitted.document, 0);
    let scale: f64 =
        c.lines().find(|l| l.ends_with(" cm") && l.starts_with("0.")).and_then(|l| l.split(' ').next()).and_then(|v| v.parse().ok()).unwrap_or(1.0);
    assert!(scale < 0.6, "scaled to fit: {scale}\n{}", &c[..200.min(c.len())]);
}

#[test]
fn wrapped_text_makes_the_row_taller() {
    let rows = row(1, &s(1, "A", 6, 0)) + &row(2, &s(2, "A", 0, 1));
    let mut b = book(&[sheet_xml(r#"<cols><col min="1" max="1" width="8" customWidth="1"/></cols>"#, &rows, "")]);
    b.shared = vec!["wrapping text across lines".into(), "below".into()];
    let c = content(&b.convert().unwrap().document, 0);
    assert!(c.matches(" Tj").count() >= 4, "the text wrapped: {c}");
}

#[test]
fn numbers_that_do_not_fit_become_hash_marks() {
    let rows = row(1, &n(1, "A", 8, "123456789012"));
    let b = book(&[sheet_xml(r#"<cols><col min="1" max="1" width="4" customWidth="1"/></cols>"#, &rows, "")]);
    let c = content(&b.convert().unwrap().document, 0);
    assert!(c.contains("(##) Tj"), "a 4-character column fits two hash marks: {c}");
}

#[test]
fn headers_and_footers_show_page_numbers() {
    let mut rows = String::new();
    for r in 1..=100 {
        rows.push_str(&row(r, &n(r, "A", 0, "1")));
    }
    let tail = r#"<headerFooter><oddHeader>&amp;L&amp;A</oddHeader><oddFooter>&amp;CPage &amp;P of &amp;N</oddFooter></headerFooter>"#;
    let out = book(&[sheet_xml("", &rows, tail)]).convert().unwrap();
    let pages_n = pages(&out.document).len();
    assert!(pages_n >= 2);
    let last = content(&out.document, pages_n - 1);
    assert!(last.contains(&format!("(Page {pages_n} of {pages_n}) Tj")), "{last}");
    assert!(content(&out.document, 0).contains("(Sheet1) Tj"));
}

#[test]
fn things_that_are_not_drawn_are_reported() {
    let b = book(&[sheet_xml(
        "",
        &row(1, &n(1, "A", 0, "1")),
        r#"<conditionalFormatting sqref="A1"><cfRule type="cellIs" priority="1" operator="greaterThan"><formula>0</formula></cfRule></conditionalFormatting><drawing r:id="rId9"/>"#,
    )]);
    let kinds: Vec<_> = b.convert().unwrap().notes.iter().map(|x| x.kind).collect();
    assert_eq!(kinds, vec![NoteKind::Drawing, NoteKind::ConditionalFormatting]);
}

#[test]
fn text_the_pdf_fonts_cannot_show_names_its_cell() {
    let mut b = book(&[sheet_xml("", &(row(1, &s(1, "A", 0, 0)) + &row(2, &s(2, "C", 0, 1))), "")]);
    b.shared = vec!["fine".into(), "日本語".into()];
    let CreateError::Invalid(message) = b.convert().err().unwrap() else { panic!() };
    assert!(message.contains("sheet 1 cell C2"), "{message}");
}

#[test]
fn symbols_and_accents_are_supported() {
    let mut b = book(&[sheet_xml("", &row(1, &s(1, "A", 0, 0)), "")]);
    b.shared = vec!["Variação ≥ 5% → ok".into()];
    let c = content(&b.convert().unwrap().document, 0);
    assert!(c.contains("(Varia\u{e7}\u{e3}o ) Tj") && c.contains("/F5 11 Tf"), "{c}");
}

#[test]
fn gridlines_are_printed_when_asked() {
    let rows = row(1, &n(1, "A", 0, "1"));
    let plain = content(&book(&[sheet_xml("", &rows, "")]).convert().unwrap().document, 0);
    let grid = content(&book(&[sheet_xml("", &rows, r#"<printOptions gridLines="1"/>"#)]).convert().unwrap().document, 0);
    assert!(!plain.contains("0.55 0.55 0.55 RG") && grid.contains("0.55 0.55 0.55 RG"));
}

#[test]
fn damaged_workbooks_are_errors_not_panics() {
    assert!(from_xlsx_layout("x", b"not a zip", &XlsxOptions::default()).is_err());
    assert!(from_xlsx_layout("x", &[], &XlsxOptions::default()).is_err());
    let empty = book(&[sheet_xml("", "", "")]);
    assert!(matches!(empty.convert(), Err(CreateError::Invalid(m)) if m.contains("nothing to print")));
}

#[test]
fn the_1904_date_system_is_honoured() {
    let rows = row(1, &n(1, "A", 2, "1"));
    let mut b = book(&[sheet_xml("", &rows, "")]);
    b.date1904 = true;
    assert!(content(&b.convert().unwrap().document, 0).contains("(1/2/1904) Tj"));
}

// ── robustness ──

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn mutate_text(rng: &mut Rng, text: &str) -> String {
    const EXTREMES: [&str; 10] = ["0", "-1", "1", "2147483647", "99999999999999", "-99999999999", "1e999", "0.00000001", "NaN", "abc"];
    let mut out = text.to_owned();
    for _ in 0..1 + rng.below(3) {
        let mut at = rng.below(out.len().max(1)).min(out.len());
        while !out.is_char_boundary(at) {
            at -= 1;
        }
        match rng.below(4) {
            0 => {
                let mut end = (at + 1 + rng.below(150)).min(out.len());
                while !out.is_char_boundary(end) {
                    end -= 1;
                }
                out.replace_range(at..end, "");
            }
            1 | 2 => {
                if let Some(start) = out[at..].find("=\"") {
                    let from = at + start + 2;
                    if let Some(len) = out[from..].find('"') {
                        out.replace_range(from..from + len, EXTREMES[rng.below(EXTREMES.len())]);
                    }
                }
            }
            _ => {
                if let Some(start) = out[at..].find("<v>") {
                    let from = at + start + 3;
                    if let Some(len) = out[from..].find('<') {
                        out.replace_range(from..from + len, EXTREMES[rng.below(EXTREMES.len())]);
                    }
                }
            }
        }
    }
    out
}

fn rich_book() -> Book {
    let mut rows = String::new();
    for r in 1..=40 {
        rows.push_str(&row(
            r,
            &format!("{}{}{}{}", s(r, "A", r % 12, 0), n(r, "B", r % 13, &format!("{}.5", r * 37)), n(r, "C", 2, "45000.25"), s(r, "D", 6, 1)),
        ));
    }
    let tail = r#"<mergeCells count="1"><mergeCell ref="A1:C2"/></mergeCells><pageSetup scale="80" orientation="landscape"/><headerFooter><oddFooter>&amp;CPage &amp;P</oddFooter></headerFooter><rowBreaks count="1"><brk id="20"/></rowBreaks>"#;
    let mut b = book(&[sheet_xml(r#"<cols><col min="1" max="4" width="14" customWidth="1"/></cols>"#, &rows, tail)]);
    b.shared = vec!["Label".into(), "a longer text that wraps in its cell".into()];
    b.defined_names = r#"<definedNames><definedName name="_xlnm.Print_Titles" localSheetId="0">Sheet1!$1:$2</definedName></definedNames>"#.into();
    b
}

#[test]
fn mutated_workbooks_never_panic() {
    let started = std::time::Instant::now();
    let deck = rich_book();
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    let (mut converted, mut refused) = (0, 0);
    let rounds = std::env::var("KARNOXPDF_FUZZ_ROUNDS").ok().and_then(|v| v.parse().ok()).unwrap_or(1500);
    for _ in 0..rounds {
        let mut target = rng.below(4);
        let bytes = deck.build_with(&mut |name, text| {
            if name.ends_with(".xml") && !name.contains("_rels") {
                if target == 0 {
                    target = usize::MAX;
                    return mutate_text(&mut rng, text);
                }
                target = target.saturating_sub(1);
            }
            text.to_owned()
        });
        match from_xlsx_layout("fuzz", &bytes, &XlsxOptions::default()) {
            Ok(_) => converted += 1,
            Err(_) => refused += 1,
        }
    }
    assert!(converted > 100 && refused > 20, "{converted} converted, {refused} refused");
    assert!(rounds > 1500 || started.elapsed().as_secs() < 120, "{:?}", started.elapsed());
}

#[test]
fn a_cell_in_the_far_corner_stays_cheap() {
    let rows = row(1, &n(1, "A", 0, "1")) + &row(1_048_576, &n(1_048_576, "XFD", 0, "1"));
    let started = std::time::Instant::now();
    let _ = book(&[sheet_xml("", &rows, "")]).convert();
    assert!(started.elapsed().as_secs() < 60, "{:?}", started.elapsed());
}

/// A report-like workbook for visual checks (set KARNOXPDF_XLSX_SAMPLES to a folder to write it).
fn sample_book() -> Book {
    let mut rows = String::new();
    rows.push_str(&row(1, &s(1, "A", 3, 0)));
    rows.push_str(&row(2, &format!("{}{}{}{}{}", s(2, "A", 3, 1), s(2, "B", 3, 2), s(2, "C", 3, 3), s(2, "D", 3, 4), s(2, "E", 3, 5))));
    for r in 3..=70 {
        let item = 6 + (r as usize % 5);
        rows.push_str(&row(
            r,
            &format!(
                "{}{}{}{}{}",
                s(r, "A", 5, item),
                n(r, "B", 8, &(r * 137).to_string()),
                n(r, "C", 12, &format!("{}.75", r * 11)),
                n(r, "D", 2, &(44000 + r).to_string()),
                n(r, "E", 11, &format!("0.{}", r % 100))
            ),
        ));
    }
    let tail = r#"<mergeCells count="1"><mergeCell ref="A1:E1"/></mergeCells><printOptions gridLines="0"/><pageSetup paperSize="9" orientation="portrait"/><headerFooter><oddHeader>&amp;L&amp;"Arial,Bold"Relatório de vendas&amp;R&amp;D</oddHeader><oddFooter>&amp;CPágina &amp;P de &amp;N</oddFooter></headerFooter>"#;
    let mut b = book(&[sheet_xml(
        r#"<cols><col min="1" max="1" width="22" customWidth="1"/><col min="2" max="5" width="14" customWidth="1"/></cols>"#,
        &rows,
        tail,
    )]);
    b.shared = [
        "Relatório de vendas — 2º trimestre",
        "Produto",
        "Unidades",
        "Receita",
        "Data",
        "Margem",
        "Licença anual",
        "Suporte premium",
        "Treinamento",
        "Consultoria",
        "Implantação",
    ]
    .map(String::from)
    .to_vec();
    b.defined_names = r#"<definedNames><definedName name="_xlnm.Print_Titles" localSheetId="0">Sheet1!$1:$2</definedName></definedNames>"#.into();
    b
}

#[test]
fn sample_workbook_converts_and_can_be_written_for_visual_checks() {
    let b = sample_book();
    let out = b.convert_with(&XlsxOptions { locale: Locale::from_language("pt-br"), today: Some(46000.0), ..XlsxOptions::default() }).unwrap();
    assert!(pages(&out.document).len() >= 2);
    if let Ok(dir) = std::env::var("KARNOXPDF_XLSX_SAMPLES") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(std::path::Path::new(&dir).join("sample.xlsx"), b.build()).unwrap();
    }
}

#[test]
fn windows_latin_letters_and_symbol_font_spaces_are_shown() {
    let mut b = book(&[sheet_xml("", &row(1, &s(1, "A", 0, 0)), "")]);
    b.shared = vec!["Škoda žlutý \u{f020}Œuvre".into()];
    let c = content(&b.convert().unwrap().document, 0);
    // š is WinAnsi 0x8A, ž 0x9E, Œ 0x8C; the private-use space is a plain space.
    assert!(c.contains("(\u{8a}koda \u{9e}lu"), "{c}");
    assert!(c.contains("\u{8c}uvre"), "{c}");
}
