//! A loaded DOCX: package + parsed main document, styles, numbering, theme.

use crate::model::{self, Block, Paragraph};
use crate::opc::{self, Package};
use crate::props::{Numbering, Resolver, SectionProps, Settings, Styles, Theme, ThemeCtx};
use crate::xml::{self, Element, Node};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct HeaderFooter {
    pub part: String,
    pub shell: Element,
    pub decl: Option<String>,
    pub blocks: Vec<Block>,
    pub dirty: bool,
}

#[derive(Clone)]
pub struct Document {
    pub package: Package,
    pub main_part: String,
    pub decl: Option<String>,
    /// w:document with its attributes and non-body children (e.g. w:background); body is regenerated.
    pub shell: Element,
    pub body: Vec<Block>,
    pub final_sectpr: Element,
    pub styles: Styles,
    pub numbering: Numbering,
    pub numbering_dirty: bool,
    pub theme: Theme,
    pub settings: Settings,
    /// (source part, rId) → target part path or URL.
    pub rel_targets: HashMap<(String, String), (String, bool)>,
    /// rId (in main part) → header/footer.
    pub header_footers: HashMap<String, HeaderFooter>,
    pub images: HashMap<String, Arc<Vec<u8>>>,
    pub path: Option<PathBuf>,
    pub warnings: Vec<String>,
}

impl Document {
    pub fn open(path: &std::path::Path) -> Result<Document, String> {
        let pkg = Package::open(path)?;
        let mut d = Self::from_package(pkg)?;
        d.path = Some(path.to_path_buf());
        Ok(d)
    }

    pub fn new_blank() -> Document {
        let pkg = crate::template::blank_package();
        Self::from_package(pkg).expect("built-in template must parse")
    }

    pub fn from_package(package: Package) -> Result<Document, String> {
        let main_part = package.main_document_path().unwrap_or_else(|| "word/document.xml".to_string());
        let doc = package.get_xml(&main_part).ok_or_else(|| format!("Missing or invalid main document part '{}'", main_part))?;
        let mut shell = doc.root.clone();
        let body_el = shell.child("w:body").cloned().unwrap_or_else(|| Element::new("w:body"));
        // Keep everything except the body in the shell; body is re-inserted on save.
        shell.children.retain(|n| !matches!(n, Node::Element(e) if e.name == "w:body"));

        let mut body = Vec::new();
        let mut final_sectpr = None;
        for e in body_el.elements() {
            if e.name == "w:sectPr" {
                final_sectpr = Some(e.clone());
            } else {
                body.push(model::parse_block(e));
            }
        }

        let mut rel_targets = HashMap::new();
        let mut warnings = Vec::new();
        let rels = package.rels(&main_part);
        let mut styles = Styles::default();
        let mut numbering = Numbering::default();
        let mut theme = Theme::default();
        let mut settings = Settings::default();
        let mut header_footers = HashMap::new();
        let mut images = HashMap::new();

        let load_rels = |part: &str, rels: &[opc::Relationship], rel_targets: &mut HashMap<(String, String), (String, bool)>| {
            for r in rels {
                let t = if r.external { r.target.clone() } else { Package::resolve_target(part, &r.target) };
                rel_targets.insert((part.to_string(), r.id.clone()), (t, r.external));
            }
        };
        load_rels(&main_part, &rels, &mut rel_targets);

        for r in &rels {
            if r.external {
                continue;
            }
            let target = Package::resolve_target(&main_part, &r.target);
            let t = r.rel_type.rsplit('/').next().unwrap_or("");
            match t {
                "styles" => match package.get_xml(&target) {
                    Some(x) => styles = Styles::parse(&x.root),
                    None => warnings.push("styles part missing or invalid".into()),
                },
                "numbering" => {
                    if let Some(x) = package.get_xml(&target) {
                        numbering = Numbering::parse(&x.root);
                    }
                }
                "theme" => {
                    if let Some(x) = package.get_xml(&target) {
                        theme = Theme::parse(&x.root);
                    }
                }
                "settings" => {
                    if let Some(x) = package.get_xml(&target) {
                        settings = Settings::parse(&x.root);
                    }
                }
                "header" | "footer" => {
                    if let Some(x) = package.get_xml(&target) {
                        let mut sh = x.root.clone();
                        let blocks = model::parse_blocks(&sh);
                        sh.children.clear();
                        let hrels = package.rels(&target);
                        load_rels(&target, &hrels, &mut rel_targets);
                        for hr in &hrels {
                            if hr.rel_type == opc::REL_IMAGE && !hr.external {
                                let p = Package::resolve_target(&target, &hr.target);
                                if let Some(b) = package.get(&p) {
                                    images.insert(p, Arc::new(b.to_vec()));
                                }
                            }
                        }
                        header_footers.insert(
                            r.id.clone(),
                            HeaderFooter { part: target.clone(), shell: sh, decl: x.decl, blocks, dirty: false },
                        );
                    }
                }
                "image" => {
                    if let Some(b) = package.get(&target) {
                        images.insert(target.clone(), Arc::new(b.to_vec()));
                    }
                }
                _ => {}
            }
        }
        if theme.minor_latin.is_empty() {
            theme = Theme::parse(&xml::parse_str(crate::template::THEME_XML).unwrap().root);
        }

        Ok(Document {
            package,
            main_part,
            decl: doc.decl,
            shell,
            body,
            final_sectpr: final_sectpr.unwrap_or_else(|| xml::parse_str(crate::template::DEFAULT_SECTPR).unwrap().root),
            styles,
            numbering,
            numbering_dirty: false,
            theme,
            settings,
            rel_targets,
            header_footers,
            images,
            path: None,
            warnings,
        })
    }

    pub fn resolver(&self) -> Resolver<'_> {
        Resolver { styles: &self.styles, numbering: &self.numbering, tc: ThemeCtx { theme: &self.theme, settings: &self.settings } }
    }

    pub fn theme_ctx(&self) -> ThemeCtx<'_> {
        ThemeCtx { theme: &self.theme, settings: &self.settings }
    }

    pub fn image_for(&self, source_part: &str, rid: &str) -> Option<(String, Arc<Vec<u8>>)> {
        let (t, ext) = self.rel_targets.get(&(source_part.to_string(), rid.to_string()))?;
        if *ext {
            return None;
        }
        self.images.get(t).map(|b| (t.clone(), b.clone()))
    }

    pub fn hyperlink_target(&self, source_part: &str, rid: &str) -> Option<String> {
        self.rel_targets.get(&(source_part.to_string(), rid.to_string())).map(|(t, _)| t.clone())
    }

    /// Serialize the model back into the package and return it.
    pub fn to_package(&self) -> Package {
        let mut pkg = self.package.clone();
        let mut root = self.shell.clone();
        let mut body = Element::new("w:body");
        model::blocks_to_xml(&self.body, &mut body);
        // Body must not end with a table (the format requires a final paragraph).
        if !matches!(self.body.last(), Some(Block::Paragraph(_))) {
            body.children.push(Node::Element(Element::new("w:p")));
        }
        body.children.push(Node::Element(self.final_sectpr.clone()));
        // Insert body after w:background if present, else first.
        let idx = root.children.iter().position(|n| matches!(n, Node::Element(e) if e.name == "w:background")).map(|i| i + 1).unwrap_or(0);
        root.children.insert(idx, Node::Element(body));
        pkg.set(&self.main_part, xml::serialize_document(&root, self.decl.as_deref()));

        for hf in self.header_footers.values() {
            if hf.dirty {
                let mut r = hf.shell.clone();
                model::blocks_to_xml(&hf.blocks, &mut r);
                if r.children.is_empty() {
                    r.children.push(Node::Element(Element::new("w:p")));
                }
                pkg.set(&hf.part, xml::serialize_document(&r, hf.decl.as_deref()));
            }
        }
        if self.numbering_dirty {
            if let Some(root) = &self.numbering.root {
                let path = self.numbering_part().unwrap_or_else(|| "word/numbering.xml".to_string());
                pkg.set(&path, xml::serialize_document(root, None));
            }
        }
        for (path, bytes) in &self.images {
            if pkg.get(path).is_none() {
                pkg.set(path, bytes.to_vec());
            }
        }
        pkg
    }

    pub fn save_as(&mut self, path: &std::path::Path) -> Result<(), String> {
        let pkg = self.to_package();
        pkg.save(path)?;
        self.package = pkg;
        for hf in self.header_footers.values_mut() {
            hf.dirty = false;
        }
        self.numbering_dirty = false;
        self.path = Some(path.to_path_buf());
        Ok(())
    }

    fn numbering_part(&self) -> Option<String> {
        self.package
            .rels(&self.main_part)
            .into_iter()
            .find(|r| r.rel_type == opc::REL_NUMBERING)
            .map(|r| Package::resolve_target(&self.main_part, &r.target))
    }

    /// Ensure a numbering part exists and add a new list definition.
    /// Returns the new numId.
    pub fn add_list(&mut self, bullet: bool) -> String {
        if self.numbering.root.is_none() || self.numbering_part().is_none() {
            let path = "word/numbering.xml";
            let root = xml::parse_str(crate::template::NUMBERING_XML).unwrap().root;
            self.package.set(path, xml::serialize_document(&root, None));
            self.package.ensure_override_content_type(path, "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml");
            let main = self.main_part.clone();
            let rid = self.package.add_rel(&main, opc::REL_NUMBERING, "numbering.xml", false);
            self.rel_targets.insert((main, rid), (path.to_string(), false));
            self.numbering = Numbering::parse(&root);
        }
        let root = self.numbering.root.as_mut().unwrap();
        let next_abs = root
            .children_named("w:abstractNum")
            .filter_map(|a| a.attr("w:abstractNumId").and_then(|v| v.parse::<i32>().ok()))
            .max()
            .unwrap_or(-1)
            + 1;
        let next_num = root.children_named("w:num").filter_map(|a| a.attr("w:numId").and_then(|v| v.parse::<i32>().ok())).max().unwrap_or(0) + 1;
        let abs_xml = if bullet { crate::template::bullet_abstract(next_abs) } else { crate::template::decimal_abstract(next_abs) };
        let mut abs = xml::parse_str(&abs_xml).unwrap().root;
        abs.remove_attr("xmlns:w");
        // abstractNum elements must precede num elements.
        let idx = root.children.iter().position(|n| matches!(n, Node::Element(e) if e.name == "w:num" || e.name == "w:numIdMacAtCleanup")).unwrap_or(root.children.len());
        root.children.insert(idx, Node::Element(abs));
        let num = Element::new("w:num")
            .with_attr("w:numId", next_num.to_string())
            .with_child(Element::new("w:abstractNumId").with_attr("w:val", next_abs.to_string()));
        let idx = root.children.iter().position(|n| matches!(n, Node::Element(e) if e.name == "w:numIdMacAtCleanup")).unwrap_or(root.children.len());
        root.children.insert(idx, Node::Element(num));
        let root = root.clone();
        self.numbering = Numbering::parse(&root);
        self.numbering_dirty = true;
        next_num.to_string()
    }

    /// Add an image part referenced from the main document; returns rId.
    pub fn add_image(&mut self, bytes: Vec<u8>, ext: &str) -> String {
        let ext = ext.to_ascii_lowercase();
        let ct = match ext.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "bmp" => "image/bmp",
            "tif" | "tiff" => "image/tiff",
            _ => "application/octet-stream",
        };
        let path = self.package.unique_part_name("word/media", "image", &ext);
        self.package.set(&path, bytes.clone());
        self.package.ensure_default_content_type(&ext, ct);
        let target = path.trim_start_matches("word/").to_string();
        let main = self.main_part.clone();
        let rid = self.package.add_rel(&main, opc::REL_IMAGE, &target, false);
        self.rel_targets.insert((main, rid.clone()), (path.clone(), false));
        self.images.insert(path, Arc::new(bytes));
        rid
    }

    /// All section property blocks in body order with the index of the block that ends each section.
    pub fn sections(&self) -> Vec<(usize, SectionProps)> {
        let tc = self.theme_ctx();
        let mut out = Vec::new();
        for (i, b) in self.body.iter().enumerate() {
            if let Block::Paragraph(p) = b {
                if let Some(sp) = p.ppr.as_ref().and_then(|pp| pp.child("w:sectPr")) {
                    out.push((i, SectionProps::parse(sp, &tc)));
                }
            }
        }
        out.push((usize::MAX, SectionProps::parse(&self.final_sectpr, &tc)));
        out
    }

    /// Mutable access to the sectPr governing body block `idx`.
    pub fn sectpr_for_block_mut(&mut self, idx: usize) -> &mut Element {
        let mut target: Option<usize> = None;
        for (i, b) in self.body.iter().enumerate().skip(idx) {
            if let Block::Paragraph(p) = b {
                if p.ppr.as_ref().and_then(|pp| pp.child("w:sectPr")).is_some() {
                    target = Some(i);
                    break;
                }
            }
        }
        match target {
            Some(i) => match &mut self.body[i] {
                Block::Paragraph(p) => p.ppr.as_mut().unwrap().child_mut("w:sectPr").unwrap(),
                _ => unreachable!(),
            },
            None => &mut self.final_sectpr,
        }
    }

    pub fn ensure_nonempty(&mut self) {
        if self.body.is_empty() {
            self.body.push(Block::Paragraph(Paragraph::new()));
        }
    }

    pub fn title(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Document1".to_string())
    }

    pub fn plain_text(&self) -> String {
        let mut s = String::new();
        fn rec(blocks: &[Block], s: &mut String) {
            for b in blocks {
                match b {
                    Block::Paragraph(p) => {
                        s.push_str(&p.plain_text());
                        s.push('\n');
                    }
                    Block::Table(t) => {
                        for r in t.rows() {
                            for c in r.cells() {
                                rec(&c.blocks, s);
                            }
                        }
                    }
                    Block::Sdt { content, .. } => rec(content, s),
                    Block::Other(_) => {}
                }
            }
        }
        rec(&self.body, &mut s);
        s
    }

    pub fn word_count(&self) -> usize {
        self.plain_text().split_whitespace().filter(|w| w.chars().any(|c| c.is_alphanumeric())).count()
    }
}
