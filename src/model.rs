//! WordprocessingML document model.
//!
//! Text content (paragraphs, runs, tables) is typed so it can be edited, while
//! property bags (`w:pPr`, `w:rPr`, `w:tblPr`, `w:tcPr`, `w:sectPr`, ...) are kept
//! as raw XML elements. That way every property in the file — including the
//! ones we don't lay out yet — is preserved on save, and the layout engine
//! reads them through typed accessors in `props.rs`.

use crate::xml::{Element, Node};

#[derive(Clone, Debug)]
pub enum Block {
    Paragraph(Paragraph),
    Table(Table),
    /// Block-level structured document tag (content control).
    Sdt { pr: Element, end_pr: Option<Element>, content: Vec<Block> },
    /// Anything else at block level (bookmarks, customXml, w:sectPr in body, ...).
    Other(Element),
}

#[derive(Clone, Debug, Default)]
pub struct Paragraph {
    pub ppr: Option<Element>,
    pub inlines: Vec<Inline>,
    /// Attributes on w:p (w14:paraId, w:rsidR, ...).
    pub attrs: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
pub enum Inline {
    Run(Run),
    /// A wrapper element with inline children: w:hyperlink, w:ins, w:del,
    /// w:smartTag, w:fldSimple, w:sdt (via sdtContent), w:customXml, w:moveTo, ...
    Container { shell: Element, children: Vec<Inline> },
    /// Inline-level unknown or non-visual element (bookmarkStart, proofErr, commentRangeStart, m:oMath, ...).
    Other(Element),
}

#[derive(Clone, Debug, Default)]
pub struct Run {
    pub rpr: Option<Element>,
    pub items: Vec<RunItem>,
    pub attrs: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
pub enum RunItem {
    Text(String),
    /// w:delText (text inside a tracked deletion)
    DelText(String),
    Tab,
    /// w:br; kind is None for a text-wrapping line break, or "page"/"column".
    Break { kind: Option<String>, clear: Option<String> },
    /// w:cr
    CarriageReturn,
    NoBreakHyphen,
    SoftHyphen,
    Sym { font: String, ch: char },
    FldChar { kind: String, raw: Element },
    InstrText(String),
    /// w:drawing (DrawingML) — kept raw, interpreted by layout.
    Drawing(Element),
    /// Footnote/endnote references and other run-level elements.
    Other(Element),
}

#[derive(Clone, Debug, Default)]
pub struct Table {
    pub tblpr: Option<Element>,
    pub grid: Vec<i32>, // gridCol widths in twips
    pub grid_raw: Option<Element>,
    pub rows: Vec<RowItem>,
    pub attrs: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
pub enum RowItem {
    Row(Row),
    Other(Element),
}

#[derive(Clone, Debug, Default)]
pub struct Row {
    pub trpr: Option<Element>,
    pub tblpr_ex: Option<Element>,
    pub cells: Vec<CellItem>,
    pub attrs: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
pub enum CellItem {
    Cell(Cell),
    Other(Element),
}

#[derive(Clone, Debug, Default)]
pub struct Cell {
    pub tcpr: Option<Element>,
    pub blocks: Vec<Block>,
}

impl Row {
    pub fn cells(&self) -> impl Iterator<Item = &Cell> {
        self.cells.iter().filter_map(|c| match c {
            CellItem::Cell(c) => Some(c),
            _ => None,
        })
    }
    pub fn cells_mut(&mut self) -> impl Iterator<Item = &mut Cell> {
        self.cells.iter_mut().filter_map(|c| match c {
            CellItem::Cell(c) => Some(c),
            _ => None,
        })
    }
}

impl Table {
    pub fn rows(&self) -> impl Iterator<Item = &Row> {
        self.rows.iter().filter_map(|r| match r {
            RowItem::Row(r) => Some(r),
            _ => None,
        })
    }
    pub fn rows_mut(&mut self) -> impl Iterator<Item = &mut Row> {
        self.rows.iter_mut().filter_map(|r| match r {
            RowItem::Row(r) => Some(r),
            _ => None,
        })
    }
}

// ---------------------------------------------------------------------------
// Parsing from XML

pub fn parse_blocks(container: &Element) -> Vec<Block> {
    let mut out = Vec::new();
    for e in container.elements() {
        out.push(parse_block(e));
    }
    out
}

pub fn parse_block(e: &Element) -> Block {
    match e.name.as_str() {
        "w:p" => Block::Paragraph(parse_paragraph(e)),
        "w:tbl" => Block::Table(parse_table(e)),
        "w:sdt" => {
            let pr = e.child("w:sdtPr").cloned().unwrap_or_else(|| Element::new("w:sdtPr"));
            let end_pr = e.child("w:sdtEndPr").cloned();
            let content = e.child("w:sdtContent").map(parse_blocks).unwrap_or_default();
            Block::Sdt { pr, end_pr, content }
        }
        _ => Block::Other(e.clone()),
    }
}

pub fn parse_paragraph(e: &Element) -> Paragraph {
    let mut p = Paragraph { ppr: None, inlines: Vec::new(), attrs: e.attrs.clone() };
    for c in e.elements() {
        if c.name == "w:pPr" {
            p.ppr = Some(c.clone());
        } else {
            p.inlines.push(parse_inline(c));
        }
    }
    p
}

const INLINE_CONTAINERS: &[&str] = &[
    "w:hyperlink", "w:ins", "w:del", "w:smartTag", "w:fldSimple", "w:customXml", "w:moveTo", "w:moveFrom", "w:dir", "w:bdo",
];

fn parse_inline(c: &Element) -> Inline {
    if c.name == "w:r" {
        return Inline::Run(parse_run(c));
    }
    if INLINE_CONTAINERS.contains(&c.name.as_str()) {
        let mut shell = Element::new(c.name.clone());
        shell.attrs = c.attrs.clone();
        let mut children = Vec::new();
        for cc in c.elements() {
            // Keep property children (customXmlPr, smartTagPr) in the shell.
            if cc.name.ends_with("Pr") && cc.name != "w:rPr" {
                shell.children.push(Node::Element(cc.clone()));
            } else {
                children.push(parse_inline(cc));
            }
        }
        return Inline::Container { shell, children };
    }
    if c.name == "w:sdt" {
        // Inline content control: shell keeps sdtPr/sdtEndPr, children from sdtContent.
        let mut shell = Element::new("w:sdt");
        shell.attrs = c.attrs.clone();
        let mut children = Vec::new();
        for cc in c.elements() {
            if cc.name == "w:sdtContent" {
                children.extend(cc.elements().map(parse_inline));
            } else {
                shell.children.push(Node::Element(cc.clone()));
            }
        }
        return Inline::Container { shell, children };
    }
    if c.name == "mc:AlternateContent" {
        // Choose a branch we can display, keep the whole thing for writing.
        return Inline::Other(c.clone());
    }
    Inline::Other(c.clone())
}

pub fn parse_run(e: &Element) -> Run {
    let mut r = Run { rpr: None, items: Vec::new(), attrs: e.attrs.clone() };
    for c in e.elements() {
        let item = match c.name.as_str() {
            "w:rPr" => {
                r.rpr = Some(c.clone());
                continue;
            }
            "w:t" => RunItem::Text(c.text()),
            "w:delText" => RunItem::DelText(c.text()),
            "w:tab" => RunItem::Tab,
            "w:br" => RunItem::Break {
                kind: c.attr("w:type").filter(|t| *t != "textWrapping").map(str::to_string),
                clear: c.attr("w:clear").map(str::to_string),
            },
            "w:cr" => RunItem::CarriageReturn,
            "w:noBreakHyphen" => RunItem::NoBreakHyphen,
            "w:softHyphen" => RunItem::SoftHyphen,
            "w:sym" => {
                let font = c.attr("w:font").unwrap_or_default().to_string();
                let code = c.attr("w:char").and_then(|h| u32::from_str_radix(h, 16).ok()).unwrap_or(0x20);
                // Symbol fonts are often written in the F000 private range.
                let ch = char::from_u32(code).unwrap_or(' ');
                RunItem::Sym { font, ch }
            }
            "w:fldChar" => RunItem::FldChar { kind: c.attr("w:fldCharType").unwrap_or_default().to_string(), raw: c.clone() },
            "w:instrText" => RunItem::InstrText(c.text()),
            "w:drawing" => RunItem::Drawing(c.clone()),
            "mc:AlternateContent" => {
                // Prefer the DrawingML choice when present.
                if let Some(d) = c.find_local("drawing").filter(|d| d.name == "w:drawing") {
                    RunItem::Other(wrap_alt(c, d))
                } else {
                    RunItem::Other(c.clone())
                }
            }
            _ => RunItem::Other(c.clone()),
        };
        r.items.push(item);
    }
    r
}

/// Marker: AlternateContent that contains a drawing we can render.
fn wrap_alt(alt: &Element, _drawing: &Element) -> Element {
    alt.clone()
}

pub fn parse_table(e: &Element) -> Table {
    let mut t = Table { attrs: e.attrs.clone(), ..Default::default() };
    for c in e.elements() {
        match c.name.as_str() {
            "w:tblPr" => t.tblpr = Some(c.clone()),
            "w:tblGrid" => {
                t.grid = c.children_named("w:gridCol").map(|g| g.attr("w:w").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) as i32).collect();
                t.grid_raw = Some(c.clone());
            }
            "w:tr" => t.rows.push(RowItem::Row(parse_row(c))),
            _ => t.rows.push(RowItem::Other(c.clone())),
        }
    }
    t
}

fn parse_row(e: &Element) -> Row {
    let mut r = Row { attrs: e.attrs.clone(), ..Default::default() };
    for c in e.elements() {
        match c.name.as_str() {
            "w:trPr" => r.trpr = Some(c.clone()),
            "w:tblPrEx" => r.tblpr_ex = Some(c.clone()),
            "w:tc" => {
                let mut cell = Cell::default();
                for cc in c.elements() {
                    if cc.name == "w:tcPr" {
                        cell.tcpr = Some(cc.clone());
                    } else {
                        cell.blocks.push(parse_block(cc));
                    }
                }
                r.cells.push(CellItem::Cell(cell));
            }
            "w:sdt" => {
                // Cell-level content control: unwrap cells, keep editing simple.
                if let Some(content) = c.child("w:sdtContent") {
                    for tc in content.children_named("w:tc") {
                        let mut cell = Cell::default();
                        for cc in tc.elements() {
                            if cc.name == "w:tcPr" {
                                cell.tcpr = Some(cc.clone());
                            } else {
                                cell.blocks.push(parse_block(cc));
                            }
                        }
                        r.cells.push(CellItem::Cell(cell));
                    }
                } else {
                    r.cells.push(CellItem::Other(c.clone()));
                }
            }
            _ => r.cells.push(CellItem::Other(c.clone())),
        }
    }
    r
}

// ---------------------------------------------------------------------------
// Serialization back to XML

pub fn blocks_to_xml(blocks: &[Block], parent: &mut Element) {
    for b in blocks {
        parent.children.push(Node::Element(block_to_xml(b)));
    }
}

pub fn block_to_xml(b: &Block) -> Element {
    match b {
        Block::Paragraph(p) => paragraph_to_xml(p),
        Block::Table(t) => table_to_xml(t),
        Block::Sdt { pr, end_pr, content } => {
            let mut e = Element::new("w:sdt").with_child(pr.clone());
            if let Some(ep) = end_pr {
                e = e.with_child(ep.clone());
            }
            let mut c = Element::new("w:sdtContent");
            blocks_to_xml(content, &mut c);
            e.with_child(c)
        }
        Block::Other(e) => e.clone(),
    }
}

pub fn paragraph_to_xml(p: &Paragraph) -> Element {
    let mut e = Element::new("w:p");
    e.attrs = p.attrs.clone();
    if let Some(ppr) = &p.ppr {
        if !ppr.children.is_empty() || !ppr.attrs.is_empty() {
            e.children.push(Node::Element(ppr.clone()));
        }
    }
    for i in &p.inlines {
        e.children.push(Node::Element(inline_to_xml(i)));
    }
    e
}

fn inline_to_xml(i: &Inline) -> Element {
    match i {
        Inline::Run(r) => run_to_xml(r),
        Inline::Container { shell, children } => {
            let mut e = shell.clone();
            if shell.name == "w:sdt" {
                let mut c = Element::new("w:sdtContent");
                for ch in children {
                    c.children.push(Node::Element(inline_to_xml(ch)));
                }
                e.children.push(Node::Element(c));
            } else {
                for ch in children {
                    e.children.push(Node::Element(inline_to_xml(ch)));
                }
            }
            e
        }
        Inline::Other(e) => e.clone(),
    }
}

fn text_el(name: &str, s: &str) -> Element {
    let mut t = Element::new(name);
    if s.starts_with(char::is_whitespace) || s.ends_with(char::is_whitespace) || s.contains("  ") {
        t.set_attr("xml:space", "preserve");
    }
    t.with_text(s)
}

pub fn run_to_xml(r: &Run) -> Element {
    let mut e = Element::new("w:r");
    e.attrs = r.attrs.clone();
    if let Some(rpr) = &r.rpr {
        if !rpr.children.is_empty() {
            e.children.push(Node::Element(rpr.clone()));
        }
    }
    for it in &r.items {
        let c = match it {
            RunItem::Text(s) => text_el("w:t", s),
            RunItem::DelText(s) => text_el("w:delText", s),
            RunItem::Tab => Element::new("w:tab"),
            RunItem::Break { kind, clear } => {
                let mut b = Element::new("w:br");
                if let Some(k) = kind {
                    b.set_attr("w:type", k.as_str());
                }
                if let Some(c) = clear {
                    b.set_attr("w:clear", c.as_str());
                }
                b
            }
            RunItem::CarriageReturn => Element::new("w:cr"),
            RunItem::NoBreakHyphen => Element::new("w:noBreakHyphen"),
            RunItem::SoftHyphen => Element::new("w:softHyphen"),
            RunItem::Sym { font, ch } => Element::new("w:sym").with_attr("w:font", font.as_str()).with_attr("w:char", format!("{:04X}", *ch as u32)),
            RunItem::FldChar { raw, .. } => raw.clone(),
            RunItem::InstrText(s) => text_el("w:instrText", s),
            RunItem::Drawing(d) => d.clone(),
            RunItem::Other(o) => o.clone(),
        };
        e.children.push(Node::Element(c));
    }
    e
}

pub fn table_to_xml(t: &Table) -> Element {
    let mut e = Element::new("w:tbl");
    e.attrs = t.attrs.clone();
    if let Some(p) = &t.tblpr {
        e.children.push(Node::Element(p.clone()));
    }
    let mut grid = t.grid_raw.clone().unwrap_or_else(|| Element::new("w:tblGrid"));
    // Rebuild gridCol children from widths, preserving tblGridChange etc.
    let extra: Vec<Node> = grid.children.iter().filter(|n| !matches!(n, Node::Element(e) if e.name == "w:gridCol")).cloned().collect();
    grid.children = t.grid.iter().map(|w| Node::Element(Element::new("w:gridCol").with_attr("w:w", w.to_string()))).collect();
    grid.children.extend(extra);
    e.children.push(Node::Element(grid));
    for r in &t.rows {
        match r {
            RowItem::Row(r) => {
                let mut re = Element::new("w:tr");
                re.attrs = r.attrs.clone();
                if let Some(x) = &r.tblpr_ex {
                    re.children.push(Node::Element(x.clone()));
                }
                if let Some(x) = &r.trpr {
                    re.children.push(Node::Element(x.clone()));
                }
                for c in &r.cells {
                    match c {
                        CellItem::Cell(c) => {
                            let mut ce = Element::new("w:tc");
                            if let Some(p) = &c.tcpr {
                                ce.children.push(Node::Element(p.clone()));
                            }
                            blocks_to_xml(&c.blocks, &mut ce);
                            // A cell must end with a paragraph.
                            if !matches!(c.blocks.last(), Some(Block::Paragraph(_))) {
                                ce.children.push(Node::Element(Element::new("w:p")));
                            }
                            re.children.push(Node::Element(ce));
                        }
                        CellItem::Other(o) => re.children.push(Node::Element(o.clone())),
                    }
                }
                e.children.push(Node::Element(re));
            }
            RowItem::Other(o) => e.children.push(Node::Element(o.clone())),
        }
    }
    e
}

// ---------------------------------------------------------------------------
// Paragraph text model for editing.
//
// A paragraph's editable content is the flattened sequence of visible "atoms":
// every character of w:t counts as one position, as do tabs, breaks, symbols and
// drawings. Field instructions and deleted text count zero.

/// Path to a leaf run inside a paragraph: indices through containers.
pub type RunPath = Vec<usize>;

impl Paragraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_style(style: &str) -> Self {
        let mut p = Self::default();
        p.set_style(Some(style));
        p
    }

    pub fn style_id(&self) -> Option<&str> {
        self.ppr.as_ref().and_then(|p| p.child("w:pStyle")).and_then(|s| s.attr("w:val"))
    }

    pub fn set_style(&mut self, style: Option<&str>) {
        let ppr = self.ppr.get_or_insert_with(|| Element::new("w:pPr"));
        match style {
            Some(s) => ppr.set_child(Element::new("w:pStyle").with_attr("w:val", s), crate::props::PPR_ORDER),
            None => ppr.remove_child("w:pStyle"),
        }
    }

    pub fn ppr_mut(&mut self) -> &mut Element {
        self.ppr.get_or_insert_with(|| Element::new("w:pPr"))
    }

    /// Visit each leaf run with its path, in document order.
    pub fn visit_runs<'a>(&'a self, f: &mut dyn FnMut(&RunPath, &'a Run, bool)) {
        fn rec<'a>(items: &'a [Inline], path: &mut RunPath, deleted: bool, f: &mut dyn FnMut(&RunPath, &'a Run, bool)) {
            for (i, it) in items.iter().enumerate() {
                path.push(i);
                match it {
                    Inline::Run(r) => f(path, r, deleted),
                    Inline::Container { shell, children } => {
                        let del = deleted || shell.name == "w:del" || shell.name == "w:moveFrom";
                        rec(children, path, del, f)
                    }
                    Inline::Other(_) => {}
                }
                path.pop();
            }
        }
        rec(&self.inlines, &mut Vec::new(), false, f);
    }

    pub fn run_at_path_mut(&mut self, path: &[usize]) -> Option<&mut Run> {
        let mut items = &mut self.inlines;
        for (depth, &i) in path.iter().enumerate() {
            let last = depth + 1 == path.len();
            match items.get_mut(i)? {
                Inline::Run(r) if last => return Some(r),
                Inline::Container { children, .. } if !last => items = children,
                _ => return None,
            }
        }
        None
    }

    pub fn inlines_at_mut(&mut self, container_path: &[usize]) -> Option<&mut Vec<Inline>> {
        let mut items = &mut self.inlines;
        for &i in container_path {
            match items.get_mut(i)? {
                Inline::Container { children, .. } => items = children,
                _ => return None,
            }
        }
        Some(items)
    }

    /// Number of editable positions.
    pub fn char_len(&self) -> usize {
        let mut n = 0;
        self.visit_runs(&mut |_, r, del| {
            if !del {
                n += r.items.iter().map(item_len).sum::<usize>();
            }
        });
        n
    }

    pub fn plain_text(&self) -> String {
        let mut s = String::new();
        self.visit_runs(&mut |_, r, del| {
            if del {
                return;
            }
            for it in &r.items {
                match it {
                    RunItem::Text(t) => s.push_str(t),
                    RunItem::Tab => s.push('\t'),
                    RunItem::Break { .. } | RunItem::CarriageReturn => s.push('\u{0b}'),
                    RunItem::NoBreakHyphen => s.push('\u{2011}'),
                    RunItem::SoftHyphen => s.push('\u{00AD}'),
                    RunItem::Sym { ch, .. } => s.push(*ch),
                    RunItem::Drawing(_) | RunItem::Other(_) => {
                        if item_len(it) == 1 {
                            s.push('\u{FFFC}')
                        }
                    }
                    _ => {}
                }
            }
        });
        s
    }

    /// Locate the run & item containing position `pos`. Returns (path, item index, char offset in item).
    /// Positions at a boundary resolve to the end of the preceding text run when `prefer_before`.
    pub fn locate(&self, pos: usize, prefer_before: bool) -> Option<(RunPath, usize, usize)> {
        let mut acc = 0usize;
        let mut found: Option<(RunPath, usize, usize)> = None;
        let mut last_text: Option<(RunPath, usize, usize)> = None;
        self.visit_runs(&mut |path, r, del| {
            if found.is_some() || del {
                return;
            }
            for (ii, it) in r.items.iter().enumerate() {
                let len = item_len(it);
                if let RunItem::Text(t) = it {
                    let n = t.chars().count();
                    if pos >= acc && pos <= acc + n {
                        if pos < acc + n || prefer_before {
                            found = Some((path.clone(), ii, pos - acc));
                            return;
                        }
                        last_text = Some((path.clone(), ii, pos - acc));
                    }
                } else if len > 0 && pos == acc && !prefer_before {
                    // Position before a non-text atom.
                    found = Some((path.clone(), ii, 0));
                    return;
                }
                acc += len;
            }
        });
        found.or(last_text)
    }

    /// The run formatting in effect at `pos` (the run before the caret).
    pub fn rpr_at(&self, pos: usize) -> Option<Element> {
        let mut acc = 0usize;
        let mut result: Option<Option<Element>> = None;
        let mut first: Option<Option<Element>> = None;
        self.visit_runs(&mut |_, r, del| {
            if del {
                return;
            }
            let len: usize = r.items.iter().map(item_len).sum();
            if first.is_none() {
                first = Some(r.rpr.clone());
            }
            if acc < pos && pos <= acc + len {
                result = Some(r.rpr.clone());
            }
            acc += len;
        });
        if pos == 0 {
            if let Some(f) = first {
                return f;
            }
        }
        match result.or(first) {
            Some(r) => r,
            // Empty paragraph: the paragraph mark's run properties.
            None => self.ppr.as_ref().and_then(|p| p.child("w:rPr")).map(|r| {
                let mut r = r.clone();
                r.remove_child("w:ins");
                r.remove_child("w:del");
                r
            }),
        }
    }

    /// Split all runs so that `pos` falls on a run boundary at the top level of
    /// its container. Returns (container path, index in container) where new
    /// content can be inserted.
    pub fn split_at(&mut self, pos: usize) -> (Vec<usize>, usize) {
        let len = self.char_len();
        if len == 0 {
            // Insert after leading non-visual inlines (e.g. bookmarks) — at the end.
            return (Vec::new(), self.inlines.len());
        }
        if pos >= len {
            // After the last visible run, at its container level.
            let mut last: Option<RunPath> = None;
            self.visit_runs(&mut |p, r, del| {
                if !del && r.items.iter().map(item_len).sum::<usize>() > 0 {
                    last = Some(p.clone());
                }
            });
            if let Some(mut p) = last {
                let i = p.pop().unwrap();
                return (p, i + 1);
            }
            return (Vec::new(), self.inlines.len());
        }
        let Some((path, item_idx, off)) = self.locate(pos, false) else {
            return (Vec::new(), self.inlines.len());
        };
        let (cpath, run_idx) = (path[..path.len() - 1].to_vec(), *path.last().unwrap());
        let run = self.run_at_path_mut(&path).unwrap();
        if item_idx == 0 && off == 0 {
            return (cpath, run_idx);
        }
        // Split run into [0, item_idx/off) and the rest.
        let mut tail_items: Vec<RunItem> = Vec::new();
        let mut head_items: Vec<RunItem> = Vec::new();
        for (i, it) in std::mem::take(&mut run.items).into_iter().enumerate() {
            if i < item_idx {
                head_items.push(it);
            } else if i == item_idx {
                if let RunItem::Text(t) = &it {
                    let b = byte_idx(t, off);
                    if b > 0 {
                        head_items.push(RunItem::Text(t[..b].to_string()));
                    }
                    if b < t.len() {
                        tail_items.push(RunItem::Text(t[b..].to_string()));
                    }
                } else {
                    tail_items.push(it);
                }
            } else {
                tail_items.push(it);
            }
        }
        run.items = head_items;
        let tail = Run { rpr: run.rpr.clone(), items: tail_items, attrs: run.attrs.clone() };
        let container = self.inlines_at_mut(&cpath).unwrap();
        container.insert(run_idx + 1, Inline::Run(tail));
        (cpath, run_idx + 1)
    }

    pub fn insert_text(&mut self, pos: usize, text: &str, rpr: Option<Element>) {
        if text.is_empty() {
            return;
        }
        // Fast path: append into the existing text item if formatting matches.
        if let Some((path, ii, off)) = self.locate(pos, true) {
            let run = self.run_at_path_mut(&path).unwrap();
            if run.rpr == rpr {
                if let Some(RunItem::Text(t)) = run.items.get_mut(ii) {
                    let b = byte_idx(t, off);
                    t.insert_str(b, text);
                    return;
                }
            }
        }
        let (cpath, idx) = self.split_at(pos);
        let run = Run { rpr, items: vec![RunItem::Text(text.to_string())], attrs: Vec::new() };
        let c = self.inlines_at_mut(&cpath).unwrap();
        c.insert(idx.min(c.len()), Inline::Run(run));
    }

    pub fn insert_item(&mut self, pos: usize, item: RunItem, rpr: Option<Element>) {
        let (cpath, idx) = self.split_at(pos);
        let run = Run { rpr, items: vec![item], attrs: Vec::new() };
        let c = self.inlines_at_mut(&cpath).unwrap();
        c.insert(idx.min(c.len()), Inline::Run(run));
    }

    /// Delete visible content in [a, b).
    pub fn delete_range(&mut self, a: usize, b: usize) {
        if a >= b {
            return;
        }
        let mut acc = 0usize;
        fn rec(items: &mut Vec<Inline>, a: usize, b: usize, acc: &mut usize) {
            for it in items.iter_mut() {
                match it {
                    Inline::Run(r) => {
                        let mut new_items = Vec::with_capacity(r.items.len());
                        for item in std::mem::take(&mut r.items) {
                            let len = item_len(&item);
                            let (s, e) = (*acc, *acc + len);
                            *acc = e;
                            if len == 0 || e <= a || s >= b {
                                new_items.push(item);
                                continue;
                            }
                            if let RunItem::Text(t) = &item {
                                let ka = a.saturating_sub(s).min(len);
                                let kb = (b - s).min(len);
                                let mut out = String::new();
                                out.push_str(&t[..byte_idx(t, ka)]);
                                out.push_str(&t[byte_idx(t, kb)..]);
                                if !out.is_empty() {
                                    new_items.push(RunItem::Text(out));
                                }
                            }
                            // Non-text atoms within range are dropped.
                        }
                        r.items = new_items;
                    }
                    Inline::Container { shell, children } => {
                        if shell.name == "w:del" || shell.name == "w:moveFrom" {
                            continue;
                        }
                        rec(children, a, b, acc);
                    }
                    Inline::Other(_) => {}
                }
            }
        }
        rec(&mut self.inlines, a, b, &mut acc);
        self.cleanup();
    }

    /// Remove empty runs and empty containers (except those carrying meaning).
    pub fn cleanup(&mut self) {
        fn rec(items: &mut Vec<Inline>) {
            items.retain_mut(|it| match it {
                Inline::Run(r) => !r.items.is_empty(),
                Inline::Container { shell, children } => {
                    rec(children);
                    !(children.is_empty() && matches!(shell.name.as_str(), "w:hyperlink" | "w:ins" | "w:smartTag"))
                }
                Inline::Other(_) => true,
            });
        }
        rec(&mut self.inlines);
        // Merge adjacent top-level runs with identical formatting.
        let mut merged: Vec<Inline> = Vec::with_capacity(self.inlines.len());
        for it in std::mem::take(&mut self.inlines) {
            if let (Some(Inline::Run(prev)), Inline::Run(cur)) = (merged.last_mut(), &it) {
                if prev.rpr == cur.rpr && prev.attrs.is_empty() && cur.attrs.is_empty() {
                    let mut cur_items = cur.items.clone();
                    if let (Some(RunItem::Text(a)), Some(RunItem::Text(b))) = (prev.items.last_mut(), cur_items.first()) {
                        a.push_str(b);
                        cur_items.remove(0);
                    }
                    prev.items.extend(cur_items);
                    continue;
                }
            }
            merged.push(it);
        }
        self.inlines = merged;
    }

    /// Split the paragraph at `pos`; self keeps the head, returns the tail.
    pub fn split_off(&mut self, pos: usize) -> Paragraph {
        let (cpath, idx) = self.split_at(pos);
        let mut tail = Paragraph { ppr: self.ppr.clone(), inlines: Vec::new(), attrs: Vec::new() };
        if cpath.is_empty() {
            tail.inlines = self.inlines.split_off(idx.min(self.inlines.len()));
        } else {
            // Split nested container: duplicate the container shells along the path.
            let top = cpath[0];
            let mut head_top = self.inlines[top].clone();
            let mut tail_top = self.inlines[top].clone();
            fn trim(it: &mut Inline, path: &[usize], idx: usize, keep_head: bool) {
                if let Inline::Container { children, .. } = it {
                    if path.is_empty() {
                        if keep_head {
                            children.truncate(idx);
                        } else {
                            children.drain(..idx.min(children.len()));
                        }
                    } else {
                        let i = path[0];
                        if keep_head {
                            children.truncate(i + 1);
                            trim(&mut children[i], &path[1..], idx, true);
                        } else {
                            children.drain(..i);
                            trim(&mut children[0], &path[1..], idx, false);
                        }
                    }
                }
            }
            trim(&mut head_top, &cpath[1..], idx, true);
            trim(&mut tail_top, &cpath[1..], idx, false);
            tail.inlines = self.inlines.split_off(top + 1);
            tail.inlines.insert(0, tail_top);
            self.inlines[top] = head_top;
        }
        // Bookmarks and other markers that ended up as leading items stay with the tail; fine.
        self.cleanup();
        tail.cleanup();
        // A new paragraph after a heading uses the style's "next" style — handled by the editor.
        tail
    }

    pub fn append(&mut self, other: Paragraph) {
        self.inlines.extend(other.inlines);
        self.cleanup();
    }

    /// Apply a mutation to the run properties of all runs intersecting [a, b).
    /// Splits runs at the boundaries first.
    pub fn format_range(&mut self, a: usize, b: usize, f: &dyn Fn(&mut Element)) {
        if a >= b {
            return;
        }
        self.split_at(b);
        self.split_at(a);
        let mut acc = 0usize;
        fn rec(items: &mut Vec<Inline>, a: usize, b: usize, acc: &mut usize, f: &dyn Fn(&mut Element)) {
            for it in items.iter_mut() {
                match it {
                    Inline::Run(r) => {
                        let len: usize = r.items.iter().map(item_len).sum();
                        if len > 0 && *acc >= a && *acc + len <= b {
                            let rpr = r.rpr.get_or_insert_with(|| Element::new("w:rPr"));
                            f(rpr);
                        }
                        *acc += len;
                    }
                    Inline::Container { shell, children } => {
                        if shell.name == "w:del" || shell.name == "w:moveFrom" {
                            continue;
                        }
                        rec(children, a, b, acc, f)
                    }
                    Inline::Other(_) => {}
                }
            }
        }
        rec(&mut self.inlines, a, b, &mut acc, f);
        self.cleanup();
    }

    /// Extract a copy of the content in [a, b) as a new paragraph (keeps formatting).
    pub fn slice(&self, a: usize, b: usize) -> Paragraph {
        let mut p = self.clone();
        let len = p.char_len();
        p.delete_range(b.min(len), len);
        p.delete_range(0, a.min(b));
        p
    }
}

pub fn item_len(it: &RunItem) -> usize {
    match it {
        RunItem::Text(t) => t.chars().count(),
        RunItem::DelText(_) | RunItem::FldChar { .. } | RunItem::InstrText(_) => 0,
        RunItem::Tab
        | RunItem::Break { .. }
        | RunItem::CarriageReturn
        | RunItem::NoBreakHyphen
        | RunItem::SoftHyphen
        | RunItem::Sym { .. }
        | RunItem::Drawing(_) => 1,
        RunItem::Other(e) => {
            if e.name == "mc:AlternateContent" && e.find_local("drawing").is_some() {
                1
            } else {
                0
            }
        }
    }
}

pub fn byte_idx(s: &str, char_idx: usize) -> usize {
    s.char_indices().nth(char_idx).map(|(b, _)| b).unwrap_or(s.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn para(text: &[(&str, bool)]) -> Paragraph {
        let mut p = Paragraph::new();
        for (t, bold) in text {
            let rpr = if *bold { Some(Element::new("w:rPr").with_child(Element::new("w:b"))) } else { None };
            p.inlines.push(Inline::Run(Run { rpr, items: vec![RunItem::Text(t.to_string())], attrs: vec![] }));
        }
        p
    }

    #[test]
    fn insert_delete() {
        let mut p = para(&[("Hello ", false), ("world", true)]);
        assert_eq!(p.char_len(), 11);
        p.insert_text(6, "big ", None);
        assert_eq!(p.plain_text(), "Hello big world");
        p.delete_range(2, 8);
        assert_eq!(p.plain_text(), "Heg world");
        let tail = p.split_off(3);
        assert_eq!(p.plain_text(), "Heg");
        assert_eq!(tail.plain_text(), " world");
    }

    #[test]
    fn format() {
        let mut p = para(&[("abcdef", false)]);
        p.format_range(2, 4, &|r| r.set_child(Element::new("w:i"), crate::props::RPR_ORDER));
        assert_eq!(p.inlines.len(), 3);
        assert_eq!(p.plain_text(), "abcdef");
    }

    #[test]
    fn hyperlink_split() {
        let mut p = Paragraph::new();
        p.inlines.push(Inline::Container {
            shell: Element::new("w:hyperlink"),
            children: vec![Inline::Run(Run { rpr: None, items: vec![RunItem::Text("link".into())], attrs: vec![] })],
        });
        let tail = p.split_off(2);
        assert_eq!(p.plain_text(), "li");
        assert_eq!(tail.plain_text(), "nk");
    }
}
