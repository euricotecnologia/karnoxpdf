//! Bounded extraction of parts and text from a DOCX package.

use std::io::Read;

use flate2::read::DeflateDecoder;

use crate::{CreateError, LETTER, from_text};

const MAX_PACKAGE: usize = 64 * 1024 * 1024;
const MAX_XML: usize = 16 * 1024 * 1024;
const MAX_TEXT: usize = 256 * 1024;

fn bad(message: &str) -> CreateError {
    CreateError::Invalid(message.into())
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at.checked_add(2)?)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at.checked_add(4)?)?.try_into().ok()?))
}

/// Read one named ZIP part without trusting the package's offsets, sizes or checksum.
pub(crate) fn part(bytes: &[u8], wanted: &[u8], limit: usize) -> Result<Option<Vec<u8>>, CreateError> {
    if bytes.len() > MAX_PACKAGE {
        return Err(bad("DOCX exceeds the 64 MB import limit"));
    }
    let eocd = bytes
        .windows(4)
        .rposition(|v| v == b"PK\x05\x06")
        .filter(|at| at.checked_add(22).is_some_and(|end| end <= bytes.len()))
        .ok_or_else(|| bad("not a valid DOCX ZIP package"))?;
    let count = u16_at(bytes, eocd + 10).ok_or_else(|| bad("invalid DOCX directory"))? as usize;
    let directory_size = u32_at(bytes, eocd + 12).ok_or_else(|| bad("invalid DOCX directory"))? as usize;
    let directory = u32_at(bytes, eocd + 16).ok_or_else(|| bad("invalid DOCX directory"))? as usize;
    if count > 10_000 || directory.checked_add(directory_size).is_none_or(|end| end > eocd) {
        return Err(bad("invalid or unsupported DOCX directory"));
    }
    let mut cursor = directory;
    let mut document = None;
    for _ in 0..count {
        if u32_at(bytes, cursor) != Some(0x0201_4b50) {
            return Err(bad("invalid DOCX directory entry"));
        }
        let flags = u16_at(bytes, cursor + 8).ok_or_else(|| bad("invalid DOCX entry"))?;
        let method = u16_at(bytes, cursor + 10).ok_or_else(|| bad("invalid DOCX entry"))?;
        let crc = u32_at(bytes, cursor + 16).ok_or_else(|| bad("invalid DOCX entry"))?;
        let compressed = u32_at(bytes, cursor + 20).ok_or_else(|| bad("invalid DOCX entry"))? as usize;
        let expanded = u32_at(bytes, cursor + 24).ok_or_else(|| bad("invalid DOCX entry"))? as usize;
        let name_len = u16_at(bytes, cursor + 28).ok_or_else(|| bad("invalid DOCX entry"))? as usize;
        let extra_len = u16_at(bytes, cursor + 30).ok_or_else(|| bad("invalid DOCX entry"))? as usize;
        let comment_len = u16_at(bytes, cursor + 32).ok_or_else(|| bad("invalid DOCX entry"))? as usize;
        let offset = u32_at(bytes, cursor + 42).ok_or_else(|| bad("invalid DOCX entry"))? as usize;
        let name_start = cursor.checked_add(46).ok_or_else(|| bad("invalid DOCX entry"))?;
        let name_end = name_start.checked_add(name_len).ok_or_else(|| bad("invalid DOCX entry"))?;
        let name = bytes.get(name_start..name_end).ok_or_else(|| bad("invalid DOCX entry"))?;
        cursor = name_end
            .checked_add(extra_len)
            .and_then(|v| v.checked_add(comment_len))
            .filter(|v| *v <= directory + directory_size)
            .ok_or_else(|| bad("invalid DOCX entry length"))?;
        if name != wanted {
            continue;
        }
        if document.is_some() {
            return Err(bad("DOCX contains duplicate document parts"));
        }
        if flags & 1 != 0 || expanded > limit || compressed > MAX_PACKAGE {
            return Err(bad("DOCX document part is encrypted or too large"));
        }
        if u32_at(bytes, offset) != Some(0x0403_4b50) {
            return Err(bad("invalid DOCX document header"));
        }
        let local_name = u16_at(bytes, offset + 26).ok_or_else(|| bad("invalid DOCX document header"))? as usize;
        let local_extra = u16_at(bytes, offset + 28).ok_or_else(|| bad("invalid DOCX document header"))? as usize;
        let start = offset
            .checked_add(30)
            .and_then(|v| v.checked_add(local_name))
            .and_then(|v| v.checked_add(local_extra))
            .ok_or_else(|| bad("invalid DOCX document offset"))?;
        let local_start = offset.checked_add(30).ok_or_else(|| bad("invalid DOCX document offset"))?;
        let local_end = local_start.checked_add(local_name).ok_or_else(|| bad("invalid DOCX document offset"))?;
        if bytes.get(local_start..local_end) != Some(name) {
            return Err(bad("DOCX document entry names differ"));
        }
        let data = bytes
            .get(start..start.checked_add(compressed).ok_or_else(|| bad("invalid DOCX document size"))?)
            .ok_or_else(|| bad("truncated DOCX document part"))?;
        let xml = match method {
            0 => data.to_vec(),
            8 => {
                let mut out = Vec::new();
                DeflateDecoder::new(data)
                    .take((limit.saturating_add(1)) as u64)
                    .read_to_end(&mut out)
                    .map_err(|e| CreateError::Invalid(format!("could not decompress DOCX text: {e}")))?;
                out
            }
            _ => return Err(bad("unsupported DOCX compression method")),
        };
        if xml.len() != expanded || xml.len() > limit || crc32fast::hash(&xml) != crc {
            return Err(bad("DOCX document part failed integrity validation"));
        }
        document = Some(xml);
    }
    Ok(document)
}

/// Extract paragraph text, tabs and line breaks from a `.docx` file.
pub fn docx_text(bytes: &[u8]) -> Result<String, CreateError> {
    let xml = part(bytes, b"word/document.xml", MAX_XML)?.ok_or_else(|| bad("DOCX has no word/document.xml part"))?;
    let xml = std::str::from_utf8(&xml).map_err(|_| bad("DOCX XML is not UTF-8"))?;
    let tree = roxmltree::Document::parse_with_options(xml, roxmltree::ParsingOptions { allow_dtd: false, ..Default::default() })
        .map_err(|e| CreateError::Invalid(format!("invalid DOCX XML: {e}")))?;
    let mut text = String::new();
    let mut first = true;
    for paragraph in tree.descendants().filter(|n| n.is_element() && n.tag_name().name() == "p") {
        if !first {
            text.push('\n');
        }
        first = false;
        for node in paragraph.descendants().filter(|n| n.is_element()) {
            match node.tag_name().name() {
                "t" => {
                    if let Some(value) = node.text() {
                        text.push_str(value);
                    }
                }
                "tab" => text.push('\t'),
                "br" => {
                    if node.attributes().any(|a| a.name() == "type" && a.value() == "page") {
                        text.push('\u{c}');
                    } else {
                        text.push('\n');
                    }
                }
                "cr" => text.push('\n'),
                _ => {}
            }
        }
        if text.len() > MAX_TEXT {
            return Err(bad("DOCX text exceeds the 256 KB import limit"));
        }
    }
    if text.trim().is_empty() {
        return Err(bad("DOCX contains no supported text; images and drawings are not converted yet"));
    }
    Ok(text)
}

/// Convert the extracted DOCX text to a new PDF. Formatting is not retained.
pub fn from_docx(title: &str, bytes: &[u8]) -> Result<karnoxpdf_cos::Document, CreateError> {
    let text = docx_text(bytes)?;
    for c in text.chars() {
        if matches!(c, '\n' | '\r' | '\t' | '\u{c}') {
            continue;
        }
        if c != '?' && crate::win_ansi(&c.to_string()) == b"?" {
            return Err(CreateError::Invalid(format!(
                "DOCX contains {c:?}, which the current PDF text font cannot encode; conversion stopped to avoid changing the text"
            )));
        }
    }
    from_text(title, &text, LETTER, 11.0)
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
    fn malformed_package_is_an_error() {
        assert!(docx_text(b"not a zip").is_err());
        assert!(docx_text(&[0; 100]).is_err());
    }

    #[test]
    fn paragraphs_tabs_entities_and_page_breaks() {
        let bytes = package(
            r#"<w:document xmlns:w="urn:word"><w:body><w:p><w:r><w:t>Alpha &amp; Beta</w:t></w:r><w:tab/><w:t>line</w:t></w:p><w:p><w:t>second</w:t><w:br w:type="page"/><w:t>end</w:t></w:p></w:body></w:document>"#,
        );
        assert_eq!(docx_text(&bytes).unwrap(), "Alpha & Beta\tline\nsecond\u{c}end");
    }

    #[test]
    fn non_latin_text_is_rejected_before_writing_pdf() {
        let bytes = package(r#"<w:document xmlns:w="urn:word"><w:body><w:p><w:t>漢字</w:t></w:p></w:body></w:document>"#);
        assert!(from_docx("word", &bytes).is_err());
    }
}
