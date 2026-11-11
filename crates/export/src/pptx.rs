//! Editable presentation export: one slide per PDF page with positioned text and images.

use std::fmt::Write;

use crate::{Page, Zip, esc};

const MAX_PAGES: usize = 1024;
const MAX_INPUT_BYTES: usize = 256 * 1024 * 1024;
const EMU_PER_POINT: f64 = 12_700.0;

struct SlideParts {
    xml: String,
    rels: String,
    media: Vec<(String, Vec<u8>)>,
}

pub fn pptx(pages: &[Page]) -> Result<Vec<u8>, String> {
    if pages.is_empty() {
        return Err("The PDF has no pages to export.".into());
    }
    if pages.len() > MAX_PAGES {
        return Err(format!("The PDF has too many pages for presentation export (maximum {MAX_PAGES})."));
    }
    let input_bytes = pages
        .iter()
        .flat_map(|p| p.blocks.iter().map(|b| b.text.len()).chain(p.images.iter().map(|i| i.bytes.len())))
        .try_fold(0usize, |n, len| n.checked_add(len))
        .ok_or("The extracted content is too large to export.")?;
    if input_bytes > MAX_INPUT_BYTES {
        return Err("The extracted content is too large to export (maximum 256 MB).".into());
    }
    let image_count = pages.iter().try_fold(0usize, |n, p| n.checked_add(p.images.len())).ok_or("The PDF contains too many images to export.")?;
    if image_count > 50_000 {
        return Err("The PDF contains too many images to export (maximum 50,000).".into());
    }
    let shape_count =
        pages.iter().try_fold(0usize, |n, p| n.checked_add(p.blocks.len())).ok_or("The PDF contains too many text blocks to export.")?;
    if shape_count > 100_000 {
        return Err("The PDF contains too many text blocks to export (maximum 100,000).".into());
    }
    let first = pages.first().ok_or("The PDF has no pages to export.")?;
    let width = dimension(first.width, 720.0);
    let height = dimension(first.height, 540.0);
    let slide_w = emu(width);
    let slide_h = emu(height);
    let mut zip = Zip::default();
    let mut types = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Default Extension=\"png\" ContentType=\"image/png\"/><Default Extension=\"jpg\" ContentType=\"image/jpeg\"/><Override PartName=\"/ppt/presentation.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml\"/><Override PartName=\"/ppt/slideMasters/slideMaster1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml\"/><Override PartName=\"/ppt/slideLayouts/slideLayout1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml\"/><Override PartName=\"/ppt/theme/theme1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.theme+xml\"/><Override PartName=\"/ppt/presProps.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.presProps+xml\"/>",
    );
    for i in 1..=pages.len() {
        let _ = write!(
            types,
            "<Override PartName=\"/ppt/slides/slide{i}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/>"
        );
    }
    types.push_str("</Types>");
    zip.add("[Content_Types].xml", types.as_bytes(), true);
    zip.add("_rels/.rels", b"<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"ppt/presentation.xml\"/></Relationships>", true);
    let mut presentation = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><p:presentation xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><p:sldMasterIdLst><p:sldMasterId id=\"2147483648\" r:id=\"rId1\"/></p:sldMasterIdLst><p:sldIdLst>",
    );
    let mut rels = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster\" Target=\"slideMasters/slideMaster1.xml\"/>",
    );
    for i in 1..=pages.len() {
        let _ = write!(presentation, "<p:sldId id=\"{}\" r:id=\"rId{}\"/>", i + 255, i + 1);
        let _ = write!(
            rels,
            "<Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide\" Target=\"slides/slide{i}.xml\"/>",
            i + 1
        );
    }
    let _ =
        write!(presentation, "</p:sldIdLst><p:sldSz cx=\"{slide_w}\" cy=\"{slide_h}\"/><p:notesSz cx=\"6858000\" cy=\"9144000\"/></p:presentation>");
    rels.push_str("<Relationship Id=\"rIdProps\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/presProps\" Target=\"presProps.xml\"/></Relationships>");
    zip.add("ppt/presentation.xml", presentation.as_bytes(), true);
    zip.add("ppt/_rels/presentation.xml.rels", rels.as_bytes(), true);
    zip.add(
        "ppt/presProps.xml",
        b"<?xml version=\"1.0\" encoding=\"UTF-8\"?><p:presentationPr xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"/>",
        true,
    );
    zip.add("ppt/slideMasters/slideMaster1.xml", b"<?xml version=\"1.0\" encoding=\"UTF-8\"?><p:sldMaster xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld><p:clrMap accent1=\"accent1\" accent2=\"accent2\" accent3=\"accent3\" accent4=\"accent4\" accent5=\"accent5\" accent6=\"accent6\" bg1=\"lt1\" bg2=\"lt2\" folHlink=\"folHlink\" hlink=\"hlink\" tx1=\"dk1\" tx2=\"dk2\"/><p:sldLayoutIdLst><p:sldLayoutId id=\"2147483649\" r:id=\"rId1\"/></p:sldLayoutIdLst><p:txStyles><p:titleStyle/><p:bodyStyle/><p:otherStyle/></p:txStyles></p:sldMaster>", true);
    zip.add("ppt/slideMasters/_rels/slideMaster1.xml.rels", b"<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout\" Target=\"../slideLayouts/slideLayout1.xml\"/><Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme\" Target=\"../theme/theme1.xml\"/></Relationships>", true);
    zip.add("ppt/slideLayouts/slideLayout1.xml", b"<?xml version=\"1.0\" encoding=\"UTF-8\"?><p:sldLayout xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" type=\"blank\" preserve=\"1\"><p:cSld name=\"Blank\"><p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>", true);
    zip.add("ppt/slideLayouts/_rels/slideLayout1.xml.rels", b"<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster\" Target=\"../slideMasters/slideMaster1.xml\"/></Relationships>", true);
    zip.add("ppt/theme/theme1.xml", b"<?xml version=\"1.0\" encoding=\"UTF-8\"?><a:theme xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" name=\"KarnoxPDF\"><a:themeElements><a:clrScheme name=\"Office\"><a:dk1><a:sysClr val=\"windowText\" lastClr=\"000000\"/></a:dk1><a:lt1><a:sysClr val=\"window\" lastClr=\"FFFFFF\"/></a:lt1><a:dk2><a:srgbClr val=\"1F2937\"/></a:dk2><a:lt2><a:srgbClr val=\"F3F4F6\"/></a:lt2><a:accent1><a:srgbClr val=\"2563EB\"/></a:accent1><a:accent2><a:srgbClr val=\"16A34A\"/></a:accent2><a:accent3><a:srgbClr val=\"EA580C\"/></a:accent3><a:accent4><a:srgbClr val=\"9333EA\"/></a:accent4><a:accent5><a:srgbClr val=\"0891B2\"/></a:accent5><a:accent6><a:srgbClr val=\"DB2777\"/></a:accent6><a:hlink><a:srgbClr val=\"2563EB\"/></a:hlink><a:folHlink><a:srgbClr val=\"9333EA\"/></a:folHlink></a:clrScheme><a:fontScheme name=\"Office\"><a:majorFont><a:latin typeface=\"Liberation Sans\"/></a:majorFont><a:minorFont><a:latin typeface=\"Liberation Sans\"/></a:minorFont></a:fontScheme><a:fmtScheme name=\"Office\"><a:fillStyleLst><a:solidFill><a:schemeClr val=\"accent1\"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w=\"9525\"><a:solidFill><a:schemeClr val=\"accent1\"/></a:solidFill></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val=\"lt1\"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>", true);
    for (i, page) in pages.iter().enumerate() {
        let parts = slide(page, width, height, i + 1)?;
        zip.add(&format!("ppt/slides/slide{}.xml", i + 1), parts.xml.as_bytes(), true);
        zip.add(&format!("ppt/slides/_rels/slide{}.xml.rels", i + 1), parts.rels.as_bytes(), true);
        for (path, bytes) in parts.media {
            zip.add(&path, &bytes, false);
        }
    }
    Ok(zip.finish())
}

fn dimension(value: f64, fallback: f64) -> f64 {
    if value.is_finite() && (1.0..=2000.0).contains(&value) { value } else { fallback }
}
fn emu(points: f64) -> i64 {
    (points.max(0.0) * EMU_PER_POINT).round() as i64
}

fn slide(page: &Page, width: f64, height: f64, page_no: usize) -> Result<SlideParts, String> {
    let source_w = dimension(page.width, width);
    let source_h = dimension(page.height, height);
    let sx = width / source_w;
    let sy = height / source_h;
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><p:sld xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>",
    );
    let mut rels = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout\" Target=\"../slideLayouts/slideLayout1.xml\"/>",
    );
    let shapes_start = xml.len();
    for (i, block) in page.blocks.iter().enumerate() {
        if block.text.trim().is_empty() {
            continue;
        }
        let [x0, y0, x1, y1] = block.rect;
        if ![x0, y0, x1, y1].iter().all(|n| n.is_finite()) {
            continue;
        }
        let x = emu(x0.max(0.0) * sx);
        let y = emu((source_h - y1).max(0.0) * sy);
        let w = emu(((x1 - x0).max(1.0) * sx).max(12.0));
        let h = emu(((y1 - y0).max(block.size * 1.3).max(1.0) * sy).max(12.0));
        let font = (block.size.clamp(6.0, 120.0) * 100.0).round() as i64;
        let bold = if block.bold { " b=\"1\"" } else { "" };
        let italic = if block.italic { " i=\"1\"" } else { "" };
        let _ = write!(
            xml,
            "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"Text {}\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x=\"{x}\" y=\"{y}\"/><a:ext cx=\"{w}\" cy=\"{h}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:noFill/><a:ln><a:noFill/></a:ln></p:spPr><p:txBody><a:bodyPr wrap=\"none\" lIns=\"0\" tIns=\"0\" rIns=\"0\" bIns=\"0\"/><a:lstStyle/>",
            i + 2,
            i + 1
        );
        for line in block.text.split('\n') {
            let _ = write!(
                xml,
                "<a:p><a:r><a:rPr lang=\"en-US\" sz=\"{font}\"{bold}{italic}/><a:t>{}</a:t></a:r><a:endParaRPr lang=\"en-US\"/></a:p>",
                esc(line)
            );
        }
        xml.push_str("</p:txBody></p:sp>");
    }
    // Pictures normally sit behind editable text, so a scanned page with an OCR layer keeps
    // that layer visible above its source image.
    let text_shapes = xml.split_off(shapes_start);
    let mut media = Vec::new();
    for (i, image) in page.images.iter().enumerate() {
        let [x0, y0, x1, y1] = image.rect;
        if ![x0, y0, x1, y1].iter().all(|n| n.is_finite()) || x1 <= x0 || y1 <= y0 {
            continue;
        }
        let ext = if image.ext == "jpg" { "jpg" } else { "png" };
        let name = format!("image{page_no}_{}.{}", i + 1, ext);
        let path = format!("ppt/media/{name}");
        let rid = i + 2;
        let _ = write!(
            rels,
            "<Relationship Id=\"rId{rid}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"../media/{name}\"/>"
        );
        let _ = write!(
            xml,
            "<p:pic><p:nvPicPr><p:cNvPr id=\"{}\" name=\"Image {}\"/><p:cNvPicPr><a:picLocks noChangeAspect=\"1\"/></p:cNvPicPr><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed=\"rId{rid}\"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr><a:xfrm><a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr></p:pic>",
            page.blocks.len() + i + 2,
            i + 1,
            emu(x0.max(0.0) * sx),
            emu((source_h - y1).max(0.0) * sy),
            emu((x1 - x0) * sx),
            emu((y1 - y0) * sy)
        );
        media.push((path, image.bytes.clone()));
    }
    xml.push_str(&text_shapes);
    xml.push_str("</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>");
    rels.push_str("</Relationships>");
    Ok(SlideParts { xml, rels, media })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn positioned_text_is_editable_and_escaped() {
        let p = Page {
            width: 612.0,
            height: 792.0,
            blocks: vec![crate::Block { text: "A & B".into(), rect: [20.0, 700.0, 100.0, 720.0], size: 12.0, bold: true, italic: false }],
            images: vec![],
        };
        let parts = slide(&p, 612.0, 792.0, 1).unwrap();
        assert!(parts.xml.contains("A &amp; B"));
        assert!(parts.xml.contains("<p:sp>"));
        assert!(pptx(&[p]).unwrap().starts_with(b"PK"));
    }
}
