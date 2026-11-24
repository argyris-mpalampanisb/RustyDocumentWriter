//! Insert → Page Number: styled page-number blocks for headers and footers.
//!
//! The markup mirrors what Word writes for its page-number gallery: a paragraph
//! wrapped in a `w:sdt` carrying `w:docPartObj` (gallery "Page Numbers (Top of
//! Page)" / "Page Numbers (Bottom of Page)"), with a PAGE (and optionally
//! NUMPAGES) complex field. Decorations use plain paragraph borders, alignment
//! and run formatting rather than drawing shapes, so they survive conversion
//! to other formats (e.g. LibreOffice / ODT) intact.

use crate::document::{Document, HeaderFooter};
use crate::model::{self, Block};
use crate::opc;
use crate::props::SECTPR_ORDER;
use crate::xml::{self, Element, Node};

const W_NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#;
/// Accent 1 of the default Office theme (2023+); used for coloured styles.
const ACCENT: &str = "156082";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Place {
    Top,
    Bottom,
}

impl Place {
    fn gallery(self) -> &'static str {
        match self {
            Place::Top => "Page Numbers (Top of Page)",
            Place::Bottom => "Page Numbers (Bottom of Page)",
        }
    }
    /// The page edge the number sits against; decorative rules go on the side facing the body text.
    fn rule_side(self) -> &'static str {
        match self {
            Place::Top => "bottom",
            Place::Bottom => "top",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// A gallery entry.
#[derive(Clone, Copy, Debug)]
pub struct PnStyle {
    pub id: &'static str,
    pub name: &'static str,
    pub align: Align,
}

/// Word's Thick Line draws a tapered shape above the number; here both rules are
/// paragraph borders so they convert to ODT (LibreOffice has no styled page numbers).
pub const STYLES: &[PnStyle] = &[
    PnStyle { id: "brackets", name: "Brackets", align: Align::Center },
    PnStyle { id: "thin_line", name: "Thin Line", align: Align::Center },
    PnStyle { id: "thick_line", name: "Thick Line", align: Align::Center },
];

pub fn style(id: &str) -> Option<&'static PnStyle> {
    STYLES.iter().find(|s| s.id == id)
}

/// Number formats offered by "Format Page Numbers": (w:fmt value, label).
pub const FORMATS: &[(&str, &str)] = &[
    ("decimal", "1, 2, 3, …"),
    ("lowerRoman", "i, ii, iii, …"),
    ("upperRoman", "I, II, III, …"),
    ("lowerLetter", "a, b, c, …"),
    ("upperLetter", "A, B, C, …"),
];

fn run(rpr: &str, inner: &str) -> String {
    format!("<w:r>{}{inner}</w:r>", if rpr.is_empty() { String::new() } else { format!("<w:rPr>{rpr}</w:rPr>") })
}

fn text(rpr: &str, t: &str) -> String {
    run(rpr, &format!(r#"<w:t xml:space="preserve">{}</w:t>"#, xml_escape(t)))
}

fn xml_escape(s: &str) -> String {
    let mut o = String::new();
    xml::escape_text(s, &mut o);
    o
}

/// A complex field (begin / instr / separate / cached result / end), as Word writes it.
fn field(rpr: &str, instr: &str, cached: &str) -> String {
    [
        run(rpr, r#"<w:fldChar w:fldCharType="begin"/>"#),
        run(rpr, &format!(r#"<w:instrText xml:space="preserve"> {instr} \* MERGEFORMAT </w:instrText>"#)),
        run(rpr, r#"<w:fldChar w:fldCharType="separate"/>"#),
        text(rpr, cached),
        run(rpr, r#"<w:fldChar w:fldCharType="end"/>"#),
    ]
    .concat()
}

fn border(side: &str, sz: u32, color: &str) -> String {
    format!(r#"<w:{side} w:val="single" w:sz="{sz}" w:space="4" w:color="{color}"/>"#)
}

/// Thin Line's tapered rule: an image relationship in the header/footer part and its size.
#[derive(Clone, Debug)]
pub struct Taper {
    pub rid: String,
    pub w_pt: f32,
    pub h_pt: f32,
}

/// Taper profile: thickness in points at the ends and at the centre.
const TAPER_END_PT: f32 = 0.25;
const TAPER_MID_PT: f32 = 1.0;
/// Raster resolution of the taper image (print quality).
const TAPER_DPI: f32 = 600.0;
const TAPER_RGB: [u8; 3] = [0xA6, 0xA6, 0xA6];
pub const TAPER_PART: &str = "word/media/pagenum-thin-line.png";

/// Height of the taper image in points (a little more than the thickest point, for anti-aliasing).
pub fn taper_height_pt() -> f32 {
    TAPER_MID_PT + 0.2
}

/// Render the tapered rule as an anti-aliased PNG on a transparent background.
/// The thickness swells smoothly (a sine profile) from `TAPER_END_PT` at both ends
/// to `TAPER_MID_PT` in the middle.
pub fn taper_png(w_pt: f32) -> Vec<u8> {
    let px_per_pt = TAPER_DPI / 72.0;
    let w = (w_pt * px_per_pt).round().max(2.0) as u32;
    let h = (taper_height_pt() * px_per_pt).ceil() as u32;
    let cy = h as f32 / 2.0;
    // Transparent pixels carry the line colour (not black) so scaling never darkens the edges.
    let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba([TAPER_RGB[0], TAPER_RGB[1], TAPER_RGB[2], 0]));
    for x in 0..w {
        let t = (x as f32 + 0.5) / w as f32;
        let thick = (TAPER_END_PT + (TAPER_MID_PT - TAPER_END_PT) * (std::f32::consts::PI * t).sin()) * px_per_pt;
        let (top, bot) = (cy - thick / 2.0, cy + thick / 2.0);
        for y in 0..h {
            // Coverage = overlap of the pixel row with [top, bot].
            let cov = ((bot.min(y as f32 + 1.0) - top.max(y as f32)).clamp(0.0, 1.0) * 255.0).round() as u8;
            img.get_pixel_mut(x, y)[3] = cov;
        }
    }
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img).write_to(&mut out, image::ImageFormat::Png).expect("png encode");
    out.into_inner()
}

/// A paragraph holding just the tapered rule, centred, with a tiny font so the line box
/// is barely taller than the picture. `gap` (twips) separates it from the number.
fn taper_para(t: &Taper, place: Place) -> String {
    let mut d = crate::edit::image_drawing(&t.rid, t.w_pt, t.h_pt, 0x7A9E, "Thin Line");
    d.remove_attr("xmlns:w");
    let gap = 80;
    // Exact line height a little above the picture's height, so Word and LibreOffice agree:
    // LibreOffice ignores the tiny font size on a picture-only line and would use the default size.
    let line = ((t.h_pt + 1.8) * 20.0).round() as i32;
    let spacing = match place {
        Place::Bottom => format!(r#"<w:spacing w:before="0" w:after="{gap}" w:line="{line}" w:lineRule="exact"/>"#),
        Place::Top => format!(r#"<w:spacing w:before="{gap}" w:after="0" w:line="{line}" w:lineRule="exact"/>"#),
    };
    let tiny = r#"<w:sz w:val="2"/><w:szCs w:val="2"/>"#;
    format!(r#"<w:p><w:pPr>{spacing}<w:jc w:val="center"/><w:rPr>{tiny}</w:rPr></w:pPr><w:r><w:rPr>{tiny}</w:rPr>{}</w:r></w:p>"#, d.to_xml())
}

/// The `w:sdt` block for one gallery entry. `taper` is required for Thin Line
/// (without it, Thin Line falls back to a plain border).
pub fn block_xml(st: &PnStyle, place: Place, taper: Option<&Taper>) -> String {
    let jc = match st.align {
        Align::Left => "left",
        Align::Center => "center",
        Align::Right => "right",
    };
    let side = place.rule_side();
    let pbdr = match st.id {
        "thin_line" if taper.is_none() => border(side, 4, "A6A6A6"),
        "thick_line" => border(side, 24, ACCENT),
        _ => String::new(),
    };
    let pbdr = if pbdr.is_empty() { pbdr } else { format!("<w:pBdr>{pbdr}</w:pBdr>") };
    // Borders span indent to indent: the fallback Thin Line is inset 1" (1440 twips) on each side.
    let ind = if st.id == "thin_line" && taper.is_none() { r#"<w:ind w:left="1440" w:right="1440"/>"# } else { "" };
    let ppr = format!(r#"<w:pPr>{pbdr}<w:spacing w:before="0" w:after="0" w:line="240" w:lineRule="auto"/>{ind}<w:jc w:val="{jc}"/></w:pPr>"#);
    let page = |rpr: &str| field(rpr, "PAGE", "1");
    let body = match st.id {
        "brackets" => format!("{}{}{}", text("", "["), page(""), text("", "]")),
        "thick_line" => page(&format!(r#"<w:b/><w:bCs/><w:color w:val="{ACCENT}"/>"#)),
        _ => page(""),
    };
    let number = format!("<w:p>{ppr}{body}</w:p>");
    let content = match (st.id, taper) {
        ("thin_line", Some(t)) => match place {
            // The rule faces the body text: above a footer number, below a header number.
            Place::Bottom => format!("{}{number}", taper_para(t, place)),
            Place::Top => format!("{number}{}", taper_para(t, place)),
        },
        _ => number,
    };
    format!(
        r#"<w:sdt {W_NS}><w:sdtPr><w:docPartObj><w:docPartGallery w:val="{}"/><w:docPartUnique/></w:docPartObj></w:sdtPr><w:sdtContent>{content}</w:sdtContent></w:sdt>"#,
        place.gallery()
    )
}

pub fn block(st: &PnStyle, place: Place, taper: Option<&Taper>) -> Block {
    let mut root = xml::parse_str(&block_xml(st, place, taper)).expect("page number template must parse").root;
    root.remove_attr("xmlns:w");
    model::parse_block(&root)
}

/// True if this block is (or contains) a page number: a PAGE field or a page-number gallery part.
pub fn is_page_number_block(b: &Block) -> bool {
    let x = model::block_to_xml(b);
    let mut gal = Vec::new();
    x.find_all_local("docPartGallery", &mut gal);
    if gal.iter().any(|g| g.attr("w:val").is_some_and(|v| v.starts_with("Page Numbers"))) {
        return true;
    }
    let mut instr = Vec::new();
    x.find_all_local("instrText", &mut instr);
    let mut simple = Vec::new();
    x.find_all_local("fldSimple", &mut simple);
    let is_page = |s: &str| {
        let kw = s.split_whitespace().next().unwrap_or("").to_ascii_uppercase();
        kw == "PAGE" || kw == "NUMPAGES" || kw == "SECTIONPAGES"
    };
    instr.iter().any(|e| is_page(&e.text())) || simple.iter().any(|e| e.attr("w:instr").is_some_and(is_page))
}

impl Document {
    /// Every sectPr in the document, in order (body section breaks, then the final one).
    fn sectprs_mut(&mut self) -> Vec<&mut Element> {
        let mut out: Vec<&mut Element> = Vec::new();
        for b in self.body.iter_mut() {
            if let Block::Paragraph(p) = b {
                if let Some(sp) = p.ppr.as_mut().and_then(|pp| pp.child_mut("w:sectPr")) {
                    out.push(sp);
                }
            }
        }
        out.push(&mut self.final_sectpr);
        out
    }

    /// rIds of the default (odd/all pages), first-page and even-page headers or footers
    /// referenced anywhere in the document.
    fn hf_rids(&mut self, footer: bool) -> Vec<String> {
        let tag = if footer { "w:footerReference" } else { "w:headerReference" };
        let mut out: Vec<String> = Vec::new();
        for sp in self.sectprs_mut() {
            for r in sp.children_named(tag) {
                if let Some(id) = r.attr("r:id") {
                    if !out.iter().any(|x| x == id) {
                        out.push(id.to_string());
                    }
                }
            }
        }
        out.retain(|id| self.header_footers.contains_key(id));
        out
    }

    /// Create an empty header/footer part and reference it as the default one of the first section.
    fn create_hf(&mut self, footer: bool) -> String {
        let (stem, root_tag, rel, ct) = if footer {
            ("footer", "w:ftr", opc::REL_FOOTER, "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml")
        } else {
            ("header", "w:hdr", opc::REL_HEADER, "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml")
        };
        let path = self.package.unique_part_name("word", stem, "xml");
        let shell = xml::parse_str(&format!("<{root_tag} {}/>", crate::template::NS_DECLS)).expect("hdr/ftr shell").root;
        // Reserve the part name right away with valid (empty) content.
        let mut placeholder = shell.clone();
        placeholder.children.push(Node::Element(Element::new("w:p")));
        self.package.set(&path, xml::serialize_document(&placeholder, None));
        self.package.ensure_override_content_type(&path, ct);
        let main = self.main_part.clone();
        let rid = self.package.add_rel(&main, rel, path.trim_start_matches("word/"), false);
        self.rel_targets.insert((main, rid.clone()), (path.clone(), false));
        self.header_footers.insert(rid.clone(), HeaderFooter { part: path, shell, decl: None, blocks: Vec::new(), dirty: true });
        // The document root must declare the r: prefix used by the reference.
        if self.shell.attr("xmlns:r").is_none() {
            self.shell.set_attr("xmlns:r", "http://schemas.openxmlformats.org/officeDocument/2006/relationships");
        }
        let tag = if footer { "w:footerReference" } else { "w:headerReference" };
        let reference = Element::new(tag).with_attr("w:type", "default").with_attr("r:id", rid.as_str());
        let first = self.sectprs_mut().into_iter().next().expect("final sectPr always exists");
        // References lead the sectPr; headers before footers.
        let idx = first
            .children
            .iter()
            .rposition(|n| matches!(n, Node::Element(e) if e.name == "w:headerReference" || (footer && e.name == "w:footerReference")))
            .map(|i| i + 1)
            .unwrap_or(0);
        first.children.insert(idx, Node::Element(reference));
        rid
    }

    /// Insert a page number in the given style at the top or bottom of every page.
    /// Replaces any existing page number in those headers/footers; other content is kept.
    pub fn insert_page_number(&mut self, st: &PnStyle, place: Place) {
        let footer = place == Place::Bottom;
        // A page number lives in one place: clear numbers from the other side first.
        self.remove_page_numbers_in(!footer);
        let mut rids = self.hf_rids(footer);
        if rids.is_empty() {
            rids.push(self.create_hf(footer));
        }
        let taper_w = {
            // 2" shorter than the text width (1" trimmed from each end), never under 1".
            let sect = self.sections();
            let s = &sect.last().expect("final section").1;
            (s.page_w - s.margin_left - s.margin_right - s.gutter - 144.0).max(72.0)
        };
        for rid in rids {
            let taper = (st.id == "thin_line").then(|| {
                let part = self.header_footers[&rid].part.clone();
                Taper { rid: self.ensure_taper_image(&part, taper_w), w_pt: taper_w, h_pt: taper_height_pt() }
            });
            let hf = self.header_footers.get_mut(&rid).expect("rid checked");
            hf.blocks.retain(|b| !is_page_number_block(b));
            // Drop leftover empty paragraphs so the footer doesn't grow with each insert.
            hf.blocks.retain(|b| !matches!(b, Block::Paragraph(p) if p.plain_text().trim().is_empty() && !model::block_to_xml(b).to_xml().contains("w:drawing")));
            let blk = block(st, place, taper.as_ref());
            match place {
                Place::Top => hf.blocks.insert(0, blk),
                Place::Bottom => hf.blocks.push(blk),
            }
            hf.dirty = true;
        }
    }

    /// Store the taper image (regenerated for this width) and make sure `part` has a relationship to it.
    fn ensure_taper_image(&mut self, part: &str, w_pt: f32) -> String {
        let bytes = taper_png(w_pt);
        self.package.set(TAPER_PART, bytes.clone());
        self.package.ensure_default_content_type("png", "image/png");
        self.images.insert(TAPER_PART.to_string(), std::sync::Arc::new(bytes));
        let existing = self
            .package
            .rels(part)
            .into_iter()
            .find(|r| r.rel_type == opc::REL_IMAGE && !r.external && opc::Package::resolve_target(part, &r.target) == TAPER_PART);
        let rid = match existing {
            Some(r) => r.id,
            None => {
                // Header/footer parts live in word/, next to media/.
                let target = TAPER_PART.trim_start_matches("word/");
                self.package.add_rel(part, opc::REL_IMAGE, target, false)
            }
        };
        self.rel_targets.insert((part.to_string(), rid.clone()), (TAPER_PART.to_string(), false));
        rid
    }

    fn remove_page_numbers_in(&mut self, footer: bool) -> usize {
        let mut n = 0;
        for rid in self.hf_rids(footer) {
            let hf = self.header_footers.get_mut(&rid).expect("rid checked");
            let before = hf.blocks.len();
            hf.blocks.retain(|b| !is_page_number_block(b));
            if hf.blocks.len() != before {
                n += before - hf.blocks.len();
                hf.dirty = true;
            }
            if hf.blocks.is_empty() {
                hf.blocks.push(Block::Paragraph(model::Paragraph::new()));
            }
        }
        n
    }

    /// Remove page numbers from all headers and footers. Returns how many blocks were removed.
    pub fn remove_page_numbers(&mut self) -> usize {
        self.remove_page_numbers_in(true) + self.remove_page_numbers_in(false)
    }

    /// Set the page-number format (w:pgNumType/@w:fmt) of every section.
    pub fn set_page_number_format(&mut self, fmt: &str) {
        for sp in self.sectprs_mut() {
            let el = sp.ensure_child("w:pgNumType", SECTPR_ORDER);
            if fmt == "decimal" {
                el.remove_attr("w:fmt");
            } else {
                el.set_attr("w:fmt", fmt);
            }
        }
    }

    /// The page-number format of the last section (what the UI shows as current).
    pub fn page_number_format(&self) -> String {
        self.final_sectpr.child("w:pgNumType").and_then(|e| e.attr("w:fmt")).unwrap_or("decimal").to_string()
    }

    /// Does any header or footer contain a page number?
    pub fn has_page_numbers(&self) -> bool {
        self.header_footers.values().any(|hf| hf.blocks.iter().any(is_page_number_block))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_style_parses_and_is_detected() {
        for st in STYLES {
            for place in [Place::Top, Place::Bottom] {
                let t = Taper { rid: "rId99".into(), w_pt: 324.0, h_pt: taper_height_pt() };
                let b = block(st, place, Some(&t));
                assert!(is_page_number_block(&b), "{} {:?}", st.id, place);
            }
        }
    }

    #[test]
    fn insert_into_blank_creates_footer_and_survives_save() {
        let mut d = Document::new_blank();
        d.insert_page_number(style("thin_line").unwrap(), Place::Bottom);
        assert!(d.has_page_numbers());
        let ftr = d.final_sectpr.child("w:footerReference").expect("footer reference added");
        assert_eq!(ftr.attr("w:type"), Some("default"));
        let rid = ftr.attr("r:id").unwrap().to_string();
        // Re-inserting replaces rather than stacks.
        d.insert_page_number(style("brackets").unwrap(), Place::Bottom);
        assert_eq!(d.header_footers[&rid].blocks.iter().filter(|b| is_page_number_block(b)).count(), 1);
        // Round-trip through a saved package.
        let dir = std::env::temp_dir().join(format!("rdw-pn-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("pn.docx");
        d.save_as(&path).unwrap();
        let back = Document::open(&path).unwrap();
        assert!(back.has_page_numbers());
        let mut back = back;
        assert_eq!(back.remove_page_numbers(), 1);
        assert!(!back.has_page_numbers());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn top_moves_number_out_of_footer() {
        let mut d = Document::new_blank();
        d.insert_page_number(style("brackets").unwrap(), Place::Bottom);
        d.insert_page_number(style("thin_line").unwrap(), Place::Top);
        assert!(d.final_sectpr.child("w:headerReference").is_some());
        let ftr = d.final_sectpr.child("w:footerReference").unwrap().attr("r:id").unwrap().to_string();
        assert!(!d.header_footers[&ftr].blocks.iter().any(is_page_number_block));
        // Header reference must precede the footer reference in the sectPr.
        let names: Vec<&str> = d.final_sectpr.elements().map(|e| e.name.as_str()).collect();
        assert!(names.iter().position(|n| *n == "w:headerReference") < names.iter().position(|n| *n == "w:footerReference"));
    }

    #[test]
    fn thin_line_taper_is_an_image_in_the_footer_part() {
        let mut d = Document::new_blank();
        d.insert_page_number(style("thin_line").unwrap(), Place::Bottom);
        let rid = d.final_sectpr.child("w:footerReference").unwrap().attr("r:id").unwrap().to_string();
        let part = d.header_footers[&rid].part.clone();
        let rel = d.package.rels(&part).into_iter().find(|r| r.rel_type == opc::REL_IMAGE).expect("image rel on footer part");
        assert_eq!(opc::Package::resolve_target(&part, &rel.target), TAPER_PART);
        // Re-inserting reuses the relationship instead of adding another.
        d.insert_page_number(style("thin_line").unwrap(), Place::Bottom);
        assert_eq!(d.package.rels(&part).into_iter().filter(|r| r.rel_type == opc::REL_IMAGE).count(), 1);
        // The image swells: more opaque pixels in the middle column than at the ends.
        let img = image::load_from_memory(d.package.get(TAPER_PART).unwrap()).unwrap().to_rgba8();
        let ink = |x: u32| (0..img.height()).map(|y| img.get_pixel(x, y)[3] as u32).sum::<u32>();
        assert!(ink(img.width() / 2) > 3 * ink(0));
    }

    #[test]
    fn format_sets_pgnumtype() {
        let mut d = Document::new_blank();
        d.set_page_number_format("lowerRoman");
        assert_eq!(d.page_number_format(), "lowerRoman");
        d.set_page_number_format("decimal");
        assert_eq!(d.page_number_format(), "decimal");
    }
}
