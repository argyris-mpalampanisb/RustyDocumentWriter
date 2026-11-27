//! Open Packaging Conventions: the zip container, relationships and content types.
//!
//! Every part is kept as raw bytes in its original order. Only parts the editor
//! actually changes are re-serialized on save, so everything we don't
//! understand (custom XML, VBA, glossary documents, signatures metadata, ...)
//! is written back byte-for-byte. Unchanged parts are even copied still
//! compressed, straight from the original zip, so saving a large document
//! costs no recompression and keeps every part's original compressed size.

use crate::xml::{self, Element};
use std::io::{Cursor, Read, Seek, Write};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Part {
    pub name: String, // without leading slash, e.g. "word/document.xml"
    /// Shared, so cloning a package (every save does) copies no part data.
    pub data: Arc<Vec<u8>>,
    /// Index of the untouched original entry in [`Package::source`], if any.
    pub src: Option<usize>,
}

impl Part {
    pub fn new(name: impl Into<String>, data: Vec<u8>) -> Part {
        Part { name: name.into(), data: Arc::new(data), src: None }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Package {
    pub parts: Vec<Part>,
    /// The zip file the package was read from (still compressed).
    pub source: Option<Arc<Vec<u8>>>,
}

#[derive(Clone, Debug)]
pub struct Relationship {
    pub id: String,
    pub rel_type: String,
    pub target: String,
    pub external: bool,
}

pub const REL_OFFICE_DOCUMENT: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
pub const REL_STYLES: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles";
pub const REL_NUMBERING: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";
pub const REL_THEME: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme";
pub const REL_SETTINGS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings";
pub const REL_IMAGE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
pub const REL_HEADER: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header";
pub const REL_FOOTER: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer";
pub const REL_HYPERLINK: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";
pub const REL_FOOTNOTES: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes";
pub const REL_ENDNOTES: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes";
pub const REL_FONT_TABLE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/fontTable";
pub const REL_WEB_SETTINGS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/webSettings";
pub const REL_CORE: &str = "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties";
pub const REL_EXTENDED: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties";

impl Package {
    pub fn open(path: &std::path::Path) -> Result<Package, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("Cannot open {}: {}", path.display(), e))?;
        Self::from_bytes(bytes)
    }

    pub fn read<R: Read>(mut r: R) -> Result<Package, String> {
        let mut bytes = Vec::new();
        r.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        Self::from_bytes(bytes)
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<Package, String> {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes.as_slice())).map_err(|e| format!("Not a valid DOCX (zip) file: {e}"))?;
        let mut parts = Vec::with_capacity(zip.len());
        for i in 0..zip.len() {
            let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
            if f.is_dir() {
                continue;
            }
            let raw = f.name().map_err(|e| e.to_string())?;
            let name = raw.trim_start_matches('/').to_string();
            // Entries that would be renamed on write (leading slash) can't be raw-copied.
            let src = (raw == name).then_some(i);
            let mut data = Vec::with_capacity(f.size() as usize);
            f.read_to_end(&mut data).map_err(|e| format!("{name}: {e}"))?;
            parts.push(Part { name, data: Arc::new(data), src });
        }
        drop(zip);
        Ok(Package { parts, source: Some(Arc::new(bytes)) })
    }

    pub fn write<W: Write + Seek>(&self, w: W) -> Result<(), String> {
        let mut zip = zip::ZipWriter::new(w);
        let mut source = match &self.source {
            Some(b) => Some(zip::ZipArchive::new(Cursor::new(b.as_slice())).map_err(|e| e.to_string())?),
            None => None,
        };
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            // Only changed parts are compressed (the rest are raw-copied), so the
            // best level is cheap: about 6 ms for a 600-page document.xml, which
            // shrinks to a quarter of level 6's size.
            .compression_level(Some(9))
            .last_modified_time(zip::DateTime::default());
        // [Content_Types].xml must come first for some consumers.
        let mut order: Vec<&Part> = self.parts.iter().collect();
        order.sort_by_key(|p| if p.name == "[Content_Types].xml" { 0 } else { 1 });
        for p in order {
            if let (Some(i), Some(src)) = (p.src, source.as_mut()) {
                let f = src.by_index_raw(i).map_err(|e| e.to_string())?;
                zip.raw_copy_file(f).map_err(|e| e.to_string())?;
                continue;
            }
            let stored = is_precompressed(&p.name);
            let o = if stored { opts.compression_method(zip::CompressionMethod::Stored).compression_level(None) } else { opts };
            zip.start_file(p.name.as_str(), o).map_err(|e| e.to_string())?;
            zip.write_all(&p.data).map_err(|e| e.to_string())?;
        }
        zip.finish().map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        // Write to a temp file first so a failure never destroys the original.
        let tmp = path.with_extension("rdw-tmp");
        {
            let f = std::fs::File::create(&tmp).map_err(|e| format!("Cannot write {}: {}", tmp.display(), e))?;
            self.write(std::io::BufWriter::new(f))?;
        }
        std::fs::rename(&tmp, path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("Cannot replace {}: {}", path.display(), e)
        })
    }

    pub fn get(&self, name: &str) -> Option<&[u8]> {
        let name = name.trim_start_matches('/');
        self.parts.iter().find(|p| p.name.eq_ignore_ascii_case(name)).map(|p| p.data.as_slice())
    }

    /// The part's bytes, shared rather than copied.
    pub fn get_shared(&self, name: &str) -> Option<Arc<Vec<u8>>> {
        let name = name.trim_start_matches('/');
        self.parts.iter().find(|p| p.name.eq_ignore_ascii_case(name)).map(|p| p.data.clone())
    }

    pub fn get_xml(&self, name: &str) -> Option<xml::Document> {
        self.get(name).and_then(|b| xml::parse(b).ok())
    }

    pub fn set(&mut self, name: &str, data: Vec<u8>) {
        let name = name.trim_start_matches('/');
        if let Some(p) = self.parts.iter_mut().find(|p| p.name.eq_ignore_ascii_case(name)) {
            // Rewriting a part with the same bytes keeps it raw-copyable.
            if *p.data != data {
                p.data = Arc::new(data);
                p.src = None;
            }
        } else {
            self.parts.push(Part::new(name, data));
        }
    }

    pub fn remove(&mut self, name: &str) {
        let name = name.trim_start_matches('/');
        self.parts.retain(|p| !p.name.eq_ignore_ascii_case(name));
    }

    pub fn rels_path_for(part: &str) -> String {
        let part = part.trim_start_matches('/');
        match part.rfind('/') {
            Some(i) => format!("{}/_rels/{}.rels", &part[..i], &part[i + 1..]),
            None => format!("_rels/{}.rels", part),
        }
    }

    pub fn rels(&self, part: &str) -> Vec<Relationship> {
        let path = Self::rels_path_for(part);
        let Some(doc) = self.get_xml(&path) else { return Vec::new() };
        doc.root
            .elements()
            .filter(|e| e.local() == "Relationship")
            .map(|e| Relationship {
                id: e.attr("Id").unwrap_or_default().to_string(),
                rel_type: e.attr("Type").unwrap_or_default().to_string(),
                target: e.attr("Target").unwrap_or_default().to_string(),
                external: e.attr("TargetMode") == Some("External"),
            })
            .collect()
    }

    /// Add a relationship to `part`'s .rels and return its new id.
    pub fn add_rel(&mut self, part: &str, rel_type: &str, target: &str, external: bool) -> String {
        let path = Self::rels_path_for(part);
        let mut root = self.get_xml(&path).map(|d| d.root).unwrap_or_else(|| {
            Element::new("Relationships").with_attr("xmlns", "http://schemas.openxmlformats.org/package/2006/relationships")
        });
        let mut n = root.elements().count() + 1;
        let id = loop {
            let cand = format!("rId{}", n);
            if !root.elements().any(|e| e.attr("Id") == Some(cand.as_str())) {
                break cand;
            }
            n += 1;
        };
        let mut rel = Element::new("Relationship").with_attr("Id", &id).with_attr("Type", rel_type).with_attr("Target", target);
        if external {
            rel.set_attr("TargetMode", "External");
        }
        root.children.push(xml::Node::Element(rel));
        self.set(&path, xml::serialize_document(&root, None));
        id
    }

    /// Resolve a relationship target relative to the source part.
    pub fn resolve_target(source_part: &str, target: &str) -> String {
        if let Some(t) = target.strip_prefix('/') {
            return t.to_string();
        }
        let base = match source_part.rfind('/') {
            Some(i) => &source_part[..i],
            None => "",
        };
        let mut segs: Vec<&str> = if base.is_empty() { Vec::new() } else { base.split('/').collect() };
        for s in target.split('/') {
            match s {
                ".." => {
                    segs.pop();
                }
                "." | "" => {}
                s => segs.push(s),
            }
        }
        segs.join("/")
    }

    /// Main document part path from the package root relationships.
    pub fn main_document_path(&self) -> Option<String> {
        self.rels("")
            .into_iter()
            .chain(self.root_rels())
            .find(|r| r.rel_type == REL_OFFICE_DOCUMENT || r.rel_type.ends_with("/officeDocument"))
            .map(|r| Self::resolve_target("", &r.target))
    }

    fn root_rels(&self) -> Vec<Relationship> {
        let Some(doc) = self.get_xml("_rels/.rels") else { return Vec::new() };
        doc.root
            .elements()
            .filter(|e| e.local() == "Relationship")
            .map(|e| Relationship {
                id: e.attr("Id").unwrap_or_default().to_string(),
                rel_type: e.attr("Type").unwrap_or_default().to_string(),
                target: e.attr("Target").unwrap_or_default().to_string(),
                external: e.attr("TargetMode") == Some("External"),
            })
            .collect()
    }

    /// Ensure the content types file declares an extension default.
    pub fn ensure_default_content_type(&mut self, ext: &str, content_type: &str) {
        let mut root = self.get_xml("[Content_Types].xml").map(|d| d.root).unwrap_or_else(|| {
            Element::new("Types").with_attr("xmlns", "http://schemas.openxmlformats.org/package/2006/content-types")
        });
        if root.elements().any(|e| e.local() == "Default" && e.attr("Extension").is_some_and(|x| x.eq_ignore_ascii_case(ext))) {
            return;
        }
        let d = Element::new("Default").with_attr("Extension", ext).with_attr("ContentType", content_type);
        // Defaults must precede Overrides.
        let idx = root.children.iter().position(|n| matches!(n, xml::Node::Element(e) if e.local() == "Override")).unwrap_or(root.children.len());
        root.children.insert(idx, xml::Node::Element(d));
        self.set("[Content_Types].xml", xml::serialize_document(&root, None));
    }

    pub fn ensure_override_content_type(&mut self, part: &str, content_type: &str) {
        let part_name = format!("/{}", part.trim_start_matches('/'));
        let mut root = self.get_xml("[Content_Types].xml").map(|d| d.root).unwrap_or_else(|| {
            Element::new("Types").with_attr("xmlns", "http://schemas.openxmlformats.org/package/2006/content-types")
        });
        if root.elements().any(|e| e.local() == "Override" && e.attr("PartName").is_some_and(|x| x.eq_ignore_ascii_case(&part_name))) {
            return;
        }
        root.children.push(xml::Node::Element(
            Element::new("Override").with_attr("PartName", &part_name).with_attr("ContentType", content_type),
        ));
        self.set("[Content_Types].xml", xml::serialize_document(&root, None));
    }

    /// Set (or add) the content type override for `part`.
    pub fn set_override_content_type(&mut self, part: &str, content_type: &str) {
        let part_name = format!("/{}", part.trim_start_matches('/'));
        let Some(mut root) = self.get_xml("[Content_Types].xml").map(|d| d.root) else { return };
        let existing = root.children.iter_mut().find_map(|n| match n {
            xml::Node::Element(e) if e.local() == "Override" && e.attr("PartName").is_some_and(|x| x.eq_ignore_ascii_case(&part_name)) => Some(e),
            _ => None,
        });
        match existing {
            Some(e) if e.attr("ContentType") == Some(content_type) => return,
            Some(e) => e.set_attr("ContentType", content_type),
            None => root.children.push(xml::Node::Element(
                Element::new("Override").with_attr("PartName", &part_name).with_attr("ContentType", content_type),
            )),
        }
        self.set("[Content_Types].xml", xml::serialize_document(&root, None));
    }

    /// Remove the VBA project (and everything hanging off it) from `main_part`.
    /// Returns true if there was one. Word refuses to open a .docx that still has macros.
    pub fn remove_macros(&mut self, main_part: &str) -> bool {
        let rels_path = Self::rels_path_for(main_part);
        let Some(mut rels) = self.get_xml(&rels_path).map(|d| d.root) else { return false };
        let is_vba = |e: &Element| e.attr("Type").is_some_and(|t| t.ends_with("/vbaProject"));
        let targets: Vec<String> = rels.elements().filter(|e| is_vba(e)).filter_map(|e| e.attr("Target")).map(|t| Self::resolve_target(main_part, t)).collect();
        if targets.is_empty() {
            return false;
        }
        rels.children.retain(|n| !matches!(n, xml::Node::Element(e) if is_vba(e)));
        self.set(&rels_path, xml::serialize_document(&rels, None));
        let mut gone: Vec<String> = Vec::new();
        for t in targets {
            // The project's own relationships point at vbaData.xml.
            for r in self.rels(&t) {
                if !r.external {
                    gone.push(Self::resolve_target(&t, &r.target));
                }
            }
            gone.push(Self::rels_path_for(&t));
            gone.push(t);
        }
        for g in &gone {
            self.remove(g);
        }
        if let Some(mut ct) = self.get_xml("[Content_Types].xml").map(|d| d.root) {
            ct.children.retain(|n| match n {
                xml::Node::Element(e) if e.local() == "Override" => {
                    let pn = e.attr("PartName").unwrap_or_default().trim_start_matches('/');
                    !gone.iter().any(|g| g.eq_ignore_ascii_case(pn))
                }
                _ => true,
            });
            self.set("[Content_Types].xml", xml::serialize_document(&ct, None));
        }
        true
    }

    pub fn unique_part_name(&self, dir: &str, stem: &str, ext: &str) -> String {
        let mut i = 1;
        loop {
            let cand = format!("{}/{}{}.{}", dir, stem, i, ext);
            if self.get(&cand).is_none() {
                return cand;
            }
            i += 1;
        }
    }
}

fn is_precompressed(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    // Not .odttf: embedded fonts are only scrambled in their first 32 bytes and deflate well.
    [".png", ".jpg", ".jpeg", ".gif", ".webp", ".wdp", ".zip", ".docx", ".xlsx", ".pptx", ".mp4", ".mp3", ".m4a"].iter().any(|e| n.ends_with(e))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolve() {
        assert_eq!(Package::resolve_target("word/document.xml", "media/image1.png"), "word/media/image1.png");
        assert_eq!(Package::resolve_target("word/document.xml", "../customXml/item1.xml"), "customXml/item1.xml");
        assert_eq!(Package::resolve_target("", "word/document.xml"), "word/document.xml");
        assert_eq!(Package::rels_path_for("word/document.xml"), "word/_rels/document.xml.rels");
    }
}
