//! Editing operations on a `Document`.
//!
//! Positions address a paragraph by its block path (see `layout::LineInfo`)
//! plus a character offset in the paragraph's flattened text.

use crate::document::Document;
use crate::layout::Story;
use crate::model::{Block, CellItem, Inline, Paragraph, RowItem, Run, RunItem, Table};
use crate::props::{PPR_ORDER, RPR_ORDER, SECTPR_ORDER};
use crate::xml::Element;
use std::cmp::Ordering;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Pos {
    pub story: Story,
    pub path: Vec<usize>,
    pub off: usize,
}

impl Pos {
    pub fn body(path: Vec<usize>, off: usize) -> Pos {
        Pos { story: Story::Body, path, off }
    }
    pub fn cmp_doc(&self, o: &Pos) -> Ordering {
        self.path.cmp(&o.path).then(self.off.cmp(&o.off))
    }
}

pub fn blocks_mut<'a>(doc: &'a mut Document, story: &Story) -> Option<&'a mut Vec<Block>> {
    match story {
        Story::Body => Some(&mut doc.body),
        Story::HeaderFooter(rid) => doc.header_footers.get_mut(rid).map(|h| {
            h.dirty = true;
            &mut h.blocks
        }),
        Story::Footnote(_) => None,
    }
}

pub fn blocks_ref<'a>(doc: &'a Document, story: &Story) -> Option<&'a Vec<Block>> {
    match story {
        Story::Body => Some(&doc.body),
        Story::HeaderFooter(rid) => doc.header_footers.get(rid).map(|h| &h.blocks),
        Story::Footnote(_) => None,
    }
}

pub fn para_in<'a>(blocks: &'a [Block], path: &[usize]) -> Option<&'a Paragraph> {
    let (&i, rest) = path.split_first()?;
    match blocks.get(i)? {
        Block::Paragraph(p) if rest.is_empty() => Some(p),
        Block::Table(t) if rest.len() >= 3 => {
            let RowItem::Row(r) = t.rows.get(rest[0])? else { return None };
            let CellItem::Cell(c) = r.cells.get(rest[1])? else { return None };
            para_in(&c.blocks, &rest[2..])
        }
        Block::Sdt { content, .. } => para_in(content, rest),
        _ => None,
    }
}

pub fn para_in_mut<'a>(blocks: &'a mut [Block], path: &[usize]) -> Option<&'a mut Paragraph> {
    let (&i, rest) = path.split_first()?;
    match blocks.get_mut(i)? {
        Block::Paragraph(p) if rest.is_empty() => Some(p),
        Block::Table(t) if rest.len() >= 3 => {
            let RowItem::Row(r) = t.rows.get_mut(rest[0])? else { return None };
            let CellItem::Cell(c) = r.cells.get_mut(rest[1])? else { return None };
            para_in_mut(&mut c.blocks, &rest[2..])
        }
        Block::Sdt { content, .. } => para_in_mut(content, rest),
        _ => None,
    }
}

/// The block list containing the block at `path` (i.e. the parent container).
fn container_mut<'a>(blocks: &'a mut Vec<Block>, path: &[usize]) -> Option<&'a mut Vec<Block>> {
    if path.len() <= 1 {
        return Some(blocks);
    }
    let i = path[0];
    match blocks.get_mut(i)? {
        Block::Table(t) if path.len() >= 4 => {
            let RowItem::Row(r) = t.rows.get_mut(path[1])? else { return None };
            let CellItem::Cell(c) = r.cells.get_mut(path[2])? else { return None };
            container_mut(&mut c.blocks, &path[3..])
        }
        Block::Sdt { content, .. } => container_mut(content, &path[1..]),
        _ => None,
    }
}

pub fn para<'a>(doc: &'a Document, pos: &Pos) -> Option<&'a Paragraph> {
    para_in(blocks_ref(doc, &pos.story)?, &pos.path)
}

pub fn para_mut<'a>(doc: &'a mut Document, story: &Story, path: &[usize]) -> Option<&'a mut Paragraph> {
    para_in_mut(blocks_mut(doc, story)?, path)
}

/// The `w:drawing` at character offset `off` of the paragraph at `path`, if there is one.
pub fn drawing_at_mut<'a>(doc: &'a mut Document, story: &Story, path: &[usize], off: usize) -> Option<&'a mut Element> {
    match drawing_item_mut(doc, story, path, off)? {
        RunItem::Drawing(d) | RunItem::Other(d) => Some(d),
        _ => None,
    }
}

/// The run item holding the drawing at `off`: a bare w:drawing, or an
/// `mc:AlternateContent` with a drawing inside (how Word stores shapes and WordArt).
pub fn drawing_item_mut<'a>(doc: &'a mut Document, story: &Story, path: &[usize], off: usize) -> Option<&'a mut RunItem> {
    let p = para_mut(doc, story, path)?;
    let (run_path, item, _) = p.locate(off, false)?;
    let it = p.run_at_path_mut(&run_path)?.items.get_mut(item)?;
    match it {
        RunItem::Drawing(_) => Some(it),
        RunItem::Other(e) if e.name == "mc:AlternateContent" && e.find_local("drawing").is_some() => Some(it),
        _ => None,
    }
}

/// All paragraph paths in document order.
pub fn para_paths(blocks: &[Block]) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    fn rec(blocks: &[Block], prefix: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        for (i, b) in blocks.iter().enumerate() {
            prefix.push(i);
            match b {
                Block::Paragraph(_) => out.push(prefix.clone()),
                Block::Table(t) => {
                    for (ri, r) in t.rows.iter().enumerate() {
                        if let RowItem::Row(r) = r {
                            for (ci, c) in r.cells.iter().enumerate() {
                                if let CellItem::Cell(c) = c {
                                    prefix.push(ri);
                                    prefix.push(ci);
                                    rec(&c.blocks, prefix, out);
                                    prefix.pop();
                                    prefix.pop();
                                }
                            }
                        }
                    }
                }
                Block::Sdt { content, .. } => rec(content, prefix, out),
                Block::Other(_) => {}
            }
            prefix.pop();
        }
    }
    rec(blocks, &mut Vec::new(), &mut out);
    out
}

pub fn para_len(doc: &Document, story: &Story, path: &[usize]) -> usize {
    blocks_ref(doc, story).and_then(|b| para_in(b, path)).map(|p| p.char_len()).unwrap_or(0)
}

pub fn start_of_doc(doc: &Document) -> Pos {
    let p = para_paths(&doc.body).into_iter().next().unwrap_or_else(|| vec![0]);
    Pos::body(p, 0)
}

pub fn end_of_doc(doc: &Document) -> Pos {
    let p = para_paths(&doc.body).into_iter().last().unwrap_or_else(|| vec![0]);
    let l = para_len(doc, &Story::Body, &p);
    Pos::body(p, l)
}

pub fn next_para(doc: &Document, story: &Story, path: &[usize]) -> Option<Vec<usize>> {
    let paths = para_paths(blocks_ref(doc, story)?);
    let i = paths.iter().position(|p| p == path)?;
    paths.get(i + 1).cloned()
}

pub fn prev_para(doc: &Document, story: &Story, path: &[usize]) -> Option<Vec<usize>> {
    let paths = para_paths(blocks_ref(doc, story)?);
    let i = paths.iter().position(|p| p == path)?;
    if i == 0 { None } else { paths.get(i - 1).cloned() }
}

pub fn clamp(doc: &Document, pos: &Pos) -> Pos {
    let mut p = pos.clone();
    if para(doc, &p).is_none() {
        let paths = blocks_ref(doc, &p.story).map(|b| para_paths(b)).unwrap_or_default();
        let best = paths.iter().filter(|q| **q <= p.path).last().or(paths.first()).cloned();
        match best {
            Some(b) => {
                p.path = b;
            }
            None => return start_of_doc(doc),
        }
    }
    let len = para(doc, &p).map(|x| x.char_len()).unwrap_or(0);
    p.off = p.off.min(len);
    p
}

// ---------------------------------------------------------------------------
// Text editing

pub fn insert_text(doc: &mut Document, pos: &Pos, text: &str, rpr: Option<Element>) -> Pos {
    let mut cur = pos.clone();
    let parts: Vec<&str> = text.split('\n').collect();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            cur = split_paragraph(doc, &cur);
        }
        let mut segs = part.split('\t').peekable();
        while let Some(seg) = segs.next() {
            let seg = seg.trim_end_matches('\r');
            if !seg.is_empty() {
                if let Some(p) = para_mut(doc, &cur.story, &cur.path) {
                    p.insert_text(cur.off, seg, rpr.clone());
                    cur.off += seg.chars().count();
                }
            }
            if segs.peek().is_some() {
                if let Some(p) = para_mut(doc, &cur.story, &cur.path) {
                    p.insert_item(cur.off, RunItem::Tab, rpr.clone());
                    cur.off += 1;
                }
            }
        }
    }
    cur
}

pub fn insert_item(doc: &mut Document, pos: &Pos, item: RunItem, rpr: Option<Element>) -> Pos {
    if let Some(p) = para_mut(doc, &pos.story, &pos.path) {
        p.insert_item(pos.off, item, rpr);
        p.cleanup();
    }
    Pos { off: pos.off + 1, ..pos.clone() }
}

/// Enter: split the paragraph at pos, returns the start of the new paragraph.
pub fn split_paragraph(doc: &mut Document, pos: &Pos) -> Pos {
    let styles = doc.styles.clone();
    let Some(container) = blocks_mut(doc, &pos.story).and_then(|b| container_mut(b, &pos.path)) else { return pos.clone() };
    let idx = *pos.path.last().unwrap();
    let Some(Block::Paragraph(p)) = container.get_mut(idx) else { return pos.clone() };
    let len = p.char_len();
    let at_end = pos.off >= len;
    // Empty list item + Enter ⇒ end the list (standard behaviour).
    if len == 0 && p.ppr.as_ref().is_some_and(|x| x.child("w:numPr").is_some()) {
        p.ppr.as_mut().unwrap().remove_child("w:numPr");
        return pos.clone();
    }
    let mut tail = p.split_off(pos.off);
    // The section break (if any) stays with the last paragraph.
    if let Some(sp) = p.ppr.as_mut().and_then(|x| {
        let s = x.child("w:sectPr").cloned();
        x.remove_child("w:sectPr");
        s
    }) {
        tail.ppr_mut().set_child(sp, PPR_ORDER);
    }
    if let Some(ppr) = tail.ppr.as_mut() {
        if at_end {
            // Next style.
            if let Some(sid) = ppr.child("w:pStyle").and_then(|s| s.attr("w:val")).map(str::to_string) {
                if let Some(next) = styles.get(&sid).and_then(|s| s.next.clone()) {
                    if next != sid {
                        ppr.set_child(Element::new("w:pStyle").with_attr("w:val", next.as_str()), PPR_ORDER);
                    }
                }
            }
        }
        ppr.remove_child("w:pageBreakBefore");
    }
    // Carry the run formatting of the end of the head into an empty tail's mark.
    if tail.char_len() == 0 {
        if let Some(rpr) = p.rpr_at(pos.off) {
            let mark = tail.ppr_mut().ensure_child("w:rPr", PPR_ORDER);
            *mark = Element { name: "w:rPr".into(), attrs: vec![], children: rpr.children.clone() };
            mark.remove_child("w:rStyle");
        }
    }
    p.attrs.retain(|(k, _)| !k.starts_with("w14:"));
    tail.attrs.clear();
    container.insert(idx + 1, Block::Paragraph(tail));
    let mut path = pos.path.clone();
    *path.last_mut().unwrap() += 1;
    Pos { story: pos.story.clone(), path, off: 0 }
}

/// Delete everything between a and b (document order).
pub fn delete_range(doc: &mut Document, a: &Pos, b: &Pos) -> Pos {
    let (a, b) = if a.cmp_doc(b) == Ordering::Greater { (b.clone(), a.clone()) } else { (a.clone(), b.clone()) };
    if a.story != b.story {
        return a;
    }
    if a.path == b.path {
        if let Some(p) = para_mut(doc, &a.story, &a.path) {
            p.delete_range(a.off, b.off);
        }
        return a;
    }
    let same_container = a.path.len() == b.path.len() && a.path[..a.path.len() - 1] == b.path[..b.path.len() - 1];
    if same_container {
        let ia = *a.path.last().unwrap();
        let ib = *b.path.last().unwrap();
        let Some(container) = blocks_mut(doc, &a.story).and_then(|bl| container_mut(bl, &a.path)) else { return a };
        let mut tail = match container.get_mut(ib) {
            Some(Block::Paragraph(p)) => {
                let mut t = p.clone();
                t.delete_range(0, b.off);
                t
            }
            _ => return a,
        };
        container.drain(ia + 1..=ib);
        if let Some(Block::Paragraph(p)) = container.get_mut(ia) {
            let len = p.char_len();
            p.delete_range(a.off, len);
            // Keep the later paragraph's section break.
            if let Some(sp) = tail.ppr.as_mut().and_then(|x| {
                let s = x.child("w:sectPr").cloned();
                x.remove_child("w:sectPr");
                s
            }) {
                p.ppr_mut().set_child(sp, PPR_ORDER);
            }
            p.append(tail);
        }
        return a;
    }
    // Range crosses container boundaries (e.g. into a table): delete text in each paragraph individually.
    let paths = blocks_ref(doc, &a.story).map(|b| para_paths(b)).unwrap_or_default();
    for p in paths.iter().filter(|p| **p >= a.path && **p <= b.path) {
        let len = para_len(doc, &a.story, p);
        let (s, e) = if *p == a.path { (a.off, len) } else if *p == b.path { (0, b.off) } else { (0, len) };
        if let Some(pp) = para_mut(doc, &a.story, p) {
            pp.delete_range(s, e);
        }
    }
    a
}

pub fn backspace(doc: &mut Document, pos: &Pos) -> Pos {
    if pos.off > 0 {
        let a = Pos { off: pos.off - 1, ..pos.clone() };
        return delete_range(doc, &a, pos);
    }
    // At paragraph start: remove list numbering / indentation first.
    if let Some(p) = para_mut(doc, &pos.story, &pos.path) {
        if let Some(ppr) = p.ppr.as_mut() {
            if ppr.child("w:numPr").is_some() {
                ppr.remove_child("w:numPr");
                return pos.clone();
            }
        }
    }
    // Merge with previous paragraph if in the same container.
    let idx = *pos.path.last().unwrap();
    if idx == 0 {
        return pos.clone();
    }
    let mut prev_path = pos.path.clone();
    *prev_path.last_mut().unwrap() -= 1;
    let prev_is_para = blocks_ref(doc, &pos.story).and_then(|b| para_in(b, &prev_path)).is_some();
    if !prev_is_para {
        return pos.clone();
    }
    let l = para_len(doc, &pos.story, &prev_path);
    let a = Pos { story: pos.story.clone(), path: prev_path, off: l };
    delete_range(doc, &a, pos)
}

pub fn delete_forward(doc: &mut Document, pos: &Pos) -> Pos {
    let len = para_len(doc, &pos.story, &pos.path);
    if pos.off < len {
        let b = Pos { off: pos.off + 1, ..pos.clone() };
        return delete_range(doc, pos, &b);
    }
    let mut next = pos.path.clone();
    *next.last_mut().unwrap() += 1;
    if blocks_ref(doc, &pos.story).and_then(|b| para_in(b, &next)).is_some() {
        let b = Pos { story: pos.story.clone(), path: next, off: 0 };
        return delete_range(doc, pos, &b);
    }
    pos.clone()
}

pub fn selected_text(doc: &Document, a: &Pos, b: &Pos) -> String {
    let (a, b) = if a.cmp_doc(b) == Ordering::Greater { (b, a) } else { (a, b) };
    let Some(blocks) = blocks_ref(doc, &a.story) else { return String::new() };
    let mut out = String::new();
    for p in para_paths(blocks).iter().filter(|p| **p >= a.path && **p <= b.path) {
        let text: Vec<char> = para_in(blocks, p).map(|x| x.plain_text()).unwrap_or_default().chars().collect();
        let s = if *p == a.path { a.off.min(text.len()) } else { 0 };
        let e = if *p == b.path { b.off.min(text.len()) } else { text.len() };
        if !out.is_empty() || (*p != a.path) {
            if *p != a.path {
                out.push('\n');
            }
        }
        // Line breaks become newlines; picture/shape placeholders are left out.
        out.extend(text[s..e.max(s)].iter().filter(|c| **c != '\u{FFFC}').map(|c| if *c == '\u{0b}' { '\n' } else { *c }));
    }
    out
}

/// Finds the boundaries of the word around `off`, for double-click selection.
pub fn word_at(p: &Paragraph, off: usize) -> (usize, usize) {
    let chars: Vec<char> = p.plain_text().chars().collect();
    if chars.is_empty() {
        return (0, 0);
    }
    let off = off.min(chars.len().saturating_sub(1));
    let is_word = |c: char| c.is_alphanumeric() || c == '_' || c == '\'';
    let kind = is_word(chars[off]);
    let mut s = off;
    while s > 0 && is_word(chars[s - 1]) == kind && !chars[s - 1].is_whitespace() {
        s -= 1;
    }
    let mut e = off;
    while e < chars.len() && is_word(chars[e]) == kind && !chars[e].is_whitespace() {
        e += 1;
    }
    // Double-click selection includes trailing spaces.
    while e < chars.len() && chars[e] == ' ' {
        e += 1;
    }
    (s, e)
}

pub fn word_left(p: &Paragraph, off: usize) -> usize {
    let chars: Vec<char> = p.plain_text().chars().collect();
    let mut i = off.min(chars.len());
    while i > 0 && chars[i - 1].is_whitespace() {
        i -= 1;
    }
    while i > 0 && !chars[i - 1].is_whitespace() {
        i -= 1;
    }
    i
}

pub fn word_right(p: &Paragraph, off: usize) -> usize {
    let chars: Vec<char> = p.plain_text().chars().collect();
    let mut i = off;
    while i < chars.len() && !chars[i].is_whitespace() {
        i += 1;
    }
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    i
}

// ---------------------------------------------------------------------------
// Formatting

/// Apply `f` to run properties across the selection.
pub fn format_runs(doc: &mut Document, a: &Pos, b: &Pos, f: &dyn Fn(&mut Element)) {
    let (a, b) = if a.cmp_doc(b) == Ordering::Greater { (b.clone(), a.clone()) } else { (a.clone(), b.clone()) };
    let paths = blocks_ref(doc, &a.story).map(|bl| para_paths(bl)).unwrap_or_default();
    for p in paths.iter().filter(|p| **p >= a.path && **p <= b.path) {
        let len = para_len(doc, &a.story, p);
        let s = if *p == a.path { a.off } else { 0 };
        let e = if *p == b.path { b.off } else { len };
        if let Some(pp) = para_mut(doc, &a.story, p) {
            pp.format_range(s, e, f);
            // Whole paragraph selected: also format the paragraph mark.
            if s == 0 && e >= len {
                let mark = pp.ppr_mut().ensure_child("w:rPr", PPR_ORDER);
                f(mark);
            }
        }
    }
}

/// Apply `f` to the pPr of every paragraph touched by the selection.
pub fn format_paras(doc: &mut Document, a: &Pos, b: &Pos, f: &dyn Fn(&mut Element)) {
    let (a, b) = if a.cmp_doc(b) == Ordering::Greater { (b.clone(), a.clone()) } else { (a.clone(), b.clone()) };
    let paths = blocks_ref(doc, &a.story).map(|bl| para_paths(bl)).unwrap_or_default();
    for p in paths.iter().filter(|p| **p >= a.path && **p <= b.path) {
        if let Some(pp) = para_mut(doc, &a.story, p) {
            f(pp.ppr_mut());
        }
    }
}

pub fn set_toggle(rpr: &mut Element, name: &str, on: bool) {
    let cs = format!("{}Cs", name);
    rpr.set_child(if on { Element::new(name) } else { Element::new(name).with_attr("w:val", "0") }, RPR_ORDER);
    if name == "w:b" || name == "w:i" {
        rpr.set_child(if on { Element::new(cs.as_str()) } else { Element::new(cs.as_str()).with_attr("w:val", "0") }, RPR_ORDER);
    }
}

pub fn set_font(rpr: &mut Element, family: &str) {
    rpr.set_child(
        Element::new("w:rFonts").with_attr("w:ascii", family).with_attr("w:hAnsi", family).with_attr("w:cs", family).with_attr("w:eastAsia", family),
        RPR_ORDER,
    );
}

pub fn set_size(rpr: &mut Element, pt: f32) {
    let hp = ((pt * 2.0).round() as i32).clamp(2, 3276).to_string();
    rpr.set_child(Element::new("w:sz").with_attr("w:val", hp.as_str()), RPR_ORDER);
    rpr.set_child(Element::new("w:szCs").with_attr("w:val", hp.as_str()), RPR_ORDER);
}

pub fn set_color(rpr: &mut Element, hex: Option<&str>) {
    match hex {
        Some(h) => rpr.set_child(Element::new("w:color").with_attr("w:val", h), RPR_ORDER),
        None => rpr.set_child(Element::new("w:color").with_attr("w:val", "auto"), RPR_ORDER),
    }
}

pub fn set_highlight(rpr: &mut Element, name: Option<&str>) {
    match name {
        Some(n) => rpr.set_child(Element::new("w:highlight").with_attr("w:val", n), RPR_ORDER),
        None => rpr.remove_child("w:highlight"),
    }
}

pub fn set_underline(rpr: &mut Element, on: bool) {
    rpr.set_child(Element::new("w:u").with_attr("w:val", if on { "single" } else { "none" }), RPR_ORDER);
}

/// Set an underline style (`single`, `double`, `thick`, `dotted`, `dash`, `wave`, `words`...),
/// keeping any underline colour. `None` writes `w:val="none"` so styles can't bring it back.
pub fn set_underline_style(rpr: &mut Element, style: Option<&str>) {
    let color = rpr.child("w:u").and_then(|u| u.attr("w:color")).map(str::to_string);
    let mut u = Element::new("w:u").with_attr("w:val", style.unwrap_or("none"));
    if let (Some(c), Some(_)) = (color, style) {
        u = u.with_attr("w:color", c);
    }
    rpr.set_child(u, RPR_ORDER);
}

/// Set the underline colour (`None` = follow the text colour). Turns a single underline on if there is none.
pub fn set_underline_color(rpr: &mut Element, hex: Option<&str>) {
    let style = rpr.child("w:u").and_then(|u| u.attr("w:val")).filter(|v| *v != "none").unwrap_or("single").to_string();
    let mut u = Element::new("w:u").with_attr("w:val", style);
    if let Some(h) = hex {
        u = u.with_attr("w:color", h);
    }
    rpr.set_child(u, RPR_ORDER);
}

/// Character effects that can't be combined: turning one on turns the others in its group off,
/// matching the reference Font dialog.
fn effect_conflicts(name: &str) -> &'static [&'static str] {
    match name {
        "w:strike" => &["w:dstrike"],
        "w:dstrike" => &["w:strike"],
        "w:caps" => &["w:smallCaps"],
        "w:smallCaps" => &["w:caps"],
        "w:emboss" => &["w:imprint", "w:outline", "w:shadow"],
        "w:imprint" => &["w:emboss", "w:outline", "w:shadow"],
        "w:outline" | "w:shadow" => &["w:emboss", "w:imprint"],
        _ => &[],
    }
}

/// Toggle a character effect (`w:outline`, `w:shadow`, `w:emboss`, `w:imprint`, `w:caps`,
/// `w:smallCaps`, `w:strike`, `w:dstrike`), clearing the effects it excludes.
pub fn set_effect(rpr: &mut Element, name: &str, on: bool) {
    if on {
        for other in effect_conflicts(name) {
            if rpr.child(other).is_some() {
                set_toggle(rpr, other, false);
            }
        }
    }
    set_toggle(rpr, name, on);
}

/// Remove the decorative effects (outline, shadow, emboss, engrave) and leave the rest alone.
pub fn clear_effects(rpr: &mut Element) {
    for n in ["w:outline", "w:shadow", "w:emboss", "w:imprint"] {
        rpr.remove_child(n);
    }
}

pub fn set_vert_align(rpr: &mut Element, v: Option<&str>) {
    match v {
        Some(v) => rpr.set_child(Element::new("w:vertAlign").with_attr("w:val", v), RPR_ORDER),
        None => rpr.remove_child("w:vertAlign"),
    }
}

pub fn set_jc(ppr: &mut Element, jc: &str) {
    ppr.set_child(Element::new("w:jc").with_attr("w:val", jc), PPR_ORDER);
}

pub fn set_style(ppr: &mut Element, id: &str) {
    ppr.set_child(Element::new("w:pStyle").with_attr("w:val", id), PPR_ORDER);
}

pub fn set_spacing(ppr: &mut Element, before: Option<f32>, after: Option<f32>, line: Option<(f32, &str)>) {
    let sp = ppr.ensure_child("w:spacing", PPR_ORDER);
    if let Some(b) = before {
        sp.set_attr("w:before", ((b * 20.0).round() as i32).to_string());
        sp.remove_attr("w:beforeAutospacing");
        sp.remove_attr("w:beforeLines");
    }
    if let Some(a) = after {
        sp.set_attr("w:after", ((a * 20.0).round() as i32).to_string());
        sp.remove_attr("w:afterAutospacing");
        sp.remove_attr("w:afterLines");
    }
    if let Some((v, rule)) = line {
        let raw = if rule == "auto" { (v * 240.0).round() as i32 } else { (v * 20.0).round() as i32 };
        sp.set_attr("w:line", raw.to_string());
        sp.set_attr("w:lineRule", rule);
    }
}

pub fn set_indent(ppr: &mut Element, left: Option<f32>, right: Option<f32>) {
    let ind = ppr.ensure_child("w:ind", PPR_ORDER);
    if let Some(l) = left {
        ind.remove_attr("w:start");
        ind.set_attr("w:left", ((l.max(-1584.0) * 20.0).round() as i32).to_string());
    }
    if let Some(r) = right {
        ind.remove_attr("w:end");
        ind.set_attr("w:right", ((r * 20.0).round() as i32).to_string());
    }
}

pub fn set_numbering(ppr: &mut Element, num_id: Option<&str>, ilvl: usize) {
    match num_id {
        Some(id) => {
            let np = Element::new("w:numPr")
                .with_child(Element::new("w:ilvl").with_attr("w:val", ilvl.to_string()))
                .with_child(Element::new("w:numId").with_attr("w:val", id));
            ppr.set_child(np, PPR_ORDER);
            ppr.remove_child("w:ind");
            if ppr.child("w:pStyle").is_none() {
                ppr.set_child(Element::new("w:pStyle").with_attr("w:val", "ListParagraph"), PPR_ORDER);
            }
        }
        None => {
            ppr.remove_child("w:numPr");
            if ppr.child("w:pStyle").and_then(|s| s.attr("w:val")) == Some("ListParagraph") {
                ppr.remove_child("w:pStyle");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Section (page setup) edits

pub fn set_page_size(sect: &mut Element, w_pt: f32, h_pt: f32, landscape: bool) {
    let mut e = Element::new("w:pgSz").with_attr("w:w", ((w_pt * 20.0).round() as i32).to_string()).with_attr("w:h", ((h_pt * 20.0).round() as i32).to_string());
    if landscape {
        e.set_attr("w:orient", "landscape");
    }
    sect.set_child(e, SECTPR_ORDER);
}

pub fn set_margins(sect: &mut Element, top: f32, right: f32, bottom: f32, left: f32) {
    let m = sect.ensure_child("w:pgMar", SECTPR_ORDER);
    for (k, v) in [("w:top", top), ("w:right", right), ("w:bottom", bottom), ("w:left", left)] {
        m.set_attr(k, ((v * 20.0).round() as i32).to_string());
    }
    for (k, v) in [("w:header", 720), ("w:footer", 720), ("w:gutter", 0)] {
        if m.attr(k).is_none() {
            m.set_attr(k, v.to_string());
        }
    }
}

pub fn set_columns(sect: &mut Element, n: usize) {
    let c = Element::new("w:cols").with_attr("w:space", "720");
    let c = if n > 1 { c.with_attr("w:num", n.to_string()) } else { c };
    sect.set_child(c, SECTPR_ORDER);
}

// ---------------------------------------------------------------------------
// Insertions

pub fn new_table(rows: usize, cols: usize, total_width_twips: i32) -> Table {
    let cw = total_width_twips / cols.max(1) as i32;
    let tblpr = Element::new("w:tblPr")
        .with_child(Element::new("w:tblStyle").with_attr("w:val", "TableGrid"))
        .with_child(Element::new("w:tblW").with_attr("w:w", "0").with_attr("w:type", "auto"))
        .with_child(Element::new("w:tblLook").with_attr("w:val", "04A0").with_attr("w:firstRow", "1").with_attr("w:lastRow", "0").with_attr("w:firstColumn", "1").with_attr("w:lastColumn", "0").with_attr("w:noHBand", "0").with_attr("w:noVBand", "1"));
    let mut t = Table { tblpr: Some(tblpr), grid: vec![cw; cols], ..Default::default() };
    for _ in 0..rows {
        let mut r = crate::model::Row::default();
        for _ in 0..cols {
            let tcpr = Element::new("w:tcPr").with_child(Element::new("w:tcW").with_attr("w:w", cw.to_string()).with_attr("w:type", "dxa"));
            r.cells.push(CellItem::Cell(crate::model::Cell { tcpr: Some(tcpr), blocks: vec![Block::Paragraph(Paragraph::new())] }));
        }
        t.rows.push(RowItem::Row(r));
    }
    t
}

/// Insert a block-level element after the paragraph at pos (splitting it if needed).
pub fn insert_block_at(doc: &mut Document, pos: &Pos, block: Block) -> Pos {
    let len = para_len(doc, &pos.story, &pos.path);
    let mut at = pos.clone();
    if pos.off > 0 && pos.off < len {
        at = split_paragraph(doc, pos);
        *at.path.last_mut().unwrap() -= 1;
    }
    let Some(container) = blocks_mut(doc, &pos.story).and_then(|b| container_mut(b, &at.path)) else { return pos.clone() };
    let idx = *at.path.last().unwrap();
    let insert_idx = if pos.off == 0 && len > 0 { idx } else { idx + 1 };
    container.insert(insert_idx, block);
    // Ensure a paragraph follows (required after a table at the end).
    if insert_idx + 1 >= container.len() {
        container.push(Block::Paragraph(Paragraph::new()));
    }
    let mut path = at.path.clone();
    *path.last_mut().unwrap() = insert_idx;
    path.push(0);
    path.push(0);
    path.push(0);
    Pos { story: pos.story.clone(), path, off: 0 }
}

pub fn image_drawing(rid: &str, w_pt: f32, h_pt: f32, id: u32, name: &str) -> Element {
    let cx = ((w_pt * 12700.0) as i64).to_string();
    let cy = ((h_pt * 12700.0) as i64).to_string();
    let xml = format!(
        r#"<w:drawing xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{cx}" cy="{cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="{id}" name="{name}"/><wp:cNvGraphicFramePr><a:graphicFrameLocks noChangeAspect="1"/></wp:cNvGraphicFramePr><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:nvPicPr><pic:cNvPr id="{id}" name="{name}"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="{rid}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing>"#
    );
    let mut e = crate::xml::parse_str(&xml).unwrap().root;
    // Namespaces are declared on the document root; drop the local declarations.
    e.attrs.retain(|(k, _)| !k.starts_with("xmlns"));
    e
}

pub fn make_run(text: &str, rpr: Option<Element>) -> Inline {
    Inline::Run(Run { rpr, items: vec![RunItem::Text(text.to_string())], attrs: vec![] })
}
