//! Open Packaging Conventions: the zip container, relationships and content types.
//!
//! Every part is kept as raw bytes in its original order. Only parts the editor
//! actually changes are re-serialized on save, so everything we don't
//! understand (custom XML, VBA, glossary documents, signatures metadata, ...)
//! is written back byte-for-byte.

use crate::xml::{self, Element};
use std::io::{Read, Seek, Write};

#[derive(Clone, Debug)]
pub struct Part {
    pub name: String, // without leading slash, e.g. "word/document.xml"
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, Default)]
pub struct Package {
    pub parts: Vec<Part>,
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
        let f = std::fs::File::open(path).map_err(|e| format!("Cannot open {}: {}", path.display(), e))?;
        Self::read(std::io::BufReader::new(f))
    }

    pub fn read<R: Read + Seek>(r: R) -> Result<Package, String> {
        let mut zip = zip::ZipArchive::new(r).map_err(|e| format!("Not a valid DOCX (zip) file: {e}"))?;
        let mut parts = Vec::with_capacity(zip.len());
        for i in 0..zip.len() {
            let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
            if f.is_dir() {
                continue;
            }
            let name = f.name().map_err(|e| e.to_string())?.trim_start_matches('/').to_string();
            let mut data = Vec::with_capacity(f.size() as usize);
            f.read_to_end(&mut data).map_err(|e| e.to_string())?;
            parts.push(Part { name, data });
        }
        Ok(Package { parts })
    }

    pub fn write<W: Write + Seek>(&self, w: W) -> Result<(), String> {
        let mut zip = zip::ZipWriter::new(w);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .last_modified_time(zip::DateTime::default());
        // [Content_Types].xml must come first for some consumers.
        let mut order: Vec<&Part> = self.parts.iter().collect();
        order.sort_by_key(|p| if p.name == "[Content_Types].xml" { 0 } else { 1 });
        for p in order {
            let stored = is_precompressed(&p.name);
            let o = if stored { opts.compression_method(zip::CompressionMethod::Stored) } else { opts };
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

    pub fn get_xml(&self, name: &str) -> Option<xml::Document> {
        self.get(name).and_then(|b| xml::parse(b).ok())
    }

    pub fn set(&mut self, name: &str, data: Vec<u8>) {
        let name = name.trim_start_matches('/');
        if let Some(p) = self.parts.iter_mut().find(|p| p.name.eq_ignore_ascii_case(name)) {
            p.data = data;
        } else {
            self.parts.push(Part { name: name.to_string(), data });
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
    [".png", ".jpg", ".jpeg", ".gif", ".zip", ".mp4", ".mp3", ".wdp", ".odttf"].iter().any(|e| n.ends_with(e))
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
