//! Bounded reader for Office Open XML packages (ISO/IEC 29500-2 OPC): a ZIP archive of parts
//! tied together by relationship parts. Nothing in the archive is trusted: sizes, offsets and
//! checksums are validated, and every expansion is capped.

use std::cell::Cell;
use std::collections::HashMap;
use std::io::Read;

use flate2::read::DeflateDecoder;
use roxmltree::Node;

use crate::CreateError;

/// Everything read out of one package, after decompression.
const MAX_TOTAL: usize = 768 * 1024 * 1024;
const MAX_ENTRIES: usize = 20_000;

pub(crate) fn bad(message: impl Into<String>) -> CreateError {
    CreateError::Invalid(message.into())
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at.checked_add(2)?)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at.checked_add(4)?)?.try_into().ok()?))
}

#[derive(Clone, Copy)]
struct Entry {
    flags: u16,
    method: u16,
    crc: u32,
    compressed: usize,
    expanded: usize,
    offset: usize,
}

/// An opened package: the ZIP central directory, read once. Part names are matched without
/// regard to case, as OPC requires.
pub(crate) struct Package<'a> {
    bytes: &'a [u8],
    limit: usize,
    entries: HashMap<String, (String, Entry)>,
    budget: Cell<usize>,
}

impl<'a> Package<'a> {
    /// Open a package of at most `limit` bytes.
    pub(crate) fn open(bytes: &'a [u8], what: &str, limit: usize) -> Result<Self, CreateError> {
        let corrupt = || bad(format!("not a valid {what} ZIP package"));
        if bytes.len() > limit {
            return Err(bad(format!("{what} exceeds the {} MB import limit", limit / (1024 * 1024))));
        }
        let eocd = bytes
            .windows(4)
            .rposition(|v| v == b"PK\x05\x06")
            .filter(|at| at.checked_add(22).is_some_and(|end| end <= bytes.len()))
            .ok_or_else(corrupt)?;
        let count = u16_at(bytes, eocd + 10).ok_or_else(corrupt)? as usize;
        let directory_size = u32_at(bytes, eocd + 12).ok_or_else(corrupt)? as usize;
        let directory = u32_at(bytes, eocd + 16).ok_or_else(corrupt)? as usize;
        if count == 0xFFFF || directory_size == 0xFFFF_FFFF || directory == 0xFFFF_FFFF {
            return Err(bad(format!("ZIP64 {what} packages are not supported")));
        }
        if count > MAX_ENTRIES || directory.checked_add(directory_size).is_none_or(|end| end > eocd) {
            return Err(bad(format!("invalid or unsupported {what} directory")));
        }
        let directory_end = directory + directory_size;
        let mut cursor = directory;
        let mut entries = HashMap::new();
        for _ in 0..count {
            if u32_at(bytes, cursor) != Some(0x0201_4b50) {
                return Err(bad(format!("invalid {what} directory entry")));
            }
            let field16 = |at: usize| u16_at(bytes, cursor + at).ok_or_else(corrupt);
            let field32 = |at: usize| u32_at(bytes, cursor + at).ok_or_else(corrupt);
            let entry = Entry {
                flags: field16(8)?,
                method: field16(10)?,
                crc: field32(16)?,
                compressed: field32(20)? as usize,
                expanded: field32(24)? as usize,
                offset: field32(42)? as usize,
            };
            let (name_len, extra_len, comment_len) = (field16(28)? as usize, field16(30)? as usize, field16(32)? as usize);
            let name_start = cursor.checked_add(46).ok_or_else(corrupt)?;
            let name_end = name_start.checked_add(name_len).ok_or_else(corrupt)?;
            let name = bytes.get(name_start..name_end).ok_or_else(corrupt)?;
            cursor = name_end
                .checked_add(extra_len)
                .and_then(|v| v.checked_add(comment_len))
                .filter(|v| *v <= directory_end)
                .ok_or_else(|| bad(format!("invalid {what} entry length")))?;
            let name = String::from_utf8_lossy(name).into_owned();
            if name.ends_with('/') {
                continue;
            }
            if entries.insert(name.to_ascii_lowercase(), (name, entry)).is_some() {
                return Err(bad(format!("{what} contains duplicate parts")));
            }
        }
        Ok(Self { bytes, limit, entries, budget: Cell::new(MAX_TOTAL) })
    }

    /// One part's bytes, checked against the directory, or `None` when the package has no such
    /// part. `limit` caps the decompressed size.
    pub(crate) fn read(&self, name: &str, limit: usize) -> Result<Option<Vec<u8>>, CreateError> {
        let Some((exact, entry)) = self.entries.get(&name.to_ascii_lowercase()) else { return Ok(None) };
        let bytes = self.bytes;
        let corrupt = || bad(format!("part {exact} is damaged"));
        if entry.flags & 1 != 0 {
            return Err(bad(format!("part {exact} is encrypted")));
        }
        if entry.expanded > limit || entry.compressed > self.limit {
            return Err(bad(format!("part {exact} is too large")));
        }
        self.budget.set(self.budget.get().checked_sub(entry.expanded).ok_or_else(|| bad("the package expands to too much data"))?);
        let offset = entry.offset;
        if u32_at(bytes, offset) != Some(0x0403_4b50) {
            return Err(corrupt());
        }
        let local_name = u16_at(bytes, offset + 26).ok_or_else(corrupt)? as usize;
        let local_extra = u16_at(bytes, offset + 28).ok_or_else(corrupt)? as usize;
        let name_start = offset.checked_add(30).ok_or_else(corrupt)?;
        let start = name_start.checked_add(local_name).and_then(|v| v.checked_add(local_extra)).ok_or_else(corrupt)?;
        if bytes.get(name_start..name_start.checked_add(local_name).ok_or_else(corrupt)?) != Some(exact.as_bytes()) {
            return Err(bad(format!("part {exact} has inconsistent names")));
        }
        let data = bytes.get(start..start.checked_add(entry.compressed).ok_or_else(corrupt)?).ok_or_else(corrupt)?;
        let out = match entry.method {
            0 => data.to_vec(),
            8 => {
                let mut out = Vec::with_capacity(entry.expanded.min(1 << 20));
                DeflateDecoder::new(data)
                    .take((limit.saturating_add(1)) as u64)
                    .read_to_end(&mut out)
                    .map_err(|e| bad(format!("could not decompress {exact}: {e}")))?;
                out
            }
            _ => return Err(bad(format!("part {exact} uses an unsupported compression method"))),
        };
        if out.len() != entry.expanded || out.len() > limit || crc32fast::hash(&out) != entry.crc {
            return Err(bad(format!("part {exact} failed integrity validation")));
        }
        Ok(Some(out))
    }

    /// A UTF-8 XML part as text (a byte-order mark is dropped).
    pub(crate) fn read_xml(&self, name: &str, limit: usize) -> Result<Option<String>, CreateError> {
        let Some(bytes) = self.read(name, limit)? else { return Ok(None) };
        let text = String::from_utf8(bytes).map_err(|_| bad(format!("part {name} is not UTF-8 XML")))?;
        Ok(Some(text.strip_prefix('\u{feff}').map(str::to_owned).unwrap_or(text)))
    }
}

pub(crate) fn parse_xml(text: &str) -> Result<roxmltree::Document<'_>, CreateError> {
    roxmltree::Document::parse_with_options(text, roxmltree::ParsingOptions { allow_dtd: false, nodes_limit: 4_000_000 })
        .map_err(|e| bad(format!("invalid XML: {e}")))
}

/// One relationship of a part (ISO/IEC 29500-2 §9.3).
#[derive(Clone, Debug)]
pub(crate) struct Rel {
    pub id: String,
    /// The last segment of the relationship type URI (`slideLayout`, `image`, ...).
    pub kind: String,
    /// The target's part name, resolved against the source part; `None` for external targets.
    pub target: Option<String>,
}

/// The directory of a part name (`ppt/slides/slide1.xml` → `ppt/slides`).
fn directory(part: &str) -> &str {
    part.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// Resolve a relationship target against the part that owns the relationship.
pub(crate) fn resolve(source_part: &str, target: &str) -> Option<String> {
    let joined = if let Some(absolute) = target.strip_prefix('/') {
        absolute.to_owned()
    } else if directory(source_part).is_empty() {
        target.to_owned()
    } else {
        format!("{}/{target}", directory(source_part))
    };
    let mut parts: Vec<&str> = Vec::new();
    for segment in joined.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    Some(parts.join("/"))
}

/// The relationships of `part` (`""` is the package itself).
pub(crate) fn relationships(package: &Package<'_>, part: &str) -> Result<Vec<Rel>, CreateError> {
    let rels_name = match part.rsplit_once('/') {
        Some((dir, file)) => format!("{dir}/_rels/{file}.rels"),
        None if part.is_empty() => "_rels/.rels".to_owned(),
        None => format!("_rels/{part}.rels"),
    };
    let Some(xml) = package.read_xml(&rels_name, 4 * 1024 * 1024)? else { return Ok(Vec::new()) };
    let doc = parse_xml(&xml)?;
    let mut out = Vec::new();
    for node in doc.descendants().filter(|n| n.is_element() && n.tag_name().name() == "Relationship").take(10_000) {
        let (Some(id), Some(kind), Some(target)) = (node.attribute("Id"), node.attribute("Type"), node.attribute("Target")) else { continue };
        let external = node.attribute("TargetMode").is_some_and(|m| m.eq_ignore_ascii_case("External"));
        out.push(Rel {
            id: id.to_owned(),
            kind: kind.rsplit('/').next().unwrap_or(kind).to_owned(),
            target: if external { None } else { resolve(part, target) },
        });
    }
    Ok(out)
}

// ── XML helpers shared by the converters (matching on local names, as the Word converter does) ──

pub(crate) fn child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    node.children().find(|n| n.is_element() && n.tag_name().name() == name)
}

pub(crate) fn children<'a, 'i>(node: Node<'a, 'i>, name: &'a str) -> impl Iterator<Item = Node<'a, 'i>> {
    node.children().filter(move |n| n.is_element() && n.tag_name().name() == name)
}

/// An attribute without a namespace.
pub(crate) fn attr<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    node.attributes().find(|a| a.name() == name && a.namespace().is_none()).map(|a| a.value())
}

/// A relationship-id attribute (`r:id`, `r:embed`, ...): the namespaced attribute with this
/// local name.
pub(crate) fn rel_attr<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    node.attributes().find(|a| a.name() == name && a.namespace().is_some()).map(|a| a.value())
}

pub(crate) fn number(node: Node<'_, '_>, name: &str) -> Option<f64> {
    attr(node, name)?.trim().parse::<f64>().ok().filter(|n| n.is_finite() && n.abs() <= 1.0e12)
}

pub(crate) fn boolean(node: Node<'_, '_>, name: &str) -> Option<bool> {
    match attr(node, name)? {
        "1" | "true" | "on" => Some(true),
        "0" | "false" | "off" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zip(files: &[(&str, &str)]) -> Vec<u8> {
        let mut zip = karnoxpdf_export::Zip::default();
        for (name, data) in files {
            zip.add(name, data.as_bytes(), true);
        }
        zip.finish()
    }

    #[test]
    fn reads_parts_case_insensitively_and_checks_integrity() {
        let bytes = zip(&[("ppt/slides/Slide1.xml", "<a/>")]);
        let package = Package::open(&bytes, "PPTX", 1 << 20).unwrap();
        assert_eq!(package.read("PPT/slides/slide1.xml", 100).unwrap().unwrap(), b"<a/>");
        assert!(package.read("missing", 100).unwrap().is_none());
        assert!(package.read("ppt/slides/slide1.xml", 2).is_err(), "limit applies");
        assert!(Package::open(b"nonsense", "PPTX", 1 << 20).is_err());
    }

    #[test]
    fn a_damaged_part_is_an_error() {
        let mut stored = karnoxpdf_export::Zip::default();
        stored.add("a.xml", b"<abcdef/>", false);
        let mut bytes = stored.finish();
        let at = bytes.windows(9).position(|w| w == b"<abcdef/>").unwrap();
        bytes[at + 2] = b'x';
        let package = Package::open(&bytes, "PPTX", 1 << 20).unwrap();
        assert!(package.read("a.xml", 100).is_err(), "the checksum no longer matches");
    }

    #[test]
    fn resolves_relationship_targets() {
        assert_eq!(resolve("ppt/slides/slide1.xml", "../media/image1.png").as_deref(), Some("ppt/media/image1.png"));
        assert_eq!(resolve("ppt/presentation.xml", "slides/slide1.xml").as_deref(), Some("ppt/slides/slide1.xml"));
        assert_eq!(resolve("", "ppt/presentation.xml").as_deref(), Some("ppt/presentation.xml"));
        assert_eq!(resolve("ppt/slides/slide1.xml", "/ppt/theme/theme1.xml").as_deref(), Some("ppt/theme/theme1.xml"));
        assert_eq!(resolve("a.xml", "../../x"), None, "targets cannot climb out of the package");
    }

    #[test]
    fn reads_relationships() {
        let bytes = zip(&[(
            "ppt/slides/_rels/slide1.xml.rels",
            r#"<Relationships xmlns="r"><Relationship Id="rId1" Type="http://x/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/><Relationship Id="rId2" Type="http://x/relationships/hyperlink" Target="https://example.com" TargetMode="External"/></Relationships>"#,
        )]);
        let package = Package::open(&bytes, "PPTX", 1 << 20).unwrap();
        let rels = relationships(&package, "ppt/slides/slide1.xml").unwrap();
        assert_eq!(rels[0].kind, "slideLayout");
        assert_eq!(rels[0].target.as_deref(), Some("ppt/slideLayouts/slideLayout1.xml"));
        assert_eq!(rels[1].target, None);
    }
}
