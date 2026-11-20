//! Paragraph layout: atoms → lines → positioned items.

use super::drawing::{self, DrawKind, DrawingObj};
use super::{Ctx, Exclusion, Flow, Frag, Item, LineInfo, Story};
use crate::fonts::FontMatch;
use crate::model::{item_len, Block, Inline, Paragraph, RunItem};
use crate::props::{Align, Border, Color, LineRule, ListCounters, ParaProps, RunProps, TableCtx, VertAlign};
use crate::xml::Element;
use std::rc::Rc;

pub struct RStyle {
    pub rp: RunProps,
    pub fm: [FontMatch; 4],
    pub size: f32,
    pub size_cs: f32,
    pub raise: f32,
}

pub struct PageNums {
    pub page: i32,
    pub total: i32,
    pub fmt: String,
}

#[derive(Clone)]
pub enum AK {
    Char(char),
    Space(char),
    Tab,
    LineBreak,
    PageBreak,
    ColumnBreak,
    Obj(Rc<DrawingObj>),
    Float(Rc<DrawingObj>),
    FootnoteRef(String),
}

#[derive(Clone)]
pub struct Atom {
    pub kind: AK,
    pub off: Option<usize>,
    pub st: Rc<RStyle>,
    pub font: Option<FontMatch>,
    pub size: f32,
    pub glyph: Option<char>,
    pub w: f32,
    pub asc: f32,
    pub desc: f32,
    pub is_obj: bool,
}

pub struct Line {
    pub start: usize,
    pub end: usize,
    pub xs: Vec<f32>,
    pub widths: Vec<f32>,
    pub height: f32,
    pub baseline: f32,
    pub y_skip: f32,
    pub left: f32,
    pub right: f32,
    pub forced: Option<AKBreak>,
    pub last: bool,
    pub label: Vec<(Atom, f32)>,
    pub tab_leaders: Vec<(f32, f32, String, Rc<RStyle>)>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum AKBreak {
    Line,
    Page,
    Column,
}

pub struct Measured {
    pub pp: ParaProps,
    pub atoms: Vec<Atom>,
    pub lines: Vec<Line>,
    pub mark: Rc<RStyle>,
    pub para_len: usize,
    pub floats: Vec<Rc<DrawingObj>>,
    pub footnotes: Vec<Vec<String>>,
}

impl Measured {
    pub fn height(&self) -> f32 {
        self.lines.iter().map(|l| l.height + l.y_skip).sum()
    }
}

// ---------------------------------------------------------------------------
// Style resolution with per-pass caching

pub fn rstyle(ctx: &Ctx, pstyle: &str, rpr: Option<&Element>, tctx: Option<&TableCtx>, is_mark: bool) -> Rc<RStyle> {
    let key = (pstyle.to_string(), rpr.map(|r| r as *const Element as usize).unwrap_or(0), Ctx::tctx_key(tctx), is_mark);
    if let Some(s) = ctx.style_cache.borrow().get(&key) {
        return s.clone();
    }
    let rp = ctx.res.run(pstyle, rpr, tctx);
    let f = |fam: &str, cs: bool| {
        let (b, i) = if cs { (rp.bold_cs, rp.italic_cs) } else { (rp.bold, rp.italic) };
        ctx.fonts.resolve(fam, b, i)
    };
    let fm = [f(&rp.font_ascii, false), f(&rp.font_hansi, false), f(&rp.font_ea, false), f(&rp.font_cs, true)];
    let (mut size, mut size_cs, mut raise) = (rp.size, rp.size_cs, rp.position);
    match rp.vert_align {
        VertAlign::Super => {
            raise += rp.size * 0.33;
            size *= 0.65;
            size_cs *= 0.65;
        }
        VertAlign::Sub => {
            raise -= rp.size * 0.14;
            size *= 0.65;
            size_cs *= 0.65;
        }
        VertAlign::Baseline => {}
    }
    let s = Rc::new(RStyle { rp, fm, size, size_cs, raise });
    ctx.style_cache.borrow_mut().insert(key, s.clone());
    s
}

fn is_cjk(c: u32) -> bool {
    matches!(c, 0x1100..=0x11FF | 0x2E80..=0x2FDF | 0x2FF0..=0x303F | 0x3040..=0x31FF | 0x3200..=0x9FFF | 0xA960..=0xA97F | 0xAC00..=0xD7FF | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFFEF | 0x20000..=0x3FFFF)
}

fn is_complex(c: u32) -> bool {
    matches!(c, 0x0590..=0x08FF | 0x0900..=0x0DFF | 0x0E00..=0x0EFF | 0x0F00..=0x0FFF | 0x1000..=0x109F | 0x1780..=0x17FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF)
}

fn slot(c: char, rp: &RunProps) -> usize {
    let cp = c as u32;
    if rp.cs || rp.rtl || is_complex(cp) {
        return 3;
    }
    if is_cjk(cp) {
        return 2;
    }
    if cp < 0x80 {
        return 0;
    }
    if rp.hint.as_deref() == Some("eastAsia") && matches!(cp, 0x2018..=0x201F | 0x2026 | 0x00B7 | 0x2014 | 0x2015) {
        return 2;
    }
    1
}

fn make_char_atom(ctx: &Ctx, c: char, st: &Rc<RStyle>, off: Option<usize>) -> Atom {
    let rp = &st.rp;
    let s = slot(c, rp);
    let mut size = if s == 3 { st.size_cs } else { st.size };
    let mut ch = c;
    if rp.caps || rp.small_caps {
        let up = c.to_uppercase().next().unwrap_or(c);
        if rp.small_caps && !rp.caps && up != c {
            size *= 0.8;
        }
        ch = up;
    }
    let mut fm = st.fm[s].clone();
    if !fm.font.has_glyph(ch) && !matches!(ch, ' ' | '\u{00A0}' | '\u{200B}') {
        if let Some(f) = ctx.fonts.fallback_for(ch, rp.bold, rp.italic) {
            fm = f;
        }
    }
    let base = fm.font.advance(ch, size);
    let w = base * rp.scale / 100.0 + rp.spacing + if fm.synth_bold { size * 0.02 } else { 0.0 };
    let f = &fm.font;
    let asc = f.ascent(size) + f.leading(size) + st.raise.max(0.0);
    let desc = f.descent(size) - st.raise.min(0.0);
    let kind = if matches!(c, ' ' | '\u{00A0}' | '\u{3000}') { AK::Space(c) } else { AK::Char(ch) };
    Atom { kind, off, st: st.clone(), font: Some(fm), size, glyph: Some(ch), w, asc, desc, is_obj: false }
}

fn simple_atom(kind: AK, st: &Rc<RStyle>, off: Option<usize>) -> Atom {
    let f = &st.fm[0].font;
    Atom {
        kind,
        off,
        st: st.clone(),
        font: Some(st.fm[0].clone()),
        size: st.size,
        glyph: None,
        w: 0.0,
        asc: f.ascent(st.size) + f.leading(st.size),
        desc: f.descent(st.size),
        is_obj: false,
    }
}

// ---------------------------------------------------------------------------
// Atom building

struct FieldState {
    stack: Vec<(String, bool)>, // (instr, in_result)
}

pub fn build_atoms(
    ctx: &Ctx,
    p: &Paragraph,
    pp: &ParaProps,
    story: &Story,
    tctx: Option<&TableCtx>,
    pn: Option<&PageNums>,
) -> (Vec<Atom>, usize, Vec<Rc<DrawingObj>>) {
    let mut atoms = Vec::new();
    let mut floats = Vec::new();
    let mut off = 0usize;
    let part = ctx.story_part(story);
    let mut fs = FieldState { stack: Vec::new() };
    let pstyle = pp.style_id.clone();

    fn hidden_by_field(fs: &FieldState) -> bool {
        fs.stack.iter().any(|(_, in_result)| !*in_result)
    }
    fn field_override(fs: &FieldState, pn: Option<&PageNums>) -> Option<String> {
        let (instr, in_result) = fs.stack.last()?;
        if !*in_result {
            return None;
        }
        let kw = instr.split_whitespace().next().unwrap_or("").to_ascii_uppercase();
        let pn = pn?;
        match kw.as_str() {
            "PAGE" => Some(crate::props::format_number(pn.page, &pn.fmt)),
            "NUMPAGES" | "SECTIONPAGES" => Some(if pn.total > 0 { pn.total.to_string() } else { "1".into() }),
            _ => None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn rec(
        ctx: &Ctx,
        items: &[Inline],
        deleted: bool,
        atoms: &mut Vec<Atom>,
        floats: &mut Vec<Rc<DrawingObj>>,
        off: &mut usize,
        fs: &mut FieldState,
        pstyle: &str,
        tctx: Option<&TableCtx>,
        part: &str,
        pn: Option<&PageNums>,
    ) {
        for it in items {
            match it {
                Inline::Run(r) => {
                    let st = rstyle(ctx, pstyle, r.rpr.as_ref(), tctx, false);
                    let hidden = st.rp.vanish;
                    for item in &r.items {
                        let len = item_len(item);
                        if deleted {
                            continue;
                        }
                        let o = *off;
                        *off += len;
                        match item {
                            RunItem::FldChar { kind, .. } => match kind.as_str() {
                                "begin" => fs.stack.push((String::new(), false)),
                                "separate" => {
                                    let ov = field_override(&FieldState { stack: fs.stack.iter().map(|(i, _)| (i.clone(), true)).collect() }, pn);
                                    if let Some(top) = fs.stack.last_mut() {
                                        top.1 = true;
                                    }
                                    if let Some(text) = ov {
                                        for c in text.chars() {
                                            atoms.push(make_char_atom(ctx, c, &st, None));
                                        }
                                        // Mark that cached result must be skipped.
                                        if let Some(top) = fs.stack.last_mut() {
                                            top.0.push_str("\u{1}SKIP");
                                        }
                                    }
                                }
                                "end" => {
                                    if let Some((instr, in_result)) = fs.stack.pop() {
                                        if !in_result {
                                            // Field without a cached result.
                                            let tmp = FieldState { stack: vec![(instr, true)] };
                                            if let Some(text) = field_override(&tmp, pn) {
                                                for c in text.chars() {
                                                    atoms.push(make_char_atom(ctx, c, &st, None));
                                                }
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            },
                            RunItem::InstrText(t) => {
                                if let Some(top) = fs.stack.last_mut() {
                                    if !top.1 {
                                        top.0.push_str(t);
                                    }
                                }
                            }
                            _ if hidden_by_field(fs) => {}
                            _ if fs.stack.last().is_some_and(|(i, r)| *r && i.ends_with("\u{1}SKIP")) => {}
                            _ if hidden => {}
                            RunItem::Text(t) => {
                                for (k, c) in t.chars().enumerate() {
                                    atoms.push(make_char_atom(ctx, c, &st, Some(o + k)));
                                }
                            }
                            RunItem::Tab => atoms.push(simple_atom(AK::Tab, &st, Some(o))),
                            RunItem::Break { kind, .. } => {
                                let k = match kind.as_deref() {
                                    Some("page") => AK::PageBreak,
                                    Some("column") => AK::ColumnBreak,
                                    _ => AK::LineBreak,
                                };
                                atoms.push(simple_atom(k, &st, Some(o)));
                            }
                            RunItem::CarriageReturn => atoms.push(simple_atom(AK::LineBreak, &st, Some(o))),
                            RunItem::NoBreakHyphen => atoms.push(make_char_atom(ctx, '\u{2011}', &st, Some(o))),
                            RunItem::SoftHyphen => {
                                let mut a = make_char_atom(ctx, '\u{00AD}', &st, Some(o));
                                a.w = 0.0;
                                a.glyph = None;
                                atoms.push(a);
                            }
                            RunItem::Sym { font, ch } => {
                                let mut rp2 = st.rp.clone();
                                rp2.font_ascii = font.clone();
                                rp2.font_hansi = font.clone();
                                let fm = ctx.fonts.resolve(font, rp2.bold, rp2.italic);
                                let s2 = Rc::new(RStyle { rp: rp2, fm: [fm.clone(), fm.clone(), fm.clone(), fm], size: st.size, size_cs: st.size_cs, raise: st.raise });
                                let mut a = make_char_atom(ctx, *ch, &s2, Some(o));
                                if let Some(f) = &a.font {
                                    if !f.font.has_glyph(*ch) && f.font.symbol {
                                        a.w = f.font.advance(*ch, st.size);
                                    }
                                }
                                atoms.push(a);
                            }
                            RunItem::Drawing(d) => push_drawing(ctx, d, &st, Some(o), part, atoms, floats),
                            RunItem::Other(e) => {
                                if e.name == "mc:AlternateContent" {
                                    if let Some(d) = drawing::alternate_drawing(e) {
                                        push_drawing(ctx, d, &st, Some(o), part, atoms, floats);
                                    } else if let Some(pict) = e.find_local("pict") {
                                        if let Some(obj) = drawing::parse_vml(pict, ctx, part) {
                                            push_obj(obj, &st, if len > 0 { Some(o) } else { None }, atoms, floats);
                                        }
                                    }
                                } else if e.name == "w:pict" || e.name == "w:object" {
                                    if let Some(obj) = drawing::parse_vml(e, ctx, part) {
                                        push_obj(obj, &st, None, atoms, floats);
                                    }
                                } else if e.name == "w:footnoteReference" || e.name == "w:endnoteReference" {
                                    let id = e.attr("w:id").unwrap_or("").to_string();
                                    let n = {
                                        let mut m = ctx.footnote_numbers.borrow_mut();
                                        let next = m.len() + 1;
                                        *m.entry(id.clone()).or_insert(next)
                                    };
                                    let mut first = true;
                                    for c in n.to_string().chars() {
                                        let mut a = make_char_atom(ctx, c, &st, None);
                                        if first && e.name == "w:footnoteReference" {
                                            a.kind = AK::FootnoteRef(id.clone());
                                            a.glyph = Some(c);
                                            first = false;
                                        }
                                        atoms.push(a);
                                    }
                                } else if e.name == "w:footnoteRef" || e.name == "w:endnoteRef" {
                                    let n = ctx.current_footnote_number();
                                    for c in n.chars() {
                                        atoms.push(make_char_atom(ctx, c, &st, None));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Inline::Container { shell, children } => {
                    let del = deleted || shell.name == "w:del" || shell.name == "w:moveFrom";
                    if shell.name == "w:fldSimple" {
                        let instr = shell.attr("w:instr").unwrap_or("").to_string();
                        let tmp = FieldState { stack: vec![(instr, true)] };
                        if let Some(text) = field_override(&tmp, pn) {
                            let st = rstyle(ctx, pstyle, None, tctx, false);
                            for c in text.chars() {
                                atoms.push(make_char_atom(ctx, c, &st, None));
                            }
                            // Count (but don't show) the cached result.
                            let mut dummy_atoms = Vec::new();
                            let mut dummy_floats = Vec::new();
                            rec(ctx, children, del, &mut dummy_atoms, &mut dummy_floats, off, fs, pstyle, tctx, part, pn);
                            continue;
                        }
                    }
                    rec(ctx, children, del, atoms, floats, off, fs, pstyle, tctx, part, pn);
                }
                Inline::Other(e) => {
                    if e.name == "m:oMath" || e.name == "m:oMathPara" {
                        let st = rstyle(ctx, pstyle, None, tctx, false);
                        let mut rp2 = st.rp.clone();
                        rp2.font_ascii = "Cambria Math".into();
                        rp2.font_hansi = "Cambria Math".into();
                        let fm = ctx.fonts.resolve("Cambria Math", false, false);
                        let s2 = Rc::new(RStyle { rp: rp2, fm: [fm.clone(), fm.clone(), fm.clone(), fm], size: st.size, size_cs: st.size_cs, raise: 0.0 });
                        let mut ts = Vec::new();
                        e.find_all_local("t", &mut ts);
                        for t in ts {
                            for c in t.text().chars() {
                                atoms.push(make_char_atom(ctx, c, &s2, None));
                            }
                        }
                    }
                }
            }
        }
    }
    rec(ctx, &p.inlines, false, &mut atoms, &mut floats, &mut off, &mut fs, &pstyle, tctx, &part, pn);
    apply_kerning(&mut atoms);
    (atoms, off, floats)
}

/// Pair kerning between adjacent characters of the same font run, when the run's
/// w:kern threshold is met (kerning applies only when w:kern is set).
fn apply_kerning(atoms: &mut [Atom]) {
    for i in 0..atoms.len().saturating_sub(1) {
        let (left, right) = atoms.split_at_mut(i + 1);
        let a = &mut left[i];
        let b = &right[0];
        let Some(th) = a.st.rp.kern else { continue };
        if a.size < th || th <= 0.0 && a.st.rp.kern.is_none() {
            continue;
        }
        if !matches!(a.kind, AK::Char(_) | AK::Space(_)) || !matches!(b.kind, AK::Char(_) | AK::Space(_)) {
            continue;
        }
        let (Some(fa), Some(fb)) = (&a.font, &b.font) else { continue };
        if !std::sync::Arc::ptr_eq(&fa.font, &fb.font) || (a.size - b.size).abs() > 0.01 || !Rc::ptr_eq(&a.st, &b.st) {
            continue;
        }
        let (Some(ca), Some(cb)) = (a.glyph, b.glyph) else { continue };
        let ga = fa.font.glyph(ca).0;
        let gb = fa.font.glyph(cb).0;
        let k = fa.font.kern(ga, gb);
        if k != 0.0 {
            a.w += k * a.size / fa.font.upm * a.st.rp.scale / 100.0;
        }
    }
}

fn push_drawing(ctx: &Ctx, d: &Element, st: &Rc<RStyle>, off: Option<usize>, part: &str, atoms: &mut Vec<Atom>, floats: &mut Vec<Rc<DrawingObj>>) {
    match drawing::parse_drawing(d, ctx, part) {
        Some(obj) => push_obj(obj, st, off, atoms, floats),
        None => {
            // Keep the offset slot so the caret model stays consistent.
            if off.is_some() {
                atoms.push(simple_atom(AK::Float(Rc::new(DrawingObj { w: 0.0, h: 0.0, kind: DrawKind::Placeholder(String::new()), anchor: None })), st, off));
            }
        }
    }
}

fn push_obj(obj: DrawingObj, st: &Rc<RStyle>, off: Option<usize>, atoms: &mut Vec<Atom>, floats: &mut Vec<Rc<DrawingObj>>) {
    let obj = Rc::new(obj);
    if obj.anchor.is_some() {
        floats.push(obj.clone());
        let mut a = simple_atom(AK::Float(obj), st, off);
        a.w = 0.0;
        a.asc = 0.0;
        a.desc = 0.0;
        atoms.push(a);
    } else {
        let mut a = simple_atom(AK::Obj(obj.clone()), st, off);
        a.w = obj.w;
        a.asc = obj.h;
        a.desc = 0.0;
        a.is_obj = true;
        atoms.push(a);
    }
}

// ---------------------------------------------------------------------------
// Line breaking

/// Maximum fraction by which compatibility mode 15 shrinks inter-word spaces in justified text.
/// Calibrated against the reference: 0.25 reproduces its line breaks exactly on our
/// justified-text corpus (0.20 and 0.30 both diverge).
fn shrink_ratio() -> f32 {
    static R: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *R.get_or_init(|| std::env::var("RDW_SHRINK").ok().and_then(|v| v.parse().ok()).unwrap_or(0.25))
}

fn can_break_after(atoms: &[Atom], i: usize) -> bool {
    let a = &atoms[i];
    let next = atoms.get(i + 1);
    match &a.kind {
        AK::Space(c) => *c != '\u{00A0}' && !matches!(next.map(|n| &n.kind), Some(AK::Space(_))),
        AK::Tab | AK::Obj(_) => true,
        AK::Char(c) => {
            let cp = *c as u32;
            if matches!(c, '-' | '\u{2013}' | '\u{2014}' | '\u{00AD}' | '\u{200B}' | '/') && *c != '/' {
                return next.is_some_and(|n| matches!(n.kind, AK::Char(nc) if nc.is_alphanumeric()));
            }
            if is_cjk(cp) {
                // Avoid breaking before closing punctuation.
                return !next.is_some_and(|n| matches!(n.kind, AK::Char(nc) if "、。，．：；？！）」』】〕〉》’”ー々".contains(nc) || matches!(nc, ',' | '.' | ')' | ':' | ';')));
            }
            next.is_some_and(|n| matches!(n.kind, AK::Char(nc) if is_cjk(nc as u32)) || matches!(n.kind, AK::Obj(_)))
        }
        _ => false,
    }
}

pub struct TabCtx<'p> {
    pub pp: &'p ParaProps,
    pub default_tab: f32,
}

impl TabCtx<'_> {
    /// Next tab stop strictly after x: (pos, kind, leader).
    fn next_stop(&self, x: f32) -> (f32, String, String) {
        let mut best: Option<(f32, String, String)> = None;
        for t in &self.pp.tabs {
            if t.kind == "bar" || t.kind == "clear" {
                continue;
            }
            if t.pos > x + 0.05 {
                best = Some((t.pos, t.kind.clone(), t.leader.clone()));
                break;
            }
        }
        // Hanging indent acts as an implicit tab stop.
        if self.pp.ind_first < 0.0 && self.pp.ind_left > x + 0.05 && best.as_ref().is_none_or(|b| b.0 > self.pp.ind_left) {
            return (self.pp.ind_left, "left".into(), "none".into());
        }
        if let Some(b) = best {
            return b;
        }
        let last_custom = self.pp.tabs.iter().filter(|t| t.kind != "bar").map(|t| t.pos).fold(0.0f32, f32::max);
        let base = x.max(last_custom);
        let dt = self.default_tab.max(1.0);
        let k = (base / dt + 1e-3).floor() + 1.0;
        (k * dt, "left".into(), "none".into())
    }
}

/// Width of content after atom `i` until the next tab/break (for right/center/decimal tabs).
fn segment_width(atoms: &[Atom], i: usize, decimal: bool) -> f32 {
    let mut w = 0.0;
    for a in &atoms[i + 1..] {
        match a.kind {
            AK::Tab | AK::LineBreak | AK::PageBreak | AK::ColumnBreak => break,
            AK::Char(c) if decimal && (c == '.' || c == ',') => break,
            _ => w += a.w,
        }
    }
    w
}

pub struct LineLimits {
    pub left: f32,
    pub right: f32,
    pub push: f32,
}

#[allow(clippy::too_many_arguments)]
pub fn break_lines(
    ctx: &Ctx,
    atoms: &mut [Atom],
    pp: &ParaProps,
    mark: &Rc<RStyle>,
    width: f32,
    label: Vec<Atom>,
    label_jc: &str,
    label_suffix: &str,
    grid_pitch: Option<f32>,
    mut limits: impl FnMut(f32, f32) -> LineLimits,
) -> Vec<Line> {
    let tabs = TabCtx { pp, default_tab: ctx.doc.settings.default_tab };
    let mut lines = Vec::new();
    let mut i = 0usize;
    let n = atoms.len();
    let mut first = true;
    let mut y_acc = 0.0f32;
    let mut widths: Vec<f32> = atoms.iter().map(|a| a.w).collect();
    let smart_justify = matches!(pp.align, Align::Justify | Align::Distribute) && ctx.doc.settings.compat_mode >= 15;
    loop {
        let start_x_base = if first { pp.ind_left + pp.ind_first } else { pp.ind_left };
        let lim = limits(y_acc, mark.fm[0].font.line_height(mark.size));
        let right = (width - pp.ind_right - lim.right).max(start_x_base + 1.0);
        let left_extra = lim.left;
        let mut x = start_x_base + left_extra;
        let mut placed_label = Vec::new();
        let mut leaders = Vec::new();
        if first && !label.is_empty() {
            let lw: f32 = label.iter().map(|a| a.w).sum();
            let mut lx = match label_jc {
                "right" | "end" => x - lw,
                "center" => x - lw / 2.0,
                _ => x,
            };
            for a in &label {
                placed_label.push((a.clone(), lx));
                lx += a.w;
            }
            x = lx;
            match label_suffix {
                "space" => {
                    x += mark.fm[0].font.advance(' ', mark.size);
                }
                "nothing" => {}
                _ => {
                    let (pos, _, _) = tabs.next_stop(x);
                    // List tab never exceeds the right indent; if past, use the next default stop.
                    x = if pos < right { pos } else { x };
                }
            }
        }
        let mut j = i;
        let mut last_break: Option<usize> = None;
        let mut forced = None;
        let mut xs = Vec::new();
        // compatibilityMode 15 may shrink inter-word spaces of justified lines.
        let mut shrinkable = 0.0f32;
        while j < n {
            let a = &atoms[j];
            match &a.kind {
                AK::LineBreak | AK::PageBreak | AK::ColumnBreak => {
                    xs.push(x);
                    widths[j] = 0.0;
                    forced = Some(match a.kind {
                        AK::PageBreak => AKBreak::Page,
                        AK::ColumnBreak => AKBreak::Column,
                        _ => AKBreak::Line,
                    });
                    j += 1;
                    break;
                }
                AK::Tab => {
                    let (pos, kind, leader) = tabs.next_stop(x);
                    let w = match kind.as_str() {
                        "right" | "end" => (pos - x - segment_width(atoms, j, false)).max(0.0),
                        "center" => (pos - x - segment_width(atoms, j, false) / 2.0).max(0.0),
                        "decimal" => (pos - x - segment_width(atoms, j, true)).max(0.0),
                        _ => pos - x,
                    };
                    let w = if x + w > right + 0.01 && kind == "left" { (right - x).max(0.0) } else { w };
                    widths[j] = w;
                    if leader != "none" && w > 0.0 {
                        leaders.push((x, w, leader, a.st.clone()));
                    }
                    xs.push(x);
                    x += w;
                    shrinkable = 0.0;
                    last_break = Some(j + 1);
                    j += 1;
                    continue;
                }
                AK::Space(c) => {
                    xs.push(x);
                    x += widths[j];
                    if *c != '\u{00A0}' {
                        shrinkable += widths[j];
                    }
                    if can_break_after(atoms, j) {
                        last_break = Some(j + 1);
                    }
                    j += 1;
                    continue;
                }
                _ => {}
            }
            let w = widths[j];
            let fits_by_shrinking = smart_justify && x + w - right <= shrinkable * shrink_ratio() + 0.01;
            if x + w > right + 0.01 && j > i && !fits_by_shrinking {
                // Overflow: break at last opportunity or before this atom.
                let brk = last_break.unwrap_or(j);
                xs.truncate(brk - i);
                j = brk;
                // Drop trailing leaders beyond break.
                break;
            }
            xs.push(x);
            x += w;
            if can_break_after(atoms, j) {
                last_break = Some(j + 1);
            }
            j += 1;
        }
        // Prevent infinite loop: ensure progress.
        if j == i && i < n {
            xs.push(x);
            j = i + 1;
        }
        let end = j;
        let last = end >= n;
        // Metrics.
        let mut asc_t: f32 = 0.0;
        let mut desc: f32 = 0.0;
        let mut asc_o: f32 = 0.0;
        for a in &atoms[i..end] {
            if matches!(a.kind, AK::Float(_)) {
                continue;
            }
            if a.is_obj {
                asc_o = asc_o.max(a.asc);
            } else {
                asc_t = asc_t.max(a.asc);
                desc = desc.max(a.desc);
            }
        }
        for (a, _) in &placed_label {
            asc_t = asc_t.max(a.asc);
            desc = desc.max(a.desc);
        }
        // The paragraph mark participates in the last line (and empty lines).
        if last || asc_t + asc_o == 0.0 {
            let f = &mark.fm[0].font;
            asc_t = asc_t.max(f.ascent(mark.size) + f.leading(mark.size));
            desc = desc.max(f.descent(mark.size));
        }
        let nat_t = asc_t + desc;
        let (mut height, mut baseline) = match pp.line {
            LineRule::Auto(m) => {
                let h_text = nat_t * m;
                let h = h_text.max(asc_o + desc);
                (h, (asc_t).max(asc_o))
            }
            LineRule::Exact(v) => (v, v - desc),
            LineRule::AtLeast(v) => {
                let nat = asc_t.max(asc_o) + desc;
                if v > nat { (v, v - desc) } else { (nat, asc_t.max(asc_o)) }
            }
        };
        if let Some(pitch) = grid_pitch {
            if pitch > 0.0 && pp.snap_to_grid && !matches!(pp.line, LineRule::Exact(_)) {
                let snapped = (height / pitch - 0.01).ceil().max(1.0) * pitch;
                baseline += (snapped - height) / 2.0;
                height = snapped;
            }
        }
        // Alignment.
        let mut content_end = start_x_base + left_extra;
        let mut last_nonspace = None;
        for (k, a) in atoms[i..end].iter().enumerate() {
            if !matches!(a.kind, AK::Space(_) | AK::LineBreak | AK::PageBreak | AK::ColumnBreak | AK::Float(_)) {
                content_end = xs[k] + widths[i + k];
                last_nonspace = Some(k);
            }
        }
        if let Some((a, lx)) = placed_label.last() {
            content_end = content_end.max(lx + a.w);
        }
        let slack = right - content_end;
        let justify = match pp.align {
            Align::Justify => !last && forced != Some(AKBreak::Page) && !(forced == Some(AKBreak::Line) && ctx.doc.settings.do_not_expand_shift_return),
            Align::Distribute => true,
            _ => false,
        };
        let mut shift = 0.0;
        match pp.align {
            Align::Center => shift = slack / 2.0,
            Align::Right => shift = slack,
            _ => {}
        }
        if shift > 0.0 {
            for x in xs.iter_mut() {
                *x += shift;
            }
            for (_, lx) in placed_label.iter_mut() {
                *lx += shift;
            }
            for l in leaders.iter_mut() {
                l.0 += shift;
            }
        }
        if (justify && slack > 0.0) || (smart_justify && slack < -0.01) {
            if let Some(ln) = last_nonspace {
                // Only spaces after the last tab are stretched.
                let first_k = atoms[i..end].iter().rposition(|a| matches!(a.kind, AK::Tab)).map(|t| t + 1).unwrap_or(0);
                let spaces: Vec<usize> = (first_k..ln).filter(|k| matches!(atoms[i + k].kind, AK::Space(c) if c != '\u{00A0}')).collect();
                if !spaces.is_empty() {
                    let per = slack / spaces.len() as f32;
                    let mut add = 0.0;
                    let mut si = 0;
                    for k in 0..xs.len() {
                        xs[k] += add;
                        if si < spaces.len() && spaces[si] == k {
                            widths[i + k] += per;
                            add += per;
                            si += 1;
                        }
                    }
                }
            }
        }
        let w_line: Vec<f32> = widths[i..end].to_vec();
        lines.push(Line {
            start: i,
            end,
            xs,
            widths: w_line,
            height,
            baseline,
            y_skip: lim.push,
            left: start_x_base + left_extra,
            right,
            forced,
            last,
            label: placed_label,
            tab_leaders: leaders,
        });
        y_acc += height + lim.push;
        first = false;
        i = end;
        if i >= n {
            // A trailing text-wrapping break leaves an empty last line for the paragraph mark.
            // (In compatibility mode 15 the mark after a page/column break stays on the break's line.)
            if forced == Some(AKBreak::Line) {
                let f = &mark.fm[0].font;
                let asc = f.ascent(mark.size) + f.leading(mark.size);
                let desc = f.descent(mark.size);
                let (h, b) = match pp.line {
                    LineRule::Auto(m) => ((asc + desc) * m, asc),
                    LineRule::Exact(v) => (v, v - desc),
                    LineRule::AtLeast(v) => {
                        if v > asc + desc { (v, v - desc) } else { (asc + desc, asc) }
                    }
                };
                if let Some(l) = lines.last_mut() {
                    l.last = false;
                }
                let x0 = pp.ind_left
                    + match pp.align {
                        Align::Center => (width - pp.ind_right - pp.ind_left) / 2.0,
                        Align::Right => width - pp.ind_right - pp.ind_left,
                        _ => 0.0,
                    };
                lines.push(Line {
                    start: n,
                    end: n,
                    xs: vec![],
                    widths: vec![],
                    height: h,
                    baseline: b,
                    y_skip: 0.0,
                    left: x0,
                    right,
                    forced: None,
                    last: true,
                    label: vec![],
                    tab_leaders: vec![],
                });
            }
            break;
        }
    }
    if lines.is_empty() {
        // Empty paragraph.
        let f = &mark.fm[0].font;
        let asc = f.ascent(mark.size) + f.leading(mark.size);
        let desc = f.descent(mark.size);
        let (mut h, mut b) = match pp.line {
            LineRule::Auto(m) => ((asc + desc) * m, asc),
            LineRule::Exact(v) => (v, v - desc),
            LineRule::AtLeast(v) => {
                if v > asc + desc { (v, v - desc) } else { (asc + desc, asc) }
            }
        };
        if let Some(pitch) = grid_pitch {
            if pitch > 0.0 && pp.snap_to_grid && !matches!(pp.line, LineRule::Exact(_)) {
                let s = (h / pitch - 0.01).ceil().max(1.0) * pitch;
                b += (s - h) / 2.0;
                h = s;
            }
        }
        let mut label_placed = Vec::new();
        let mut x = pp.ind_left + pp.ind_first;
        let lw: f32 = label.iter().map(|a| a.w).sum();
        x = match label_jc {
            "right" => x - lw,
            "center" => x - lw / 2.0,
            _ => x,
        };
        for a in &label {
            label_placed.push((a.clone(), x));
            x += a.w;
        }
        let x0 = match pp.align {
            Align::Center => (pp.ind_left + width - pp.ind_right) / 2.0,
            Align::Right => width - pp.ind_right,
            _ => {
                if label.is_empty() {
                    pp.ind_left + pp.ind_first
                } else {
                    let (pos, _, _) = tabs.next_stop(x);
                    if label_suffix == "tab" { pos } else { x }
                }
            }
        };
        lines.push(Line {
            start: 0,
            end: 0,
            xs: vec![],
            widths: vec![],
            height: h,
            baseline: b,
            y_skip: 0.0,
            left: x0,
            right: width - pp.ind_right,
            forced: None,
            last: true,
            label: label_placed,
            tab_leaders: vec![],
        });
    }
    // Write back stretched/tab widths.
    for l in &lines {
        for (k, w) in l.widths.iter().enumerate() {
            if let Some(a) = atoms.get_mut(l.start + k) {
                a.w = *w;
            }
        }
    }
    lines
}

// ---------------------------------------------------------------------------
// Measure a paragraph

#[allow(clippy::too_many_arguments)]
pub fn measure(
    ctx: &Ctx,
    p: &Paragraph,
    story: &Story,
    width: f32,
    tctx: Option<&TableCtx>,
    counters: &mut ListCounters,
    pn: Option<&PageNums>,
    grid_pitch: Option<f32>,
    limits: impl FnMut(f32, f32) -> LineLimits,
) -> Measured {
    let pp = ctx.res.para(p.ppr.as_ref(), tctx);
    let mark = rstyle(ctx, &pp.style_id, p.ppr.as_ref().and_then(|x| x.child("w:rPr")), tctx, true);
    let (mut atoms, para_len, floats) = build_atoms(ctx, p, &pp, story, tctx, pn);
    // List label.
    let mut label = Vec::new();
    let mut label_jc = "left".to_string();
    let mut suffix = "tab".to_string();
    if let Some((num_id, ilvl)) = &pp.num {
        if let Some((text, level)) = ctx.res.list_label(num_id, *ilvl, counters) {
            // Label formatting: paragraph mark rPr + level rPr.
            let mut lrpr = p.ppr.as_ref().and_then(|x| x.child("w:rPr")).cloned().unwrap_or_else(|| Element::new("w:rPr"));
            // Remove properties that shouldn't carry to labels.
            lrpr.remove_child("w:u");
            if let Some(lr) = &level.rpr {
                for c in lr.elements() {
                    lrpr.set_child(c.clone(), crate::props::RPR_ORDER);
                }
            }
            // Leak the label rPr into the cache key space via a boxed element kept alive in the style cache.
            let st = {
                let rp = ctx.res.run(&pp.style_id, Some(&lrpr), tctx);
                let f = |fam: &str| ctx.fonts.resolve(fam, rp.bold, rp.italic);
                let fm = [f(&rp.font_ascii), f(&rp.font_hansi), f(&rp.font_ea), f(&rp.font_cs)];
                Rc::new(RStyle { size: rp.size, size_cs: rp.size_cs, raise: rp.position, fm, rp })
            };
            for c in text.chars() {
                let c = if st.fm[0].font.symbol || !st.fm[0].font.has_glyph(c) {
                    // Symbol-font bullets on systems without the font fall back to a generic bullet.
                    if (0xF000..0xF100).contains(&(c as u32)) && !st.fm[0].font.symbol {
                        match c as u32 {
                            0xF0B7 => '•',
                            0xF0A7 => '▪',
                            0xF0D8 => '➢',
                            0xF0FC => '✓',
                            _ => '•',
                        }
                    } else {
                        c
                    }
                } else {
                    c
                };
                label.push(make_char_atom(ctx, c, &st, None));
            }
            label_jc = level.jc.clone();
            suffix = level.suff.clone();
        }
    }
    let lines = break_lines(ctx, &mut atoms, &pp, &mark, width, label, &label_jc, &suffix, grid_pitch, limits);
    let footnotes = lines
        .iter()
        .map(|l| {
            atoms[l.start..l.end]
                .iter()
                .filter_map(|a| if let AK::FootnoteRef(id) = &a.kind { Some(id.clone()) } else { None })
                .collect()
        })
        .collect();
    Measured { pp, atoms, lines, mark, para_len, floats, footnotes }
}

// ---------------------------------------------------------------------------
// Emission

fn auto_color(bg: Option<Color>) -> Color {
    match bg {
        Some(c) if c.luminance() < 0.4 => Color::WHITE,
        _ => Color::BLACK,
    }
}

/// Emit one line at (ox, top) into items/lines.
#[allow(clippy::too_many_arguments)]
pub fn emit_line(
    ctx: &Ctx,
    m: &Measured,
    li: usize,
    ox: f32,
    top: f32,
    story: &Story,
    path: &[usize],
    bg: Option<Color>,
    items: &mut Vec<Item>,
    infos: &mut Vec<LineInfo>,
) {
    let line = &m.lines[li];
    let top = top + line.y_skip;
    let base = top + line.baseline;
    let atoms = &m.atoms[line.start..line.end];
    let para_bg = m.pp.shading.or(bg);

    // Backgrounds: highlight & run shading.
    for (k, a) in atoms.iter().enumerate() {
        let col = a.st.rp.highlight.or(a.st.rp.shading);
        if let Some(c) = col {
            if matches!(a.kind, AK::LineBreak | AK::PageBreak | AK::ColumnBreak | AK::Float(_)) {
                continue;
            }
            let f = a.font.as_ref().map(|f| f.font.clone()).unwrap_or_else(|| a.st.fm[0].font.clone());
            let y0 = base - f.ascent(a.size) - a.st.raise.max(0.0);
            let h = f.ascent(a.size) + f.descent(a.size);
            items.push(Item::Rect { x: ox + line.xs[k], y: y0, w: a.w + 0.3, h, color: c });
        }
    }

    // Tab leaders.
    for (x, w, leader, st) in &line.tab_leaders {
        let ch = match leader.as_str() {
            "dot" | "middleDot" => '.',
            "hyphen" => '-',
            "underscore" | "heavy" => '_',
            _ => continue,
        };
        let f = &st.fm[0];
        let cw = f.font.advance(ch, st.size).max(1.0);
        let mut gx = x + cw * 0.5;
        let mut glyphs = Vec::new();
        let gid = f.font.glyph(ch).0;
        while gx + cw < x + w - cw * 0.3 {
            glyphs.push((gid, ox + gx));
            gx += cw;
        }
        if !glyphs.is_empty() {
            items.push(Item::Glyphs {
                font: f.font.clone(),
                size: st.size,
                color: st.rp.color.unwrap_or(auto_color(para_bg)),
                y: base,
                glyphs,
                synth_bold: f.synth_bold,
                synth_italic: f.synth_italic,
                scale_x: 1.0,
            });
        }
    }

    // Label.
    for (a, x) in &line.label {
        if let (Some(f), Some(ch)) = (&a.font, a.glyph) {
            let gid = f.font.glyph(ch).0;
            items.push(Item::Glyphs {
                font: f.font.clone(),
                size: a.size,
                color: a.st.rp.color.unwrap_or(auto_color(para_bg)),
                y: base - a.st.raise,
                glyphs: vec![(gid, ox + x)],
                synth_bold: f.synth_bold,
                synth_italic: f.synth_italic,
                scale_x: a.st.rp.scale / 100.0,
            });
        }
    }

    // Glyph runs.
    let mut k = 0;
    while k < atoms.len() {
        let a = &atoms[k];
        match &a.kind {
            AK::Char(_) | AK::FootnoteRef(_) => {
                let f = a.font.clone().unwrap();
                let color = a.st.rp.color.unwrap_or(auto_color(a.st.rp.highlight.or(a.st.rp.shading).or(para_bg)));
                let mut glyphs = Vec::new();
                let mut kk = k;
                while kk < atoms.len() {
                    let b = &atoms[kk];
                    let same = matches!(b.kind, AK::Char(_) | AK::FootnoteRef(_))
                        && b.font.as_ref().is_some_and(|bf| std::sync::Arc::ptr_eq(&bf.font, &f.font) && bf.synth_bold == f.synth_bold && bf.synth_italic == f.synth_italic)
                        && (b.size - a.size).abs() < 0.01
                        && Rc::ptr_eq(&b.st, &a.st);
                    if !same {
                        break;
                    }
                    if let Some(ch) = b.glyph {
                        if ch != '\u{00AD}' {
                            glyphs.push((f.font.glyph(ch).0, ox + line.xs[kk]));
                        }
                    }
                    kk += 1;
                }
                if !glyphs.is_empty() {
                    items.push(Item::Glyphs {
                        font: f.font.clone(),
                        size: a.size,
                        color,
                        y: base - a.st.raise,
                        glyphs,
                        synth_bold: f.synth_bold,
                        synth_italic: f.synth_italic,
                        scale_x: a.st.rp.scale / 100.0,
                    });
                }
                k = kk;
                continue;
            }
            AK::Obj(obj) => {
                let x = ox + line.xs[k];
                let y = base - obj.h - a.st.raise;
                emit_obj(ctx, obj, x, y, items);
            }
            _ => {}
        }
        k += 1;
    }

    // Decorations: underline & strike, grouped by style.
    let last_visible = atoms.iter().rposition(|a| !matches!(a.kind, AK::Space(_) | AK::LineBreak | AK::PageBreak | AK::ColumnBreak | AK::Float(_)));
    let mut k = 0;
    while k < atoms.len() {
        let a = &atoms[k];
        let rp = &a.st.rp;
        let has_u = rp.underline.is_some();
        let has_s = rp.strike || rp.dstrike;
        if !(has_u || has_s) || matches!(a.kind, AK::Float(_) | AK::LineBreak | AK::PageBreak | AK::ColumnBreak) || last_visible.is_none_or(|lv| k > lv) {
            k += 1;
            continue;
        }
        let mut kk = k;
        while kk < atoms.len() && Rc::ptr_eq(&atoms[kk].st, &a.st) && last_visible.is_some_and(|lv| kk <= lv) {
            kk += 1;
        }
        let x0 = ox + line.xs[k];
        let x1 = ox + line.xs[kk - 1] + atoms[kk - 1].w;
        let f = a.font.clone().unwrap_or_else(|| a.st.fm[0].clone());
        let fs = a.size / f.font.upm;
        let color = rp.color.unwrap_or(auto_color(para_bg));
        if let Some(u) = &rp.underline {
            let uc = rp.underline_color.unwrap_or(color);
            let th = (f.font.underline_thickness * fs).max(0.5);
            let uy = base - f.font.underline_pos * fs - a.st.raise;
            let words_only = u == "words";
            let segs: Vec<(f32, f32)> = if words_only {
                let mut v = Vec::new();
                let mut s: Option<f32> = None;
                for q in k..kk {
                    let is_space = matches!(atoms[q].kind, AK::Space(_));
                    let qx = ox + line.xs[q];
                    match (s, is_space) {
                        (None, false) => s = Some(qx),
                        (Some(sx), true) => {
                            v.push((sx, qx));
                            s = None;
                        }
                        _ => {}
                    }
                }
                if let Some(sx) = s {
                    v.push((sx, x1));
                }
                v
            } else {
                vec![(x0, x1)]
            };
            for (sx, ex) in segs {
                match u.as_str() {
                    "double" => {
                        items.push(Item::Rect { x: sx, y: uy, w: ex - sx, h: th, color: uc });
                        items.push(Item::Rect { x: sx, y: uy + th * 2.0, w: ex - sx, h: th, color: uc });
                    }
                    "thick" | "dottedHeavy" | "dashedHeavy" | "wavyHeavy" => {
                        items.push(Item::Rect { x: sx, y: uy, w: ex - sx, h: th * 2.0, color: uc });
                    }
                    "dotted" | "dash" | "dashLong" | "dotDash" | "dotDotDash" => {
                        let step = if u == "dotted" { th * 2.0 } else { th * 6.0 };
                        let mut x = sx;
                        while x < ex {
                            items.push(Item::Rect { x, y: uy, w: (step / 2.0).min(ex - x), h: th, color: uc });
                            x += step;
                        }
                    }
                    _ => items.push(Item::Rect { x: sx, y: uy, w: ex - sx, h: th, color: uc }),
                }
            }
        }
        if has_s {
            let th = (f.font.strike_thickness * fs).max(0.5);
            let sy = base - f.font.strike_pos * fs - a.st.raise;
            if rp.dstrike {
                items.push(Item::Rect { x: x0, y: sy - th, w: x1 - x0, h: th, color });
                items.push(Item::Rect { x: x0, y: sy + th, w: x1 - x0, h: th, color });
            } else {
                items.push(Item::Rect { x: x0, y: sy, w: x1 - x0, h: th, color });
            }
        }
        k = kk;
    }

    // Formatting marks.
    if ctx.opts.show_marks {
        let mark_color = Color(0x46, 0x6E, 0xA0);
        let mf = &m.mark.fm[0];
        let push_mark = |ch: char, x: f32, size: f32, items: &mut Vec<Item>| {
            let fm = if mf.font.has_glyph(ch) { mf.clone() } else { ctx.fonts.fallback_for(ch, false, false).unwrap_or(mf.clone()) };
            items.push(Item::Glyphs { font: fm.font.clone(), size, color: mark_color, y: base, glyphs: vec![(fm.font.glyph(ch).0, x)], synth_bold: false, synth_italic: false, scale_x: 1.0 });
        };
        for (k, a) in atoms.iter().enumerate() {
            match a.kind {
                AK::Space(_) => {
                    let dw = mf.font.advance('·', a.size);
                    push_mark('·', ox + line.xs[k] + (a.w - dw) / 2.0, a.size, items);
                }
                AK::Tab => push_mark('→', ox + line.xs[k] + (a.w - mf.font.advance('→', a.size)).max(0.0) / 2.0, a.size, items),
                AK::LineBreak => push_mark('↵', ox + line.xs[k], a.size, items),
                _ => {}
            }
        }
        if line.last {
            let x = atoms
                .iter()
                .enumerate()
                .filter(|(_, a)| !matches!(a.kind, AK::Float(_)))
                .map(|(k, a)| line.xs[k] + a.w)
                .fold(line.left, f32::max);
            push_mark('¶', ox + x, m.mark.size, items);
        }
    }

    // Caret map.
    let mut carets: Vec<(usize, f32)> = Vec::new();
    let mut start = usize::MAX;
    let mut end = 0usize;
    for (k, a) in atoms.iter().enumerate() {
        if let Some(o) = a.off {
            carets.push((o, ox + line.xs[k]));
            start = start.min(o);
            let next_x = ox + line.xs[k] + a.w;
            end = end.max(o + 1);
            if !matches!(a.kind, AK::LineBreak | AK::PageBreak | AK::ColumnBreak) {
                // Provisional caret after this atom (replaced if another atom follows).
                if carets.iter().all(|(oo, _)| *oo != o + 1) {
                    carets.push((o + 1, next_x));
                }
            }
        }
    }
    carets.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    carets.dedup_by(|b, a| {
        if a.0 == b.0 {
            // Keep the leftmost position for an offset (start of the following atom).
            a.1 = a.1.min(b.1);
            true
        } else {
            false
        }
    });
    if start == usize::MAX {
        // Empty line: caret at the line start.
        let s = if line.start < m.atoms.len() { m.atoms[line.start..].iter().find_map(|a| a.off).unwrap_or(m.para_len) } else { m.para_len };
        let lx = ox + line.left;
        carets = vec![(s, lx)];
        start = s;
        end = s;
    }
    if line.last {
        end = m.para_len;
        if carets.last().is_none_or(|c| c.0 < m.para_len) {
            let x = carets.last().map(|c| c.1).unwrap_or(ox + line.left);
            carets.push((m.para_len, x));
        }
    }
    let lh = line.height;
    infos.push(LineInfo {
        story: story.clone(),
        path: path.to_vec(),
        start,
        end,
        para_len: m.para_len,
        top,
        bottom: top + lh,
        baseline: base,
        carets,
        left: ox + line.left,
        right: ox + line.right,
    });
}

pub fn emit_obj(ctx: &Ctx, obj: &DrawingObj, x: f32, y: f32, items: &mut Vec<Item>) {
    match &obj.kind {
        DrawKind::Image { key, data, crop } => {
            items.push(Item::Image { key: key.clone(), data: data.clone(), x, y, w: obj.w, h: obj.h, crop: *crop });
        }
        DrawKind::Shape { fill, line, text, insets, v_anchor, .. } => {
            if let Some(f) = fill {
                items.push(Item::Rect { x, y, w: obj.w, h: obj.h, color: *f });
            }
            if let Some((c, w)) = line {
                items.push(Item::Rect { x, y, w: obj.w, h: *w, color: *c });
                items.push(Item::Rect { x, y: y + obj.h - w, w: obj.w, h: *w, color: *c });
                items.push(Item::Rect { x, y, w: *w, h: obj.h, color: *c });
                items.push(Item::Rect { x: x + obj.w - w, y, w: *w, h: obj.h, color: *c });
            }
            if !text.is_empty() {
                let inner_w = (obj.w - insets[0] - insets[2]).max(1.0);
                let mut counters = ListCounters::default();
                let mut fr = layout_blocks(ctx, text, inner_w, &Story::Body, &[usize::MAX], None, &mut counters, None);
                let inner_h = obj.h - insets[1] - insets[3];
                let dy = match v_anchor.as_str() {
                    "ctr" => (inner_h - fr.height) / 2.0,
                    "b" => inner_h - fr.height,
                    _ => 0.0,
                };
                fr.translate(x + insets[0], y + insets[1] + dy.max(0.0));
                items.extend(fr.items);
            }
        }
        DrawKind::Placeholder(label) => {
            if obj.w > 0.0 && obj.h > 0.0 {
                items.push(Item::Placeholder { x, y, w: obj.w, h: obj.h, label: label.clone() });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Paragraph borders & shading

#[derive(Clone, PartialEq)]
pub struct BorderKey {
    left: Option<Border>,
    right: Option<Border>,
    top: Option<Border>,
    bottom: Option<Border>,
    ind: (i32, i32),
}

fn visible(b: &Option<Border>) -> Option<&Border> {
    b.as_ref().filter(|b| b.is_visible())
}

/// Draw shading/borders for a paragraph fragment spanning [y0, y1] (text area).
#[allow(clippy::too_many_arguments)]
fn decorate(pp: &ParaProps, ox: f32, width: f32, y0: f32, y1: f32, draw_top: bool, draw_bottom: bool, items: &mut Vec<Item>, insert_at: usize) {
    let lb = visible(&pp.border_left);
    let rb = visible(&pp.border_right);
    let tb = if draw_top { visible(&pp.border_top) } else { None };
    let bb = if draw_bottom { visible(&pp.border_bottom) } else { None };
    if pp.shading.is_none() && lb.is_none() && rb.is_none() && tb.is_none() && bb.is_none() {
        return;
    }
    let x0 = ox + pp.ind_left - lb.map(|b| b.space + b.width).unwrap_or(0.0);
    let x1 = ox + width - pp.ind_right + rb.map(|b| b.space + b.width).unwrap_or(0.0);
    let top = y0 - tb.map(|b| b.space + b.width).unwrap_or(0.0);
    let bottom = y1 + bb.map(|b| b.space + b.width).unwrap_or(0.0);
    let mut v = Vec::new();
    if let Some(s) = pp.shading {
        v.push(Item::Rect { x: x0, y: top, w: x1 - x0, h: bottom - top, color: s });
    }
    let col = |b: &Border| b.color.unwrap_or(Color::BLACK);
    if let Some(b) = tb {
        v.push(Item::Rect { x: x0, y: top, w: x1 - x0, h: b.width, color: col(b) });
    }
    if let Some(b) = bb {
        v.push(Item::Rect { x: x0, y: bottom - b.width, w: x1 - x0, h: b.width, color: col(b) });
    }
    if let Some(b) = lb {
        v.push(Item::Rect { x: x0, y: top, w: b.width, h: bottom - top, color: col(b) });
    }
    if let Some(b) = rb {
        v.push(Item::Rect { x: x1 - b.width, y: top, w: b.width, h: bottom - top, color: col(b) });
    }
    let at = insert_at.min(items.len());
    items.splice(at..at, v);
}

fn border_extra(pp: &ParaProps) -> (f32, f32) {
    let t = visible(&pp.border_top).map(|b| b.space + b.width).unwrap_or(0.0);
    let b = visible(&pp.border_bottom).map(|b| b.space + b.width).unwrap_or(0.0);
    (t, b)
}

// ---------------------------------------------------------------------------
// Non-paginated layout (cells, headers, text boxes, footnotes)

#[allow(clippy::too_many_arguments)]
pub fn layout_blocks(
    ctx: &Ctx,
    blocks: &[Block],
    width: f32,
    story: &Story,
    path_prefix: &[usize],
    tctx: Option<&TableCtx>,
    counters: &mut ListCounters,
    pn: Option<&PageNums>,
) -> Frag {
    let mut fr = Frag::default();
    let mut y = 0.0f32;
    let mut pending_after = 0.0f32;
    let mut prev: Option<(String, bool)> = None;
    fn walk<'b>(blocks: &'b [Block], prefix: &[usize], out: &mut Vec<(Vec<usize>, &'b Block)>) {
        for (i, b) in blocks.iter().enumerate() {
            let mut p = prefix.to_vec();
            p.push(i);
            match b {
                Block::Sdt { content, .. } => walk(content, &p, out),
                _ => out.push((p, b)),
            }
        }
    }
    let mut flat = Vec::new();
    walk(blocks, path_prefix, &mut flat);
    for (path, b) in flat {
        match b {
            Block::Paragraph(p) => {
                let m = measure(ctx, p, story, width, tctx, counters, pn, None, |_, _| LineLimits { left: 0.0, right: 0.0, push: 0.0 });
                let same = prev.as_ref().is_some_and(|(s, _)| *s == m.pp.style_id);
                let prev_ctx = prev.as_ref().is_some_and(|(_, c)| *c);
                let before = if m.pp.contextual && same { 0.0 } else { m.pp.space_before };
                let after_prev = if prev_ctx && same { 0.0 } else { pending_after };
                // Use the larger of space-after and space-before (not their sum).
                y += after_prev.max(before);
                let (bt, bb) = border_extra(&m.pp);
                y += bt;
                let y_start = y;
                let insert_at = fr.items.len();
                for li in 0..m.lines.len() {
                    let cbg = *ctx.cell_bg.borrow();
                    emit_line(ctx, &m, li, 0.0, y, story, &path, cbg, &mut fr.items, &mut fr.lines);
                    y += m.lines[li].height + m.lines[li].y_skip;
                    fr.breaks.push(y);
                }
                decorate(&m.pp, 0.0, width, y_start, y, true, true, &mut fr.items, insert_at);
                y += bb;
                // Floating objects anchored in this paragraph (relative to paragraph).
                for f in &m.floats {
                    if let Some(a) = &f.anchor {
                        let x = a.h_off;
                        let fy = if matches!(a.v_rel.as_str(), "paragraph" | "line") { y_start + a.v_off } else { a.v_off };
                        let page_rel = matches!(a.v_rel.as_str(), "page" | "margin" | "topMargin" | "bottomMargin");
                        let mut v = Vec::new();
                        let (hx, vy) = if page_rel { float_page_pos(ctx, f) } else { (x, fy) };
                        emit_obj(ctx, f, hx, vy, &mut v);
                        for it in v {
                            fr.floats.push((it, page_rel));
                        }
                    }
                }
                pending_after = m.pp.space_after;
                prev = Some((m.pp.style_id.clone(), m.pp.contextual));
            }
            Block::Table(t) => {
                y += pending_after;
                pending_after = 0.0;
                let mut tf = super::table::layout_table_frag(ctx, t, width, story, &path, counters, pn);
                tf.translate(0.0, y);
                y += tf.height;
                fr.items.extend(tf.items);
                fr.lines.extend(tf.lines);
                fr.breaks.extend(tf.breaks);
                fr.floats.extend(tf.floats);
                prev = None;
            }
            _ => {}
        }
    }
    fr.height = y + pending_after;
    fr
}

/// Page-relative position for floats inside headers/footers.
fn float_page_pos(ctx: &Ctx, f: &DrawingObj) -> (f32, f32) {
    let s = ctx.cur_sect.borrow();
    let a = f.anchor.as_ref().unwrap();
    let (fx, fw) = match a.h_rel.as_str() {
        "page" => (0.0, s.page_w),
        "leftMargin" | "insideMargin" => (0.0, s.margin_left),
        "rightMargin" | "outsideMargin" => (s.page_w - s.margin_right, s.margin_right),
        _ => (s.margin_left, s.page_w - s.margin_left - s.margin_right),
    };
    let x = match a.h_align.as_deref() {
        Some("center") => fx + (fw - f.w) / 2.0,
        Some("right") | Some("outside") => fx + fw - f.w,
        Some(_) => fx,
        None => fx + a.h_off,
    };
    let (fy, fh) = match a.v_rel.as_str() {
        "page" => (0.0, s.page_h),
        "topMargin" => (0.0, s.margin_top),
        "bottomMargin" => (s.page_h - s.margin_bottom, s.margin_bottom),
        _ => (s.margin_top, s.page_h - s.margin_top - s.margin_bottom),
    };
    let y = match a.v_align.as_deref() {
        Some("center") => fy + (fh - f.h) / 2.0,
        Some("bottom") | Some("outside") => fy + fh - f.h,
        Some(_) => fy,
        None => fy + a.v_off,
    };
    (x, y)
}

pub fn footnote_frag(ctx: &Ctx, id: &str, width: f32) -> Option<Frag> {
    let blocks = ctx.footnotes.get(id)?;
    *ctx.current_footnote.borrow_mut() = Some(id.to_string());
    let mut counters = ListCounters::default();
    let fr = layout_blocks(ctx, blocks, width, &Story::Footnote(id.to_string()), &[], None, &mut counters, None);
    *ctx.current_footnote.borrow_mut() = None;
    Some(fr)
}

// ---------------------------------------------------------------------------
// Paginated placement in the body flow

fn grid_pitch(flow: &Flow) -> Option<f32> {
    if matches!(flow.sect.grid_type.as_str(), "lines" | "linesAndChars" | "snapToChars") {
        Some(flow.sect.grid_pitch)
    } else {
        None
    }
}

/// Estimate the height of the first lines of a paragraph (for keep-with-next chains).
fn first_lines_height(flow: &Flow, p: &Paragraph, count: usize) -> (f32, bool, f32, f32, f32) {
    let mut counters = flow.counters.clone();
    let m = measure(flow.ctx, p, &Story::Body, flow.col_w(), None, &mut counters, None, grid_pitch(flow), |_, _| LineLimits { left: 0.0, right: 0.0, push: 0.0 });
    let n = if m.pp.widow_control && m.lines.len() > count { count.max(2).min(m.lines.len()) } else { count.min(m.lines.len()) };
    let h: f32 = m.lines[..n].iter().map(|l| l.height).sum();
    let full: f32 = m.height();
    (h, m.pp.keep_next, m.pp.space_before, full, m.pp.space_after)
}

pub fn place_paragraph(flow: &mut Flow, p: &Paragraph, path: Vec<usize>, siblings: &[Block], idx: usize) {
    let ctx = flow.ctx;
    let pp0 = ctx.res.para(p.ppr.as_ref(), None);
    if pp0.page_break_before && !(flow.at_top && flow.pages.len() <= 1 && flow.page().lines.is_empty()) && !flow.at_top {
        flow.page_break();
    }
    // Keep-with-next chain lookahead.
    if pp0.keep_next && !flow.at_top {
        let mut chain_h = 0.0;
        let mut counters = flow.counters.clone();
        let m = measure(ctx, p, &Story::Body, flow.col_w(), None, &mut counters, None, grid_pitch(flow), |_, _| LineLimits { left: 0.0, right: 0.0, push: 0.0 });
        // Spacing between paragraphs follows the max(after, before) rule.
        chain_h += flow.pending_after.max(m.pp.space_before) + m.height();
        let mut prev_after = m.pp.space_after;
        let mut j = idx + 1;
        let mut ok_chain = true;
        let mut guard = 0;
        while j < siblings.len() && guard < 20 {
            guard += 1;
            match &siblings[j] {
                Block::Paragraph(np) => {
                    let (h1, kn, before, full, after) = first_lines_height(flow, np, 1);
                    if kn {
                        chain_h += prev_after.max(before) + full;
                        prev_after = after;
                        j += 1;
                        continue;
                    }
                    chain_h += prev_after.max(before) + h1;
                    break;
                }
                Block::Table(t) => {
                    chain_h += prev_after + super::table::leading_height(flow, t);
                    break;
                }
                _ => {
                    ok_chain = false;
                    break;
                }
            }
        }
        let page_h = flow.body_bottom - flow.col_top;
        if ok_chain && chain_h > flow.avail() + 0.01 && chain_h <= page_h {
            flow.next_column(true);
        }
    }

    let story = Story::Body;
    let same = flow.prev_style.as_ref().is_some_and(|(s, _)| *s == pp0.style_id);
    let prev_ctx = flow.prev_style.as_ref().is_some_and(|(_, c)| *c);
    let mut before = if pp0.contextual && same { 0.0 } else { pp0.space_before };
    let after_prev = if prev_ctx && same { 0.0 } else { flow.pending_after };
    if flow.at_top && flow.natural_break {
        before = 0.0;
    }
    if flow.at_top && flow.section_carry > 0.0 {
        before = (before - flow.section_carry).max(0.0);
    }
    flow.section_carry = 0.0;
    // Use the larger of the previous space-after and this space-before.
    let mut gap = after_prev.max(before);
    let (bt, bb) = border_extra(&pp0);

    // Measure with floats/exclusions on the current page.
    let col_x = flow.col_x();
    let col_w = flow.col_w();
    let pn = PageNums { page: flow.page_number, total: 0, fmt: flow.sect.pg_num_fmt.clone() };
    let mut counters_snapshot = flow.counters.clone();
    let gp = grid_pitch(flow);
    let mut m = {
        let excl = flow.exclusions.clone();
        let y0 = flow.y + gap + bt;
        measure(ctx, p, &story, col_w, None, &mut flow.counters, Some(&pn), gp, |yr, h| limits_for(&excl, col_x, col_w, y0 + yr, h))
    };
    // Position floats anchored here, then re-measure if they introduce exclusions.
    if !m.floats.is_empty() {
        let para_top = flow.y + gap;
        let mut added = false;
        let floats = m.floats.clone();
        for f in &floats {
            if place_float(flow, f, para_top, m.pp.ind_left) {
                added = true;
            }
        }
        if added {
            flow.counters = counters_snapshot.clone();
            let excl = flow.exclusions.clone();
            let y0 = flow.y + gap + bt;
            m = measure(ctx, p, &story, col_w, None, &mut flow.counters, Some(&pn), gp, |yr, h| limits_for(&excl, col_x, col_w, y0 + yr, h));
        }
    }
    counters_snapshot = flow.counters.clone();
    let _ = counters_snapshot;

    let n = m.lines.len();
    // keepLines: move whole paragraph if it doesn't fit but would on a fresh page.
    let total_h = m.height();
    if m.pp.keep_lines && !flow.at_top && gap + bt + total_h > flow.avail() + 0.01 && total_h <= flow.body_bottom - flow.col_top {
        flow.next_column(true);
        gap = 0.0;
    }

    // Outline entry.
    if let Some(lvl) = m.pp.outline_lvl {
        if lvl < 9 {
            let text = p.plain_text();
            if !text.trim().is_empty() {
                let pi = flow.pages.len() - 1;
                flow.outline.push((text.trim().to_string(), lvl, pi, flow.y, path.clone()));
            }
        }
    }

    let mut li = 0usize;
    let mut first_frag = true;
    // Border grouping: identical borders on consecutive paragraphs merge.
    let key = BorderKey {
        left: m.pp.border_left.clone(),
        right: m.pp.border_right.clone(),
        top: m.pp.border_top.clone(),
        bottom: m.pp.border_bottom.clone(),
        ind: (m.pp.ind_left as i32, m.pp.ind_right as i32),
    };
    let grouped_with_prev = flow.last_border_group.as_ref() == Some(&key) && (visible(&key.top).is_some() || visible(&key.left).is_some());
    while li < n {
        let gap_here = if first_frag { gap } else { 0.0 };
        let top_extra = if first_frag && !grouped_with_prev { bt } else { 0.0 };
        let avail = flow.avail() - gap_here - top_extra;
        // How many lines fit?
        let mut k = 0usize;
        let mut acc = 0.0;
        let mut fn_extra = 0.0;
        while li + k < n {
            let l = &m.lines[li + k];
            let mut extra = 0.0;
            for id in &m.footnotes[li + k] {
                if !flow.page_footnotes.contains(id) {
                    if let Some(fr) = footnote_frag(ctx, id, flow.sect.page_w - flow.sect.margin_left - flow.sect.margin_right) {
                        extra += fr.height + if flow.page_footnotes.is_empty() && fn_extra == 0.0 { 12.0 } else { 0.0 };
                    }
                }
            }
            let h = l.height + l.y_skip;
            if acc + h + fn_extra + extra > avail + 0.01 {
                break;
            }
            acc += h;
            fn_extra += extra;
            k += 1;
            if l.forced == Some(AKBreak::Page) || l.forced == Some(AKBreak::Column) {
                break;
            }
        }
        let remaining = n - li;
        let page_top = flow.at_top;
        let ends_forced = k > 0 && matches!(m.lines[li + k - 1].forced, Some(AKBreak::Page) | Some(AKBreak::Column));
        if k < remaining && m.pp.widow_control && n >= 2 && !ends_forced {
            // Orphan: don't leave a single first line at the bottom.
            if li == 0 && k == 1 && !page_top {
                k = 0;
            }
            // Widow: don't carry a single last line over.
            else if remaining - k == 1 && k >= 2 && !(m.lines[li + k - 1].forced.is_some()) {
                k -= 1;
            }
        }
        if k == 0 {
            if page_top {
                k = 1; // Must make progress.
            } else {
                flow.next_column(true);
                // Moved to a new page/column: the inter-paragraph gap no longer applies.
                gap = 0.0;
                continue;
            }
        }
        // Place k lines.
        if first_frag {
            flow.y += gap_here;
        }
        flow.y += top_extra;
        let y_start = flow.y;
        let insert_at = flow.page().items.len();
        let mut forced = None;
        for q in 0..k {
            let l = &m.lines[li + q];
            for id in &m.footnotes[li + q] {
                flow.reserve_footnote(id);
            }
            let y = flow.y;
            let page = flow.pages.last_mut().unwrap();
            emit_line(ctx, &m, li + q, col_x, y, &story, &path, None, &mut page.items, &mut page.lines);
            flow.y += l.height + l.y_skip;
            forced = l.forced;
        }
        li += k;
        let finished = li >= n;
        let y_end = flow.y;
        let page = flow.pages.last_mut().unwrap();
        decorate(&m.pp, col_x, col_w, y_start, y_end, first_frag && !grouped_with_prev, finished, &mut page.items, insert_at);
        if finished {
            flow.y += bb;
        }
        flow.at_top = false;
        first_frag = false;
        match forced {
            Some(AKBreak::Page) if !finished || li <= n => {
                if !finished {
                    flow.page_break();
                    flow.natural_break = false;
                }
            }
            Some(AKBreak::Column) => {
                if !finished {
                    flow.next_column(false);
                }
            }
            _ => {
                if !finished {
                    flow.next_column(true);
                }
            }
        }
    }
    flow.last_border_group = Some(key);
    flow.pending_after = m.pp.space_after;
    flow.prev_style = Some((m.pp.style_id.clone(), m.pp.contextual));
    flow.at_top = false;
    // A paragraph ending in a page/column break: the next block starts on the new page/column,
    // and (like after a natural break) its space-before is suppressed.
    match m.lines.last().and_then(|l| l.forced) {
        Some(AKBreak::Page) => {
            flow.page_break();
            flow.natural_break = true;
        }
        Some(AKBreak::Column) => flow.next_column(true),
        _ => {}
    }
}

fn limits_for(excl: &[Exclusion], col_x: f32, col_w: f32, y: f32, h: f32) -> LineLimits {
    let mut l = LineLimits { left: 0.0, right: 0.0, push: 0.0 };
    let (lx0, lx1) = (col_x, col_x + col_w);
    let mut y = y;
    // Push below full-width exclusions.
    for _ in 0..4 {
        let mut moved = false;
        for e in excl {
            let overlaps = y + h > e.y0 && y < e.y1;
            if !overlaps {
                continue;
            }
            let free_left = e.x0 - lx0;
            let free_right = lx1 - e.x1;
            if e.full || (free_left < 36.0 && free_right < 36.0) {
                l.push += e.y1 - y;
                y = e.y1;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    for e in excl {
        if e.full || !(y + h > e.y0 && y < e.y1) {
            continue;
        }
        let free_left = e.x0 - lx0;
        let free_right = lx1 - e.x1;
        if free_right >= free_left {
            l.left = l.left.max(e.x1 - lx0);
        } else {
            l.right = l.right.max(lx1 - e.x0);
        }
    }
    l
}

/// Position an anchored object on the current page. Returns true if it adds a text exclusion.
fn place_float(flow: &mut Flow, f: &Rc<DrawingObj>, para_top: f32, ind_left: f32) -> bool {
    let Some(a) = &f.anchor else { return false };
    let s = &flow.sect;
    let col_x = flow.col_x();
    let col_w = flow.col_w();
    let (fx, fw) = match a.h_rel.as_str() {
        "page" => (0.0, s.page_w),
        "margin" => (s.margin_left, s.page_w - s.margin_left - s.margin_right),
        "leftMargin" | "insideMargin" => (0.0, s.margin_left),
        "rightMargin" | "outsideMargin" => (s.page_w - s.margin_right, s.margin_right),
        "character" => (col_x + ind_left, col_w - ind_left),
        _ => (col_x, col_w),
    };
    let x = match a.h_align.as_deref() {
        Some("center") => fx + (fw - f.w) / 2.0,
        Some("right") | Some("outside") => fx + fw - f.w,
        Some(_) => fx,
        None => fx + a.h_off,
    };
    let (fy, fh) = match a.v_rel.as_str() {
        "page" => (0.0, s.page_h),
        "margin" => (s.margin_top, s.page_h - s.margin_top - s.margin_bottom),
        "topMargin" => (0.0, s.margin_top),
        "bottomMargin" => (s.page_h - s.margin_bottom, s.margin_bottom),
        _ => (para_top, 0.0),
    };
    let y = match a.v_align.as_deref() {
        Some("center") => fy + (fh - f.h) / 2.0,
        Some("bottom") => fy + fh - f.h,
        Some(_) => fy,
        None => fy + a.v_off,
    };
    let mut v = Vec::new();
    emit_obj(flow.ctx, f, x, y, &mut v);
    let page = flow.page();
    if a.behind {
        page.items.splice(0..0, v);
    } else {
        page.items.extend(v);
    }
    match a.wrap.as_str() {
        "wrapSquare" | "wrapTight" | "wrapThrough" => {
            flow.exclusions.push(Exclusion { x0: x - a.dist[2], y0: y - a.dist[0], x1: x + f.w + a.dist[3], y1: y + f.h + a.dist[1], full: false });
            true
        }
        "wrapTopAndBottom" => {
            flow.exclusions.push(Exclusion { x0: x, y0: y - a.dist[0], x1: x + f.w, y1: y + f.h + a.dist[1], full: true });
            true
        }
        _ => false,
    }
}
