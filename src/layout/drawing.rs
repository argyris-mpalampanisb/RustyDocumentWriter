//! DrawingML (wp:inline / wp:anchor) and legacy VML picture interpretation.

use crate::model::Block;
use crate::props::Color;
use crate::xml::Element;
use std::sync::Arc;

pub const EMU_PER_PT: f32 = 12700.0;

#[derive(Clone, Debug)]
pub enum DrawKind {
    Image { key: String, data: Arc<Vec<u8>>, crop: [f32; 4] },
    Shape { fill: Option<Color>, line: Option<(Color, f32)>, text: Vec<Block>, insets: [f32; 4], ellipse: bool, v_anchor: String },
    Placeholder(String),
}

#[derive(Clone, Debug)]
pub struct Anchor {
    pub h_rel: String,
    pub h_off: f32,
    pub h_align: Option<String>,
    pub v_rel: String,
    pub v_off: f32,
    pub v_align: Option<String>,
    pub wrap: String,
    pub wrap_side: String,
    pub behind: bool,
    pub dist: [f32; 4], // t, b, l, r
}

#[derive(Clone, Debug)]
pub struct DrawingObj {
    pub w: f32,
    pub h: f32,
    pub kind: DrawKind,
    pub anchor: Option<Anchor>,
    /// Character offset of the drawing in its paragraph, when it comes from editable body text.
    pub src_off: Option<usize>,
}

fn emu(e: Option<&str>) -> f32 {
    e.and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) as f32 / EMU_PER_PT
}

/// Resolve a DrawingML color element (a:srgbClr, a:schemeClr, ...) with lumMod/lumOff/tint/shade.
pub fn dml_color(fill: &Element, tc: &crate::props::ThemeCtx) -> Option<Color> {
    let c = fill.elements().next()?;
    let mut base = match c.local() {
        "srgbClr" => Color::from_hex(c.attr("val")?)?,
        "schemeClr" => {
            let v = c.attr("val")?;
            let name = match v {
                "tx1" => "text1",
                "tx2" => "text2",
                "bg1" => "background1",
                "bg2" => "background2",
                "phClr" => "accent1",
                o => o,
            };
            tc.theme_color(name)?
        }
        "sysClr" => c.attr("lastClr").and_then(Color::from_hex).unwrap_or(Color::BLACK),
        "prstClr" => match c.attr("val")? {
            "black" => Color::BLACK,
            "white" => Color::WHITE,
            "red" => Color(255, 0, 0),
            "blue" => Color(0, 0, 255),
            "green" => Color(0, 128, 0),
            "yellow" => Color(255, 255, 0),
            _ => Color(128, 128, 128),
        },
        _ => return None,
    };
    let pct = |e: &Element| e.attr("val").and_then(|v| v.parse::<f32>().ok()).unwrap_or(100000.0) / 100000.0;
    let mut lm = 1.0;
    let mut lo = 0.0;
    for m in c.elements() {
        match m.local() {
            "lumMod" => lm = pct(m),
            "lumOff" => lo = pct(m),
            "tint" => base = base.tint(pct(m)),
            "shade" => base = base.shade(pct(m)),
            _ => {}
        }
    }
    if lm != 1.0 || lo != 0.0 {
        base = base.lum_mod_off(lm, lo);
    }
    Some(base)
}

pub fn parse_drawing(drawing: &Element, ctx: &super::Ctx, story_part: &str) -> Option<DrawingObj> {
    let container = drawing.elements().find(|e| e.local() == "inline" || e.local() == "anchor")?;
    let extent = container.child_local("extent")?;
    let w = emu(extent.attr("cx"));
    let h = emu(extent.attr("cy"));
    let tc = ctx.doc.theme_ctx();
    let graphic_data = container.find_local("graphicData");
    let uri = graphic_data.and_then(|g| g.attr("uri")).unwrap_or("");
    let kind = if let Some(blip) = container.find_local("blip") {
        let rid = blip.attr("r:embed").or_else(|| blip.attr_local("embed")).unwrap_or("");
        match ctx.doc.image_for(story_part, rid) {
            Some((key, data)) => {
                let mut crop = [0.0; 4];
                if let Some(sr) = container.find_local("srcRect") {
                    let f = |n: &str| sr.attr(n).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0) / 100000.0;
                    crop = [f("l"), f("t"), f("r"), f("b")];
                }
                DrawKind::Image { key, data, crop }
            }
            None => DrawKind::Placeholder("Linked picture".into()),
        }
    } else if uri.ends_with("wordprocessingShape") {
        let wsp = graphic_data.and_then(|g| g.child_local("wsp"))?;
        let sppr = wsp.child_local("spPr");
        let ellipse = sppr.and_then(|s| s.child_local("prstGeom")).and_then(|g| g.attr("prst")).is_some_and(|p| p == "ellipse");
        let mut fill = sppr.and_then(|s| s.child_local("solidFill")).and_then(|f| dml_color(f, &tc));
        let mut line = sppr.and_then(|s| s.child_local("ln")).and_then(|ln| {
            if ln.child_local("noFill").is_some() {
                return None;
            }
            let wpt = emu(ln.attr("w")).max(0.75);
            let col = ln.child_local("solidFill").and_then(|f| dml_color(f, &tc))?;
            Some((col, wpt))
        });
        let no_fill = sppr.is_some_and(|s| s.child_local("noFill").is_some());
        // Style references (wps:style) supply defaults.
        if let Some(st) = wsp.child_local("style") {
            if fill.is_none() && !no_fill {
                fill = st.child_local("fillRef").and_then(|f| dml_color(f, &tc));
            }
            if line.is_none() && sppr.and_then(|s| s.child_local("ln")).is_none() {
                line = st.child_local("lnRef").and_then(|f| dml_color(f, &tc)).map(|c| (c, 1.0));
            }
        }
        let text = wsp.find_local("txbxContent").map(crate::model::parse_blocks).unwrap_or_default();
        let body = wsp.child_local("bodyPr");
        let ins = |n: &str, d: f32| body.and_then(|b| b.attr(n)).map(|v| emu(Some(v))).unwrap_or(d);
        let insets = [ins("lIns", 7.2), ins("tIns", 3.6), ins("rIns", 7.2), ins("bIns", 3.6)];
        let v_anchor = body.and_then(|b| b.attr("anchor")).unwrap_or("t").to_string();
        DrawKind::Shape { fill, line, text, insets, ellipse, v_anchor }
    } else if uri.contains("chart") {
        DrawKind::Placeholder("Chart".into())
    } else if uri.contains("diagram") {
        DrawKind::Placeholder("SmartArt".into())
    } else if uri.ends_with("wordprocessingGroup") || uri.contains("wordprocessingCanvas") {
        DrawKind::Placeholder("Drawing group".into())
    } else {
        DrawKind::Placeholder("Object".into())
    };

    let anchor = if container.local() == "anchor" {
        let ph = container.child_local("positionH");
        let pv = container.child_local("positionV");
        let off = |p: Option<&Element>| p.and_then(|p| p.child_local("posOffset")).map(|o| emu(Some(o.text().trim()))).unwrap_or(0.0);
        let align = |p: Option<&Element>| p.and_then(|p| p.child_local("align")).map(|a| a.text().trim().to_string());
        let wrap_el = container.elements().find(|e| e.local().starts_with("wrap"));
        let wrap = wrap_el.map(|e| e.local().to_string()).unwrap_or_else(|| "wrapNone".into());
        let dist = |n: &str| emu(container.attr(n));
        Some(Anchor {
            h_rel: ph.and_then(|p| p.attr("relativeFrom")).unwrap_or("column").to_string(),
            h_off: off(ph),
            h_align: align(ph),
            v_rel: pv.and_then(|p| p.attr("relativeFrom")).unwrap_or("paragraph").to_string(),
            v_off: off(pv),
            v_align: align(pv),
            wrap_side: wrap_el.and_then(|e| e.attr("wrapText")).unwrap_or("bothSides").to_string(),
            wrap,
            behind: container.attr("behindDoc").is_some_and(|v| v == "1" || v == "true"),
            dist: [dist("distT"), dist("distB"), dist("distL"), dist("distR")],
        })
    } else {
        None
    };
    Some(DrawingObj { w, h, kind, anchor, src_off: None })
}

/// mc:AlternateContent → the w:drawing inside the first Choice, if any.
pub fn alternate_drawing(alt: &Element) -> Option<&Element> {
    for c in alt.elements() {
        if c.local() == "Choice" {
            if let Some(d) = c.elements().find(|e| e.name == "w:drawing") {
                return Some(d);
            }
        }
    }
    None
}

/// Legacy VML picture (w:pict / w:object with v:imagedata).
pub fn parse_vml(pict: &Element, ctx: &super::Ctx, story_part: &str) -> Option<DrawingObj> {
    let shape = pict.elements().find(|e| matches!(e.local(), "shape" | "rect" | "roundrect" | "oval"))?;
    let style = shape.attr("style").unwrap_or("");
    let mut w = 0.0;
    let mut h = 0.0;
    for part in style.split(';') {
        let mut kv = part.splitn(2, ':');
        let (k, v) = (kv.next().unwrap_or("").trim(), kv.next().unwrap_or("").trim());
        let num = |v: &str| -> f32 {
            let v = v.trim();
            if let Some(n) = v.strip_suffix("pt") {
                n.parse().unwrap_or(0.0)
            } else if let Some(n) = v.strip_suffix("in") {
                n.parse::<f32>().unwrap_or(0.0) * 72.0
            } else if let Some(n) = v.strip_suffix("cm") {
                n.parse::<f32>().unwrap_or(0.0) * 72.0 / 2.54
            } else if let Some(n) = v.strip_suffix("mm") {
                n.parse::<f32>().unwrap_or(0.0) * 72.0 / 25.4
            } else if let Some(n) = v.strip_suffix("px") {
                n.parse::<f32>().unwrap_or(0.0) * 0.75
            } else {
                v.parse::<f32>().unwrap_or(0.0) * 0.75
            }
        };
        match k {
            "width" => w = num(v),
            "height" => h = num(v),
            _ => {}
        }
    }
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let img = shape.find_local("imagedata")?;
    let rid = img.attr("r:id").or_else(|| img.attr_local("id"))?;
    let (key, data) = ctx.doc.image_for(story_part, rid)?;
    Some(DrawingObj { w, h, kind: DrawKind::Image { key, data, crop: [0.0; 4] }, anchor: None, src_off: None })
}
