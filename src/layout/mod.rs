//! Faithful DOCX page layout.
//!
//! Produces a display list per page plus line geometry for caret placement
//! and hit testing. The algorithms follow the reference layout rules:
//! GDI line metrics, no collapsing of paragraph spacing, space-before
//! suppression after natural page breaks, widow/orphan and keep rules,
//! default tab stops measured from the column edge, table grids taken from
//! w:tblGrid as saved in the file, etc.

mod drawing;
mod para;
mod table;

pub use drawing::{DrawingObj, DrawKind};

use crate::document::Document;
use crate::fonts::{FontDb, LoadedFont};
use crate::model::Block;
use crate::props::{Color, Resolver, SectionProps, TableCtx};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

pub use para::RStyle;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Story {
    Body,
    HeaderFooter(String),
    Footnote(String),
}

#[derive(Clone)]
pub enum Item {
    Glyphs {
        font: Arc<LoadedFont>,
        size: f32,
        color: Color,
        y: f32,
        glyphs: Vec<(ab_glyph::GlyphId, f32)>,
        synth_bold: bool,
        synth_italic: bool,
        scale_x: f32,
    },
    Rect { x: f32, y: f32, w: f32, h: f32, color: Color },
    Image { key: String, data: Arc<Vec<u8>>, x: f32, y: f32, w: f32, h: f32, crop: [f32; 4] },
    /// A placeholder frame for content we can't draw yet (charts, SmartArt...).
    Placeholder { x: f32, y: f32, w: f32, h: f32, label: String },
}

impl Item {
    pub fn translate(&mut self, dx: f32, dy: f32) {
        match self {
            Item::Glyphs { y, glyphs, .. } => {
                *y += dy;
                for g in glyphs {
                    g.1 += dx;
                }
            }
            Item::Rect { x, y, .. } | Item::Image { x, y, .. } | Item::Placeholder { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
        }
    }
    fn top_bottom(&self) -> (f32, f32) {
        match self {
            Item::Glyphs { y, size, .. } => (*y - size, *y + size * 0.3),
            Item::Rect { y, h, .. } | Item::Image { y, h, .. } | Item::Placeholder { y, h, .. } => (*y, *y + *h),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LineInfo {
    pub story: Story,
    pub path: Vec<usize>,
    pub start: usize,
    pub end: usize,
    pub para_len: usize,
    pub top: f32,
    pub bottom: f32,
    pub baseline: f32,
    pub carets: Vec<(usize, f32)>,
    pub left: f32,
    pub right: f32,
}

impl LineInfo {
    fn translate(&mut self, dx: f32, dy: f32) {
        self.top += dy;
        self.bottom += dy;
        self.baseline += dy;
        self.left += dx;
        self.right += dx;
        for c in &mut self.carets {
            c.1 += dx;
        }
    }

    pub fn caret_x(&self, pos: usize) -> f32 {
        let mut best = self.carets.first().map(|c| c.1).unwrap_or(self.left);
        for (o, x) in &self.carets {
            if *o <= pos {
                best = *x;
            }
        }
        best
    }

    pub fn pos_at_x(&self, x: f32) -> usize {
        let mut best = (f32::MAX, self.start);
        for (i, (o, cx)) in self.carets.iter().enumerate() {
            // Don't place caret after the last position of a wrapped line (it belongs to the next line).
            if *o == self.end && self.end < self.para_len && i + 1 == self.carets.len() && self.carets.len() > 1 {
                continue;
            }
            let d = (cx - x).abs();
            if d < best.0 {
                best = (d, *o);
            }
        }
        best.1
    }
}

#[derive(Clone)]
pub struct Page {
    pub width: f32,
    pub height: f32,
    pub items: Vec<Item>,
    pub lines: Vec<LineInfo>,
    pub number: i32,
    pub section: usize,
    pub background: Option<Color>,
    pub margins: (f32, f32, f32, f32), // top, right, bottom, left
    pub header_area: (f32, f32),
    pub footer_area: (f32, f32),
}

#[derive(Clone, Default)]
pub struct LayoutOptions {
    pub show_marks: bool,
}

pub struct Layout {
    pub pages: Vec<Page>,
    /// Outline entries (heading text, level, page index, y) for the navigation pane.
    pub outline: Vec<(String, u8, usize, f32, Vec<usize>)>,
}

/// A laid-out region that is not paginated (table cell, header, text box).
#[derive(Clone, Default)]
pub struct Frag {
    pub items: Vec<Item>,
    pub lines: Vec<LineInfo>,
    pub height: f32,
    /// y positions (relative) where the fragment may be split across pages.
    pub breaks: Vec<f32>,
    pub floats: Vec<(Item, bool)>,
}

impl Frag {
    pub fn translate(&mut self, dx: f32, dy: f32) {
        for i in &mut self.items {
            i.translate(dx, dy);
        }
        for l in &mut self.lines {
            l.translate(dx, dy);
        }
        for b in &mut self.breaks {
            *b += dy;
        }
        for (f, page_rel) in &mut self.floats {
            if !*page_rel {
                f.translate(dx, dy);
            }
        }
    }

    /// Split at relative y: returns the part below `y` (shifted up to start at 0).
    pub fn split_off(&mut self, y: f32) -> Frag {
        let mut rest = Frag::default();
        let items = std::mem::take(&mut self.items);
        for it in items {
            let (t, b) = it.top_bottom();
            if t >= y - 0.01 || (b > y + 0.5 && matches!(it, Item::Glyphs { .. })) {
                rest.items.push(it);
            } else {
                self.items.push(it);
            }
        }
        let lines = std::mem::take(&mut self.lines);
        for l in lines {
            if l.top >= y - 0.01 || l.bottom > y + 0.5 {
                rest.lines.push(l);
            } else {
                self.lines.push(l);
            }
        }
        rest.breaks = self.breaks.iter().filter(|b| **b > y + 0.01).map(|b| b - y).collect();
        self.breaks.retain(|b| *b <= y + 0.01);
        rest.height = (self.height - y).max(0.0);
        self.height = y;
        // Shift remaining content up so it starts at the top.
        let min_top = rest.lines.iter().map(|l| l.top).fold(f32::MAX, f32::min);
        let shift = if min_top < f32::MAX { min_top.min(y) } else { y };
        rest.translate(0.0, -shift);
        rest.height = (rest.height + y - shift).max(0.0);
        rest
    }
}

/// Floating object exclusion zone (page coordinates).
#[derive(Clone, Debug)]
pub struct Exclusion {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    /// Top-and-bottom wrapping: no text beside it at all.
    pub full: bool,
}

pub struct Ctx<'a> {
    pub doc: &'a Document,
    pub res: Resolver<'a>,
    pub fonts: &'a FontDb,
    pub opts: &'a LayoutOptions,
    style_cache: RefCell<HashMap<(String, usize, String, bool), Rc<RStyle>>>,
    pub footnotes: HashMap<String, Vec<Block>>,
    pub footnote_numbers: RefCell<HashMap<String, usize>>,
    pub current_footnote: RefCell<Option<String>>,
    pub cur_sect: RefCell<SectionProps>,
    pub cell_bg: RefCell<Option<Color>>,
}

impl<'a> Ctx<'a> {
    pub fn new(doc: &'a Document, fonts: &'a FontDb, opts: &'a LayoutOptions) -> Ctx<'a> {
        let mut footnotes = HashMap::new();
        if let Some(path) = doc.package.rels(&doc.main_part).into_iter().find(|r| r.rel_type == crate::opc::REL_FOOTNOTES) {
            let p = crate::opc::Package::resolve_target(&doc.main_part, &path.target);
            if let Some(x) = doc.package.get_xml(&p) {
                for f in x.root.children_named("w:footnote") {
                    if f.attr("w:type").is_none_or(|t| t == "normal") {
                        if let Some(id) = f.attr("w:id") {
                            footnotes.insert(id.to_string(), crate::model::parse_blocks(f));
                        }
                    }
                }
            }
        }
        Ctx {
            doc,
            res: doc.resolver(),
            fonts,
            opts,
            style_cache: RefCell::new(HashMap::new()),
            footnotes,
            footnote_numbers: RefCell::new(HashMap::new()),
            current_footnote: RefCell::new(None),
            cur_sect: RefCell::new(SectionProps::default()),
            cell_bg: RefCell::new(None),
        }
    }

    pub fn current_footnote_number(&self) -> String {
        let id = self.current_footnote.borrow().clone();
        match id {
            Some(id) => {
                let mut m = self.footnote_numbers.borrow_mut();
                let next = m.len() + 1;
                m.entry(id).or_insert(next).to_string()
            }
            None => String::new(),
        }
    }

    pub fn tctx_key(t: Option<&TableCtx>) -> String {
        match t {
            None => String::new(),
            Some(t) => format!("{}:{}", t.style_id.as_deref().unwrap_or(""), t.cond.types.join(",")),
        }
    }

    pub fn story_part(&self, story: &Story) -> String {
        match story {
            Story::Body => self.doc.main_part.clone(),
            Story::HeaderFooter(rid) => self.doc.header_footers.get(rid).map(|h| h.part.clone()).unwrap_or_default(),
            Story::Footnote(_) => self
                .doc
                .package
                .rels(&self.doc.main_part)
                .into_iter()
                .find(|r| r.rel_type == crate::opc::REL_FOOTNOTES)
                .map(|r| crate::opc::Package::resolve_target(&self.doc.main_part, &r.target))
                .unwrap_or_default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Pagination

struct PageCtx {
    hdr: [Option<String>; 3],
    ftr: [Option<String>; 3],
}

pub(crate) struct Flow<'c, 'a> {
    pub ctx: &'c Ctx<'a>,
    pub pages: Vec<Page>,
    pub sect: SectionProps,
    pub sect_idx: usize,
    pub cols: Vec<(f32, f32)>,
    pub col: usize,
    pub y: f32,
    pub col_top: f32,
    pub body_top: f32,
    pub body_bottom: f32,
    pub at_top: bool,
    pub natural_break: bool,
    pub pending_after: f32,
    pub prev_style: Option<(String, bool)>,
    pub exclusions: Vec<Exclusion>,
    pub counters: crate::props::ListCounters,
    pub page_in_section: usize,
    pub page_number: i32,
    pub footnote_height: f32,
    pub page_footnotes: Vec<String>,
    hf: PageCtx,
    pub outline: Vec<(String, u8, usize, f32, Vec<usize>)>,
    pub last_border_group: Option<para::BorderKey>,
    /// Space-after carried across a next-page section break (subtracted from the
    /// first paragraph's space-before on the new page).
    pub section_carry: f32,
}

impl<'c, 'a> Flow<'c, 'a> {
    pub fn page(&mut self) -> &mut Page {
        self.pages.last_mut().unwrap()
    }

    pub fn col_x(&self) -> f32 {
        self.sect.margin_left + self.sect.gutter + self.cols[self.col.min(self.cols.len() - 1)].0
    }

    pub fn col_w(&self) -> f32 {
        self.cols[self.col.min(self.cols.len() - 1)].1
    }

    pub fn avail(&self) -> f32 {
        self.body_bottom - self.footnote_height - self.y
    }

    fn header_frag(&self, rid: &Option<String>, page_no: i32, total: i32) -> Option<Frag> {
        let rid = rid.as_ref()?;
        let hf = self.ctx.doc.header_footers.get(rid)?;
        let w = self.sect.page_w - self.sect.margin_left - self.sect.margin_right - self.sect.gutter;
        let mut counters = crate::props::ListCounters::default();
        let pc = para::PageNums { page: page_no, total, fmt: self.sect.pg_num_fmt.clone() };
        Some(para::layout_blocks(self.ctx, &hf.blocks, w, &Story::HeaderFooter(rid.clone()), &[], None, &mut counters, Some(&pc)))
    }

    fn hf_choice(&self, page_no: i32, first_of_section: bool) -> (Option<String>, Option<String>) {
        let idx = if first_of_section && self.sect.title_pg {
            1
        } else if self.ctx.doc.settings.even_and_odd_headers && page_no % 2 == 0 {
            2
        } else {
            0
        };
        (self.hf.hdr[idx].clone(), self.hf.ftr[idx].clone())
    }

    pub fn new_page(&mut self) {
        self.page_in_section += 1;
        self.page_number += 1;
        let first = self.page_in_section == 1;
        let (h, f) = self.hf_choice(self.page_number, first);
        let s = &self.sect;
        let mut body_top = s.margin_top;
        let mut body_bottom = s.page_h - s.margin_bottom;
        if let Some(fr) = self.header_frag(&h, self.page_number, 0) {
            if !s.top_fixed {
                body_top = body_top.max(s.margin_header + fr.height);
            }
        }
        if let Some(fr) = self.header_frag(&f, self.page_number, 0) {
            if !s.bottom_fixed {
                body_bottom = body_bottom.min(s.page_h - s.margin_footer - fr.height);
            }
        }
        let bg = self
            .ctx
            .doc
            .shell
            .child("w:background")
            .and_then(|b| self.ctx.doc.theme_ctx().color_of(b, "w:color", "w:themeColor", "w:themeTint", "w:themeShade"))
            .flatten();
        self.pages.push(Page {
            width: s.page_w,
            height: s.page_h,
            items: Vec::new(),
            lines: Vec::new(),
            number: self.page_number,
            section: self.sect_idx,
            background: bg,
            margins: (s.margin_top, s.margin_right, s.margin_bottom, s.margin_left),
            header_area: (s.margin_header, body_top),
            footer_area: (body_bottom, s.page_h - s.margin_footer),
        });
        self.body_top = body_top;
        self.body_bottom = body_bottom;
        self.y = body_top;
        self.col_top = body_top;
        self.col = 0;
        self.at_top = true;
        self.pending_after = 0.0;
        self.exclusions.clear();
        self.footnote_height = 0.0;
        self.page_footnotes.clear();
    }

    /// Advance to the next column or page. `natural` = caused by overflow.
    pub fn next_column(&mut self, natural: bool) {
        self.draw_footnotes();
        if self.col + 1 < self.cols.len() {
            self.col += 1;
            self.y = self.col_top;
        } else {
            self.draw_col_separators();
            self.new_page();
        }
        self.at_top = true;
        self.natural_break = natural;
        self.pending_after = 0.0;
        self.last_border_group = None;
    }

    pub fn page_break(&mut self) {
        self.draw_footnotes();
        self.draw_col_separators();
        self.new_page();
        self.natural_break = false;
        self.last_border_group = None;
    }

    fn draw_col_separators(&mut self) {
        if self.sect.col_sep && self.cols.len() > 1 {
            let (top, bottom) = (self.col_top, self.body_bottom);
            let xs: Vec<f32> = self.cols.windows(2).map(|w| self.sect.margin_left + self.sect.gutter + (w[0].0 + w[0].1 + w[1].0) / 2.0).collect();
            for x in xs {
                self.page().items.push(Item::Rect { x: x - 0.25, y: top, w: 0.5, h: bottom - top, color: Color::BLACK });
            }
        }
    }

    fn draw_footnotes(&mut self) {
        if self.page_footnotes.is_empty() {
            return;
        }
        let ids = std::mem::take(&mut self.page_footnotes);
        let w = self.sect.page_w - self.sect.margin_left - self.sect.margin_right - self.sect.gutter;
        let x = self.sect.margin_left + self.sect.gutter;
        let mut y = self.body_bottom - self.footnote_height;
        // Separator: 144pt rule (the default).
        self.page().items.push(Item::Rect { x, y: y + 6.0, w: 144.0, h: 0.5, color: Color::BLACK });
        y += 12.0;
        for id in ids {
            if let Some(mut fr) = para::footnote_frag(self.ctx, &id, w) {
                fr.translate(x, y);
                y += fr.height;
                let p = self.page();
                p.items.extend(fr.items);
                p.lines.extend(fr.lines);
            }
        }
        self.footnote_height = 0.0;
    }

    pub fn reserve_footnote(&mut self, id: &str) -> bool {
        if self.page_footnotes.iter().any(|f| f == id) {
            return true;
        }
        let w = self.sect.page_w - self.sect.margin_left - self.sect.margin_right - self.sect.gutter;
        let h = para::footnote_frag(self.ctx, id, w).map(|f| f.height).unwrap_or(0.0);
        let extra = if self.page_footnotes.is_empty() { 12.0 } else { 0.0 };
        self.footnote_height += h + extra;
        self.page_footnotes.push(id.to_string());
        true
    }

    fn begin_section(&mut self, idx: usize, sp: SectionProps, first: bool) {
        let prev_size = (self.sect.page_w, self.sect.page_h);
        let kind = sp.kind.clone();
        // Inherit header/footer references not specified by this section.
        for i in 0..3 {
            if sp.headers[i].is_some() {
                self.hf.hdr[i] = sp.headers[i].clone();
            }
            if sp.footers[i].is_some() {
                self.hf.ftr[i] = sp.footers[i].clone();
            }
        }
        self.sect = sp;
        self.sect_idx = idx;
        self.cols = self.sect.columns();
        *self.ctx.cur_sect.borrow_mut() = self.sect.clone();
        let restart = self.sect.pg_num_start;
        let same_size = (prev_size.0 - self.sect.page_w).abs() < 0.5 && (prev_size.1 - self.sect.page_h).abs() < 0.5;
        if first {
            self.page_in_section = 0;
            if let Some(s) = restart {
                self.page_number = s - 1;
            }
            self.new_page();
            return;
        }
        if kind == "continuous" && same_size && !self.pages.is_empty() {
            // New columns start below the content so far on this page.
            self.page_in_section = self.page_in_section.max(1);
            if self.col > 0 {
                // Use the furthest content as start.
                let cur = self.y;
                self.y = self.page().lines.iter().map(|l| l.bottom).fold(cur, f32::max);
            }
            self.col = 0;
            // New columns start below the previous paragraph's space-after.
            self.col_top = self.y + self.pending_after;
            return;
        }
        let carry = self.pending_after;
        self.draw_footnotes();
        self.draw_col_separators();
        self.page_in_section = 0;
        if let Some(s) = restart {
            self.page_number = s - 1;
        }
        self.new_page();
        let n = self.page_number;
        let need_blank = (kind == "evenPage" && n % 2 == 1) || (kind == "oddPage" && n % 2 == 0);
        if need_blank {
            self.page_in_section = 0;
            self.new_page();
        }
        self.natural_break = false;
        self.section_carry = carry;
    }
}

pub fn layout_document(doc: &Document, fonts: &FontDb, opts: &LayoutOptions) -> Layout {
    let ctx = Ctx::new(doc, fonts, opts);
    let sections = doc.sections();
    let mut flow = Flow {
        ctx: &ctx,
        pages: Vec::new(),
        sect: sections[0].1.clone(),
        sect_idx: 0,
        cols: sections[0].1.columns(),
        col: 0,
        y: 0.0,
        col_top: 0.0,
        body_top: 0.0,
        body_bottom: 0.0,
        at_top: true,
        natural_break: false,
        pending_after: 0.0,
        prev_style: None,
        exclusions: Vec::new(),
        counters: crate::props::ListCounters::default(),
        page_in_section: 0,
        page_number: 0,
        footnote_height: 0.0,
        page_footnotes: Vec::new(),
        hf: PageCtx { hdr: [None, None, None], ftr: [None, None, None] },
        outline: Vec::new(),
        last_border_group: None,
        section_carry: 0.0,
    };
    flow.begin_section(0, sections[0].1.clone(), true);

    let mut sec_i = 0usize;
    let n = doc.body.len();
    let mut i = 0usize;
    while i < n {
        let block = &doc.body[i];
        match block {
            Block::Paragraph(p) => {
                para::place_paragraph(&mut flow, p, vec![i], &doc.body, i);
            }
            Block::Table(t) => {
                table::place_table(&mut flow, t, vec![i]);
                flow.pending_after = 0.0;
                flow.prev_style = None;
            }
            Block::Sdt { content, .. } => {
                for (j, b) in content.iter().enumerate() {
                    match b {
                        Block::Paragraph(p) => para::place_paragraph(&mut flow, p, vec![i, j], content, j),
                        Block::Table(t) => {
                            table::place_table(&mut flow, t, vec![i, j]);
                            flow.pending_after = 0.0;
                        }
                        _ => {}
                    }
                }
            }
            Block::Other(_) => {}
        }
        // Section boundary after this block?
        if sec_i < sections.len() - 1 && sections[sec_i].0 == i {
            sec_i += 1;
            flow.begin_section(sec_i, sections[sec_i].1.clone(), false);
        }
        i += 1;
    }
    flow.draw_footnotes();
    flow.draw_col_separators();

    // Headers & footers with final page numbers.
    let total = flow.pages.len() as i32;
    let mut hf_state = PageCtx { hdr: [None, None, None], ftr: [None, None, None] };
    let mut last_sec = usize::MAX;
    let mut page_in_sec = 0;
    let mut pages = std::mem::take(&mut flow.pages);
    for page in pages.iter_mut() {
        if page.section != last_sec {
            let sp = &sections[page.section.min(sections.len() - 1)].1;
            for k in 0..3 {
                if sp.headers[k].is_some() {
                    hf_state.hdr[k] = sp.headers[k].clone();
                }
                if sp.footers[k].is_some() {
                    hf_state.ftr[k] = sp.footers[k].clone();
                }
            }
            last_sec = page.section;
            page_in_sec = 0;
        }
        page_in_sec += 1;
        let sp = sections[page.section.min(sections.len() - 1)].1.clone();
        flow.sect = sp.clone();
        *ctx.cur_sect.borrow_mut() = sp.clone();
        flow.hf = PageCtx { hdr: hf_state.hdr.clone(), ftr: hf_state.ftr.clone() };
        let (h, f) = flow.hf_choice(page.number, page_in_sec == 1);
        let x = sp.margin_left + sp.gutter;
        let mut extra_items = Vec::new();
        let mut extra_lines = Vec::new();
        if let Some(mut fr) = flow.header_frag(&h, page.number, total) {
            fr.translate(x, sp.margin_header);
            for (it, _) in fr.floats.drain(..) {
                extra_items.push(it);
            }
            extra_items.extend(fr.items);
            extra_lines.extend(fr.lines);
        }
        if let Some(mut fr) = flow.header_frag(&f, page.number, total) {
            let y = sp.page_h - sp.margin_footer - fr.height;
            fr.translate(x, y);
            for (it, _) in fr.floats.drain(..) {
                extra_items.push(it);
            }
            extra_items.extend(fr.items);
            extra_lines.extend(fr.lines);
        }
        // Headers/footers are drawn beneath body text.
        extra_items.append(&mut page.items);
        page.items = extra_items;
        page.lines.extend(extra_lines);
        // Page borders.
        for (side, b) in &sp.page_borders {
            if !b.is_visible() {
                continue;
            }
            let c = b.color.unwrap_or(Color::BLACK);
            let (l, t, r, btm) = (sp.margin_left - b.space - b.width, sp.margin_top - b.space - b.width, sp.page_w - sp.margin_right + b.space, sp.page_h - sp.margin_bottom + b.space);
            let it = match side.as_str() {
                "top" => Item::Rect { x: l, y: t, w: r - l + b.width, h: b.width, color: c },
                "bottom" => Item::Rect { x: l, y: btm, w: r - l + b.width, h: b.width, color: c },
                "left" => Item::Rect { x: l, y: t, w: b.width, h: btm - t + b.width, color: c },
                "right" => Item::Rect { x: r, y: t, w: b.width, h: btm - t + b.width, color: c },
                _ => continue,
            };
            page.items.push(it);
        }
    }
    Layout { pages, outline: flow.outline }
}
