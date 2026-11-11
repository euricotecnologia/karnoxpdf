//! PowerPoint conversion tests on synthetic presentations: a master, a layout and a theme built
//! the way PowerPoint lays a package out, plus the slides under test.

use karnoxpdf_cos::{Document, Object};

use crate::{CreateError, NoteKind, PptxConversion, from_pptx_layout};

const NS: &str = r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#;

const THEME: &str = r#"<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Office"><a:themeElements><a:clrScheme name="Office"><a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1><a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="44546A"/></a:dk2><a:lt2><a:srgbClr val="E7E6E6"/></a:lt2><a:accent1><a:srgbClr val="4472C4"/></a:accent1><a:accent2><a:srgbClr val="ED7D31"/></a:accent2><a:accent3><a:srgbClr val="A5A5A5"/></a:accent3><a:accent4><a:srgbClr val="FFC000"/></a:accent4><a:accent5><a:srgbClr val="5B9BD5"/></a:accent5><a:accent6><a:srgbClr val="70AD47"/></a:accent6><a:hlink><a:srgbClr val="0563C1"/></a:hlink><a:folHlink><a:srgbClr val="954F72"/></a:folHlink></a:clrScheme><a:fmtScheme name="Office"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:gradFill><a:gsLst><a:gs pos="0"><a:schemeClr val="phClr"><a:lumMod val="110000"/></a:schemeClr></a:gs><a:gs pos="100000"><a:schemeClr val="phClr"><a:lumMod val="90000"/></a:schemeClr></a:gs></a:gsLst><a:lin ang="5400000"/></a:gradFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="12700"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="19050"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst><a:effectStyleLst/><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>"#;

fn master(extra_shapes: &str) -> String {
    format!(
        r#"<p:sldMaster {NS}><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/><p:sp><p:nvSpPr><p:cNvPr id="2" name="Title"/><p:cNvSpPr/><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="838200" y="365125"/><a:ext cx="10515600" cy="1325563"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr anchor="ctr"><a:normAutofit/></a:bodyPr><a:lstStyle/><a:p><a:r><a:t>Click to edit Master title style</a:t></a:r></a:p></p:txBody></p:sp><p:sp><p:nvSpPr><p:cNvPr id="3" name="Body"/><p:cNvSpPr/><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="838200" y="1825625"/><a:ext cx="10515600" cy="4351338"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>Click to edit Master text styles</a:t></a:r></a:p></p:txBody></p:sp>{extra_shapes}</p:spTree></p:cSld><p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/><p:txStyles><p:titleStyle><a:lvl1pPr algn="l"><a:lnSpc><a:spcPct val="90000"/></a:lnSpc><a:spcBef><a:spcPct val="0"/></a:spcBef><a:buNone/><a:defRPr sz="4400" b="1"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill></a:defRPr></a:lvl1pPr></p:titleStyle><p:bodyStyle><a:lvl1pPr marL="228600" indent="-228600" algn="l"><a:lnSpc><a:spcPct val="90000"/></a:lnSpc><a:spcBef><a:spcPts val="1000"/></a:spcBef><a:buFont typeface="Arial"/><a:buChar char="•"/><a:defRPr sz="2800"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill></a:defRPr></a:lvl1pPr><a:lvl2pPr marL="685800" indent="-228600" algn="l"><a:spcBef><a:spcPts val="500"/></a:spcBef><a:buFont typeface="Arial"/><a:buChar char="•"/><a:defRPr sz="2400"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill></a:defRPr></a:lvl2pPr></p:bodyStyle><p:otherStyle><a:lvl1pPr marL="0" algn="l"><a:defRPr sz="1800"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill></a:defRPr></a:lvl1pPr></p:otherStyle></p:txStyles></p:sldMaster>"#
    )
}

const LAYOUT: &str = r#"<p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld name="Title and Content"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/><p:sp><p:nvSpPr><p:cNvPr id="2" name="Title 1"/><p:cNvSpPr/><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>Click to edit Master title style</a:t></a:r></a:p></p:txBody></p:sp><p:sp><p:nvSpPr><p:cNvPr id="3" name="Content"/><p:cNvSpPr/><p:nvPr><p:ph idx="1"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>Edit Master text styles</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sldLayout>"#;

fn slide(shapes: &str) -> String {
    format!(
        r#"<p:sld {NS}><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>{shapes}</p:spTree></p:cSld></p:sld>"#
    )
}

fn title_shape(text: &str) -> String {
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="Title"/><p:cNvSpPr/><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="pt-BR"/><a:t>{text}</a:t></a:r></a:p></p:txBody></p:sp>"#
    )
}

/// A box of the given EMU geometry holding one paragraph.
fn text_box(x: i64, y: i64, w: i64, h: i64, paragraph: &str) -> String {
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="9" name="Box"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{w}" cy="{h}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/></p:spPr><p:txBody><a:bodyPr wrap="square"/><a:lstStyle/>{paragraph}</p:txBody></p:sp>"#
    )
}

struct Deck {
    slides: Vec<String>,
    master_extra: String,
    slide_attrs: Vec<String>,
    media: Vec<(String, Vec<u8>)>,
    slide_rels_extra: Vec<String>,
    size: (i64, i64),
}

fn deck(slides: &[String]) -> Deck {
    Deck {
        slides: slides.to_vec(),
        master_extra: String::new(),
        slide_attrs: Vec::new(),
        media: Vec::new(),
        slide_rels_extra: Vec::new(),
        size: (12_192_000, 6_858_000),
    }
}

impl Deck {
    fn build(&self) -> Vec<u8> {
        self.build_with(&mut |_, text| text.to_owned())
    }

    /// The package, with every XML part passed through `mutate` (part name, text) on the way in.
    fn build_with(&self, mutate: &mut dyn FnMut(&str, &str) -> String) -> Vec<u8> {
        let mut zip = karnoxpdf_export::Zip::default();
        let mut add = |zip: &mut karnoxpdf_export::Zip, name: &str, text: &str| zip.add(name, mutate(name, text).as_bytes(), true);
        add(
            &mut zip,
            "_rels/.rels",
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/></Relationships>"#,
        );
        let mut pres_rels = String::from(
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>"#,
        );
        let mut ids = String::new();
        for i in 0..self.slides.len() {
            pres_rels.push_str(&format!(
                r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{}.xml"/>"#,
                i + 10,
                i + 1
            ));
            ids.push_str(&format!(r#"<p:sldId id="{}" r:id="rId{}"/>"#, 256 + i, i + 10));
        }
        pres_rels.push_str("</Relationships>");
        add(&mut zip, "ppt/_rels/presentation.xml.rels", &pres_rels);
        add(
            &mut zip,
            "ppt/presentation.xml",
            &format!(
                r#"<p:presentation {NS}><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst><p:sldIdLst>{ids}</p:sldIdLst><p:sldSz cx="{}" cy="{}"/><p:defaultTextStyle><a:lvl1pPr marL="0" algn="l"><a:defRPr sz="1800"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill></a:defRPr></a:lvl1pPr></p:defaultTextStyle></p:presentation>"#,
                self.size.0, self.size.1
            ),
        );
        add(&mut zip, "ppt/slideMasters/slideMaster1.xml", &master(&self.master_extra));
        add(
            &mut zip,
            "ppt/slideMasters/_rels/slideMaster1.xml.rels",
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/></Relationships>"#,
        );
        add(&mut zip, "ppt/slideLayouts/slideLayout1.xml", LAYOUT);
        add(
            &mut zip,
            "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#,
        );
        add(&mut zip, "ppt/theme/theme1.xml", THEME);
        for (i, body) in self.slides.iter().enumerate() {
            let mut xml = body.clone();
            if let Some(attrs) = self.slide_attrs.get(i).filter(|a| !a.is_empty()) {
                xml = xml.replacen("<p:sld ", &format!("<p:sld {attrs} "), 1);
            }
            add(&mut zip, &format!("ppt/slides/slide{}.xml", i + 1), &xml);
            let extra = self.slide_rels_extra.get(i).cloned().unwrap_or_default();
            add(
                &mut zip,
                &format!("ppt/slides/_rels/slide{}.xml.rels", i + 1),
                &format!(
                    r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>{extra}</Relationships>"#
                ),
            );
        }
        for (name, data) in &self.media {
            zip.add(name, data, false);
        }
        zip.finish()
    }

    fn convert(&self) -> Result<PptxConversion, CreateError> {
        from_pptx_layout("Deck", &self.build())
    }
}

fn pages(doc: &Document) -> Vec<karnoxpdf_cos::ObjRef> {
    let root = doc.root().unwrap();
    let tree = doc.get(root).as_dict().unwrap().reference(b"Pages").unwrap();
    doc.get(tree).as_dict().unwrap().get(b"Kids").unwrap().as_array().unwrap().iter().filter_map(|k| k.as_ref()).collect()
}

/// A page's content stream, with each byte as one character.
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
    let boxed = dict.as_dict().unwrap().get(b"MediaBox").unwrap().as_array().unwrap().clone();
    boxed.iter().map(|o| o.as_f64().unwrap()).collect()
}

fn run(paragraph_text: &str) -> String {
    format!(r#"<a:p><a:r><a:rPr lang="en-US"/><a:t>{paragraph_text}</a:t></a:r></a:p>"#)
}

#[test]
fn each_slide_is_a_page_of_the_slide_size() {
    let out = deck(&[slide(&title_shape("One")), slide(&title_shape("Two"))]).convert().unwrap();
    assert_eq!(pages(&out.document).len(), 2);
    assert_eq!(media_box(&out.document, 0), vec![0.0, 0.0, 960.0, 540.0]);
    assert!(out.notes.is_empty(), "{:?}", out.notes);
}

#[test]
fn a_four_by_three_deck_gets_a_four_by_three_page() {
    let mut d = deck(&[slide(&title_shape("x"))]);
    d.size = (9_144_000, 6_858_000);
    assert_eq!(media_box(&d.convert().unwrap().document, 0), vec![0.0, 0.0, 720.0, 540.0]);
}

#[test]
fn placeholders_inherit_position_and_text_style_from_the_master() {
    let out = deck(&[slide(&title_shape("Olá, mundo"))]).convert().unwrap();
    let c = content(&out.document, 0);
    // The title's box comes from the master (838200, 365125 EMU = 66 pt, 28.75 pt) ...
    assert!(c.contains("1 0 0 1 66 28.75 cm"), "{c}");
    // ... and its size and weight from the master's title style: 44 pt bold.
    assert!(c.contains("/F2 44 Tf"), "{c}");
    // WinAnsi: "á" is byte 0xE1.
    assert!(c.contains("(Ol\u{e1}, mundo) Tj"), "{c}");
}

#[test]
fn the_master_text_style_gives_bullets_and_levels() {
    let body = r#"<p:sp><p:nvSpPr><p:cNvPr id="3" name="Content"/><p:cNvSpPr/><p:nvPr><p:ph idx="1"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>First</a:t></a:r></a:p><a:p><a:pPr lvl="1"/><a:r><a:t>Nested</a:t></a:r></a:p></p:txBody></p:sp>"#;
    let out = deck(&[slide(body)]).convert().unwrap();
    let c = content(&out.document, 0);
    assert!(c.contains("/F1 28 Tf"), "level 1 is 28 pt: {c}");
    assert!(c.contains("/F1 24 Tf"), "level 2 is 24 pt: {c}");
    assert_eq!(c.matches("(\u{95}) Tj").count(), 2, "one bullet per paragraph: {c}");
}

#[test]
fn explicit_run_formatting_beats_inherited_formatting() {
    let p = r#"<a:p><a:r><a:rPr sz="2000" b="1" i="1"><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></a:rPr><a:t>Alert</a:t></a:r></a:p>"#;
    let out = deck(&[slide(&text_box(914_400, 914_400, 4_572_000, 914_400, p))]).convert().unwrap();
    let c = content(&out.document, 0);
    assert!(c.contains("/F4 20 Tf"), "bold italic at 20 pt: {c}");
    assert!(c.contains("1 0 0 rg"), "{c}");
    assert!(c.contains("1 0 0 1 72 72 cm"), "{c}");
}

#[test]
fn scheme_colors_are_resolved_through_the_theme() {
    let p = r#"<a:p><a:r><a:rPr><a:solidFill><a:schemeClr val="accent2"/></a:solidFill></a:rPr><a:t>Orange</a:t></a:r></a:p>"#;
    let out = deck(&[slide(&text_box(0, 0, 2_000_000, 500_000, p))]).convert().unwrap();
    let c = content(&out.document, 0);
    // ED7D31 → 0.929 0.49 0.192
    assert!(c.contains("0.929 0.49 0.192 rg"), "{c}");
}

#[test]
fn text_wraps_inside_the_box() {
    let text = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau upsilon";
    let out = deck(&[slide(&text_box(0, 0, 2_540_000, 3_000_000, &run(text)))]).convert().unwrap();
    let c = content(&out.document, 0);
    let lines = c.matches(" Tj").count();
    assert!(lines >= 4, "a 200 pt box needs several lines for this text, got {lines}: {c}");
}

#[test]
fn hidden_slides_are_left_out_and_reported() {
    let mut d = deck(&[slide(&title_shape("shown")), slide(&title_shape("hidden"))]);
    d.slide_attrs = vec![String::new(), r#"show="0""#.into()];
    let out = d.convert().unwrap();
    assert_eq!(pages(&out.document).len(), 1);
    assert_eq!(out.notes, vec![crate::ConversionNote { kind: NoteKind::HiddenSlide, slides: vec![2] }]);
}

#[test]
fn only_hidden_slides_is_an_error() {
    let mut d = deck(&[slide(&title_shape("hidden"))]);
    d.slide_attrs = vec![r#"show="0""#.into()];
    assert!(matches!(d.convert(), Err(CreateError::Invalid(m)) if m.contains("no visible slides")));
}

#[test]
fn charts_and_smartart_are_reported_not_dropped_silently() {
    let frame = |uri: &str| {
        format!(
            r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="4" name="Obj"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><p:xfrm><a:off x="0" y="0"/><a:ext cx="100" cy="100"/></p:xfrm><a:graphic><a:graphicData uri="{uri}"/></a:graphic></p:graphicFrame>"#
        )
    };
    let shapes = format!(
        "{}{}",
        frame("http://schemas.openxmlformats.org/drawingml/2006/chart"),
        frame("http://schemas.openxmlformats.org/drawingml/2006/diagram")
    );
    let out = deck(&[slide(&shapes)]).convert().unwrap();
    let kinds: Vec<_> = out.notes.iter().map(|n| n.kind).collect();
    assert_eq!(kinds, vec![NoteKind::Chart, NoteKind::Diagram]);
}

#[test]
fn text_the_pdf_fonts_cannot_show_stops_the_conversion() {
    let err = deck(&[slide(&title_shape("fine")), slide(&title_shape("日本語"))]).convert().err().unwrap();
    let CreateError::Invalid(message) = err else { panic!("{err:?}") };
    assert!(message.contains("slide 2"), "{message}");
}

#[test]
fn shapes_use_style_references_for_fill_and_line() {
    let sp = r#"<p:sp><p:nvSpPr><p:cNvPr id="5" name="Rect"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="1270000" y="1270000"/><a:ext cx="2540000" cy="1270000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:style><a:lnRef idx="2"><a:schemeClr val="accent1"><a:shade val="50000"/></a:schemeClr></a:lnRef><a:fillRef idx="1"><a:schemeClr val="accent1"/></a:fillRef><a:effectRef idx="0"><a:schemeClr val="accent1"/></a:effectRef><a:fontRef idx="minor"><a:schemeClr val="lt1"/></a:fontRef></p:style><p:txBody><a:bodyPr anchor="ctr"/><a:lstStyle/><a:p><a:pPr algn="ctr"/><a:r><a:t>Hi</a:t></a:r></a:p></p:txBody></p:sp>"#;
    let out = deck(&[slide(sp)]).convert().unwrap();
    let c = content(&out.document, 0);
    // fillRef 1 is the theme's solid fill with accent 1 (4472C4).
    assert!(c.contains("0.267 0.447 0.769 rg"), "{c}");
    // lnRef 2 is the theme's 1 pt line, drawn.
    assert!(c.contains("1 w"), "{c}");
    // fontRef gives the text white, over the master's default black.
    assert!(c.contains("1 1 1 rg"), "{c}");
}

#[test]
fn rotation_and_flips_become_matrices() {
    let sp = |attrs: &str| {
        format!(
            r#"<p:sp><p:nvSpPr><p:cNvPr id="5" name="Rect"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm {attrs}><a:off x="1270000" y="1270000"/><a:ext cx="2540000" cy="1270000"/></a:xfrm><a:prstGeom prst="triangle"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="00FF00"/></a:solidFill></p:spPr></p:sp>"#
        )
    };
    let c = content(&deck(&[slide(&sp(r#"rot="5400000""#))]).convert().unwrap().document, 0);
    // A quarter turn about the centre (200, 150) of the 200×100 box at (100, 100): e = cx + cy = 350, f = cy - cx = -50.
    assert!(c.contains("0 1 -1 0 350 -50 cm"), "{c}");
    let c = content(&deck(&[slide(&sp(r#"flipH="1""#))]).convert().unwrap().document, 0);
    assert!(c.contains("-1 0 0 1 200 0 cm"), "{c}");
}

#[test]
fn group_children_are_mapped_into_the_group_frame() {
    let group = r#"<p:grpSp><p:nvGrpSpPr><p:cNvPr id="6" name="Group"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="1270000" y="1270000"/><a:ext cx="2540000" cy="2540000"/><a:chOff x="0" y="0"/><a:chExt cx="1270000" cy="1270000"/></a:xfrm></p:grpSpPr><p:sp><p:nvSpPr><p:cNvPr id="7" name="Child"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="635000" y="635000"/><a:ext cx="635000" cy="635000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="0000FF"/></a:solidFill></p:spPr></p:sp></p:grpSp>"#;
    let c = content(&deck(&[slide(group)]).convert().unwrap().document, 0);
    // The group doubles its children: the child's (50, 50) 50×50 box lands at (100+100, 100+100) 100×100.
    assert!(c.contains("1 0 0 1 200 200 cm"), "{c}");
    assert!(c.contains("100 100 l"), "{c}");
}

#[test]
fn alternate_content_takes_the_fallback() {
    let shapes = format!(
        r#"<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:Choice Requires="p14">{}</mc:Choice><mc:Fallback>{}</mc:Fallback></mc:AlternateContent>"#,
        title_shape("newer"),
        title_shape("older")
    );
    let c = content(&deck(&[slide(&shapes)]).convert().unwrap().document, 0);
    assert!(c.contains("(older) Tj"), "{c}");
    assert!(!c.contains("(newer) Tj"), "{c}");
}

#[test]
fn master_shapes_are_drawn_unless_the_slide_hides_them() {
    let band = r#"<p:sp><p:nvSpPr><p:cNvPr id="20" name="Band"/><p:cNvSpPr/><p:nvPr userDrawn="1"/></p:nvSpPr><p:spPr><a:xfrm><a:off x="0" y="6400000"/><a:ext cx="12192000" cy="458000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="123456"/></a:solidFill></p:spPr></p:sp>"#;
    let mut d = deck(&[slide(&title_shape("a")), slide(&title_shape("b"))]);
    d.master_extra = band.into();
    d.slide_attrs = vec![String::new(), r#"showMasterSp="0""#.into()];
    let out = d.convert().unwrap();
    assert!(content(&out.document, 0).contains("0.071 0.204 0.337 rg"));
    assert!(!content(&out.document, 1).contains("0.071 0.204 0.337 rg"));
}

#[test]
fn master_placeholders_are_not_drawn_on_slides() {
    let c = content(&deck(&[slide("")]).convert().unwrap().document, 0);
    assert!(!c.contains("Click to edit"), "{c}");
}

#[test]
fn automatic_numbering_counts_within_a_level() {
    let p = |n: &str| {
        format!(
            r#"<a:p><a:pPr marL="342900" indent="-342900"><a:buFont typeface="+mj-lt"/><a:buAutoNum type="arabicPeriod"/></a:pPr><a:r><a:t>{n}</a:t></a:r></a:p>"#
        )
    };
    let paragraphs = format!("{}{}{}", p("a"), p("b"), p("c"));
    let c = content(&deck(&[slide(&text_box(0, 0, 4_000_000, 2_000_000, &paragraphs))]).convert().unwrap().document, 0);
    for label in ["(1.) Tj", "(2.) Tj", "(3.) Tj"] {
        assert!(c.contains(label), "{label} in {c}");
    }
}

#[test]
fn autofit_shrinks_text_that_would_overflow() {
    let long = "word ".repeat(300);
    let sp = format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="9" name="Box"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="3000000" cy="1500000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr><a:normAutofit/></a:bodyPr><a:lstStyle/><a:p><a:r><a:rPr sz="2400"/><a:t>{long}</a:t></a:r></a:p></p:txBody></p:sp>"#
    );
    let c = content(&deck(&[slide(&sp)]).convert().unwrap().document, 0);
    let size: f64 = c.split(" Tf").next().unwrap().rsplit(' ').next().unwrap().parse().unwrap();
    assert!(size < 24.0, "text was shrunk from 24 pt: {size}");
}

#[test]
fn tables_draw_cells_text_fills_and_borders() {
    let tbl = r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="4" name="Table"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><p:xfrm><a:off x="914400" y="914400"/><a:ext cx="4000000" cy="800000"/></p:xfrm><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table"><a:tbl><a:tblPr firstRow="1"><a:tableStyleId>{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}</a:tableStyleId></a:tblPr><a:tblGrid><a:gridCol w="2000000"/><a:gridCol w="2000000"/></a:tblGrid><a:tr h="400000"><a:tc><a:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>Name</a:t></a:r></a:p></a:txBody><a:tcPr><a:solidFill><a:srgbClr val="FFFF00"/></a:solidFill></a:tcPr></a:tc><a:tc><a:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>Qty</a:t></a:r></a:p></a:txBody><a:tcPr><a:lnB w="12700"><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></a:lnB></a:tcPr></a:tc></a:tr><a:tr h="400000"><a:tc><a:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>Apples</a:t></a:r></a:p></a:txBody><a:tcPr/></a:tc><a:tc><a:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>3</a:t></a:r></a:p></a:txBody><a:tcPr/></a:tc></a:tr></a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#;
    let out = deck(&[slide(tbl)]).convert().unwrap();
    let c = content(&out.document, 0);
    for text in ["(Name) Tj", "(Qty) Tj", "(Apples) Tj", "(3) Tj"] {
        assert!(c.contains(text), "{text} in {c}");
    }
    assert!(c.contains("1 1 0 rg"), "explicit cell fill: {c}");
    assert!(c.contains("1 0 0 RG"), "explicit bottom border: {c}");
    // The style named by the table is not in the package (no tableStyles part): reported.
    assert_eq!(out.notes.iter().map(|n| n.kind).collect::<Vec<_>>(), vec![NoteKind::TableStyle]);
}

fn tiny_png() -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, 2, 2);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0]).unwrap();
    }
    out
}

fn picture(embed: &str, src_rect: &str) -> String {
    format!(
        r#"<p:pic><p:nvPicPr><p:cNvPr id="8" name="Pic"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="{embed}"/>{src_rect}<a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr><a:xfrm><a:off x="1270000" y="1270000"/><a:ext cx="2540000" cy="2540000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#
    )
}

#[test]
fn pictures_are_embedded_once_and_placed() {
    let rel =
        r#"<Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image1.png"/>"#;
    let mut d = deck(&[slide(&picture("rId5", "")), slide(&picture("rId5", r#"<a:srcRect l="25000" r="25000"/>"#))]);
    d.media = vec![("ppt/media/image1.png".into(), tiny_png())];
    d.slide_rels_extra = vec![rel.into(), rel.into()];
    let out = d.convert().unwrap();
    let c = content(&out.document, 0);
    assert!(c.contains("/Im0 Do"), "{c}");
    assert!(c.contains("200 0 0 -200 0 200 cm"), "flipped into the y-down page: {c}");
    let cropped = content(&out.document, 1);
    assert!(cropped.contains("0 0 200 200 re W n"), "crop clips to the frame: {cropped}");
    assert!(cropped.contains("400 0 0 -200 -100 200 cm"), "{cropped}");
    let images = out
        .document
        .object_numbers()
        .into_iter()
        .filter_map(|n| out.document.try_get(n).ok())
        .filter(|o| matches!(&**o, Object::Stream(s) if s.dict.name(b"Subtype") == Some(b"Image".as_slice())))
        .count();
    assert_eq!(images, 1, "the shared image is stored once");
    assert!(out.notes.is_empty(), "{:?}", out.notes);
}

#[test]
fn unsupported_pictures_are_reported() {
    let rel =
        r#"<Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image1.emf"/>"#;
    let mut d = deck(&[slide(&picture("rId5", ""))]);
    d.media = vec![("ppt/media/image1.emf".into(), vec![1, 0, 0, 0, 0x6c, 0, 0, 0])];
    d.slide_rels_extra = vec![rel.into()];
    let out = d.convert().unwrap();
    assert_eq!(out.notes, vec![crate::ConversionNote { kind: NoteKind::Picture, slides: vec![1] }]);
}

#[test]
fn custom_geometry_paths_are_drawn() {
    let sp = r#"<p:sp><p:nvSpPr><p:cNvPr id="5" name="Free"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1270000" cy="1270000"/></a:xfrm><a:custGeom><a:avLst/><a:pathLst><a:path w="100" h="100"><a:moveTo><a:pt x="0" y="0"/></a:moveTo><a:lnTo><a:pt x="100" y="0"/></a:lnTo><a:cubicBezTo><a:pt x="100" y="50"/><a:pt x="50" y="100"/><a:pt x="0" y="100"/></a:cubicBezTo><a:close/></a:path></a:pathLst></a:custGeom><a:solidFill><a:srgbClr val="808080"/></a:solidFill></p:spPr></p:sp>"#;
    let out = deck(&[slide(sp)]).convert().unwrap();
    let c = content(&out.document, 0);
    assert!(c.contains("100 0 l"), "{c}");
    assert!(c.contains("100 50 50 100 0 100 c") || c.contains("100 50 50 100 0 100 c\n"), "{c}");
    assert!(out.notes.is_empty());
}

#[test]
fn unknown_preset_shapes_are_drawn_as_rectangles_and_reported() {
    let sp = r#"<p:sp><p:nvSpPr><p:cNvPr id="5" name="Star"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1270000" cy="1270000"/></a:xfrm><a:prstGeom prst="star5"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="FF00FF"/></a:solidFill></p:spPr></p:sp>"#;
    let out = deck(&[slide(sp)]).convert().unwrap();
    assert_eq!(out.notes.iter().map(|n| n.kind).collect::<Vec<_>>(), vec![NoteKind::ShapeGeometry]);
    assert!(content(&out.document, 0).contains("100 100 l"));
}

#[test]
fn gradients_become_axial_shadings() {
    let sp = r#"<p:sp><p:nvSpPr><p:cNvPr id="5" name="Grad"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="2540000" cy="1270000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:gradFill><a:gsLst><a:gs pos="0"><a:srgbClr val="FF0000"/></a:gs><a:gs pos="100000"><a:srgbClr val="0000FF"/></a:gs></a:gsLst><a:lin ang="0"/></a:gradFill></p:spPr></p:sp>"#;
    let out = deck(&[slide(sp)]).convert().unwrap();
    assert!(content(&out.document, 0).contains("/Sh0 sh"));
}

#[test]
fn transparency_uses_an_opacity_state() {
    let sp = r#"<p:sp><p:nvSpPr><p:cNvPr id="5" name="Glass"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1270000" cy="1270000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="000000"><a:alpha val="50000"/></a:srgbClr></a:solidFill></p:spPr></p:sp>"#;
    assert!(content(&deck(&[slide(sp)]).convert().unwrap().document, 0).contains("/A0 gs"));
}

#[test]
fn damaged_packages_are_errors_not_panics() {
    assert!(from_pptx_layout("x", b"not a zip").is_err());
    assert!(from_pptx_layout("x", &[]).is_err());
    let mut zip = karnoxpdf_export::Zip::default();
    zip.add("ppt/presentation.xml", b"<p:presentation xmlns:p=\"urn:p\"/>", true);
    assert!(from_pptx_layout("x", &zip.finish()).is_err(), "no slide size or slides");
    let mut d = deck(&[slide(&title_shape("x"))]);
    d.size = (0, 0);
    assert!(d.convert().is_err(), "a zero-sized slide");
}

#[test]
fn slide_numbers_fields_show_the_real_number() {
    let p = r#"<a:p><a:fld id="{00000000-0000-0000-0000-000000000000}" type="slidenum"><a:rPr lang="en-US"/><a:t>‹#›</a:t></a:fld></a:p>"#;
    let out = deck(&[slide(&title_shape("a")), slide(&text_box(0, 0, 1_000_000, 500_000, p))]).convert().unwrap();
    assert!(content(&out.document, 1).contains("(2) Tj"));
}

// ── sample decks for visual comparison (set KARNOXPDF_PPTX_SAMPLES to a folder to write them) ──

fn gradient_png() -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, 64, 64);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        let mut data = Vec::new();
        for y in 0..64u32 {
            for x in 0..64u32 {
                data.extend([(x * 4) as u8, (y * 4) as u8, 160]);
            }
        }
        writer.write_image_data(&data).unwrap();
    }
    out
}

fn sample_deck() -> Deck {
    let body = |paragraphs: &str| {
        format!(
            r#"<p:sp><p:nvSpPr><p:cNvPr id="3" name="Content"/><p:cNvSpPr/><p:nvPr><p:ph idx="1"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr><a:normAutofit/></a:bodyPr><a:lstStyle/>{paragraphs}</p:txBody></p:sp>"#
        )
    };
    let para = |text: &str| format!("<a:p><a:r><a:rPr lang=\"pt-BR\"/><a:t>{text}</a:t></a:r></a:p>");
    let level2 = |text: &str| format!("<a:p><a:pPr lvl=\"1\"/><a:r><a:rPr lang=\"pt-BR\"/><a:t>{text}</a:t></a:r></a:p>");
    let slide1 = slide(&format!(
        "{}{}",
        title_shape("Relatório trimestral: visão geral"),
        body(&format!(
            "{}{}{}{}",
            para("Receita cresceu 12% em relação ao trimestre anterior, puxada pelo segmento corporativo"),
            level2("Contratos novos: 48 clientes"),
            level2("Renovações: 93% da base"),
            para("Próximos passos: contratar duas pessoas de engenharia e revisar o plano de metas para o segundo semestre")
        ))
    ));
    let shape = |id: u32, x: i64, y: i64, w: i64, h: i64, prst: &str, extra: &str, text: &str| {
        format!(
            r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="S{id}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm {extra}><a:off x="{x}" y="{y}"/><a:ext cx="{w}" cy="{h}"/></a:xfrm><a:prstGeom prst="{prst}"><a:avLst/></a:prstGeom></p:spPr><p:style><a:lnRef idx="2"><a:schemeClr val="accent1"><a:shade val="50000"/></a:schemeClr></a:lnRef><a:fillRef idx="1"><a:schemeClr val="accent1"/></a:fillRef><a:effectRef idx="0"><a:schemeClr val="accent1"/></a:effectRef><a:fontRef idx="minor"><a:schemeClr val="lt1"/></a:fontRef></p:style><p:txBody><a:bodyPr rtlCol="0" anchor="ctr"/><a:lstStyle/><a:p><a:pPr algn="ctr"/><a:r><a:rPr lang="pt-BR" sz="1800"/><a:t>{text}</a:t></a:r></a:p></p:txBody></p:sp>"#
        )
    };
    let gradient = r#"<p:sp><p:nvSpPr><p:cNvPr id="20" name="G"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="6858000" y="1371600"/><a:ext cx="3200400" cy="1828800"/></a:xfrm><a:prstGeom prst="roundRect"><a:avLst/></a:prstGeom><a:gradFill><a:gsLst><a:gs pos="0"><a:srgbClr val="FFC000"/></a:gs><a:gs pos="100000"><a:srgbClr val="C00000"/></a:gs></a:gsLst><a:lin ang="2700000"/></a:gradFill><a:ln w="38100"><a:solidFill><a:srgbClr val="203864"/></a:solidFill><a:prstDash val="dash"/></a:ln></p:spPr></p:sp>"#;
    let line = r#"<p:cxnSp><p:nvCxnSpPr><p:cNvPr id="21" name="L"/><p:cNvCxnSpPr/><p:nvPr/></p:nvCxnSpPr><p:spPr><a:xfrm><a:off x="914400" y="4572000"/><a:ext cx="4572000" cy="914400"/></a:xfrm><a:prstGeom prst="line"><a:avLst/></a:prstGeom><a:ln w="28575"><a:solidFill><a:srgbClr val="70AD47"/></a:solidFill></a:ln></p:spPr></p:cxnSp>"#;
    let group = r#"<p:grpSp><p:nvGrpSpPr><p:cNvPr id="22" name="Grp"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="7315200" y="4114800"/><a:ext cx="3200400" cy="1828800"/><a:chOff x="0" y="0"/><a:chExt cx="1600200" cy="914400"/></a:xfrm></p:grpSpPr><p:sp><p:nvSpPr><p:cNvPr id="23" name="a"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="800100" cy="457200"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="5B9BD5"/></a:solidFill></p:spPr></p:sp><p:sp><p:nvSpPr><p:cNvPr id="24" name="b"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="800100" y="457200"/><a:ext cx="800100" cy="457200"/></a:xfrm><a:prstGeom prst="ellipse"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="ED7D31"/></a:solidFill></p:spPr></p:sp></p:grpSp>"#;
    let slide2 = slide(&format!(
        "{}{}{}{}{}{}{}",
        title_shape("Formas e transformações"),
        shape(10, 914400, 1371600, 2743200, 1371600, "rect", "", "Retângulo"),
        shape(11, 3962400, 1371600, 2286000, 1371600, "ellipse", "", "Elipse"),
        shape(12, 914400, 3200400, 1828800, 1143000, "triangle", r#"rot="1200000""#, "Girado"),
        gradient,
        line,
        group
    ));
    let cell = |text: &str, align: &str, props: &str, color: &str| {
        format!(
            r#"<a:tc><a:txBody><a:bodyPr/><a:lstStyle/><a:p><a:pPr algn="{align}"/><a:r><a:rPr lang="pt-BR" {color}/><a:t>{text}</a:t></a:r></a:p></a:txBody><a:tcPr {props}</a:tc>"#
        )
    };
    let head = |text: &str, align: &str| {
        cell(text, align, r#"anchor="ctr"><a:solidFill><a:srgbClr val="4472C4"/></a:solidFill></a:tcPr>"#, r#"b="1""#).replace(
            "<a:rPr lang=\"pt-BR\" b=\"1\"/>",
            "<a:rPr lang=\"pt-BR\" b=\"1\"><a:solidFill><a:srgbClr val=\"FFFFFF\"/></a:solidFill></a:rPr>",
        )
    };
    let plain =
        |text: &str, align: &str| cell(text, align, r#"><a:lnB w="12700"><a:solidFill><a:srgbClr val="A5A5A5"/></a:solidFill></a:lnB></a:tcPr>"#, "");
    let band = |text: &str, align: &str| cell(text, align, r#"><a:solidFill><a:srgbClr val="DAE3F3"/></a:solidFill></a:tcPr>"#, "");
    let table = format!(
        r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="4" name="Table"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><p:xfrm><a:off x="838200" y="1600200"/><a:ext cx="6400800" cy="1600200"/></p:xfrm><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table"><a:tbl><a:tblPr firstRow="1" bandRow="1"/><a:tblGrid><a:gridCol w="2400000"/><a:gridCol w="2000000"/><a:gridCol w="2000800"/></a:tblGrid><a:tr h="457200">{}{}{}</a:tr><a:tr h="457200">{}{}{}</a:tr><a:tr h="457200">{}{}{}</a:tr></a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#,
        head("Produto", "l"),
        head("Quantidade", "r"),
        head("Total (R$)", "r"),
        plain("Licença anual", "l"),
        plain("12", "r"),
        plain("14.400,00", "r"),
        band("Suporte premium com atendimento estendido", "l"),
        band("3", "r"),
        band("2.700,00", "r")
    );
    let slide3 =
        slide(&format!("{}{}{}", title_shape("Tabela e imagem"), table, picture("rId5", r#"<a:srcRect l="10000" t="10000" r="10000" b="10000"/>"#)));
    let styled = |align: &str, runs: &str| format!(r#"<a:p><a:pPr algn="{align}"/>{runs}</a:p>"#);
    let r = |attrs: &str, text: &str| format!(r#"<a:r><a:rPr lang="pt-BR" sz="1800" {attrs}/><a:t>{text}</a:t></a:r>"#);
    let numbered = |text: &str| {
        format!(
            r#"<a:p><a:pPr marL="342900" indent="-342900"><a:buFont typeface="+mj-lt"/><a:buAutoNum type="arabicPeriod"/></a:pPr>{}</a:p>"#,
            r("", text)
        )
    };
    let paragraphs = [
        styled("l", &format!("{}{}{}{}", r("", "Normal, "), r(r#"b="1""#, "negrito, "), r(r#"i="1""#, "itálico, "), r(r#"u="sng""#, "sublinhado"))),
        styled("ctr", &r("", "Centralizado: ação, coração, útil, ÇÃO")),
        styled("r", &r(r#"strike="sngStrike""#, "À direita, tachado")),
        styled(
            "just",
            &r(
                "",
                "Justificado: este parágrafo é longo o bastante para que o texto seja distribuído de margem a margem em mais de uma linha, mostrando o espaçamento entre as palavras.",
            ),
        ),
        styled("l", &format!("{}{}{}{}", r("", "H"), r(r#"baseline="-25000""#, "2"), r("", "O e E=mc"), r(r#"baseline="30000""#, "2"))),
        styled("l", &r(r#"cap="all""#, "texto em caixa alta")),
        numbered("Primeiro item numerado"),
        numbered("Segundo item numerado"),
        styled("l", &r("", "Coluna A\tColuna B\tColuna C")),
    ]
    .concat();
    let text_slide = slide(&format!("{}{}", title_shape("Texto"), text_box(914_400, 1_463_040, 10_363_200, 4_500_000, &paragraphs)));
    let mut d = deck(&[slide1, slide2, slide3, text_slide]);
    d.media = vec![("ppt/media/image1.png".into(), gradient_png())];
    let rel =
        r#"<Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image1.png"/>"#;
    d.slide_rels_extra = vec![String::new(), String::new(), rel.into(), String::new()];
    d
}

#[test]
fn sample_deck_converts_and_can_be_written_for_visual_checks() {
    let d = sample_deck();
    let out = d.convert().unwrap();
    assert_eq!(pages(&out.document).len(), 4);
    if let Ok(dir) = std::env::var("KARNOXPDF_PPTX_SAMPLES") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(std::path::Path::new(&dir).join("sample.pptx"), d.build()).unwrap();
    }
}

#[test]
fn symbols_helvetica_lacks_use_the_standard_symbol_fonts() {
    let p = format!(r#"<a:p><a:r><a:rPr lang="en-US"/><a:t>x {} 5 {} ok {}</a:t></a:r></a:p>"#, '≥', '→', '✓');
    let c = content(&deck(&[slide(&text_box(0, 0, 4_000_000, 600_000, &p))]).convert().unwrap().document, 0);
    assert!(c.contains("/F5 18 Tf") && c.contains("(\u{b3}) Tj"), "≥ is Symbol code 0xB3: {c}");
    assert!(c.contains("(\u{ae}) Tj"), "→ is Symbol code 0xAE: {c}");
    assert!(c.contains("/F6 18 Tf") && c.contains("(3) Tj"), "✓ is ZapfDingbats code 0x33: {c}");
    assert!(c.contains("(x ) Tj") && c.contains("( ok ) Tj"), "the text around them stays Helvetica: {c}");
}

#[test]
fn private_use_symbols_follow_the_runs_symbol_font() {
    let run = |font: &str, text: &str| format!(r#"<a:p><a:r><a:rPr lang="en-US"><a:sym typeface="{font}"/></a:rPr><a:t>{text}</a:t></a:r></a:p>"#);
    let d = deck(&[slide(&text_box(0, 0, 4_000_000, 600_000, &run("Symbol", "\u{f06d}\u{f0b0}")))]);
    let c = content(&d.convert().unwrap().document, 0);
    // μ and ° in the Symbol font are its codes 0x6D and 0xB0.
    assert!(c.contains("/F5 18 Tf") && c.contains("(m\u{b0}) Tj"), "{c}");
    let wingdings = deck(&[slide(&text_box(0, 0, 4_000_000, 600_000, &run("Wingdings", "\u{f0fc}")))]);
    assert!(content(&wingdings.convert().unwrap().document, 0).contains("(4) Tj"), "a Wingdings check mark is ZapfDingbats 0x34");
    let unknown = deck(&[slide(&text_box(0, 0, 4_000_000, 600_000, &run("Webdings", "\u{f041}")))]);
    let CreateError::Invalid(message) = unknown.convert().err().unwrap() else { panic!() };
    assert!(message.contains("U+F041") && message.contains("Webdings") && message.contains("slide 1"), "{message}");
}

#[test]
fn symbol_font_bullets_are_drawn_with_their_glyph() {
    let p = r#"<a:p><a:pPr marL="342900" indent="-342900"><a:buFont typeface="Symbol"/><a:buChar char="&#xF0B7;"/></a:pPr><a:r><a:rPr lang="en-US"/><a:t>item</a:t></a:r></a:p>"#;
    let c = content(&deck(&[slide(&text_box(0, 0, 4_000_000, 600_000, p))]).convert().unwrap().document, 0);
    assert!(c.contains("/F5 18 Tf") && c.contains("(\u{b7}) Tj"), "{c}");
}

#[test]
fn fonts_resource_includes_the_standard_symbolic_fonts() {
    let out = deck(&[slide(&title_shape("x"))]).convert().unwrap();
    let page = pages(&out.document)[0];
    let page_dict = out.document.get(page);
    let resources = page_dict.as_dict().unwrap().get(b"Resources").unwrap().as_dict().unwrap().clone();
    let fonts = resources.get(b"Font").unwrap().as_dict().unwrap().clone();
    let base_font = |key: &[u8]| {
        let object = out.document.get(fonts.get(key).unwrap().as_ref().unwrap());
        object.as_dict().unwrap().name(b"BaseFont").map(|n| String::from_utf8_lossy(n).into_owned())
    };
    assert_eq!(base_font(b"F5").as_deref(), Some("Symbol"));
    assert_eq!(base_font(b"F6").as_deref(), Some("ZapfDingbats"));
}

fn symbol_deck() -> Deck {
    let line = |text: &str| format!(r#"<a:p><a:r><a:rPr lang="pt-BR" sz="2400"/><a:t>{text}</a:t></a:r></a:p>"#);
    let sym = |font: &str, text: &str| {
        format!(r#"<a:p><a:r><a:rPr lang="pt-BR" sz="2400"><a:sym typeface="{font}"/></a:rPr><a:t>{text}</a:t></a:r></a:p>"#)
    };
    let paragraphs = [
        line("Maior ou igual: x ≥ 5, seta → direita, ← esquerda, ↔ ambos"),
        line("Raiz √2 ≈ 1,41; infinito ∞; diferente ≠; soma ∑; delta Δ; alfa α β γ; mu μ"),
        line("Marcas: ✓ ✔ ✗ ✘ ★ ● ■ ▲ ▼ ◆ ► ➔ ➢"),
        sym("Symbol", "Símbolo PUA: \u{f06d}g/mL a 37\u{f0b0}C \u{f0b3} \u{f0ae}"),
        sym("Wingdings", "Wingdings: \u{f0fc} \u{f0fb} \u{f06c} \u{f06e} \u{f0d8}"),
    ]
    .concat();
    deck(&[slide(&format!("{}{}", title_shape("Símbolos"), text_box(914_400, 1_463_040, 10_363_200, 4_500_000, &paragraphs)))])
}

#[test]
fn symbol_sample_deck_for_visual_checks() {
    let d = symbol_deck();
    d.convert().unwrap();
    if let Ok(dir) = std::env::var("KARNOXPDF_PPTX_SAMPLES") {
        std::fs::write(std::path::Path::new(&dir).join("symbols.pptx"), d.build()).unwrap();
    }
}

// ── robustness: damaged and hostile decks must come back as errors, never as panics or hangs ──

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

fn boundary(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

fn mutate_text(rng: &mut Rng, text: &str) -> String {
    const EXTREMES: [&str; 12] =
        ["0", "-1", "1", "2147483647", "-2147483648", "99999999999999", "-99999999999", "1e999", "0.00000001", "NaN", "", "abc"];
    const TAGS: [&str; 10] = ["p:sp", "p:pic", "p:grpSp", "a:tbl", "a:tc", "a:p", "a:r", "a:ln", "a:xfrm", "p:graphicFrame"];
    let mut out = text.to_owned();
    for _ in 0..1 + rng.below(3) {
        let at = boundary(&out, rng.below(out.len().max(1)));
        match rng.below(6) {
            0 => {
                let end = boundary(&out, at + 1 + rng.below(200));
                out.replace_range(at..end, "");
            }
            1 => {
                let end = boundary(&out, at + 1 + rng.below(300));
                let copy = out[at..end].to_owned();
                out.insert_str(at, &copy);
            }
            2 | 3 => {
                // Replace an attribute value (or a number) with an extreme one.
                if let Some(start) = out[at..].find("=\"") {
                    let from = at + start + 2;
                    if let Some(len) = out[from..].find('"') {
                        out.replace_range(from..from + len, EXTREMES[rng.below(EXTREMES.len())]);
                    }
                }
            }
            4 => {
                let end = boundary(&out, at);
                out.truncate(end);
            }
            _ => {
                // Rename a tag: structure in the wrong place.
                if let Some(start) = out[at..].find('<') {
                    let from = at + start + 1;
                    if out[from..].starts_with(|c: char| c.is_ascii_alphabetic()) {
                        let len = out[from..].find(|c: char| !(c.is_ascii_alphanumeric() || c == ':')).unwrap_or(0);
                        out.replace_range(from..from + len, TAGS[rng.below(TAGS.len())]);
                    }
                }
            }
        }
    }
    out
}

#[test]
fn mutated_decks_never_panic() {
    let started = std::time::Instant::now();
    let decks = [sample_deck(), symbol_deck()];
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let (mut converted, mut refused) = (0, 0);
    let rounds = std::env::var("KARNOXPDF_FUZZ_ROUNDS").ok().and_then(|v| v.parse().ok()).unwrap_or(1500);
    for round in 0..rounds {
        let deck = &decks[round % decks.len()];
        let mut target = rng.below(8);
        let bytes = deck.build_with(&mut |name, text| {
            // One in three parts is damaged: the part picked changes with the round.
            if name.ends_with(".xml") && !name.contains("_rels") {
                if target == 0 {
                    target = usize::MAX;
                    return mutate_text(&mut rng, text);
                }
                target = target.saturating_sub(1);
            }
            text.to_owned()
        });
        match from_pptx_layout("fuzz", &bytes) {
            Ok(_) => converted += 1,
            Err(_) => refused += 1,
        }
    }
    assert!(converted > 100 && refused > 50, "the mutations should both break and spare decks ({converted} converted, {refused} refused)");
    assert!(rounds > 1500 || started.elapsed().as_secs() < 120, "fuzzing took {:?}", started.elapsed());
}

#[test]
fn hostile_inputs_are_bounded() {
    // One enormous unbreakable word in a narrow box is laid out in linear time.
    let word = "W".repeat(300_000);
    let started = std::time::Instant::now();
    let out = deck(&[slide(&text_box(0, 0, 100_000, 100_000, &run(&word)))]).convert();
    assert!(out.is_ok());
    assert!(started.elapsed().as_secs() < 20, "{:?}", started.elapsed());
    // Too much text in one box is refused.
    let huge = "word ".repeat(100_000);
    assert!(
        matches!(deck(&[slide(&text_box(0, 0, 100_000, 100_000, &run(&huge)))]).convert(), Err(CreateError::Invalid(m)) if m.contains("too much text"))
    );
    // Groups nested far beyond any real deck stop at the depth limit instead of recursing forever.
    let mut nested = String::from(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="9" name="x"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="10" cy="10"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="000000"/></a:solidFill></p:spPr></p:sp>"#,
    );
    for _ in 0..200 {
        nested = format!(
            r#"<p:grpSp><p:nvGrpSpPr><p:cNvPr id="3" name="g"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="10" cy="10"/><a:chOff x="0" y="0"/><a:chExt cx="10" cy="10"/></a:xfrm></p:grpSpPr>{nested}</p:grpSp>"#
        );
    }
    let out = deck(&[slide(&nested)]).convert().unwrap();
    assert!(out.notes.iter().any(|n| n.kind == NoteKind::Other), "{:?}", out.notes);
}

#[test]
fn a_slide_can_override_the_color_mapping_of_its_master() {
    let mapping = r#"<p:clrMapOvr><a:overrideClrMapping bg1="dk1" tx1="lt1" bg2="dk2" tx2="lt2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/></p:clrMapOvr>"#;
    let dark = slide(&title_shape("Dark")).replace("</p:cSld>", &format!("</p:cSld>{mapping}"));
    let out = deck(&[dark, slide(&title_shape("Light"))]).convert().unwrap();
    let (dark_page, light_page) = (content(&out.document, 0), content(&out.document, 1));
    // The master's background is bg1: white normally, black when bg1 is remapped to dk1; its title
    // text is tx1: black normally, white when remapped to lt1.
    assert!(dark_page.starts_with("1 0 0 -1 0 540 cm\nq\n0 0 0 rg"), "{dark_page}");
    assert!(dark_page.contains("1 1 1 rg\nBT /F2 44 Tf"), "{dark_page}");
    assert!(light_page.starts_with("1 0 0 -1 0 540 cm\nq\n1 1 1 rg"), "{light_page}");
    assert!(light_page.contains("0 0 0 rg\nBT /F2 44 Tf"), "{light_page}");
}

#[test]
fn hyperlink_runs_use_the_theme_hyperlink_color_and_an_underline() {
    let p = r#"<a:p><a:r><a:rPr lang="en-US"><a:hlinkClick r:id="rId9"/></a:rPr><a:t>site</a:t></a:r></a:p>"#;
    let c = content(&deck(&[slide(&text_box(0, 0, 2_000_000, 500_000, p))]).convert().unwrap().document, 0);
    assert!(c.contains("0.02 0.388 0.757 rg"), "hlink is 0563C1: {c}");
    assert!(c.contains("0.02 0.388 0.757 RG"), "underlined: {c}");
}

#[test]
fn shape_kinds_the_converter_does_not_know_are_reported() {
    let out = deck(&[slide(r#"<p:contentPart r:id="rId9"/>"#)]).convert().unwrap();
    assert_eq!(out.notes.iter().map(|n| n.kind).collect::<Vec<_>>(), vec![NoteKind::Other]);
}

#[test]
fn combining_accents_are_composed_instead_of_refused() {
    let p = format!(r#"<a:p><a:r><a:rPr lang="pt-BR"/><a:t>a{}ç{}</a:t></a:r></a:p>"#, '\u{301}', '\u{327}');
    // "á" then a cedilla that follows "ç" has no base to compose with: refused.
    assert!(deck(&[slide(&text_box(0, 0, 4_000_000, 600_000, &p))]).convert().is_err());
    let p = format!(r#"<a:p><a:r><a:rPr lang="pt-BR"/><a:t>informac{}a{}o</a:t></a:r></a:p>"#, '\u{327}', '\u{303}');
    let c = content(&deck(&[slide(&text_box(0, 0, 4_000_000, 600_000, &p))]).convert().unwrap().document, 0);
    assert!(c.contains("(informa\u{e7}\u{e3}o) Tj"), "{c}");
}
