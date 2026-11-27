//! WordArt: decorative text in a floating text box, styled with Word 2010 text
//! effects (`w14:textFill`, `w14:textOutline`, `w14:glow`, `w14:shadow`,
//! `w14:reflection`, `w14:scene3d` and `w14:props3d`).
//!
//! The XML matches what Word itself writes for WordArt: a `wp:anchor` holding a
//! `wps:wsp` text box with no fill or line, whose run carries the effects. A
//! plain `w:color` is written too, so applications without `w14` support still
//! show coloured text.

use crate::fonts::FontDb;
use crate::xml::{self, Element};

const EMU: f32 = 12700.0;

#[derive(Clone, Copy, Debug)]
pub enum Fill {
    Solid(&'static str),
    /// Stops as (position %, colour) and the angle in degrees (90 = top to bottom).
    Gradient(&'static [(u8, &'static str)], u16),
    None,
}

#[derive(Clone, Copy, Debug)]
pub struct WordArtStyle {
    pub name: &'static str,
    pub fill: Fill,
    /// Width (pt) and colour.
    pub outline: Option<(f32, &'static str)>,
    /// Blur (pt), distance (pt), direction (degrees, 90 = down), colour, transparency %.
    pub shadow: Option<(f32, f32, u16, &'static str, u8)>,
    /// Radius (pt), colour, transparency %.
    pub glow: Option<(f32, &'static str, u8)>,
    pub reflection: bool,
    /// Top bevel size (pt).
    pub bevel: Option<f32>,
    /// Extrusion depth (pt) and colour, seen through `camera`.
    pub extrusion: Option<(f32, &'static str)>,
    pub camera: &'static str,
}

const BASE: WordArtStyle = WordArtStyle {
    name: "",
    fill: Fill::Solid("000000"),
    outline: None,
    shadow: None,
    glow: None,
    reflection: false,
    bevel: None,
    extrusion: None,
    camera: "orthographicFront",
};

pub const STYLES: &[WordArtStyle] = &[
    WordArtStyle { name: "Ocean Blue", fill: Fill::Solid("156082"), shadow: Some((4.0, 3.0, 45, "000000", 60)), ..BASE },
    WordArtStyle {
        name: "Gold 3D",
        fill: Fill::Gradient(&[(0, "FFF1B5"), (45, "F2C43B"), (55, "C9961A"), (100, "8A5D00")], 90),
        outline: Some((0.5, "7A5200")),
        bevel: Some(4.0),
        extrusion: Some((14.0, "6B4700")),
        camera: "obliqueBottomRight",
        ..BASE
    },
    WordArtStyle { name: "Neon", fill: Fill::Solid("FFFFFF"), outline: Some((1.0, "0F9ED5")), glow: Some((10.0, "0F9ED5", 30)), ..BASE },
    WordArtStyle {
        name: "Chrome",
        fill: Fill::Gradient(&[(0, "FFFFFF"), (46, "B4B4B4"), (54, "595959"), (100, "E6E6E6")], 90),
        outline: Some((0.75, "404040")),
        bevel: Some(2.0),
        reflection: true,
        ..BASE
    },
    WordArtStyle { name: "Fire", fill: Fill::Gradient(&[(0, "FFE46B"), (50, "FF8A00"), (100, "C00000")], 90), glow: Some((6.0, "FF6A00", 50)), ..BASE },
    WordArtStyle { name: "Reflection", fill: Fill::Gradient(&[(0, "4FC3F7"), (100, "0E2841")], 90), reflection: true, ..BASE },
    WordArtStyle {
        name: "Retro 3D",
        fill: Fill::Solid("FFC000"),
        outline: Some((0.75, "000000")),
        extrusion: Some((18.0, "C00000")),
        camera: "obliqueBottomRight",
        ..BASE
    },
    WordArtStyle { name: "Outline", fill: Fill::None, outline: Some((2.0, "156082")), shadow: Some((0.0, 3.0, 45, "156082", 70)), ..BASE },
    WordArtStyle {
        name: "Candy",
        fill: Fill::Gradient(&[(0, "FFB3E1"), (100, "A02B93")], 90),
        bevel: Some(4.0),
        shadow: Some((6.0, 3.0, 90, "A02B93", 60)),
        ..BASE
    },
    WordArtStyle {
        name: "Emerald 3D",
        fill: Fill::Gradient(&[(0, "8FE38A"), (100, "196B24")], 90),
        bevel: Some(3.0),
        extrusion: Some((12.0, "0D3D14")),
        camera: "obliqueBottomLeft",
        ..BASE
    },
    WordArtStyle { name: "Ice", fill: Fill::Gradient(&[(0, "F2FBFF"), (100, "6CCBF5")], 90), outline: Some((1.0, "0F7FB5")), glow: Some((4.0, "6FCBFF", 40)), ..BASE },
    WordArtStyle {
        name: "Rainbow",
        fill: Fill::Gradient(&[(0, "C00000"), (20, "FF8A00"), (40, "FFC000"), (60, "4EA72E"), (80, "0F9ED5"), (100, "7030A0")], 0),
        shadow: Some((3.0, 2.0, 45, "000000", 65)),
        ..BASE
    },
];

fn emu(pt: f32) -> i64 {
    (pt * EMU).round() as i64
}

fn clr(hex: &str, transparency: Option<u8>) -> String {
    match transparency {
        Some(t) => format!(r#"<w14:srgbClr w14:val="{hex}"><w14:alpha w14:val="{}"/></w14:srgbClr>"#, t as u32 * 1000),
        None => format!(r#"<w14:srgbClr w14:val="{hex}"/>"#),
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

impl WordArtStyle {
    /// The colour written as plain `w:color` for consumers without `w14` support.
    /// Near-white fills (which rely on their outline or glow to show) fall back to that colour instead.
    pub fn fallback_color(&self) -> &'static str {
        let fill = match self.fill {
            Fill::Solid(c) => Some(c),
            Fill::Gradient(stops, _) => Some(stops[stops.len() / 2].1),
            Fill::None => None,
        };
        let pale = |hex: &str| crate::props::Color::from_hex(hex).is_some_and(|c| c.luminance() > 0.85);
        match fill {
            Some(c) if !pale(c) => c,
            _ => self.outline.map(|o| o.1).or(self.glow.map(|g| g.1)).unwrap_or("000000"),
        }
    }

    /// The `w14` effect elements, in the order the schema requires.
    pub fn effects_xml(&self) -> String {
        let mut s = String::new();
        if let Some((rad, c, t)) = self.glow {
            s += &format!(r#"<w14:glow w14:rad="{}">{}</w14:glow>"#, emu(rad), clr(c, Some(t)));
        }
        if let Some((blur, dist, dir, c, t)) = self.shadow {
            s += &format!(
                r#"<w14:shadow w14:blurRad="{}" w14:dist="{}" w14:dir="{}" w14:sx="100000" w14:sy="100000" w14:kx="0" w14:ky="0" w14:algn="tl">{}</w14:shadow>"#,
                emu(blur),
                emu(dist),
                dir as u32 * 60000,
                clr(c, Some(t))
            );
        }
        if self.reflection {
            s += r#"<w14:reflection w14:blurRad="6350" w14:stA="50000" w14:stPos="0" w14:endA="300" w14:endPos="55000" w14:dist="0" w14:dir="5400000" w14:fadeDir="5400000" w14:sx="100000" w14:sy="-100000" w14:kx="0" w14:ky="0" w14:algn="bl"/>"#;
        }
        if let Some((w, c)) = self.outline {
            s += &format!(
                r#"<w14:textOutline w14:w="{}" w14:cap="flat" w14:cmpd="sng" w14:algn="ctr"><w14:solidFill>{}</w14:solidFill><w14:prstDash w14:val="solid"/><w14:round/></w14:textOutline>"#,
                emu(w),
                clr(c, None)
            );
        }
        s += &match self.fill {
            Fill::Solid(c) => format!("<w14:textFill><w14:solidFill>{}</w14:solidFill></w14:textFill>", clr(c, None)),
            Fill::Gradient(stops, ang) => {
                let gs: String = stops.iter().map(|(p, c)| format!(r#"<w14:gs w14:pos="{}">{}</w14:gs>"#, *p as u32 * 1000, clr(c, None))).collect();
                format!(r#"<w14:textFill><w14:gradFill><w14:gsLst>{gs}</w14:gsLst><w14:lin w14:ang="{}" w14:scaled="0"/></w14:gradFill></w14:textFill>"#, ang as u32 * 60000)
            }
            Fill::None => "<w14:textFill><w14:noFill/></w14:textFill>".into(),
        };
        if self.bevel.is_some() || self.extrusion.is_some() {
            s += &format!(r#"<w14:scene3d><w14:camera w14:prst="{}"/><w14:lightRig w14:rig="threePt" w14:dir="t"/></w14:scene3d>"#, self.camera);
            let bevel = self.bevel.map(|b| format!(r#"<w14:bevelT w14:w="{0}" w14:h="{0}" w14:prst="circle"/>"#, emu(b))).unwrap_or_default();
            let (depth, ecol) = match self.extrusion {
                Some((d, c)) => (emu(d), format!("<w14:extrusionClr>{}</w14:extrusionClr>", clr(c, None))),
                None => (0, String::new()),
            };
            s += &format!(r#"<w14:props3d w14:extrusionH="{depth}" w14:contourW="0" w14:prstMaterial="warmMatte">{bevel}{ecol}</w14:props3d>"#);
        }
        s
    }

    /// A complete `w:rPr` for WordArt text in this style.
    pub fn rpr_xml(&self, font: &str, size: f32) -> String {
        let f = esc(font);
        let hs = (size * 2.0).round() as i32;
        format!(
            r#"<w:rPr><w:rFonts w:ascii="{f}" w:hAnsi="{f}" w:cs="{f}" w:eastAsia="{f}"/><w:b/><w:bCs/><w:color w:val="{}"/><w:sz w:val="{hs}"/><w:szCs w:val="{hs}"/>{}</w:rPr>"#,
            self.fallback_color(),
            self.effects_xml()
        )
    }

    /// Room the effects need around the text (pt): (sides, top, bottom).
    fn overhang(&self, size: f32) -> (f32, f32, f32) {
        let mut side: f32 = 0.0;
        let mut bottom: f32 = 0.0;
        if let Some((rad, ..)) = self.glow {
            side = side.max(rad);
            bottom = bottom.max(rad);
        }
        if let Some((blur, dist, ..)) = self.shadow {
            side = side.max(blur + dist);
            bottom = bottom.max(blur + dist);
        }
        if let Some((d, _)) = self.extrusion {
            side = side.max(d * 0.6);
            bottom = bottom.max(d * 0.6);
        }
        if self.reflection {
            bottom = bottom.max(size * 0.62);
        }
        (side, side.min(8.0), bottom)
    }
}

/// What the WordArt dialog edits.
#[derive(Clone, Debug, PartialEq)]
pub struct WordArt {
    pub style: usize,
    pub text: String,
    pub font: String,
    pub size: f32,
}

const NAME_PREFIX: &str = "WordArt: ";

/// The w:drawing for `art`. `keep` is an existing WordArt drawing whose position and wrapping are kept.
pub fn drawing(art: &WordArt, fonts: &FontDb, id: u32, keep: Option<&Element>) -> Element {
    let style = STYLES.get(art.style).copied().unwrap_or(STYLES[0]);
    let lines: Vec<&str> = if art.text.is_empty() { vec![" "] } else { art.text.lines().collect() };
    // Measure the text the way layout will: bold face, GDI line height.
    let fm = fonts.resolve(&art.font, true, false);
    let width = lines.iter().map(|l| l.chars().map(|c| fm.font.advance(c, art.size)).sum::<f32>()).fold(0.0, f32::max);
    let line_h = fm.font.ascent(art.size) + fm.font.descent(art.size);
    let (side, top, bottom) = style.overhang(art.size);
    // 7.2 pt insets on each side, plus slack so the line never wraps.
    let w = width + 14.4 + 6.0 + side * 2.0;
    let h = line_h * lines.len() as f32 + 7.2 + top + bottom;
    let (cx, cy) = (emu(w), emu(h));
    let rpr = style.rpr_xml(&art.font, art.size);
    let paras: String = lines
        .iter()
        .map(|l| format!(r#"<w:p><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/><w:jc w:val="center"/></w:pPr><w:r>{rpr}<w:t xml:space="preserve">{}</w:t></w:r></w:p>"#, esc(l)))
        .collect();
    let name = esc(&format!("{NAME_PREFIX}{}", style.name));
    let (pos_h, pos_v, wrap) = match keep.and_then(|k| k.elements().find(|e| e.local() == "anchor")) {
        Some(a) => {
            let ser = |n: &str| a.child_local(n).map(|e| e.to_xml());
            (
                ser("positionH").unwrap_or_default(),
                ser("positionV").unwrap_or_default(),
                a.elements().find(|e| e.local().starts_with("wrap")).map(|e| e.to_xml()).unwrap_or_else(|| "<wp:wrapTopAndBottom/>".into()),
            )
        }
        None => (
            r#"<wp:positionH relativeFrom="column"><wp:align>center</wp:align></wp:positionH>"#.to_string(),
            r#"<wp:positionV relativeFrom="paragraph"><wp:posOffset>0</wp:posOffset></wp:positionV>"#.to_string(),
            "<wp:wrapTopAndBottom/>".to_string(),
        ),
    };
    let x = format!(
        r#"<w:drawing xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml"><wp:anchor distT="0" distB="0" distL="114300" distR="114300" simplePos="0" relativeHeight="251659264" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/>{pos_h}{pos_v}<wp:extent cx="{cx}" cy="{cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/>{wrap}<wp:docPr id="{id}" name="{name}"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp><wps:cNvSpPr txBox="1"/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/><a:ln><a:noFill/></a:ln></wps:spPr><wps:txbx><w:txbxContent>{paras}</w:txbxContent></wps:txbx><wps:bodyPr rot="0" spcFirstLastPara="0" vertOverflow="overflow" horzOverflow="overflow" vert="horz" wrap="none" lIns="{ins_lr}" tIns="{ins_t}" rIns="{ins_lr}" bIns="0" anchor="t" anchorCtr="0"><a:prstTxWarp prst="textNoShape"><a:avLst/></a:prstTxWarp><a:noAutofit/></wps:bodyPr></wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing>"#,
        ins_lr = emu(7.2 + side),
        ins_t = emu(3.6 + top),
    );
    xml::parse_str(&x).expect("WordArt XML").root
}

/// WordArt the way Word 2010+ stores it: `mc:AlternateContent` holding the
/// DrawingML text box, plus a VML text box with the same text, size, position
/// and wrapping for readers that predate DrawingML shapes (Word 2007, Apache
/// OpenOffice 4). `keep` is the existing item (or drawing) being replaced.
pub fn run_item(art: &WordArt, fonts: &FontDb, id: u32, keep: Option<&Element>) -> crate::model::RunItem {
    let d = drawing(art, fonts, id, keep.map(inner_drawing));
    crate::model::RunItem::Other(with_fallback(&d, id))
}

/// The w:drawing inside an `mc:AlternateContent` (or `e` itself).
pub fn inner_drawing(e: &Element) -> &Element {
    if e.local() == "AlternateContent" {
        for c in e.elements().filter(|c| c.local() == "Choice") {
            if let Some(d) = c.elements().find(|d| d.name == "w:drawing") {
                return d;
            }
        }
    }
    e
}

/// Wrap an anchored DrawingML text box in `mc:AlternateContent` with a VML fallback.
pub fn with_fallback(d: &Element, id: u32) -> Element {
    let pt = |emu: Option<&str>| emu.and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) / 12700.0;
    let anchor = d.child_local("anchor");
    let ext = anchor.and_then(|a| a.child_local("extent"));
    let (w, h) = (pt(ext.and_then(|e| e.attr("cx"))), pt(ext.and_then(|e| e.attr("cy"))));
    let dist = |k: &str| pt(anchor.and_then(|a| a.attr(k)));
    // DrawingML position → VML style (offset, alignment, what it is relative to).
    let position = |axis: &str, map: &[(&str, &str)]| -> (f64, String, String) {
        let p = anchor.and_then(|a| a.child_local(axis));
        let rel = p.and_then(|p| p.attr("relativeFrom")).unwrap_or("");
        let rel = map.iter().find(|(k, _)| *k == rel).map(|(_, v)| *v).unwrap_or("text").to_string();
        let align = p.and_then(|p| p.child_local("align")).map(|a| a.text());
        let off = pt(p.and_then(|p| p.child_local("posOffset")).map(|o| o.text()).as_deref());
        (off, align.unwrap_or_else(|| "absolute".into()), rel)
    };
    let (x, h_align, h_rel) = position(
        "positionH",
        &[("column", "text"), ("margin", "margin"), ("page", "page"), ("character", "char"), ("leftMargin", "left-margin-area"), ("rightMargin", "right-margin-area"), ("insideMargin", "inner-margin-area"), ("outsideMargin", "outer-margin-area")],
    );
    let (y, v_align, v_rel) = position(
        "positionV",
        &[("paragraph", "text"), ("margin", "margin"), ("page", "page"), ("line", "line"), ("topMargin", "top-margin-area"), ("bottomMargin", "bottom-margin-area")],
    );
    let wrap = match anchor.and_then(|a| a.elements().find(|e| e.local().starts_with("wrap"))).map(|e| e.local().to_string()).as_deref() {
        Some("wrapTopAndBottom") => r#"<w10:wrap type="topAndBottom"/>"#,
        Some("wrapSquare") => r#"<w10:wrap type="square"/>"#,
        Some("wrapTight") => r#"<w10:wrap type="tight"/>"#,
        Some("wrapThrough") => r#"<w10:wrap type="through"/>"#,
        _ => "",
    };
    let body = d.find_local("bodyPr");
    let ins = |k: &str| pt(body.and_then(|b| b.attr(k)));
    let content = d.find_local("txbxContent").map(|c| c.to_xml()).unwrap_or_default();
    let style = format!(
        "position:absolute;margin-left:{x:.2}pt;margin-top:{y:.2}pt;width:{w:.2}pt;height:{h:.2}pt;z-index:251659264;visibility:visible;mso-wrap-style:none;\
         mso-wrap-distance-left:{:.2}pt;mso-wrap-distance-top:{:.2}pt;mso-wrap-distance-right:{:.2}pt;mso-wrap-distance-bottom:{:.2}pt;\
         mso-position-horizontal:{h_align};mso-position-horizontal-relative:{h_rel};mso-position-vertical:{v_align};mso-position-vertical-relative:{v_rel}",
        dist("distL"),
        dist("distT"),
        dist("distR"),
        dist("distB"),
    );
    let x = format!(
        r#"<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:w10="urn:schemas-microsoft-com:office:word" xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml"><mc:Choice Requires="wps">{drawing}</mc:Choice><mc:Fallback><w:pict><v:shape id="WordArt{id}" o:spid="_x0000_s{spid}" coordsize="21600,21600" o:spt="202" path="m,l,21600r21600,l21600,xe" style="{style}" filled="f" stroked="f"><v:textbox inset="{l:.2}pt,{t:.2}pt,{r:.2}pt,{b:.2}pt">{content}</v:textbox>{wrap}</v:shape></w:pict></mc:Fallback></mc:AlternateContent>"#,
        drawing = d.to_xml(),
        spid = 1024u64 + id as u64,
        l = ins("lIns"),
        t = ins("tIns"),
        r = ins("rIns"),
        b = ins("bIns"),
    );
    xml::parse_str(&x).expect("WordArt fallback XML").root
}

/// Read back a WordArt drawing written by [`drawing`] (identified by its `WordArt: <style>` name).
pub fn read(drawing: &Element) -> Option<WordArt> {
    let name = drawing.find_local("docPr")?.attr("name")?;
    let style_name = name.strip_prefix(NAME_PREFIX)?;
    let style = STYLES.iter().position(|s| s.name == style_name).unwrap_or(0);
    let content = drawing.find_local("txbxContent")?;
    let paras: Vec<&Element> = content.elements().filter(|p| p.local() == "p").collect();
    let text = paras
        .iter()
        .map(|p| {
            let mut s = String::new();
            collect_text(p, &mut s);
            s
        })
        .collect::<Vec<_>>()
        .join("\n");
    let rpr = content.find_local("rPr");
    let font = rpr.and_then(|r| r.child_local("rFonts")).and_then(|f| f.attr("w:ascii")).unwrap_or("Arial").to_string();
    let size = rpr.and_then(|r| r.child_local("sz")).and_then(|s| s.attr("w:val")).and_then(|v| v.parse::<f32>().ok()).map(|v| v / 2.0).unwrap_or(40.0);
    Some(WordArt { style, text, font, size })
}

/// The largest `wp:docPr` id among the drawings in `blocks` (ids must be unique in a document).
pub fn max_docpr_id(blocks: &[crate::model::Block]) -> u32 {
    use crate::model::{Block, RowItem, RunItem};
    let mut max = 0;
    for b in blocks {
        match b {
            Block::Paragraph(p) => p.visit_runs(&mut |_, r, _| {
                for it in &r.items {
                    let d = match it {
                        RunItem::Drawing(d) => d,
                        RunItem::Other(e) if e.name == "mc:AlternateContent" => e,
                        _ => continue,
                    };
                    if let Some(id) = d.find_local("docPr").and_then(|e| e.attr("id")).and_then(|v| v.parse::<u32>().ok()) {
                        max = max.max(id);
                    }
                }
            }),
            Block::Table(t) => {
                for row in &t.rows {
                    if let RowItem::Row(r) = row {
                        for c in r.cells() {
                            max = max.max(max_docpr_id(&c.blocks));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    max
}

fn collect_text(e: &Element, out: &mut String) {
    for c in e.elements() {
        if c.local() == "t" {
            out.push_str(&c.text());
        } else {
            collect_text(c, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styles_round_trip_through_the_run_property_parser() {
        let fonts = FontDb::load_system();
        let doc = crate::document::Document::new_blank();
        for (i, st) in STYLES.iter().enumerate() {
            let art = WordArt { style: i, text: "Hello\nWorld".into(), font: "Arial".into(), size: 36.0 };
            let d = drawing(&art, &fonts, 7, None);
            assert_eq!(read(&d).as_ref(), Some(&art), "{}", st.name);
            // The run's effects come back from the same parser layout uses.
            let rpr = d.find_local("rPr").unwrap();
            let rp = doc.resolver().run("Normal", Some(rpr), None);
            let fx = rp.fx14.unwrap_or_else(|| panic!("{} has no effects", st.name));
            assert_eq!(fx.no_fill, matches!(st.fill, Fill::None), "{}", st.name);
            // Without w14 support the text must still be visible on a white page.
            let fb = crate::props::Color::from_hex(st.fallback_color()).unwrap();
            assert!(fb.luminance() < 0.85, "{}: fallback colour too pale", st.name);
            assert_eq!(fx.extrusion.is_some(), st.extrusion.is_some(), "{}", st.name);
            assert_eq!(fx.glow.is_some(), st.glow.is_some(), "{}", st.name);
            assert_eq!(fx.reflection.is_some(), st.reflection, "{}", st.name);
            if let Some((_, _, _, _, t)) = st.shadow {
                let a = fx.shadow.as_ref().unwrap().alpha;
                assert!((a - (1.0 - t as f32 / 100.0)).abs() < 0.01, "{}: w14:alpha is transparency", st.name);
            }
        }
    }

    #[test]
    fn editing_keeps_position_and_wrapping() {
        let fonts = FontDb::load_system();
        let art = WordArt { style: 1, text: "Gold".into(), font: "Arial".into(), size: 40.0 };
        let mut d = drawing(&art, &fonts, 3, None);
        // Pretend the user (or Word) moved it and changed the wrapping.
        let anchor = d.elements_mut().find(|e| e.local() == "anchor").unwrap();
        for c in anchor.elements_mut() {
            if c.local() == "wrapTopAndBottom" {
                c.name = "wp:wrapSquare".into();
                c.set_attr("wrapText", "bothSides");
            }
        }
        let moved = xml::parse_str(r#"<wp:positionH xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" relativeFrom="page"><wp:posOffset>914400</wp:posOffset></wp:positionH>"#).unwrap().root;
        for c in anchor.children.iter_mut() {
            if matches!(c, xml::Node::Element(e) if e.local() == "positionH") {
                *c = xml::Node::Element(moved.clone());
            }
        }
        let edited = drawing(&WordArt { text: "Gold!".into(), ..art.clone() }, &fonts, 3, Some(&d));
        let a = edited.elements().find(|e| e.local() == "anchor").unwrap();
        assert_eq!(a.child_local("positionH").unwrap().attr("relativeFrom"), Some("page"));
        assert_eq!(a.child_local("wrapSquare").and_then(|w| w.attr("wrapText")), Some("bothSides"));
        assert!(a.child_local("wrapTopAndBottom").is_none());
        assert_eq!(read(&edited).unwrap().text, "Gold!");
    }
}
