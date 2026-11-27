//! Resolution of WordprocessingML formatting: styles, themes, numbering,
//! section and table properties.
//!
//! The cascade follows ECMA-376 §17.7 the way it is applied in practice:
//!   docDefaults → table style (conditional) → paragraph style chain →
//!   numbering level → direct paragraph formatting
//! and for runs:
//!   docDefaults → table style → paragraph style → character style → direct.
//! Toggle properties (bold, italic, caps, ...) are XOR-ed between the table,
//! paragraph and character style layers, and set absolutely by direct formatting.

use crate::xml::Element;
use std::collections::HashMap;

pub const RPR_ORDER: &[&str] = &[
    "w:rStyle", "w:rFonts", "w:b", "w:bCs", "w:i", "w:iCs", "w:caps", "w:smallCaps", "w:strike", "w:dstrike", "w:outline",
    "w:shadow", "w:emboss", "w:imprint", "w:noProof", "w:snapToGrid", "w:vanish", "w:webHidden", "w:color", "w:spacing", "w:w",
    "w:kern", "w:position", "w:sz", "w:szCs", "w:highlight", "w:u", "w:effect", "w:bdr", "w:shd", "w:fitText", "w:vertAlign",
    "w:rtl", "w:cs", "w:em", "w:lang", "w:eastAsianLayout", "w:specVanish", "w:oMath",
];

pub const PPR_ORDER: &[&str] = &[
    "w:pStyle", "w:keepNext", "w:keepLines", "w:pageBreakBefore", "w:framePr", "w:widowControl", "w:numPr",
    "w:suppressLineNumbers", "w:pBdr", "w:shd", "w:tabs", "w:suppressAutoHyphens", "w:kinsoku", "w:wordWrap",
    "w:overflowPunct", "w:topLinePunct", "w:autoSpaceDE", "w:autoSpaceDN", "w:bidi", "w:adjustRightInd", "w:snapToGrid",
    "w:spacing", "w:ind", "w:contextualSpacing", "w:mirrorIndents", "w:suppressOverlap", "w:jc", "w:textDirection",
    "w:textAlignment", "w:textboxTightWrap", "w:outlineLvl", "w:divId", "w:cnfStyle", "w:rPr", "w:sectPr", "w:pPrChange",
];

pub const SECTPR_ORDER: &[&str] = &[
    "w:headerReference", "w:footerReference", "w:footnotePr", "w:endnotePr", "w:type", "w:pgSz", "w:pgMar", "w:paperSrc",
    "w:pgBorders", "w:lnNumType", "w:pgNumType", "w:cols", "w:formProt", "w:vAlign", "w:noEndnote", "w:titlePg",
    "w:textDirection", "w:bidi", "w:rtlGutter", "w:docGrid", "w:printerSettings", "w:sectPrChange",
];

pub const TBLPR_ORDER: &[&str] = &[
    "w:tblStyle", "w:tblpPr", "w:tblOverlap", "w:bidiVisual", "w:tblStyleRowBandSize", "w:tblStyleColBandSize", "w:tblW",
    "w:jc", "w:tblCellSpacing", "w:tblInd", "w:tblBorders", "w:shd", "w:tblLayout", "w:tblCellMar", "w:tblLook",
    "w:tblCaption", "w:tblDescription",
];

pub const TCPR_ORDER: &[&str] = &[
    "w:cnfStyle", "w:tcW", "w:gridSpan", "w:hMerge", "w:vMerge", "w:tcBorders", "w:shd", "w:noWrap", "w:tcMar",
    "w:textDirection", "w:tcFitText", "w:vAlign", "w:hideMark",
];

// ---------------------------------------------------------------------------
// Basic value helpers

pub fn twips(v: &str) -> Option<f32> {
    // Values may be integers, decimals, or carry units ("12pt", "1in") in transitional docs.
    let v = v.trim();
    if let Some(n) = v.strip_suffix("pt") {
        return n.parse::<f32>().ok().map(|x| x * 20.0);
    }
    if let Some(n) = v.strip_suffix("in") {
        return n.parse::<f32>().ok().map(|x| x * 1440.0);
    }
    if let Some(n) = v.strip_suffix("cm") {
        return n.parse::<f32>().ok().map(|x| x * 1440.0 / 2.54);
    }
    if let Some(n) = v.strip_suffix("mm") {
        return n.parse::<f32>().ok().map(|x| x * 144.0 / 2.54);
    }
    v.parse::<f32>().ok()
}

pub fn tw_attr(e: &Element, name: &str) -> Option<f32> {
    e.attr(name).and_then(twips)
}

/// Pt value of a twips attribute.
pub fn pt_attr(e: &Element, name: &str) -> Option<f32> {
    tw_attr(e, name).map(|t| t / 20.0)
}

/// ST_OnOff: absent val means true.
pub fn on_off(e: &Element) -> bool {
    match e.attr("w:val") {
        None => true,
        Some(v) => !matches!(v, "0" | "false" | "off" | "none"),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color(pub u8, pub u8, pub u8);

impl Color {
    pub const BLACK: Color = Color(0, 0, 0);
    pub const WHITE: Color = Color(255, 255, 255);

    pub fn from_hex(s: &str) -> Option<Color> {
        let s = s.trim().trim_start_matches('#');
        if s.len() != 6 {
            return None;
        }
        let v = u32::from_str_radix(s, 16).ok()?;
        Some(Color((v >> 16) as u8, (v >> 8) as u8, v as u8))
    }

    pub fn to_hex(self) -> String {
        format!("{:02X}{:02X}{:02X}", self.0, self.1, self.2)
    }

    pub fn luminance(self) -> f32 {
        (0.299 * self.0 as f32 + 0.587 * self.1 as f32 + 0.114 * self.2 as f32) / 255.0
    }

    fn to_hsl(self) -> (f32, f32, f32) {
        let (r, g, b) = (self.0 as f32 / 255.0, self.1 as f32 / 255.0, self.2 as f32 / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;
        if (max - min).abs() < 1e-6 {
            return (0.0, 0.0, l);
        }
        let d = max - min;
        let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
        let h = if max == r {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if max == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        (h / 6.0, s, l)
    }

    fn from_hsl(h: f32, s: f32, l: f32) -> Color {
        if s <= 0.0 {
            let v = (l * 255.0).round().clamp(0.0, 255.0) as u8;
            return Color(v, v, v);
        }
        fn hue(p: f32, q: f32, mut t: f32) -> f32 {
            if t < 0.0 {
                t += 1.0
            }
            if t > 1.0 {
                t -= 1.0
            }
            if t < 1.0 / 6.0 {
                return p + (q - p) * 6.0 * t;
            }
            if t < 0.5 {
                return q;
            }
            if t < 2.0 / 3.0 {
                return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
            }
            p
        }
        let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
        let p = 2.0 * l - q;
        let c = |x: f32| (x * 255.0).round().clamp(0.0, 255.0) as u8;
        Color(c(hue(p, q, h + 1.0 / 3.0)), c(hue(p, q, h)), c(hue(p, q, h - 1.0 / 3.0)))
    }

    /// w:themeTint (fraction of 255): blend towards white in RGB space.
    /// (Verified against reference output, e.g. accent1 156082 + tint 99 ⇒ 9ACDE6-ish ranges.)
    pub fn tint(self, t: f32) -> Color {
        let f = |c: u8| (c as f32 * t + 255.0 * (1.0 - t)).floor().clamp(0.0, 255.0) as u8;
        Color(f(self.0), f(self.1), f(self.2))
    }

    /// w:themeShade: RGB multiply, truncated.
    /// (Default Heading 1: accent1 156082 × BF ⇒ 0F4761.)
    pub fn shade(self, t: f32) -> Color {
        let f = |c: u8| (c as f32 * t).floor().clamp(0.0, 255.0) as u8;
        Color(f(self.0), f(self.1), f(self.2))
    }

    pub fn lum_mod_off(self, m: f32, o: f32) -> Color {
        let (h, s, l) = self.to_hsl();
        Color::from_hsl(h, s, (l * m + o).clamp(0.0, 1.0))
    }

    pub fn blend(self, other: Color, t: f32) -> Color {
        let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Color(m(self.0, other.0), m(self.1, other.1), m(self.2, other.2))
    }
}

pub fn highlight_color(name: &str) -> Option<Color> {
    Some(match name {
        "yellow" => Color(255, 255, 0),
        "green" => Color(0, 255, 0),
        "cyan" => Color(0, 255, 255),
        "magenta" => Color(255, 0, 255),
        "blue" => Color(0, 0, 255),
        "red" => Color(255, 0, 0),
        "darkBlue" => Color(0, 0, 128),
        "darkCyan" => Color(0, 128, 128),
        "darkGreen" => Color(0, 128, 0),
        "darkMagenta" => Color(128, 0, 128),
        "darkRed" => Color(128, 0, 0),
        "darkYellow" => Color(128, 128, 0),
        "darkGray" => Color(128, 128, 128),
        "lightGray" => Color(192, 192, 192),
        "black" => Color(0, 0, 0),
        "white" => Color(255, 255, 255),
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Theme

#[derive(Clone, Debug, Default)]
pub struct Theme {
    pub colors: HashMap<String, Color>,
    pub major_latin: String,
    pub minor_latin: String,
    pub major_ea: String,
    pub minor_ea: String,
    pub major_cs: String,
    pub minor_cs: String,
    pub major_scripts: HashMap<String, String>,
    pub minor_scripts: HashMap<String, String>,
}

impl Theme {
    pub fn parse(root: &Element) -> Theme {
        let mut t = Theme::default();
        if let Some(cs) = root.find_local("clrScheme") {
            for c in cs.elements() {
                let val = c.elements().next().and_then(|v| match v.local() {
                    "srgbClr" => v.attr("val").and_then(Color::from_hex),
                    "sysClr" => v.attr("lastClr").and_then(Color::from_hex).or_else(|| {
                        Some(if v.attr("val") == Some("window") { Color::WHITE } else { Color::BLACK })
                    }),
                    _ => None,
                });
                if let Some(v) = val {
                    t.colors.insert(c.local().to_string(), v);
                }
            }
        }
        if let Some(fs) = root.find_local("fontScheme") {
            let read = |el: Option<&Element>, latin: &mut String, ea: &mut String, cs: &mut String, scripts: &mut HashMap<String, String>| {
                if let Some(el) = el {
                    for c in el.elements() {
                        let tf = c.attr("typeface").unwrap_or_default().to_string();
                        match c.local() {
                            "latin" => *latin = tf,
                            "ea" => *ea = tf,
                            "cs" => *cs = tf,
                            "font" => {
                                if let Some(s) = c.attr("script") {
                                    scripts.insert(s.to_string(), tf);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            };
            read(fs.child_local("majorFont"), &mut t.major_latin, &mut t.major_ea, &mut t.major_cs, &mut t.major_scripts);
            read(fs.child_local("minorFont"), &mut t.minor_latin, &mut t.minor_ea, &mut t.minor_cs, &mut t.minor_scripts);
        }
        t
    }

    pub fn font(&self, theme_ref: &str) -> Option<String> {
        let s = match theme_ref {
            "majorAscii" | "majorHAnsi" => &self.major_latin,
            "minorAscii" | "minorHAnsi" => &self.minor_latin,
            "majorEastAsia" => &self.major_ea,
            "minorEastAsia" => &self.minor_ea,
            "majorBidi" => &self.major_cs,
            "minorBidi" => &self.minor_cs,
            _ => return None,
        };
        if s.is_empty() { None } else { Some(s.clone()) }
    }
}

// ---------------------------------------------------------------------------
// Settings

#[derive(Clone, Debug)]
pub struct Settings {
    pub default_tab: f32, // pt
    pub compat_mode: u32,
    pub even_and_odd_headers: bool,
    pub mirror_margins: bool,
    pub clr_map: HashMap<String, String>,
    pub do_not_expand_shift_return: bool,
    pub balance_sbdcs: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            default_tab: 36.0,
            compat_mode: 12,
            even_and_odd_headers: false,
            mirror_margins: false,
            clr_map: HashMap::new(),
            do_not_expand_shift_return: false,
            balance_sbdcs: false,
        }
    }
}

impl Settings {
    pub fn parse(root: &Element) -> Settings {
        let mut s = Settings::default();
        if let Some(t) = root.child("w:defaultTabStop").and_then(|e| pt_attr(e, "w:val")) {
            if t > 0.0 {
                s.default_tab = t;
            }
        }
        s.even_and_odd_headers = root.child("w:evenAndOddHeaders").is_some_and(on_off);
        s.mirror_margins = root.child("w:mirrorMargins").is_some_and(on_off);
        if let Some(c) = root.child("w:compat") {
            for cs in c.children_named("w:compatSetting") {
                if cs.attr("w:name") == Some("compatibilityMode") {
                    s.compat_mode = cs.attr("w:val").and_then(|v| v.parse().ok()).unwrap_or(12);
                }
            }
            s.do_not_expand_shift_return = c.child("w:doNotExpandShiftReturn").is_some_and(on_off);
            s.balance_sbdcs = c.child("w:balanceSingleByteDoubleByteWidth").is_some_and(on_off);
        } else {
            // No compat element at all: treated as a legacy (mode 11) document.
            s.compat_mode = 11;
        }
        if let Some(m) = root.child("w:clrSchemeMapping") {
            for (k, v) in &m.attrs {
                s.clr_map.insert(crate::xml::local(k).to_string(), v.clone());
            }
        }
        s
    }
}

// ---------------------------------------------------------------------------
// Styles

#[derive(Clone, Debug, PartialEq, Eq, Copy)]
pub enum StyleKind {
    Paragraph,
    Character,
    Table,
    Numbering,
}

#[derive(Clone, Debug)]
pub struct Style {
    pub id: String,
    pub name: String,
    pub kind: StyleKind,
    pub based_on: Option<String>,
    pub next: Option<String>,
    pub link: Option<String>,
    pub is_default: bool,
    pub q_format: bool,
    pub hidden: bool,
    pub ui_priority: i32,
    pub ppr: Option<Element>,
    pub rpr: Option<Element>,
    pub tblpr: Option<Element>,
    pub trpr: Option<Element>,
    pub tcpr: Option<Element>,
    pub conditional: Vec<(String, Element)>,
}

#[derive(Clone, Debug, Default)]
pub struct Styles {
    pub map: HashMap<String, Style>,
    pub order: Vec<String>,
    pub default_ppr: Option<Element>,
    pub default_rpr: Option<Element>,
    pub default_para: Option<String>,
    pub default_char: Option<String>,
    pub default_table: Option<String>,
}

impl Styles {
    pub fn parse(root: &Element) -> Styles {
        let mut s = Styles::default();
        if let Some(dd) = root.child("w:docDefaults") {
            s.default_rpr = dd.child("w:rPrDefault").and_then(|r| r.child("w:rPr")).cloned();
            s.default_ppr = dd.child("w:pPrDefault").and_then(|r| r.child("w:pPr")).cloned();
        }
        for st in root.children_named("w:style") {
            let kind = match st.attr("w:type") {
                Some("character") => StyleKind::Character,
                Some("table") => StyleKind::Table,
                Some("numbering") => StyleKind::Numbering,
                _ => StyleKind::Paragraph,
            };
            let id = st.attr("w:styleId").unwrap_or_default().to_string();
            let val = |n: &str| st.child(n).and_then(|e| e.attr("w:val")).map(str::to_string);
            let style = Style {
                id: id.clone(),
                name: val("w:name").unwrap_or_else(|| id.clone()),
                kind,
                based_on: val("w:basedOn"),
                next: val("w:next"),
                link: val("w:link"),
                is_default: st.attr("w:default").is_some_and(|v| v == "1" || v == "true" || v == "on"),
                q_format: st.child("w:qFormat").is_some(),
                hidden: st.child("w:semiHidden").is_some() || st.child("w:hidden").is_some(),
                ui_priority: st.child("w:uiPriority").and_then(|e| e.attr("w:val")).and_then(|v| v.parse().ok()).unwrap_or(99),
                ppr: st.child("w:pPr").cloned(),
                rpr: st.child("w:rPr").cloned(),
                tblpr: st.child("w:tblPr").cloned(),
                trpr: st.child("w:trPr").cloned(),
                tcpr: st.child("w:tcPr").cloned(),
                conditional: st
                    .children_named("w:tblStylePr")
                    .map(|c| (c.attr("w:type").unwrap_or_default().to_string(), c.clone()))
                    .collect(),
            };
            if style.is_default {
                match kind {
                    StyleKind::Paragraph => s.default_para = Some(id.clone()),
                    StyleKind::Character => s.default_char = Some(id.clone()),
                    StyleKind::Table => s.default_table = Some(id.clone()),
                    _ => {}
                }
            }
            if !s.map.contains_key(&id) {
                s.order.push(id.clone());
            }
            s.map.insert(id, style);
        }
        s
    }

    pub fn get(&self, id: &str) -> Option<&Style> {
        self.map.get(id)
    }

    /// The basedOn chain from root ancestor to `id` (inclusive), cycle-safe.
    pub fn chain(&self, id: &str) -> Vec<&Style> {
        let mut out = Vec::new();
        let mut cur = self.map.get(id);
        while let Some(s) = cur {
            if out.iter().any(|o: &&Style| o.id == s.id) || out.len() > 32 {
                break;
            }
            out.push(s);
            cur = s.based_on.as_deref().and_then(|b| self.map.get(b));
        }
        out.reverse();
        out
    }

    pub fn find_by_name(&self, name: &str) -> Option<&Style> {
        self.map.values().find(|s| s.name.eq_ignore_ascii_case(name))
    }

    pub fn display_name(&self, id: &str) -> String {
        let n = self.map.get(id).map(|s| s.name.clone()).unwrap_or_else(|| id.to_string());
        // Built-in names are stored lowercase ("heading 1"); they are shown capitalized.
        let mut c = n.chars();
        match c.next() {
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            None => n,
        }
    }
}

// ---------------------------------------------------------------------------
// Numbering

#[derive(Clone, Debug, Default)]
pub struct Level {
    pub start: i32,
    pub num_fmt: String,
    pub lvl_text: String,
    pub jc: String,
    pub suff: String,
    pub ppr: Option<Element>,
    pub rpr: Option<Element>,
    pub is_lgl: bool,
    pub restart: Option<i32>,
    pub pstyle: Option<String>,
}

impl Level {
    fn parse(e: &Element) -> Level {
        let val = |n: &str| e.child(n).and_then(|c| c.attr("w:val")).map(str::to_string);
        Level {
            start: val("w:start").and_then(|v| v.parse().ok()).unwrap_or(0),
            num_fmt: val("w:numFmt").unwrap_or_else(|| "decimal".into()),
            lvl_text: e.child("w:lvlText").and_then(|c| c.attr("w:val")).unwrap_or("").to_string(),
            jc: val("w:lvlJc").unwrap_or_else(|| "left".into()),
            suff: val("w:suff").unwrap_or_else(|| "tab".into()),
            ppr: e.child("w:pPr").cloned(),
            rpr: e.child("w:rPr").cloned(),
            is_lgl: e.child("w:isLgl").is_some_and(on_off),
            restart: val("w:lvlRestart").and_then(|v| v.parse().ok()),
            pstyle: val("w:pStyle"),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct AbstractNum {
    pub levels: Vec<Option<Level>>,
    pub num_style_link: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Num {
    pub abstract_id: String,
    pub start_overrides: HashMap<usize, i32>,
    pub level_overrides: HashMap<usize, Level>,
}

#[derive(Clone, Debug, Default)]
pub struct Numbering {
    pub abstracts: HashMap<String, AbstractNum>,
    pub nums: HashMap<String, Num>,
    pub root: Option<Element>,
}

impl Numbering {
    pub fn parse(root: &Element) -> Numbering {
        let mut n = Numbering { root: Some(root.clone()), ..Default::default() };
        for a in root.children_named("w:abstractNum") {
            let id = a.attr("w:abstractNumId").unwrap_or_default().to_string();
            let mut an = AbstractNum { levels: vec![None; 9], num_style_link: None };
            for l in a.children_named("w:lvl") {
                let ilvl: usize = l.attr("w:ilvl").and_then(|v| v.parse().ok()).unwrap_or(0);
                if ilvl < 9 {
                    an.levels[ilvl] = Some(Level::parse(l));
                }
            }
            an.num_style_link = a.child("w:numStyleLink").and_then(|e| e.attr("w:val")).map(str::to_string);
            n.abstracts.insert(id, an);
        }
        for num in root.children_named("w:num") {
            let id = num.attr("w:numId").unwrap_or_default().to_string();
            let mut nn = Num {
                abstract_id: num.child("w:abstractNumId").and_then(|e| e.attr("w:val")).unwrap_or_default().to_string(),
                ..Default::default()
            };
            for o in num.children_named("w:lvlOverride") {
                let ilvl: usize = o.attr("w:ilvl").and_then(|v| v.parse().ok()).unwrap_or(0);
                if let Some(s) = o.child("w:startOverride").and_then(|e| e.attr("w:val")).and_then(|v| v.parse().ok()) {
                    nn.start_overrides.insert(ilvl, s);
                }
                if let Some(l) = o.child("w:lvl") {
                    nn.level_overrides.insert(ilvl, Level::parse(l));
                }
            }
            n.nums.insert(id, nn);
        }
        n
    }

    /// Effective level definition for (numId, ilvl), following numStyleLink.
    pub fn level(&self, num_id: &str, ilvl: usize, styles: &Styles) -> Option<Level> {
        let num = self.nums.get(num_id)?;
        if let Some(l) = num.level_overrides.get(&ilvl) {
            return Some(l.clone());
        }
        let mut abs = self.abstracts.get(&num.abstract_id)?;
        // numStyleLink → numbering style → its numPr numId → that abstract.
        for _ in 0..4 {
            let Some(link) = &abs.num_style_link else { break };
            let target = styles
                .get(link)
                .and_then(|s| s.ppr.as_ref())
                .and_then(|p| p.child("w:numPr"))
                .and_then(|np| np.child("w:numId"))
                .and_then(|e| e.attr("w:val"))
                .and_then(|id| self.nums.get(id))
                .and_then(|n| self.abstracts.get(&n.abstract_id));
            match target {
                Some(t) if !std::ptr::eq(t, abs) => abs = t,
                _ => break,
            }
        }
        let mut l = abs.levels.get(ilvl)?.clone()?;
        if let Some(s) = num.start_overrides.get(&ilvl) {
            l.start = *s;
        }
        Some(l)
    }

    pub fn counter_key(&self, num_id: &str) -> String {
        match self.nums.get(num_id) {
            Some(n) if n.start_overrides.is_empty() && n.level_overrides.is_empty() => format!("a{}", n.abstract_id),
            _ => format!("n{}", num_id),
        }
    }

    pub fn has_start_override(&self, num_id: &str) -> bool {
        self.nums.get(num_id).is_some_and(|n| !n.start_overrides.is_empty())
    }
}

pub fn format_number(n: i32, fmt: &str) -> String {
    match fmt {
        "decimal" => n.to_string(),
        "decimalZero" => format!("{:02}", n),
        "upperRoman" => roman(n).to_uppercase(),
        "lowerRoman" => roman(n),
        "upperLetter" => letters(n).to_uppercase(),
        "lowerLetter" => letters(n),
        "ordinal" => {
            let suf = match (n % 10, n % 100) {
                (1, x) if x != 11 => "st",
                (2, x) if x != 12 => "nd",
                (3, x) if x != 13 => "rd",
                _ => "th",
            };
            format!("{}{}", n, suf)
        }
        "none" => String::new(),
        "bullet" => String::new(),
        "decimalEnclosedParen" => format!("({})", n),
        "decimalEnclosedCircle" => char::from_u32(0x2460 + (n.clamp(1, 20) - 1) as u32).map(String::from).unwrap_or_default(),
        "greekLower" | "lowerGreek" => greek(n, false),
        "greekUpper" | "upperGreek" => greek(n, true),
        "cardinalText" => cardinal_text(n),
        _ => n.to_string(),
    }
}

fn roman(mut n: i32) -> String {
    if n <= 0 {
        return n.to_string();
    }
    let table = [
        (1000, "m"), (900, "cm"), (500, "d"), (400, "cd"), (100, "c"), (90, "xc"), (50, "l"), (40, "xl"), (10, "x"), (9, "ix"),
        (5, "v"), (4, "iv"), (1, "i"),
    ];
    let mut s = String::new();
    for (v, r) in table {
        while n >= v {
            s.push_str(r);
            n -= v;
        }
    }
    s
}

/// Letter numbering: a..z, aa..zz, aaa.. (repeat, not base-26).
fn letters(n: i32) -> String {
    if n <= 0 {
        return String::new();
    }
    let n = n - 1;
    let c = (b'a' + (n % 26) as u8) as char;
    std::iter::repeat_n(c, (n / 26 + 1) as usize).collect()
}

fn greek(n: i32, upper: bool) -> String {
    let lower = ['α', 'β', 'γ', 'δ', 'ε', 'ζ', 'η', 'θ', 'ι', 'κ', 'λ', 'μ', 'ν', 'ξ', 'ο', 'π', 'ρ', 'σ', 'τ', 'υ', 'φ', 'χ', 'ψ', 'ω'];
    if n <= 0 {
        return String::new();
    }
    let i = ((n - 1) % 24) as usize;
    let c = lower[i];
    let c = if upper { c.to_uppercase().next().unwrap() } else { c };
    std::iter::repeat_n(c, ((n - 1) / 24 + 1) as usize).collect()
}

fn cardinal_text(n: i32) -> String {
    const W: [&str; 20] = [
        "Zero", "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine", "Ten", "Eleven", "Twelve", "Thirteen",
        "Fourteen", "Fifteen", "Sixteen", "Seventeen", "Eighteen", "Nineteen",
    ];
    const T: [&str; 10] = ["", "", "Twenty", "Thirty", "Forty", "Fifty", "Sixty", "Seventy", "Eighty", "Ninety"];
    if (0..20).contains(&n) {
        return W[n as usize].to_string();
    }
    if (20..100).contains(&n) {
        let t = T[(n / 10) as usize];
        return if n % 10 == 0 { t.to_string() } else { format!("{}-{}", t, W[(n % 10) as usize].to_lowercase()) };
    }
    n.to_string()
}

// ---------------------------------------------------------------------------
// Resolved run properties

#[derive(Clone, Debug, PartialEq, Copy, Default)]
pub enum VertAlign {
    #[default]
    Baseline,
    Super,
    Sub,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RunProps {
    pub font_ascii: String,
    pub font_hansi: String,
    pub font_ea: String,
    pub font_cs: String,
    pub hint: Option<String>,
    pub size: f32,
    pub size_cs: f32,
    pub bold: bool,
    pub italic: bool,
    pub bold_cs: bool,
    pub italic_cs: bool,
    pub caps: bool,
    pub small_caps: bool,
    pub strike: bool,
    pub dstrike: bool,
    pub outline: bool,
    pub shadow: bool,
    pub emboss: bool,
    pub imprint: bool,
    pub vanish: bool,
    pub underline: Option<String>,
    pub underline_color: Option<Color>,
    pub color: Option<Color>,
    pub highlight: Option<Color>,
    pub shading: Option<Color>,
    pub vert_align: VertAlign,
    pub spacing: f32,
    pub position: f32,
    pub scale: f32,
    pub kern: Option<f32>,
    pub rtl: bool,
    pub cs: bool,
    pub lang: Option<String>,
    pub style_id: Option<String>,
    /// Word 2010+ text effects (`w14:*`): gradient fill, outline, glow, shadow, reflection, 3D.
    pub fx14: Option<std::sync::Arc<RichFx>>,
}

impl Default for RunProps {
    fn default() -> Self {
        // Built-in defaults when docDefaults are absent.
        RunProps {
            font_ascii: "Times New Roman".into(),
            font_hansi: "Times New Roman".into(),
            font_ea: "Times New Roman".into(),
            font_cs: "Times New Roman".into(),
            hint: None,
            size: 10.0,
            size_cs: 10.0,
            bold: false,
            italic: false,
            bold_cs: false,
            italic_cs: false,
            caps: false,
            small_caps: false,
            strike: false,
            dstrike: false,
            outline: false,
            shadow: false,
            emboss: false,
            imprint: false,
            vanish: false,
            underline: None,
            underline_color: None,
            color: None,
            highlight: None,
            shading: None,
            vert_align: VertAlign::Baseline,
            spacing: 0.0,
            position: 0.0,
            scale: 100.0,
            kern: None,
            rtl: false,
            cs: false,
            lang: None,
            style_id: None,
            fx14: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Word 2010 text effects (w14 namespace) — what WordArt is made of.

/// A DrawingML paint: a solid colour or a linear gradient, with opacity.
#[derive(Clone, Debug, PartialEq)]
pub enum Paint {
    Solid(Color, f32),
    /// Stops are (position 0..1, colour, opacity); `angle` in degrees, 0 = left to right, 90 = top to bottom.
    Linear { stops: Vec<(f32, Color, f32)>, angle: f32 },
}

impl Paint {
    /// Colour and opacity at `t` (0..1) along the gradient.
    pub fn at(&self, t: f32) -> (Color, f32) {
        match self {
            Paint::Solid(c, a) => (*c, *a),
            Paint::Linear { stops, .. } => {
                let Some(first) = stops.first() else { return (Color::BLACK, 1.0) };
                if t <= first.0 {
                    return (first.1, first.2);
                }
                for w in stops.windows(2) {
                    let (a, b) = (&w[0], &w[1]);
                    if t <= b.0 {
                        let k = if b.0 > a.0 { (t - a.0) / (b.0 - a.0) } else { 1.0 };
                        let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * k).round() as u8;
                        return (Color(m(a.1 .0, b.1 .0), m(a.1 .1, b.1 .1), m(a.1 .2, b.1 .2)), a.2 + (b.2 - a.2) * k);
                    }
                }
                let last = stops.last().unwrap();
                (last.1, last.2)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FxShadow {
    pub blur: f32,
    pub dist: f32,
    /// Direction in degrees, clockwise from pointing right (90 = straight down).
    pub dir: f32,
    pub color: Color,
    pub alpha: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FxReflection {
    pub blur: f32,
    pub start_alpha: f32,
    pub end_alpha: f32,
    /// Where the fade ends, as a fraction of the text height.
    pub end_pos: f32,
    pub dist: f32,
}

/// Text effects in points/degrees/0..1 opacities.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct RichFx {
    /// `None` = the run's colour.
    pub fill: Option<Paint>,
    pub no_fill: bool,
    /// Stroke width (pt) and paint.
    pub outline: Option<(f32, Paint)>,
    pub shadow: Option<FxShadow>,
    /// Radius (pt), colour, opacity.
    pub glow: Option<(f32, Color, f32)>,
    pub reflection: Option<FxReflection>,
    /// Top bevel width and height (pt).
    pub bevel: Option<(f32, f32)>,
    /// Extrusion depth (pt) and colour (`None` = a darker shade of the fill).
    pub extrusion: Option<(f32, Option<Color>)>,
    /// `w14:scene3d` camera preset, which decides the direction the extrusion shows.
    pub camera: Option<String>,
}

fn emu_pt(e: &Element, name: &str) -> f32 {
    e.attr_local(name).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0) / 12700.0
}

fn per100k(e: &Element, name: &str, default: f32) -> f32 {
    e.attr_local(name).and_then(|v| v.parse::<f32>().ok()).map(|v| v / 100000.0).unwrap_or(default)
}

/// The first colour child of `e` (srgbClr / schemeClr / prstClr) with its modifiers: (colour, opacity).
fn w14_color(e: &Element, tc: &ThemeCtx) -> Option<(Color, f32)> {
    let c = e.elements().find(|c| matches!(c.local(), "srgbClr" | "schemeClr" | "prstClr"))?;
    let v = c.attr_local("val")?;
    let mut col = match c.local() {
        "srgbClr" => Color::from_hex(v)?,
        "schemeClr" => tc.theme_color(match v {
            "tx1" => "text1",
            "tx2" => "text2",
            "bg1" => "background1",
            "bg2" => "background2",
            "phClr" => "accent1",
            o => o,
        })?,
        _ => match v {
            "white" => Color::WHITE,
            _ => Color::BLACK,
        },
    };
    let (mut lm, mut lo, mut alpha) = (1.0, 0.0, 1.0);
    for m in c.elements() {
        match m.local() {
            "lumMod" => lm = per100k(m, "val", 1.0),
            "lumOff" => lo = per100k(m, "val", 0.0),
            "tint" => col = col.tint(per100k(m, "val", 1.0)),
            "shade" => col = col.shade(per100k(m, "val", 1.0)),
            // Unlike DrawingML's a:alpha (opacity), w14:alpha is the transparency.
            "alpha" => alpha = 1.0 - per100k(m, "val", 0.0),
            _ => {}
        }
    }
    if lm != 1.0 || lo != 0.0 {
        col = col.lum_mod_off(lm, lo);
    }
    Some((col, alpha))
}

/// A fill-like element's paint (`solidFill` / `gradFill` child); `Some(None)` for `noFill`.
fn w14_paint(e: &Element, tc: &ThemeCtx) -> Option<Option<Paint>> {
    for c in e.elements() {
        match c.local() {
            "noFill" => return Some(None),
            "solidFill" => return w14_color(c, tc).map(|(col, a)| Some(Paint::Solid(col, a))),
            "gradFill" => {
                let mut stops: Vec<(f32, Color, f32)> = c
                    .child_local("gsLst")
                    .map(|l| l.elements().filter_map(|gs| w14_color(gs, tc).map(|(col, a)| (per100k(gs, "pos", 0.0), col, a))).collect())
                    .unwrap_or_default();
                stops.sort_by(|a, b| a.0.total_cmp(&b.0));
                let angle = c.child_local("lin").and_then(|l| l.attr_local("ang")).and_then(|v| v.parse::<f32>().ok()).map(|v| v / 60000.0).unwrap_or(90.0);
                return (!stops.is_empty()).then_some(Some(Paint::Linear { stops, angle }));
            }
            _ => {}
        }
    }
    None
}

/// Apply one `w14:*` rPr child to the run's effects.
fn apply_w14(rp: &mut RunProps, c: &Element, tc: &ThemeCtx) {
    let mut fx = rp.fx14.as_deref().cloned().unwrap_or_default();
    match c.local() {
        "textFill" => match w14_paint(c, tc) {
            Some(Some(p)) => {
                fx.fill = Some(p);
                fx.no_fill = false;
            }
            Some(None) => fx.no_fill = true,
            None => {}
        },
        "textOutline" => {
            fx.outline = match w14_paint(c, tc) {
                Some(Some(p)) => Some((emu_pt(c, "w").max(0.25), p)),
                _ => None,
            }
        }
        "shadow" => {
            fx.shadow = w14_color(c, tc).map(|(color, alpha)| FxShadow {
                blur: emu_pt(c, "blurRad"),
                dist: emu_pt(c, "dist"),
                dir: c.attr_local("dir").and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0) / 60000.0,
                color,
                alpha,
            })
        }
        "glow" => fx.glow = w14_color(c, tc).map(|(col, a)| (emu_pt(c, "rad"), col, a)),
        "reflection" => {
            fx.reflection = Some(FxReflection {
                blur: emu_pt(c, "blurRad"),
                start_alpha: per100k(c, "stA", 0.5),
                end_alpha: per100k(c, "endA", 0.0),
                end_pos: per100k(c, "endPos", 0.9).max(0.01),
                dist: emu_pt(c, "dist"),
            })
        }
        "props3d" => {
            let depth = emu_pt(c, "extrusionH");
            let ecol = c.child_local("extrusionClr").and_then(|e| w14_color(e, tc)).map(|(col, _)| col);
            fx.extrusion = (depth > 0.0).then_some((depth, ecol));
            fx.bevel = c.child_local("bevelT").map(|b| {
                let w = b.attr_local("w").map(|_| emu_pt(b, "w")).unwrap_or(6.0);
                let h = b.attr_local("h").map(|_| emu_pt(b, "h")).unwrap_or(6.0);
                (w, h)
            });
        }
        "scene3d" => fx.camera = c.child_local("camera").and_then(|cam| cam.attr_local("prst")).map(str::to_string),
        _ => return,
    }
    rp.fx14 = Some(std::sync::Arc::new(fx));
}

#[derive(Clone, Debug, Default, Copy)]
pub struct Toggles {
    pub b: Option<bool>,
    pub b_cs: Option<bool>,
    pub i: Option<bool>,
    pub i_cs: Option<bool>,
    pub caps: Option<bool>,
    pub small_caps: Option<bool>,
    pub strike: Option<bool>,
    pub dstrike: Option<bool>,
    pub outline: Option<bool>,
    pub shadow: Option<bool>,
    pub emboss: Option<bool>,
    pub imprint: Option<bool>,
    pub vanish: Option<bool>,
}

impl Toggles {
    fn fields(&mut self) -> [&mut Option<bool>; 13] {
        [
            &mut self.b, &mut self.b_cs, &mut self.i, &mut self.i_cs, &mut self.caps, &mut self.small_caps, &mut self.strike,
            &mut self.dstrike, &mut self.outline, &mut self.shadow, &mut self.emboss, &mut self.imprint, &mut self.vanish,
        ]
    }
    fn xor_into(&mut self, other: &Toggles) {
        let mut o = *other;
        for (a, b) in self.fields().into_iter().zip(o.fields()) {
            if let Some(bv) = *b {
                *a = Some(a.unwrap_or(false) ^ bv);
            }
        }
    }
    fn override_with(&mut self, other: &Toggles) {
        let mut o = *other;
        for (a, b) in self.fields().into_iter().zip(o.fields()) {
            if b.is_some() {
                *a = *b;
            }
        }
    }
    fn write(&self, rp: &mut RunProps) {
        let set = |dst: &mut bool, v: Option<bool>| {
            if let Some(v) = v {
                *dst = v
            }
        };
        set(&mut rp.bold, self.b);
        set(&mut rp.bold_cs, self.b_cs);
        set(&mut rp.italic, self.i);
        set(&mut rp.italic_cs, self.i_cs);
        set(&mut rp.caps, self.caps);
        set(&mut rp.small_caps, self.small_caps);
        set(&mut rp.strike, self.strike);
        set(&mut rp.dstrike, self.dstrike);
        set(&mut rp.outline, self.outline);
        set(&mut rp.shadow, self.shadow);
        set(&mut rp.emboss, self.emboss);
        set(&mut rp.imprint, self.imprint);
        set(&mut rp.vanish, self.vanish);
    }
}

/// Context needed to resolve theme references.
pub struct ThemeCtx<'a> {
    pub theme: &'a Theme,
    pub settings: &'a Settings,
}

impl ThemeCtx<'_> {
    pub fn theme_color(&self, name: &str) -> Option<Color> {
        // Map text/background aliases through the document's clrSchemeMapping.
        let mapped = match name {
            "text1" | "dark1" => self.settings.clr_map.get("t1").map(|s| s.as_str()).unwrap_or("dk1"),
            "text2" | "dark2" => self.settings.clr_map.get("t2").map(|s| s.as_str()).unwrap_or("dk2"),
            "background1" | "light1" => self.settings.clr_map.get("bg1").map(|s| s.as_str()).unwrap_or("lt1"),
            "background2" | "light2" => self.settings.clr_map.get("bg2").map(|s| s.as_str()).unwrap_or("lt2"),
            "hyperlink" => "hlink",
            "followedHyperlink" => "folHlink",
            other => other,
        };
        let mapped = match mapped {
            "dark1" => "dk1",
            "dark2" => "dk2",
            "light1" => "lt1",
            "light2" => "lt2",
            m => m,
        };
        self.theme.colors.get(mapped).copied().or_else(|| match mapped {
            "dk1" => Some(Color::BLACK),
            "lt1" => Some(Color::WHITE),
            _ => None,
        })
    }

    /// Resolve a color from an element with w:val / w:themeColor / w:themeTint / w:themeShade
    /// (or w:fill / w:themeFill... when `prefix` is "Fill").
    pub fn color_of(&self, e: &Element, val_attr: &str, theme_attr: &str, tint_attr: &str, shade_attr: &str) -> Option<Option<Color>> {
        if let Some(tc) = e.attr(theme_attr) {
            if let Some(mut c) = self.theme_color(tc) {
                if let Some(t) = e.attr(tint_attr).and_then(|v| u8::from_str_radix(v, 16).ok()) {
                    c = c.tint(t as f32 / 255.0);
                }
                if let Some(s) = e.attr(shade_attr).and_then(|v| u8::from_str_radix(v, 16).ok()) {
                    c = c.shade(s as f32 / 255.0);
                }
                return Some(Some(c));
            }
        }
        match e.attr(val_attr) {
            Some("auto") => Some(None),
            Some(v) => Color::from_hex(v).map(Some),
            None => None,
        }
    }

    pub fn shading(&self, e: &Element) -> Option<Color> {
        let fill = self.color_of(e, "w:fill", "w:themeFill", "w:themeFillTint", "w:themeFillShade").flatten();
        let val = e.attr("w:val").unwrap_or("clear");
        let pattern = self.color_of(e, "w:color", "w:themeColor", "w:themeTint", "w:themeShade").flatten();
        match val {
            "clear" | "nil" => fill,
            "solid" => Some(pattern.unwrap_or(Color::BLACK)),
            v if v.starts_with("pct") => {
                let pct: f32 = v[3..].parse().unwrap_or(0.0) / 100.0;
                let base = fill.unwrap_or(Color::WHITE);
                Some(base.blend(pattern.unwrap_or(Color::BLACK), pct))
            }
            _ => fill,
        }
    }
}

fn apply_rpr(rp: &mut RunProps, tg: &mut Toggles, e: &Element, tc: &ThemeCtx) {
    for c in e.elements() {
        if c.name.starts_with("w14:") {
            apply_w14(rp, c, tc);
            continue;
        }
        match c.name.as_str() {
            "w:rFonts" => {
                let theme = |a: &str| c.attr(a).and_then(|t| tc.theme.font(t));
                if let Some(f) = theme("w:asciiTheme").or_else(|| c.attr("w:ascii").map(str::to_string)) {
                    rp.font_ascii = f;
                }
                if let Some(f) = theme("w:hAnsiTheme").or_else(|| c.attr("w:hAnsi").map(str::to_string)) {
                    rp.font_hansi = f;
                }
                if let Some(f) = theme("w:eastAsiaTheme").or_else(|| c.attr("w:eastAsia").map(str::to_string)) {
                    rp.font_ea = f;
                }
                if let Some(f) = theme("w:cstheme").or_else(|| c.attr("w:cs").map(str::to_string)) {
                    rp.font_cs = f;
                }
                if let Some(h) = c.attr("w:hint") {
                    rp.hint = Some(h.to_string());
                }
            }
            "w:b" => tg.b = Some(on_off(c)),
            "w:bCs" => tg.b_cs = Some(on_off(c)),
            "w:i" => tg.i = Some(on_off(c)),
            "w:iCs" => tg.i_cs = Some(on_off(c)),
            "w:caps" => tg.caps = Some(on_off(c)),
            "w:smallCaps" => tg.small_caps = Some(on_off(c)),
            "w:strike" => tg.strike = Some(on_off(c)),
            "w:dstrike" => tg.dstrike = Some(on_off(c)),
            "w:outline" => tg.outline = Some(on_off(c)),
            "w:shadow" => tg.shadow = Some(on_off(c)),
            "w:emboss" => tg.emboss = Some(on_off(c)),
            "w:imprint" => tg.imprint = Some(on_off(c)),
            "w:vanish" => tg.vanish = Some(on_off(c)),
            "w:color" => {
                if let Some(col) = tc.color_of(c, "w:val", "w:themeColor", "w:themeTint", "w:themeShade") {
                    rp.color = col;
                }
            }
            "w:sz" => {
                if let Some(v) = c.attr("w:val").and_then(|v| v.parse::<f32>().ok()) {
                    rp.size = v / 2.0;
                }
            }
            "w:szCs" => {
                if let Some(v) = c.attr("w:val").and_then(|v| v.parse::<f32>().ok()) {
                    rp.size_cs = v / 2.0;
                }
            }
            "w:u" => {
                let v = c.attr("w:val").unwrap_or("single");
                rp.underline = if v == "none" { None } else { Some(v.to_string()) };
                rp.underline_color = tc.color_of(c, "w:color", "w:themeColor", "w:themeTint", "w:themeShade").flatten();
            }
            "w:highlight" => rp.highlight = c.attr("w:val").and_then(highlight_color),
            "w:shd" => rp.shading = tc.shading(c),
            "w:vertAlign" => {
                rp.vert_align = match c.attr("w:val") {
                    Some("superscript") => VertAlign::Super,
                    Some("subscript") => VertAlign::Sub,
                    _ => VertAlign::Baseline,
                }
            }
            "w:spacing" => rp.spacing = pt_attr(c, "w:val").unwrap_or(0.0),
            "w:position" => rp.position = c.attr("w:val").and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0) / 2.0,
            "w:w" => rp.scale = c.attr("w:val").and_then(|v| v.trim_end_matches('%').parse::<f32>().ok()).unwrap_or(100.0),
            "w:kern" => rp.kern = c.attr("w:val").and_then(|v| v.parse::<f32>().ok()).map(|v| v / 2.0),
            "w:rtl" => rp.rtl = on_off(c),
            "w:cs" => rp.cs = on_off(c),
            "w:lang" => rp.lang = c.attr("w:val").map(str::to_string),
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Resolved paragraph properties

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
    Justify,
    Distribute,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineRule {
    Auto(f32),
    Exact(f32),
    AtLeast(f32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct TabStop {
    pub pos: f32,
    pub kind: String,
    pub leader: String,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Border {
    pub style: String,
    pub width: f32, // pt
    pub space: f32, // pt
    pub color: Option<Color>,
}

impl Border {
    pub fn is_visible(&self) -> bool {
        !matches!(self.style.as_str(), "" | "nil" | "none") && self.width > 0.0
    }
    pub fn parse(e: &Element, tc: &ThemeCtx) -> Border {
        let style = e.attr("w:val").unwrap_or("none").to_string();
        let sz = e.attr("w:sz").and_then(|v| v.parse::<f32>().ok()).unwrap_or(4.0) / 8.0;
        let mut width = sz.max(0.25);
        if style == "double" {
            width = width * 3.0;
        }
        Border {
            style,
            width,
            space: e.attr("w:space").and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0),
            color: tc.color_of(e, "w:color", "w:themeColor", "w:themeTint", "w:themeShade").flatten(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParaProps {
    pub style_id: String,
    pub align: Align,
    pub ind_left: f32,
    pub ind_right: f32,
    pub ind_first: f32, // positive = first line indent, negative = hanging
    pub space_before: f32,
    pub space_after: f32,
    pub before_auto: bool,
    pub after_auto: bool,
    pub line: LineRule,
    pub contextual: bool,
    pub keep_next: bool,
    pub keep_lines: bool,
    pub page_break_before: bool,
    pub widow_control: bool,
    pub tabs: Vec<TabStop>,
    pub num: Option<(String, usize)>,
    pub shading: Option<Color>,
    pub border_top: Option<Border>,
    pub border_bottom: Option<Border>,
    pub border_left: Option<Border>,
    pub border_right: Option<Border>,
    pub border_between: Option<Border>,
    pub outline_lvl: Option<u8>,
    pub bidi: bool,
    pub snap_to_grid: bool,
    pub suppress_auto_hyphens: bool,
    pub frame: bool,
}

impl Default for ParaProps {
    fn default() -> Self {
        ParaProps {
            style_id: String::new(),
            align: Align::Left,
            ind_left: 0.0,
            ind_right: 0.0,
            ind_first: 0.0,
            space_before: 0.0,
            space_after: 0.0,
            before_auto: false,
            after_auto: false,
            line: LineRule::Auto(1.0),
            contextual: false,
            keep_next: false,
            keep_lines: false,
            page_break_before: false,
            widow_control: true,
            tabs: Vec::new(),
            num: None,
            shading: None,
            border_top: None,
            border_bottom: None,
            border_left: None,
            border_right: None,
            border_between: None,
            outline_lvl: None,
            bidi: false,
            snap_to_grid: true,
            suppress_auto_hyphens: false,
            frame: false,
        }
    }
}

fn apply_ppr(pp: &mut ParaProps, e: &Element, tc: &ThemeCtx) {
    for c in e.elements() {
        match c.name.as_str() {
            "w:jc" => {
                pp.align = match c.attr("w:val").unwrap_or("left") {
                    "center" => Align::Center,
                    "right" | "end" => Align::Right,
                    "both" | "lowKashida" | "mediumKashida" | "highKashida" | "thaiDistribute" => Align::Justify,
                    "distribute" => Align::Distribute,
                    _ => Align::Left,
                }
            }
            "w:ind" => {
                if let Some(v) = pt_attr(c, "w:left").or_else(|| pt_attr(c, "w:start")) {
                    pp.ind_left = v;
                }
                if let Some(v) = pt_attr(c, "w:right").or_else(|| pt_attr(c, "w:end")) {
                    pp.ind_right = v;
                }
                if let Some(v) = pt_attr(c, "w:hanging") {
                    pp.ind_first = -v;
                } else if let Some(v) = pt_attr(c, "w:firstLine") {
                    pp.ind_first = v;
                }
            }
            "w:spacing" => {
                if let Some(v) = pt_attr(c, "w:before") {
                    pp.space_before = v;
                }
                if let Some(v) = pt_attr(c, "w:after") {
                    pp.space_after = v;
                }
                if let Some(v) = c.attr("w:beforeLines").and_then(|v| v.parse::<f32>().ok()) {
                    pp.space_before = v / 100.0 * 12.0;
                }
                if let Some(v) = c.attr("w:afterLines").and_then(|v| v.parse::<f32>().ok()) {
                    pp.space_after = v / 100.0 * 12.0;
                }
                if let Some(a) = c.attr("w:beforeAutospacing") {
                    pp.before_auto = !matches!(a, "0" | "false" | "off");
                }
                if let Some(a) = c.attr("w:afterAutospacing") {
                    pp.after_auto = !matches!(a, "0" | "false" | "off");
                }
                if let Some(line) = tw_attr(c, "w:line") {
                    pp.line = match c.attr("w:lineRule").unwrap_or("auto") {
                        "exact" => LineRule::Exact(line.abs() / 20.0),
                        "atLeast" => LineRule::AtLeast(line / 20.0),
                        _ => LineRule::Auto(line / 240.0),
                    };
                }
            }
            "w:contextualSpacing" => pp.contextual = on_off(c),
            "w:keepNext" => pp.keep_next = on_off(c),
            "w:keepLines" => pp.keep_lines = on_off(c),
            "w:pageBreakBefore" => pp.page_break_before = on_off(c),
            "w:widowControl" => pp.widow_control = on_off(c),
            "w:bidi" => pp.bidi = on_off(c),
            "w:snapToGrid" => pp.snap_to_grid = on_off(c),
            "w:suppressAutoHyphens" => pp.suppress_auto_hyphens = on_off(c),
            "w:framePr" => pp.frame = true,
            "w:outlineLvl" => pp.outline_lvl = c.attr("w:val").and_then(|v| v.parse().ok()),
            "w:numPr" => {
                let id = c.child("w:numId").and_then(|e| e.attr("w:val")).map(str::to_string);
                let lvl = c.child("w:ilvl").and_then(|e| e.attr("w:val")).and_then(|v| v.parse().ok());
                match (id, lvl) {
                    (Some(id), lvl) => {
                        if id == "0" {
                            pp.num = None;
                        } else {
                            pp.num = Some((id, lvl.or(pp.num.as_ref().map(|n| n.1)).unwrap_or(0)));
                        }
                    }
                    (None, Some(lvl)) => {
                        if let Some(n) = &mut pp.num {
                            n.1 = lvl;
                        }
                    }
                    _ => {}
                }
            }
            "w:tabs" => {
                for t in c.children_named("w:tab") {
                    let pos = pt_attr(t, "w:pos").unwrap_or(0.0);
                    let kind = t.attr("w:val").unwrap_or("left").to_string();
                    pp.tabs.retain(|x| (x.pos - pos).abs() > 0.05);
                    if kind != "clear" {
                        pp.tabs.push(TabStop { pos, kind, leader: t.attr("w:leader").unwrap_or("none").to_string() });
                    }
                }
                pp.tabs.sort_by(|a, b| a.pos.total_cmp(&b.pos));
            }
            "w:shd" => pp.shading = tc.shading(c),
            "w:pBdr" => {
                for b in c.elements() {
                    let bd = Some(Border::parse(b, tc));
                    match b.name.as_str() {
                        "w:top" => pp.border_top = bd,
                        "w:bottom" => pp.border_bottom = bd,
                        "w:left" | "w:start" => pp.border_left = bd,
                        "w:right" | "w:end" => pp.border_right = bd,
                        "w:between" => pp.border_between = bd,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Table style conditional formatting context

#[derive(Clone, Debug, Default)]
pub struct CellCond {
    /// Conditional formatting types that apply to the cell, in increasing priority order.
    pub types: Vec<&'static str>,
}

impl CellCond {
    pub fn compute(look: &TblLook, row: usize, nrows: usize, col: usize, ncols: usize, header_rows: usize, row_band: usize, col_band: usize) -> CellCond {
        let mut t: Vec<&'static str> = vec!["wholeTable"];
        let first_row = look.first_row && row < header_rows.max(1);
        let last_row = look.last_row && row + 1 == nrows;
        let first_col = look.first_col && col == 0;
        let last_col = look.last_col && col + 1 == ncols;
        if !look.no_vband {
            let c = if look.first_col { col.saturating_sub(1) } else { col };
            if !(first_col || last_col) {
                t.push(if (c / col_band.max(1)) % 2 == 0 { "band1Vert" } else { "band2Vert" });
            }
        }
        if !look.no_hband {
            let r = if look.first_row { row.saturating_sub(header_rows.max(1)) } else { row };
            if !(first_row || last_row) {
                t.push(if (r / row_band.max(1)) % 2 == 0 { "band1Horz" } else { "band2Horz" });
            }
        }
        if first_col {
            t.push("firstCol");
        }
        if last_col {
            t.push("lastCol");
        }
        if first_row {
            t.push("firstRow");
        }
        if last_row {
            t.push("lastRow");
        }
        if first_row && first_col {
            t.push("nwCell");
        }
        if first_row && last_col {
            t.push("neCell");
        }
        if last_row && first_col {
            t.push("swCell");
        }
        if last_row && last_col {
            t.push("seCell");
        }
        CellCond { types: t }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TblLook {
    pub first_row: bool,
    pub last_row: bool,
    pub first_col: bool,
    pub last_col: bool,
    pub no_hband: bool,
    pub no_vband: bool,
}

impl Default for TblLook {
    fn default() -> Self {
        // Default look: 04A0
        TblLook { first_row: true, last_row: false, first_col: true, last_col: false, no_hband: false, no_vband: true }
    }
}

impl TblLook {
    pub fn parse(e: &Element) -> TblLook {
        let mut l = TblLook::default();
        if let Some(v) = e.attr("w:val").and_then(|v| u32::from_str_radix(v, 16).ok()) {
            l.first_row = v & 0x0020 != 0;
            l.last_row = v & 0x0040 != 0;
            l.first_col = v & 0x0080 != 0;
            l.last_col = v & 0x0100 != 0;
            l.no_hband = v & 0x0200 != 0;
            l.no_vband = v & 0x0400 != 0;
        }
        let b = |n: &str| e.attr(n).map(|v| v == "1" || v == "true" || v == "on");
        if let Some(x) = b("w:firstRow") {
            l.first_row = x
        }
        if let Some(x) = b("w:lastRow") {
            l.last_row = x
        }
        if let Some(x) = b("w:firstColumn") {
            l.first_col = x
        }
        if let Some(x) = b("w:lastColumn") {
            l.last_col = x
        }
        if let Some(x) = b("w:noHBand") {
            l.no_hband = x
        }
        if let Some(x) = b("w:noVBand") {
            l.no_vband = x
        }
        l
    }
}

/// Table style + conditional context for content inside a cell.
#[derive(Clone, Debug, Default)]
pub struct TableCtx {
    pub style_id: Option<String>,
    pub cond: CellCond,
}

// ---------------------------------------------------------------------------
// The resolver: everything needed to compute effective formatting.

pub struct Resolver<'a> {
    pub styles: &'a Styles,
    pub numbering: &'a Numbering,
    pub tc: ThemeCtx<'a>,
}

impl<'a> Resolver<'a> {
    fn table_style_layers(&self, tctx: Option<&TableCtx>) -> Vec<&'a Element> {
        // Returns the conditional elements (tblStylePr) in priority order for a cell.
        let mut out = Vec::new();
        let Some(tctx) = tctx else { return out };
        let Some(id) = &tctx.style_id else { return out };
        for st in self.styles.chain(id) {
            for t in &tctx.cond.types {
                if let Some((_, e)) = st.conditional.iter().find(|(k, _)| k == t) {
                    out.push(e);
                }
            }
        }
        out
    }

    pub fn para(&self, ppr: Option<&Element>, tctx: Option<&TableCtx>) -> ParaProps {
        let mut pp = ParaProps::default();
        if let Some(d) = &self.styles.default_ppr {
            apply_ppr(&mut pp, d, &self.tc);
        }
        // Table style paragraph properties (whole table + conditional).
        if let Some(tctx) = tctx {
            if let Some(id) = &tctx.style_id {
                for st in self.styles.chain(id) {
                    if let Some(p) = &st.ppr {
                        apply_ppr(&mut pp, p, &self.tc);
                    }
                }
            }
            for e in self.table_style_layers(Some(tctx)) {
                if let Some(p) = e.child("w:pPr") {
                    apply_ppr(&mut pp, p, &self.tc);
                }
            }
        }
        let style_id = ppr
            .and_then(|p| p.child("w:pStyle"))
            .and_then(|s| s.attr("w:val"))
            .map(str::to_string)
            .filter(|s| self.styles.get(s).is_some())
            .or_else(|| self.styles.default_para.clone())
            .unwrap_or_default();
        // Table-style paragraph formatting is overridden by a paragraph style only when
        // that style isn't the document default (standard behaviour for "Normal").
        for st in self.styles.chain(&style_id) {
            if let Some(p) = &st.ppr {
                apply_ppr(&mut pp, p, &self.tc);
            }
        }
        pp.style_id = style_id;
        // Numbering level indentation (from direct or style numPr).
        let direct_num = ppr.and_then(|p| p.child("w:numPr"));
        if let Some(np) = direct_num {
            let mut tmp = ParaProps { num: pp.num.clone(), ..Default::default() };
            apply_ppr(&mut tmp, &Element::new("w:pPr").with_child(np.clone()), &self.tc);
            pp.num = tmp.num;
        }
        if let Some((id, lvl)) = pp.num.clone() {
            if let Some(level) = self.numbering.level(&id, lvl, self.styles) {
                if let Some(lp) = &level.ppr {
                    apply_ppr(&mut pp, lp, &self.tc);
                }
            }
        }
        if let Some(p) = ppr {
            // Direct numPr already merged; apply everything else (ind wins over numbering).
            apply_ppr(&mut pp, p, &self.tc);
            if let Some(np) = direct_num {
                // Re-apply so that a direct numId of 0 removes numbering.
                let mut tmp = ParaProps { num: pp.num.clone(), ..Default::default() };
                apply_ppr(&mut tmp, &Element::new("w:pPr").with_child(np.clone()), &self.tc);
                pp.num = tmp.num;
            }
        }
        if pp.before_auto {
            pp.space_before = 14.0;
        }
        if pp.after_auto {
            pp.space_after = 14.0;
        }
        pp
    }

    /// Run properties for a run in a paragraph with style `pstyle`.
    pub fn run(&self, pstyle: &str, rpr: Option<&Element>, tctx: Option<&TableCtx>) -> RunProps {
        let mut rp = RunProps::default();
        let mut base = Toggles::default();
        if let Some(d) = &self.styles.default_rpr {
            apply_rpr(&mut rp, &mut base, d, &self.tc);
        }
        let mut acc = Toggles::default();
        // Table style layer.
        if let Some(tctx) = tctx {
            let mut tl = Toggles::default();
            if let Some(id) = &tctx.style_id {
                for st in self.styles.chain(id) {
                    if let Some(r) = &st.rpr {
                        apply_rpr(&mut rp, &mut tl, r, &self.tc);
                    }
                }
            }
            for e in self.table_style_layers(Some(tctx)) {
                if let Some(r) = e.child("w:rPr") {
                    apply_rpr(&mut rp, &mut tl, r, &self.tc);
                }
            }
            acc.xor_into(&tl);
        }
        // Paragraph style layer.
        let mut pl = Toggles::default();
        for st in self.styles.chain(pstyle) {
            if let Some(r) = &st.rpr {
                apply_rpr(&mut rp, &mut pl, r, &self.tc);
            }
        }
        acc.xor_into(&pl);
        // Character style layer.
        let rstyle = rpr.and_then(|r| r.child("w:rStyle")).and_then(|s| s.attr("w:val"));
        let rstyle = rstyle.or(self.styles.default_char.as_deref());
        if let Some(rs) = rstyle {
            let mut cl = Toggles::default();
            for st in self.styles.chain(rs) {
                if let Some(r) = &st.rpr {
                    apply_rpr(&mut rp, &mut cl, r, &self.tc);
                }
            }
            acc.xor_into(&cl);
            rp.style_id = Some(rs.to_string());
        }
        base.override_with(&acc);
        // Direct formatting: absolute.
        if let Some(r) = rpr {
            let mut dl = Toggles::default();
            apply_rpr(&mut rp, &mut dl, r, &self.tc);
            base.override_with(&dl);
        }
        base.write(&mut rp);
        rp
    }

    /// Run properties of the paragraph mark (used for empty paragraphs and list labels).
    pub fn mark_run(&self, pstyle: &str, ppr: Option<&Element>, tctx: Option<&TableCtx>) -> RunProps {
        let rpr = ppr.and_then(|p| p.child("w:rPr"));
        self.run(pstyle, rpr, tctx)
    }

    pub fn table_style_tblpr(&self, style_id: Option<&str>) -> Vec<&'a Element> {
        let mut v = Vec::new();
        let id = style_id.map(str::to_string).or_else(|| self.styles.default_table.clone());
        if let Some(id) = id {
            for st in self.styles.chain(&id) {
                if let Some(p) = &st.tblpr {
                    v.push(p);
                }
            }
        }
        v
    }

    pub fn table_style_tcpr(&self, tctx: &TableCtx) -> Vec<&'a Element> {
        let mut v = Vec::new();
        if let Some(id) = &tctx.style_id {
            for st in self.styles.chain(id) {
                if let Some(p) = &st.tcpr {
                    v.push(p);
                }
            }
        }
        for e in self.table_style_layers(Some(tctx)) {
            if let Some(p) = e.child("w:tcPr") {
                v.push(p);
            }
        }
        v
    }

    /// Conditional tblPr (borders) from table style layers.
    pub fn table_style_cond_tblpr(&self, tctx: &TableCtx) -> Vec<&'a Element> {
        self.table_style_layers(Some(tctx)).into_iter().filter_map(|e| e.child("w:tblPr")).collect()
    }

    /// Text of the list label for a numbered paragraph, updating counters.
    pub fn list_label(&self, num_id: &str, ilvl: usize, counters: &mut ListCounters) -> Option<(String, Level)> {
        let level = self.numbering.level(num_id, ilvl, self.styles)?;
        let key = self.numbering.counter_key(num_id);
        let entry = counters.map.entry(key.clone()).or_insert_with(|| [None; 9]);
        // Collect starts for all levels lazily.
        let start_of = |l: usize| self.numbering.level(num_id, l, self.styles).map(|x| x.start).unwrap_or(1);
        let cur = entry[ilvl].map(|v| v + 1).unwrap_or_else(|| start_of(ilvl));
        entry[ilvl] = Some(cur);
        // Reset deeper levels (respecting lvlRestart).
        for l in ilvl + 1..9 {
            let restart = self.numbering.level(num_id, l, self.styles).and_then(|x| x.restart);
            match restart {
                Some(0) => {}
                Some(r) if (r as usize) <= ilvl => entry[l] = None,
                Some(_) => {}
                None => entry[l] = None,
            }
        }
        let vals = *entry;
        if level.num_fmt == "bullet" {
            return Some((level.lvl_text.clone(), level));
        }
        let mut out = String::new();
        let mut chars = level.lvl_text.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '%' {
                if let Some(d) = chars.peek().and_then(|d| d.to_digit(10)) {
                    chars.next();
                    let l = (d as usize).saturating_sub(1);
                    let v = vals.get(l).copied().flatten().unwrap_or_else(|| start_of(l));
                    let fmt = if level.is_lgl && l < ilvl {
                        "decimal".to_string()
                    } else {
                        self.numbering.level(num_id, l, self.styles).map(|x| x.num_fmt).unwrap_or_else(|| "decimal".into())
                    };
                    let fmt = if level.is_lgl { "decimal".to_string() } else { fmt };
                    out.push_str(&format_number(v, &fmt));
                    continue;
                }
            }
            out.push(c);
        }
        Some((out, level))
    }
}

#[derive(Clone, Debug, Default)]
pub struct ListCounters {
    pub map: HashMap<String, [Option<i32>; 9]>,
}

// ---------------------------------------------------------------------------
// Section properties

#[derive(Clone, Debug, PartialEq)]
pub struct SectionProps {
    pub page_w: f32,
    pub page_h: f32,
    pub landscape: bool,
    pub margin_top: f32,
    pub margin_bottom: f32,
    pub margin_left: f32,
    pub margin_right: f32,
    pub margin_header: f32,
    pub margin_footer: f32,
    pub gutter: f32,
    pub top_fixed: bool,
    pub bottom_fixed: bool,
    pub cols: usize,
    pub col_space: f32,
    pub col_widths: Vec<(f32, f32)>,
    pub col_sep: bool,
    pub kind: String,
    pub title_pg: bool,
    pub headers: [Option<String>; 3], // default, first, even (relationship ids)
    pub footers: [Option<String>; 3],
    pub pg_num_start: Option<i32>,
    pub pg_num_fmt: String,
    pub v_align: String,
    pub grid_type: String,
    pub grid_pitch: f32,
    pub page_borders: Vec<(String, Border)>,
}

impl Default for SectionProps {
    fn default() -> Self {
        SectionProps {
            page_w: 612.0,
            page_h: 792.0,
            landscape: false,
            margin_top: 72.0,
            margin_bottom: 72.0,
            margin_left: 72.0,
            margin_right: 72.0,
            margin_header: 36.0,
            margin_footer: 36.0,
            gutter: 0.0,
            top_fixed: false,
            bottom_fixed: false,
            cols: 1,
            col_space: 36.0,
            col_widths: Vec::new(),
            col_sep: false,
            kind: "nextPage".into(),
            title_pg: false,
            headers: [None, None, None],
            footers: [None, None, None],
            pg_num_start: None,
            pg_num_fmt: "decimal".into(),
            v_align: "top".into(),
            grid_type: "default".into(),
            grid_pitch: 0.0,
            page_borders: Vec::new(),
        }
    }
}

impl SectionProps {
    pub fn parse(e: &Element, tc: &ThemeCtx) -> SectionProps {
        let mut s = SectionProps::default();
        for c in e.elements() {
            match c.name.as_str() {
                "w:pgSz" => {
                    if let Some(w) = pt_attr(c, "w:w") {
                        s.page_w = w;
                    }
                    if let Some(h) = pt_attr(c, "w:h") {
                        s.page_h = h;
                    }
                    s.landscape = c.attr("w:orient") == Some("landscape");
                }
                "w:pgMar" => {
                    if let Some(v) = pt_attr(c, "w:top") {
                        s.top_fixed = v < 0.0;
                        s.margin_top = v.abs();
                    }
                    if let Some(v) = pt_attr(c, "w:bottom") {
                        s.bottom_fixed = v < 0.0;
                        s.margin_bottom = v.abs();
                    }
                    if let Some(v) = pt_attr(c, "w:left").or_else(|| pt_attr(c, "w:start")) {
                        s.margin_left = v;
                    }
                    if let Some(v) = pt_attr(c, "w:right").or_else(|| pt_attr(c, "w:end")) {
                        s.margin_right = v;
                    }
                    if let Some(v) = pt_attr(c, "w:header") {
                        s.margin_header = v;
                    }
                    if let Some(v) = pt_attr(c, "w:footer") {
                        s.margin_footer = v;
                    }
                    if let Some(v) = pt_attr(c, "w:gutter") {
                        s.gutter = v;
                    }
                }
                "w:cols" => {
                    s.cols = c.attr("w:num").and_then(|v| v.parse().ok()).unwrap_or(1).max(1);
                    s.col_space = pt_attr(c, "w:space").unwrap_or(36.0);
                    s.col_sep = c.attr("w:sep").is_some_and(|v| v == "1" || v == "true" || v == "on");
                    let equal = c.attr("w:equalWidth").is_none_or(|v| v == "1" || v == "true" || v == "on");
                    if !equal {
                        s.col_widths = c.children_named("w:col").map(|col| (pt_attr(col, "w:w").unwrap_or(0.0), pt_attr(col, "w:space").unwrap_or(0.0))).collect();
                        if !s.col_widths.is_empty() {
                            s.cols = s.col_widths.len();
                        }
                    }
                }
                "w:type" => s.kind = c.attr("w:val").unwrap_or("nextPage").to_string(),
                "w:titlePg" => s.title_pg = on_off(c),
                "w:headerReference" | "w:footerReference" => {
                    let idx = match c.attr("w:type").unwrap_or("default") {
                        "first" => 1,
                        "even" => 2,
                        _ => 0,
                    };
                    let id = c.attr("r:id").or_else(|| c.attr_local("id")).map(str::to_string);
                    if c.name == "w:headerReference" {
                        s.headers[idx] = id;
                    } else {
                        s.footers[idx] = id;
                    }
                }
                "w:pgNumType" => {
                    s.pg_num_start = c.attr("w:start").and_then(|v| v.parse().ok());
                    if let Some(f) = c.attr("w:fmt") {
                        s.pg_num_fmt = f.to_string();
                    }
                }
                "w:vAlign" => s.v_align = c.attr("w:val").unwrap_or("top").to_string(),
                "w:docGrid" => {
                    s.grid_type = c.attr("w:type").unwrap_or("default").to_string();
                    s.grid_pitch = pt_attr(c, "w:linePitch").unwrap_or(0.0);
                }
                "w:pgBorders" => {
                    for b in c.elements() {
                        s.page_borders.push((b.local().to_string(), Border::parse(b, tc)));
                    }
                }
                _ => {}
            }
        }
        s
    }

    /// Column x-offsets (relative to left margin) and widths.
    pub fn columns(&self) -> Vec<(f32, f32)> {
        let body_w = self.page_w - self.margin_left - self.margin_right - self.gutter;
        if !self.col_widths.is_empty() {
            let mut x = 0.0;
            return self
                .col_widths
                .iter()
                .map(|(w, sp)| {
                    let r = (x, *w);
                    x += w + sp;
                    r
                })
                .collect();
        }
        let n = self.cols.max(1) as f32;
        let w = ((body_w - self.col_space * (n - 1.0)) / n).max(18.0);
        (0..self.cols.max(1)).map(|i| (i as f32 * (w + self.col_space), w)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(format_number(4, "upperRoman"), "IV");
        assert_eq!(format_number(28, "lowerLetter"), "bb");
        assert_eq!(format_number(2, "ordinal"), "2nd");
    }

    #[test]
    fn tint_shade() {
        let c = Color::from_hex("156082").unwrap();
        assert_eq!(c.shade(191.0 / 255.0).to_hex(), "0F4761");
        let w = Color::WHITE.shade(0xBF as f32 / 255.0);
        assert_eq!(w.to_hex(), "BFBFBF");
    }
}
