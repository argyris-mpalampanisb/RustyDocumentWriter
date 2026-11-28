//! The RustyDocumentWriter desktop application (egui/eframe).

mod backstage;
mod canvas;
mod icons;
mod ribbon;
mod wordart_ui;
pub mod theme;
mod files;
pub mod i18n;
pub mod settings;

pub use i18n::{tf, tr};

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use rusty_document_writer::document::{Document, HeaderFooter};
use rusty_document_writer::edit::{self, Pos};
use rusty_document_writer::fonts::FontDb;
use rusty_document_writer::layout::{self, Layout, LayoutOptions, Story};
use rusty_document_writer::model::{Block, Paragraph, RunItem};
use rusty_document_writer::props::{Align, LineRule};
use rusty_document_writer::render::Renderer;
use rusty_document_writer::xml::Element;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Home,
    Insert,
    Design,
    Layout,
    References,
    Mailings,
    Review,
    View,
    Help,
}

impl Tab {
    pub const ALL: [Tab; 9] = [Tab::Home, Tab::Insert, Tab::Design, Tab::Layout, Tab::References, Tab::Mailings, Tab::Review, Tab::View, Tab::Help];
    pub fn label(self) -> &'static str {
        match self {
            Tab::Home => "Home",
            Tab::Insert => "Insert",
            Tab::Design => "Design",
            Tab::Layout => "Layout",
            Tab::References => "References",
            Tab::Mailings => "Mailings",
            Tab::Review => "Review",
            Tab::View => "View",
            Tab::Help => "Help",
        }
    }
}

#[derive(Clone, Debug)]
pub enum Cmd {
    New,
    Open,
    OpenPath(PathBuf),
    Export,
    ToggleRibbon,
    RemoveRecent(PathBuf),
    ClearRecent,
    Save,
    SaveAs,
    CloseDoc,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Toggle(&'static str),
    Underline,
    UnderlineStyle(&'static str),
    UnderlineColor(Option<String>),
    /// Toggle a character effect (`w:outline`, `w:shadow`, `w:emboss`, `w:imprint`,
    /// `w:caps`, `w:smallCaps`, `w:dstrike`), clearing the ones it excludes.
    Effect(&'static str),
    ClearEffects,
    TextTheme(usize),
    VertAlign(&'static str),
    GrowFont,
    ShrinkFont,
    ClearFormat,
    ChangeCase(u8),
    FontFamily(String),
    FontSize(f32),
    FontColor(Option<String>),
    Highlight(Option<String>),
    Align(&'static str),
    Bullets,
    Numbering,
    IndentInc,
    IndentDec,
    LineSpacing(f32),
    ParaShading(Option<String>),
    ParaBorder(bool),
    ShowMarks,
    Style(String),
    Find,
    SelectAll,
    PageBreak,
    BlankPage,
    InsertTable(usize, usize),
    InsertPicture,
    /// Insert shape `i` of the built-in 3D shape library, in the current shape colour.
    InsertShape(usize),
    /// Set the colour new shapes are inserted in (`None` = neutral silver).
    ShapeColor(Option<String>),
    /// Open the WordArt dialog for a new WordArt in style `i`.
    NewWordArt(usize),
    /// Open the WordArt dialog for the drawing at (body paragraph path, offset).
    EditWordArt(Vec<usize>, usize),
    /// Insert or update the WordArt from the open dialog.
    ApplyWordArt,
    InsertSymbol(char),
    InsertDateTime,
    PageNumber(&'static str, rusty_document_writer::pagenum::Place),
    PageNumberFormat(&'static str),
    RemovePageNumbers,
    Margins(f32, f32, f32, f32),
    Orientation(bool),
    PageSize(f32, f32),
    Columns(usize),
    ColumnBreak,
    SectionBreak(&'static str),
    IndentLeft(f32),
    IndentRight(f32),
    SpaceBefore(f32),
    SpaceAfter(f32),
    PageColor(Option<String>),
    Zoom(f32),
    ZoomOnePage,
    ZoomPageWidth,
    ToggleRuler,
    ToggleNav,
    DarkPage,
    WordCount,
    About,
    Unsupported(&'static str),
}

struct Snapshot {
    body: Vec<Block>,
    final_sectpr: Element,
    header_footers: HashMap<String, HeaderFooter>,
    numbering: rusty_document_writer::props::Numbering,
    numbering_dirty: bool,
    shell: Element,
    caret: Pos,
    anchor: Pos,
}

#[derive(Default)]
pub struct Toolbar {
    pub font: String,
    pub size: f32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    /// The underline style at the caret (`single`, `double`, `wave`...), if any.
    pub underline_style: Option<String>,
    pub strike: bool,
    pub dstrike: bool,
    pub caps: bool,
    pub small_caps: bool,
    pub outline: bool,
    pub shadow: bool,
    pub emboss: bool,
    pub imprint: bool,
    pub sub: bool,
    pub sup: bool,
    pub align: Option<Align>,
    pub style: String,
    pub ind_left: f32,
    pub ind_right: f32,
    pub before: f32,
    pub after: f32,
    pub line: f32,
    pub list: Option<bool>,
}

pub enum Dialog {
    None,
    Message(String, String),
    ConfirmClose { then: Box<Cmd> },
    WordCount,
    Find,
}

pub struct App {
    pub fonts: Arc<FontDb>,
    pub doc: Document,
    pub layout: Option<Layout>,
    pub layout_dirty: bool,
    pub layout_version: u64,
    pub renderer: Renderer,
    pub textures: HashMap<usize, (egui::TextureHandle, u32, u64)>,
    pub caret: Pos,
    pub anchor: Pos,
    pub pending_rpr: Option<Element>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    last_edit: Option<(&'static str, Instant)>,
    pub zoom: f32,
    pub tab: Tab,
    pub show_marks: bool,
    pub show_ruler: bool,
    pub show_nav: bool,
    pub dark_page: bool,
    /// Colour for inserted library shapes (hex), `None` for the neutral render.
    pub shape_color: Option<String>,
    /// Gallery thumbnails of the shape library, tinted for the given colour.
    pub shape_thumbs: Option<(Option<String>, Vec<egui::TextureHandle>)>,
    /// WordArt gallery tiles, rendered on first use.
    pub wordart_tiles: Option<Vec<egui::TextureHandle>>,
    /// The open WordArt dialog, if any.
    pub wordart_edit: Option<wordart_ui::WordArtEdit>,
    pub backstage: bool,
    pub modified: bool,
    pub desired_x: Option<f32>,
    pub scroll_to_caret: bool,
    pub caret_time: Instant,
    pub font_names: Vec<String>,
    pub tb: Toolbar,
    pub dialog: Dialog,
    pub text_focus: bool,
    pub find_text: String,
    pub replace_text: String,
    pub recent: Vec<PathBuf>,
    /// (layout_version, words): the word count is only recomputed when the document changes.
    word_cache: std::cell::Cell<(u64, usize)>,
    pub dragging: bool,
    pub pending_scroll: Option<f32>,
    pub view_width: f32,
    pub icon_font: bool,
    pub frame_count: u64,
    pub last_zoom: f32,
    pub zoom_changed: Instant,
    /// A conversion running in the background (see `files`).
    pub job: Option<files::Job>,
    pub settings: settings::Settings,
    /// Ribbon shown temporarily while collapsed (a tab was clicked).
    pub ribbon_peek: bool,
}

fn recent_path() -> Option<PathBuf> {
    std::env::var("LOCALAPPDATA").or_else(|_| std::env::var("HOME")).ok().map(|b| PathBuf::from(b).join("RustyDocumentWriter").join("recent.txt"))
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, open: Option<PathBuf>) -> App {
        let icon_font = theme::install(&cc.egui_ctx);
        let fonts = Arc::new(FontDb::load_system());
        let mut font_names: Vec<String> = fonts.family_names().iter().map(|f| fonts.display_family(f)).collect();
        font_names.sort();
        font_names.dedup();
        let doc = Document::new_blank();
        let caret = edit::start_of_doc(&doc);
        let mut recent: Vec<PathBuf> = Vec::new();
        for p in recent_path().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default().lines() {
            // Canonical form so "docs/a.docx" and "/home/me/docs/a.docx" are one entry.
            if let Ok(p) = std::fs::canonicalize(p) {
                if !recent.contains(&p) {
                    recent.push(p);
                }
            }
        }
        let settings = settings::Settings::load();
        let mut app = App {
            fonts,
            doc,
            layout: None,
            layout_dirty: true,
            layout_version: 0,
            renderer: Renderer::new(),
            textures: HashMap::new(),
            anchor: caret.clone(),
            caret,
            pending_rpr: None,
            undo: Vec::new(),
            redo: Vec::new(),
            last_edit: None,
            zoom: settings.start_zoom,
            tab: Tab::Home,
            show_marks: false,
            show_ruler: settings.show_ruler,
            show_nav: settings.show_nav,
            dark_page: false,
            shape_color: Some("156082".into()),
            shape_thumbs: None,
            wordart_tiles: None,
            wordart_edit: None,
            backstage: false,
            modified: false,
            desired_x: None,
            scroll_to_caret: false,
            caret_time: Instant::now(),
            font_names,
            tb: Toolbar::default(),
            dialog: Dialog::None,
            text_focus: false,
            find_text: String::new(),
            replace_text: String::new(),
            recent,
            word_cache: std::cell::Cell::new((u64::MAX, 0)),
            dragging: false,
            pending_scroll: None,
            view_width: 1000.0,
            icon_font,
            frame_count: 0,
            last_zoom: settings.start_zoom,
            zoom_changed: Instant::now(),
            job: None,
            settings,
            ribbon_peek: false,
        };
        app.settings.apply(&cc.egui_ctx);
        if let Some(p) = open {
            app.open_path(&p);
        }
        app
    }

    // ----------------------------------------------------------------- layout

    pub fn ensure_layout(&mut self) {
        if !self.layout_dirty && self.layout.is_some() {
            return;
        }
        let opts = LayoutOptions { show_marks: self.show_marks };
        self.layout = Some(layout::layout_document(&self.doc, &self.fonts, &opts));
        self.layout_dirty = false;
        self.layout_version += 1;
        self.caret = edit::clamp(&self.doc, &self.caret);
        self.anchor = edit::clamp(&self.doc, &self.anchor);
        self.update_toolbar();
    }

    pub fn invalidate(&mut self) {
        self.layout_dirty = true;
        self.modified = true;
    }

    /// Find the (page, line) showing a position.
    pub fn locate(&self, pos: &Pos) -> Option<(usize, &layout::LineInfo)> {
        let l = self.layout.as_ref()?;
        let mut best: Option<(usize, &layout::LineInfo)> = None;
        for (pi, p) in l.pages.iter().enumerate() {
            for li in &p.lines {
                if li.story != pos.story || li.path != pos.path {
                    continue;
                }
                if pos.off >= li.start && (pos.off < li.end || (pos.off == li.end && li.end == li.para_len)) {
                    return Some((pi, li));
                }
                if li.start <= pos.off {
                    best = Some((pi, li));
                }
                if best.is_none() {
                    best = Some((pi, li));
                }
            }
            // Headers repeat on every page: stop at the first page that has the story.
            if pos.story != Story::Body && best.is_some() {
                break;
            }
        }
        best
    }

    // ----------------------------------------------------------------- undo

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            body: self.doc.body.clone(),
            final_sectpr: self.doc.final_sectpr.clone(),
            header_footers: self.doc.header_footers.clone(),
            numbering: self.doc.numbering.clone(),
            numbering_dirty: self.doc.numbering_dirty,
            shell: self.doc.shell.clone(),
            caret: self.caret.clone(),
            anchor: self.anchor.clone(),
        }
    }

    fn restore(&mut self, s: Snapshot) {
        self.doc.body = s.body;
        self.doc.final_sectpr = s.final_sectpr;
        self.doc.header_footers = s.header_footers;
        self.doc.numbering = s.numbering;
        self.doc.numbering_dirty = s.numbering_dirty;
        self.doc.shell = s.shell;
        self.caret = s.caret;
        self.anchor = s.anchor;
        self.invalidate();
        self.scroll_to_caret = true;
    }

    /// Record an undo point. Consecutive edits of the same kind within 1.5s are merged.
    pub fn checkpoint(&mut self, kind: &'static str) {
        let merge = matches!(self.last_edit, Some((k, t)) if k == kind && (kind == "type" || kind == "delete") && t.elapsed().as_secs_f32() < 1.5);
        if !merge {
            let s = self.snapshot();
            self.undo.push(s);
            if self.undo.len() > 200 {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.last_edit = Some((kind, Instant::now()));
    }

    pub fn undo(&mut self) {
        if let Some(s) = self.undo.pop() {
            let cur = self.snapshot();
            self.redo.push(cur);
            self.restore(s);
            self.last_edit = None;
        }
    }

    pub fn redo(&mut self) {
        if let Some(s) = self.redo.pop() {
            let cur = self.snapshot();
            self.undo.push(cur);
            self.restore(s);
            self.last_edit = None;
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    // ----------------------------------------------------------------- selection

    pub fn has_sel(&self) -> bool {
        self.caret != self.anchor
    }

    pub fn sel(&self) -> (Pos, Pos) {
        if self.anchor.cmp_doc(&self.caret) == std::cmp::Ordering::Greater {
            (self.caret.clone(), self.anchor.clone())
        } else {
            (self.anchor.clone(), self.caret.clone())
        }
    }

    pub fn set_caret(&mut self, p: Pos, extend: bool) {
        self.caret = edit::clamp(&self.doc, &p);
        if !extend {
            self.anchor = self.caret.clone();
        }
        self.caret_time = Instant::now();
        self.scroll_to_caret = true;
        self.pending_rpr = None;
        self.update_toolbar();
    }

    fn delete_selection(&mut self) {
        if self.has_sel() {
            let (a, b) = self.sel();
            let p = edit::delete_range(&mut self.doc, &a, &b);
            self.caret = p.clone();
            self.anchor = p;
            self.invalidate();
        }
    }

    pub fn current_rpr(&self) -> Option<Element> {
        if let Some(p) = &self.pending_rpr {
            return Some(p.clone());
        }
        edit::para(&self.doc, &self.caret).and_then(|p| p.rpr_at(self.caret.off)).map(|mut r| {
            // Typing after a hyperlink doesn't continue the Hyperlink character style.
            if r.child("w:rStyle").and_then(|s| s.attr("w:val")) == Some("Hyperlink") {
                r.remove_child("w:rStyle");
            }
            r
        })
    }

    // ----------------------------------------------------------------- typing

    pub fn type_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.checkpoint("type");
        let rpr = self.current_rpr();
        self.delete_selection();
        let p = edit::insert_text(&mut self.doc, &self.caret.clone(), text, rpr);
        self.caret = p.clone();
        self.anchor = p;
        self.pending_rpr = None;
        self.invalidate();
        self.after_edit();
    }

    pub fn enter(&mut self) {
        self.checkpoint("enter");
        self.delete_selection();
        let p = edit::split_paragraph(&mut self.doc, &self.caret.clone());
        self.caret = p.clone();
        self.anchor = p;
        self.invalidate();
        self.after_edit();
    }

    pub fn insert_break(&mut self, kind: Option<&str>) {
        self.checkpoint("break");
        self.delete_selection();
        let rpr = self.current_rpr();
        let p = edit::insert_item(&mut self.doc, &self.caret.clone(), RunItem::Break { kind: kind.map(str::to_string), clear: None }, rpr);
        self.caret = p.clone();
        self.anchor = p;
        self.invalidate();
        self.after_edit();
    }

    pub fn backspace(&mut self, word: bool) {
        self.checkpoint("delete");
        if self.has_sel() {
            self.delete_selection();
        } else if word {
            let start = edit::para(&self.doc, &self.caret).map(|p| edit::word_left(p, self.caret.off)).unwrap_or(0);
            let a = Pos { off: start, ..self.caret.clone() };
            let b = self.caret.clone();
            let p = edit::delete_range(&mut self.doc, &a, &b);
            self.caret = p.clone();
            self.anchor = p;
        } else {
            let p = edit::backspace(&mut self.doc, &self.caret.clone());
            self.caret = p.clone();
            self.anchor = p;
        }
        self.invalidate();
        self.after_edit();
    }

    pub fn delete(&mut self, word: bool) {
        self.checkpoint("delete");
        if self.has_sel() {
            self.delete_selection();
        } else if word {
            let end = edit::para(&self.doc, &self.caret).map(|p| edit::word_right(p, self.caret.off)).unwrap_or(0);
            let b = Pos { off: end, ..self.caret.clone() };
            let a = self.caret.clone();
            edit::delete_range(&mut self.doc, &a, &b);
        } else {
            edit::delete_forward(&mut self.doc, &self.caret.clone());
        }
        self.invalidate();
        self.after_edit();
    }

    fn after_edit(&mut self) {
        self.caret_time = Instant::now();
        self.scroll_to_caret = true;
        self.desired_x = None;
    }

    // ----------------------------------------------------------------- formatting helpers

    /// Insert a picture at the caret, `w_pt` x `h_pt`, shrunk to fit the text column.
    fn insert_picture(&mut self, bytes: Vec<u8>, ext: &str, name: &str, mut w_pt: f32, mut h_pt: f32) {
        self.checkpoint("picture");
        let sect = self.doc.sections();
        let s = &sect.last().unwrap().1;
        let max_w = s.page_w - s.margin_left - s.margin_right;
        if w_pt > max_w {
            h_pt *= max_w / w_pt;
            w_pt = max_w;
        }
        let rid = self.doc.add_image(bytes, ext);
        let id = self.doc.next_drawing_id();
        let d = edit::image_drawing(&rid, w_pt, h_pt, id, name);
        self.delete_selection();
        let p = edit::insert_item(&mut self.doc, &self.caret.clone(), RunItem::Drawing(d), None);
        self.caret = p.clone();
        self.anchor = p;
        self.invalidate();
    }

    fn format_run(&mut self, f: &dyn Fn(&mut Element)) {
        if self.has_sel() {
            self.checkpoint("format");
            let (a, b) = self.sel();
            edit::format_runs(&mut self.doc, &a, &b, f);
            self.invalidate();
        } else {
            // Apply to the next typed text (standard behaviour), or the paragraph mark if empty.
            let mut r = self.current_rpr().unwrap_or_else(|| Element::new("w:rPr"));
            f(&mut r);
            let empty = edit::para(&self.doc, &self.caret).is_some_and(|p| p.char_len() == 0);
            if empty {
                self.checkpoint("format");
                let c = self.caret.clone();
                edit::format_runs(&mut self.doc, &c, &c, f);
                if let Some(p) = edit::para_mut(&mut self.doc, &c.story, &c.path) {
                    let mark = p.ppr_mut().ensure_child("w:rPr", rusty_document_writer::props::PPR_ORDER);
                    f(mark);
                }
                self.invalidate();
            }
            self.pending_rpr = Some(r);
        }
        self.update_toolbar();
    }

    fn format_para(&mut self, f: &dyn Fn(&mut Element)) {
        self.checkpoint("para");
        let (a, b) = self.sel();
        edit::format_paras(&mut self.doc, &a, &b, f);
        self.invalidate();
        self.update_toolbar();
    }

    fn section_edit(&mut self, f: &dyn Fn(&mut Element)) {
        self.checkpoint("section");
        let idx = self.caret.path.first().copied().unwrap_or(0);
        let s = self.doc.sectpr_for_block_mut(idx);
        f(s);
        self.invalidate();
    }

    pub fn update_toolbar(&mut self) {
        let res = self.doc.resolver();
        let p = edit::para(&self.doc, &self.caret);
        let ppr = p.and_then(|p| p.ppr.as_ref());
        let pp = res.para(ppr, None);
        let rpr = self.pending_rpr.clone().or_else(|| p.and_then(|p| p.rpr_at(self.caret.off)));
        let rp = res.run(&pp.style_id, rpr.as_ref(), None);
        let theme = &self.doc.theme;
        self.tb.font = if rp.font_ascii == theme.minor_latin {
            tf("{} (Body)", &[&rp.font_ascii])
        } else if rp.font_ascii == theme.major_latin {
            tf("{} (Headings)", &[&rp.font_ascii])
        } else {
            rp.font_ascii.clone()
        };
        self.tb.size = rp.size;
        self.tb.bold = rp.bold;
        self.tb.italic = rp.italic;
        self.tb.underline = rp.underline.is_some();
        self.tb.underline_style = rp.underline.clone();
        self.tb.strike = rp.strike;
        self.tb.dstrike = rp.dstrike;
        self.tb.caps = rp.caps;
        self.tb.small_caps = rp.small_caps;
        self.tb.outline = rp.outline;
        self.tb.shadow = rp.shadow;
        self.tb.emboss = rp.emboss;
        self.tb.imprint = rp.imprint;
        self.tb.sub = rp.vert_align == rusty_document_writer::props::VertAlign::Sub;
        self.tb.sup = rp.vert_align == rusty_document_writer::props::VertAlign::Super;
        self.tb.align = Some(pp.align);
        self.tb.style = pp.style_id.clone();
        self.tb.ind_left = pp.ind_left;
        self.tb.ind_right = pp.ind_right;
        self.tb.before = pp.space_before;
        self.tb.after = pp.space_after;
        self.tb.line = match pp.line {
            LineRule::Auto(m) => m,
            _ => 1.0,
        };
        self.tb.list = pp.num.as_ref().and_then(|(id, l)| self.doc.numbering.level(id, *l, &self.doc.styles)).map(|l| l.num_fmt == "bullet");
    }

    // ----------------------------------------------------------------- files

    fn after_load(&mut self) {
        self.doc.ensure_nonempty();
        self.caret = edit::start_of_doc(&self.doc);
        self.anchor = self.caret.clone();
        self.undo.clear();
        self.redo.clear();
        self.layout_dirty = true;
        self.layout = None;
        self.textures.clear();
        self.modified = false;
        self.backstage = false;
        self.pending_scroll = Some(0.0);
        self.pending_rpr = None;
    }

    fn push_recent(&mut self, p: &Path) {
        let p = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        self.recent.retain(|x| x != &p);
        self.recent.insert(0, p);
        self.recent.truncate(15);
        self.save_recent();
    }

    pub fn remove_recent(&mut self, p: &Path) {
        self.recent.retain(|x| x != p);
        self.save_recent();
    }

    fn save_recent(&self) {
        if let Some(rp) = recent_path() {
            let _ = std::fs::create_dir_all(rp.parent().unwrap());
            let _ = std::fs::write(rp, self.recent.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("\n"));
        }
    }

    fn save(&mut self, force_dialog: bool) -> bool {
        let Some(path) = self.save_target(force_dialog) else { return false };
        let dropped_macros = files::drops_macros(&self.doc, &path);
        match self.doc.save_as(&path) {
            Ok(()) => {
                self.modified = false;
                self.push_recent(&path);
                if dropped_macros {
                    self.dialog = Dialog::Message(
                        tr("Macros were removed").into(),
                        tr("This type of file can't contain macros, so they were left out. To keep them, save as a Word Macro-Enabled Document (.docm).").into(),
                    );
                }
                true
            }
            Err(e) => {
                self.dialog = Dialog::Message(tr("Couldn't save the document").into(), e);
                false
            }
        }
    }

    // ----------------------------------------------------------------- commands

    pub fn run(&mut self, cmd: Cmd, ctx: &egui::Context) {
        match cmd {
            Cmd::New => {
                if self.modified {
                    self.dialog = Dialog::ConfirmClose { then: Box::new(Cmd::New) };
                    return;
                }
                self.doc = Document::new_blank();
                self.after_load();
            }
            Cmd::Open => {
                if self.modified {
                    self.dialog = Dialog::ConfirmClose { then: Box::new(Cmd::Open) };
                    return;
                }
                let all: Vec<&str> = files::WORD_EXTS.iter().chain(files::IMPORT_EXTS).copied().collect();
                let picked = rfd::FileDialog::new()
                    .add_filter(tr("All Documents"), &all)
                    .add_filter(tr("Word Documents"), files::WORD_EXTS)
                    .add_filter(tr("Other Documents (converted with LibreOffice)"), files::IMPORT_EXTS)
                    .pick_file();
                if let Some(p) = picked {
                    self.open_path(&p);
                }
            }
            Cmd::OpenPath(p) => {
                if self.modified {
                    self.dialog = Dialog::ConfirmClose { then: Box::new(Cmd::OpenPath(p)) };
                    return;
                }
                self.open_path(&p);
            }
            Cmd::RemoveRecent(p) => self.remove_recent(&p),
            Cmd::ClearRecent => {
                self.recent.clear();
                self.save_recent();
            }
            Cmd::Save => {
                self.save(false);
            }
            Cmd::SaveAs => {
                self.save(true);
            }
            Cmd::Export => self.export(),
            Cmd::CloseDoc => {
                if self.modified {
                    self.dialog = Dialog::ConfirmClose { then: Box::new(Cmd::CloseDoc) };
                    return;
                }
                self.doc = Document::new_blank();
                self.after_load();
            }
            Cmd::Undo => self.undo(),
            Cmd::Redo => self.redo(),
            Cmd::Copy => {
                if self.has_sel() {
                    let (a, b) = self.sel();
                    ctx.copy_text(edit::selected_text(&self.doc, &a, &b));
                }
            }
            Cmd::Cut => {
                if self.has_sel() {
                    let (a, b) = self.sel();
                    ctx.copy_text(edit::selected_text(&self.doc, &a, &b));
                    self.backspace(false);
                }
            }
            Cmd::Paste => {
                // egui delivers clipboard contents as an Event::Paste handled by the canvas.
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
            }
            Cmd::Toggle(name) => {
                let on = match name {
                    "w:b" => !self.tb.bold,
                    "w:i" => !self.tb.italic,
                    "w:strike" => !self.tb.strike,
                    _ => true,
                };
                if name == "w:strike" {
                    self.format_run(&move |r| edit::set_effect(r, name, on));
                } else {
                    self.format_run(&move |r| edit::set_toggle(r, name, on));
                }
            }
            Cmd::Effect(name) => {
                let on = !match name {
                    "w:dstrike" => self.tb.dstrike,
                    "w:caps" => self.tb.caps,
                    "w:smallCaps" => self.tb.small_caps,
                    "w:outline" => self.tb.outline,
                    "w:shadow" => self.tb.shadow,
                    "w:emboss" => self.tb.emboss,
                    "w:imprint" => self.tb.imprint,
                    _ => false,
                };
                self.format_run(&move |r| edit::set_effect(r, name, on));
            }
            Cmd::ClearEffects => self.format_run(&edit::clear_effects),
            Cmd::TextTheme(i) => {
                if let Some(t) = rusty_document_writer::texttheme::THEMES.get(i).copied() {
                    self.format_run(&move |r| t.apply(r));
                }
            }
            Cmd::UnderlineStyle(style) => {
                // Picking the style already applied turns the underline off, like the toolbar toggle.
                let off = style == "none" || self.tb.underline_style.as_deref() == Some(style);
                self.format_run(&move |r| edit::set_underline_style(r, if off { None } else { Some(style) }));
            }
            Cmd::UnderlineColor(c) => self.format_run(&move |r| edit::set_underline_color(r, c.as_deref())),
            Cmd::Underline => {
                let on = !self.tb.underline;
                self.format_run(&move |r| edit::set_underline(r, on));
            }
            Cmd::VertAlign(v) => {
                let cur = if v == "subscript" { self.tb.sub } else { self.tb.sup };
                self.format_run(&move |r| edit::set_vert_align(r, if cur { None } else { Some(v) }));
            }
            Cmd::GrowFont | Cmd::ShrinkFont => {
                const SIZES: [f32; 16] = [8.0, 9.0, 10.0, 10.5, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0, 36.0, 48.0, 72.0, 96.0];
                let cur = self.tb.size;
                let n = if matches!(cmd, Cmd::GrowFont) {
                    SIZES.iter().copied().find(|s| *s > cur + 0.01).unwrap_or(cur + 10.0)
                } else {
                    SIZES.iter().rev().copied().find(|s| *s < cur - 0.01).unwrap_or((cur - 1.0).max(1.0))
                };
                self.format_run(&move |r| edit::set_size(r, n));
            }
            Cmd::FontSize(s) => self.format_run(&move |r| edit::set_size(r, s)),
            Cmd::FontFamily(f) => self.format_run(&move |r| edit::set_font(r, &f)),
            Cmd::FontColor(c) => self.format_run(&move |r| edit::set_color(r, c.as_deref())),
            Cmd::Highlight(c) => self.format_run(&move |r| edit::set_highlight(r, c.as_deref())),
            Cmd::ClearFormat => {
                self.checkpoint("format");
                let (a, b) = self.sel();
                edit::format_runs(&mut self.doc, &a, &b, &|r| r.children.clear());
                edit::format_paras(&mut self.doc, &a, &b, &|p| {
                    let keep: Vec<Element> = ["w:numPr", "w:sectPr"].iter().filter_map(|n| p.child(n).cloned()).collect();
                    p.children.clear();
                    for k in keep {
                        p.set_child(k, rusty_document_writer::props::PPR_ORDER);
                    }
                });
                self.pending_rpr = None;
                self.invalidate();
                self.update_toolbar();
            }
            Cmd::ChangeCase(mode) => {
                if self.has_sel() {
                    let (a, b) = self.sel();
                    let text = edit::selected_text(&self.doc, &a, &b);
                    let new = match mode {
                        0 => text.to_uppercase(),
                        1 => text.to_lowercase(),
                        _ => text
                            .split(' ')
                            .map(|w| {
                                let mut c = w.chars();
                                c.next().map(|f| f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase()).unwrap_or_default()
                            })
                            .collect::<Vec<_>>()
                            .join(" "),
                    };
                    if new.chars().count() == text.chars().count() && !text.contains('\n') {
                        self.checkpoint("format");
                        let rpr = edit::para(&self.doc, &a).and_then(|p| p.rpr_at(a.off + 1));
                        edit::delete_range(&mut self.doc, &a, &b);
                        let end = edit::insert_text(&mut self.doc, &a, &new, rpr);
                        self.anchor = a;
                        self.caret = end;
                        self.invalidate();
                    }
                }
            }
            Cmd::Align(jc) => self.format_para(&move |p| edit::set_jc(p, jc)),
            Cmd::Style(id) => {
                let id2 = id.clone();
                self.format_para(&move |p| edit::set_style(p, &id2));
            }
            Cmd::Bullets | Cmd::Numbering => {
                let bullet = matches!(cmd, Cmd::Bullets);
                if self.tb.list == Some(bullet) {
                    self.format_para(&|p| edit::set_numbering(p, None, 0));
                } else {
                    self.checkpoint("list");
                    let id = self.doc.add_list(bullet);
                    let (a, b) = self.sel();
                    edit::format_paras(&mut self.doc, &a, &b, &|p| edit::set_numbering(p, Some(&id), 0));
                    self.invalidate();
                    self.update_toolbar();
                }
            }
            Cmd::IndentInc | Cmd::IndentDec => {
                let inc = matches!(cmd, Cmd::IndentInc);
                // In a list, change level; otherwise move the left indent by 0.5".
                let in_list = self.tb.list.is_some();
                if in_list {
                    self.format_para(&move |p| {
                        if let Some(np) = p.child_mut("w:numPr") {
                            let il = np.child("w:ilvl").and_then(|e| e.attr("w:val")).and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
                            let nl = (il + if inc { 1 } else { -1 }).clamp(0, 8);
                            np.set_child(Element::new("w:ilvl").with_attr("w:val", nl.to_string()), &["w:ilvl", "w:numId"]);
                        }
                    });
                } else {
                    let cur = self.tb.ind_left;
                    let n = if inc { ((cur / 36.0).floor() + 1.0) * 36.0 } else { ((cur / 36.0).ceil() - 1.0).max(0.0) * 36.0 };
                    self.format_para(&move |p| edit::set_indent(p, Some(n), None));
                }
            }
            Cmd::LineSpacing(m) => self.format_para(&move |p| edit::set_spacing(p, None, None, Some((m, "auto")))),
            Cmd::SpaceBefore(v) => self.format_para(&move |p| edit::set_spacing(p, Some(v), None, None)),
            Cmd::SpaceAfter(v) => self.format_para(&move |p| edit::set_spacing(p, None, Some(v), None)),
            Cmd::IndentLeft(v) => self.format_para(&move |p| edit::set_indent(p, Some(v), None)),
            Cmd::IndentRight(v) => self.format_para(&move |p| edit::set_indent(p, None, Some(v))),
            Cmd::ParaShading(c) => self.format_para(&move |p| match &c {
                Some(h) => p.set_child(Element::new("w:shd").with_attr("w:val", "clear").with_attr("w:color", "auto").with_attr("w:fill", h.as_str()), rusty_document_writer::props::PPR_ORDER),
                None => p.remove_child("w:shd"),
            }),
            Cmd::ParaBorder(on) => self.format_para(&move |p| {
                if on {
                    let b = |n: &str| Element::new(n).with_attr("w:val", "single").with_attr("w:sz", "4").with_attr("w:space", "1").with_attr("w:color", "auto");
                    p.set_child(Element::new("w:pBdr").with_child(b("w:top")).with_child(b("w:left")).with_child(b("w:bottom")).with_child(b("w:right")), rusty_document_writer::props::PPR_ORDER);
                } else {
                    p.remove_child("w:pBdr");
                }
            }),
            Cmd::ShowMarks => {
                self.show_marks = !self.show_marks;
                self.layout_dirty = true;
            }
            Cmd::Find => self.dialog = Dialog::Find,
            Cmd::SelectAll => {
                let s = edit::start_of_doc(&self.doc);
                let e = edit::end_of_doc(&self.doc);
                self.anchor = s;
                self.caret = e;
            }
            Cmd::PageBreak => self.insert_break(Some("page")),
            Cmd::ColumnBreak => self.insert_break(Some("column")),
            Cmd::BlankPage => {
                self.insert_break(Some("page"));
                self.insert_break(Some("page"));
                let mut p = self.caret.clone();
                p.off -= 1;
                self.set_caret(p, false);
            }
            Cmd::SectionBreak(kind) => {
                self.checkpoint("section");
                let c = self.caret.clone();
                let next = edit::split_paragraph(&mut self.doc, &c);
                // The current section's properties move to the paragraph before the break.
                let sect_idx = c.path[0];
                let mut sp = self.doc.sectpr_for_block_mut(next.path[0]).clone();
                sp.set_child(Element::new("w:type").with_attr("w:val", kind), rusty_document_writer::props::SECTPR_ORDER);
                let cur = self.doc.sectpr_for_block_mut(next.path[0]);
                cur.set_child(Element::new("w:type").with_attr("w:val", kind), rusty_document_writer::props::SECTPR_ORDER);
                sp.remove_child("w:type");
                if let Some(Block::Paragraph(p)) = self.doc.body.get_mut(sect_idx) {
                    p.ppr_mut().set_child(sp, rusty_document_writer::props::PPR_ORDER);
                }
                self.caret = next.clone();
                self.anchor = next;
                self.invalidate();
            }
            Cmd::InsertTable(r, c) => {
                self.checkpoint("table");
                let sect = self.doc.sections();
                let s = &sect.iter().find(|(end, _)| *end >= self.caret.path[0]).unwrap_or(sect.last().unwrap()).1;
                let w = ((s.page_w - s.margin_left - s.margin_right) * 20.0) as i32;
                let t = edit::new_table(r, c, w);
                let p = edit::insert_block_at(&mut self.doc, &self.caret.clone(), Block::Table(t));
                self.caret = p.clone();
                self.anchor = p;
                self.invalidate();
            }
            Cmd::InsertPicture => {
                if let Some(path) = rfd::FileDialog::new().add_filter(tr("Pictures"), &["png", "jpg", "jpeg", "gif", "bmp", "tif", "tiff"]).pick_file() {
                    match std::fs::read(&path) {
                        Ok(bytes) => match image::load_from_memory(&bytes).map(|i| (i.width(), i.height())) {
                            Ok((w, h)) => {
                                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("png").to_string();
                                let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                                self.insert_picture(bytes, &ext, &name, w as f32 * 0.75, h as f32 * 0.75);
                            }
                            Err(e) => self.dialog = Dialog::Message(tr("Unsupported picture").into(), e.to_string()),
                        },
                        Err(e) => self.dialog = Dialog::Message(tr("Couldn't read the picture").into(), e.to_string()),
                    }
                }
            }
            Cmd::InsertShape(i) => {
                use rusty_document_writer::shapes;
                if let Some(shape) = shapes::SHAPES.get(i) {
                    let color = self.shape_color.as_deref().and_then(rusty_document_writer::props::Color::from_hex);
                    let png = shapes::tinted_png(shape, color);
                    if let Ok(img) = image::load_from_memory(&png) {
                        // 1.5 inches on the longer side.
                        let k = 108.0 / img.width().max(img.height()) as f32;
                        let (w, h) = (img.width() as f32 * k, img.height() as f32 * k);
                        self.insert_picture(png, "png", shape.name, w, h);
                    }
                }
            }
            Cmd::ShapeColor(c) => self.shape_color = c,
            Cmd::NewWordArt(style) => {
                let theme_font = self.doc.theme.major_latin.clone();
                let font = if theme_font.is_empty() { "Arial".to_string() } else { theme_font };
                let art = rusty_document_writer::wordart::WordArt { style, text: tr("Your text here").into(), font, size: 40.0 };
                self.wordart_edit = Some(wordart_ui::WordArtEdit::new(None, art));
            }
            Cmd::EditWordArt(path, off) => {
                let art = edit::drawing_at_mut(&mut self.doc, &Story::Body, &path, off).and_then(|d| rusty_document_writer::wordart::read(d));
                if let Some(art) = art {
                    self.wordart_edit = Some(wordart_ui::WordArtEdit::new(Some((path, off)), art));
                }
            }
            Cmd::ApplyWordArt => {
                use rusty_document_writer::wordart;
                if let Some(ed) = self.wordart_edit.take() {
                    self.checkpoint("wordart");
                    let fonts = self.fonts.clone();
                    match ed.target {
                        Some((path, off)) => {
                            if let Some(item) = edit::drawing_item_mut(&mut self.doc, &Story::Body, &path, off) {
                                let old = match item {
                                    RunItem::Drawing(d) | RunItem::Other(d) => d.clone(),
                                    _ => unreachable!(),
                                };
                                let id = old.find_local("docPr").and_then(|e| e.attr("id")).and_then(|v| v.parse().ok()).unwrap_or(1);
                                // Replace the whole item so the fallback copy never goes stale.
                                *item = wordart::run_item(&ed.art, &fonts, id, Some(&old));
                            }
                        }
                        None => {
                            let id = self.doc.next_drawing_id();
                            let item = wordart::run_item(&ed.art, &fonts, id, None);
                            self.delete_selection();
                            // Anchor it in its own paragraph after the caret's, so it never lands on top of
                            // another float anchored in the same paragraph (they would share a position).
                            let mut at = self.caret.clone();
                            let len = edit::para(&self.doc, &at).map(|p| p.char_len()).unwrap_or(0);
                            if len > 0 {
                                at.off = len;
                                at = edit::split_paragraph(&mut self.doc, &at);
                            }
                            let p = edit::insert_item(&mut self.doc, &at, item, None);
                            self.caret = p.clone();
                            self.anchor = p;
                        }
                    }
                    self.invalidate();
                }
            }
            Cmd::InsertSymbol(c) => self.type_text(&c.to_string()),
            Cmd::PageNumber(id, place) => {
                if let Some(st) = rusty_document_writer::pagenum::style(id) {
                    self.checkpoint("pagenum");
                    self.doc.insert_page_number(st, place);
                    self.invalidate();
                }
            }
            Cmd::PageNumberFormat(fmt) => {
                self.checkpoint("pagenum");
                self.doc.set_page_number_format(fmt);
                self.invalidate();
            }
            Cmd::RemovePageNumbers => {
                self.checkpoint("pagenum");
                self.doc.remove_page_numbers();
                self.invalidate();
            }
            Cmd::InsertDateTime => {
                let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
                let (y, m, d) = civil_from_days(secs / 86400);
                self.type_text(&format!("{}/{}/{}", m, d, y));
            }
            Cmd::Margins(t, r, b, l) => self.section_edit(&move |s| edit::set_margins(s, t, r, b, l)),
            Cmd::Orientation(landscape) => {
                let sect = self.doc.sections();
                let idx = self.caret.path[0];
                let s = sect.iter().find(|(end, _)| *end >= idx).map(|x| x.1.clone()).unwrap_or_default();
                if s.landscape != landscape || (landscape != (s.page_w > s.page_h)) {
                    let (w, h) = if landscape { (s.page_w.max(s.page_h), s.page_w.min(s.page_h)) } else { (s.page_w.min(s.page_h), s.page_w.max(s.page_h)) };
                    self.section_edit(&move |sp| edit::set_page_size(sp, w, h, landscape));
                }
            }
            Cmd::PageSize(w, h) => {
                let landscape = self.doc.sections().last().map(|s| s.1.landscape).unwrap_or(false);
                let (w, h) = if landscape { (h, w) } else { (w, h) };
                self.section_edit(&move |sp| edit::set_page_size(sp, w, h, landscape));
            }
            Cmd::Columns(n) => self.section_edit(&move |sp| edit::set_columns(sp, n)),
            Cmd::PageColor(c) => {
                self.checkpoint("design");
                match c {
                    Some(h) => {
                        self.doc.shell.set_child(Element::new("w:background").with_attr("w:color", h.as_str()), &["w:background", "w:body"]);
                    }
                    None => self.doc.shell.remove_child("w:background"),
                }
                self.invalidate();
            }
            Cmd::Zoom(z) => self.zoom = z.clamp(0.1, 5.0),
            Cmd::ZoomOnePage | Cmd::ZoomPageWidth => {
                if let Some(p) = self.layout.as_ref().and_then(|l| l.pages.first()) {
                    let k = 96.0 / 72.0;
                    self.zoom = if matches!(cmd, Cmd::ZoomPageWidth) {
                        ((self.view_width - 40.0) / (p.width * k)).clamp(0.1, 5.0)
                    } else {
                        (((self.view_width - 40.0) / (p.width * k)).min(700.0 / (p.height * k))).clamp(0.1, 5.0)
                    };
                }
            }
            Cmd::ToggleRuler => {
                self.show_ruler = !self.show_ruler;
                let on = self.show_ruler;
                self.change_settings(|s| s.show_ruler = on);
            }
            Cmd::ToggleNav => {
                self.show_nav = !self.show_nav;
                let on = self.show_nav;
                self.change_settings(|s| s.show_nav = on);
            }
            Cmd::ToggleRibbon => {
                self.ribbon_peek = false;
                self.change_settings(|s| s.ribbon_collapsed = !s.ribbon_collapsed);
            }
            Cmd::DarkPage => {
                self.dark_page = !self.dark_page;
                self.textures.clear();
            }
            Cmd::WordCount => self.dialog = Dialog::WordCount,
            Cmd::About => {
                self.dialog = Dialog::Message(
                    tr("About RustyDocumentWriter").into(),
                    tr("RustyDocumentWriter — an open-source word processor built around faithful DOCX layout.\n\nWritten in Rust. Layout follows the format's real-world rules (GDI line metrics, tab, spacing, widow/orphan and table conventions) and documents round-trip without losing unknown content.").into(),
                )
            }
            Cmd::Unsupported(what) => {
                self.dialog = Dialog::Message(tr("Not available yet").into(), tf("“{}” isn't implemented in this version of RustyDocumentWriter yet. Any existing content of this kind in your documents is preserved when you save.", &[&tr(what)]))
            }
        }
    }

    /// Change preferences and remember them (they take effect on the next frame).
    pub fn change_settings(&mut self, f: impl FnOnce(&mut settings::Settings)) {
        let before = self.settings.clone();
        f(&mut self.settings);
        if self.settings != before {
            self.settings.save();
        }
        if self.settings.language != before.language {
            // Labels built from document state ("Aptos (Body)") are refreshed in the new language.
            i18n::set(i18n::resolve(&self.settings.language));
            self.update_toolbar();
        }
    }

    /// Word count, cached per layout version (the status bar asks for it every frame).
    pub fn word_count(&self) -> usize {
        let (v, n) = self.word_cache.get();
        if v == self.layout_version && !self.layout_dirty {
            return n;
        }
        let n = self.doc.word_count();
        self.word_cache.set((self.layout_version, n));
        n
    }
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.frame_count += 1;
        // Window close with unsaved changes.
        if ctx.input(|i| i.viewport().close_requested()) && self.modified && !matches!(self.dialog, Dialog::ConfirmClose { .. }) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.dialog = Dialog::ConfirmClose { then: Box::new(Cmd::Unsupported("__quit")) };
        }
        // Dropped files.
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
        if let Some(p) = dropped.into_iter().find(|p| files::is_openable(p)) {
            self.run(Cmd::OpenPath(p), &ctx);
        }
        let title = format!("{}{} - RustyDocumentWriter", self.doc.title(), if self.modified { "*" } else { "" });
        if self.frame_count % 30 == 1 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
        }

        self.ensure_layout();
        let mut cmds: Vec<Cmd> = Vec::new();
        // Theme (it may follow the system's appearance), accent and interface size.
        self.settings.apply(&ctx);
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::F1)) {
            cmds.push(Cmd::ToggleRibbon);
        }

        egui::Frame::new().fill(theme::pal().canvas).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            ribbon::window_chrome(self, ui, &title, &mut cmds);
            if self.backstage {
                backstage::show(self, ui, &mut cmds);
            } else {
                ribbon::tabs_row(self, ui, &mut cmds);
                if !self.settings.ribbon_collapsed || self.ribbon_peek {
                    ribbon::ribbon(self, ui, &mut cmds);
                }
                if self.settings.show_status_bar {
                    ribbon::status_bar(self, ui, &mut cmds);
                }
                canvas::document_area(self, ui, &mut cmds);
            }
        });
        ribbon::resize_edges(&ctx);
        self.poll_job(&ctx);
        self.dialogs(&ctx, &mut cmds);
        wordart_ui::dialog(self, &ctx, &mut cmds);

        for c in cmds {
            self.run(c, &ctx);
        }
        if self.layout_dirty {
            ctx.request_repaint();
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        theme::pal().canvas.to_normalized_gamma_f32()
    }
}

impl App {
    fn dialogs(&mut self, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
        let mut close = false;
        match &self.dialog {
            Dialog::None => {}
            Dialog::Message(title, body) => {
                let (title, body) = (title.clone(), body.clone());
                egui::Modal::new(egui::Id::NULL.with("msg")).show(ctx, |ui| {
                    ui.set_width(380.0);
                    ui.heading(title);
                    ui.add_space(6.0);
                    ui.label(body);
                    ui.add_space(10.0);
                    if ui.button(format!("  {}  ", tr("OK"))).clicked() {
                        close = true;
                    }
                });
            }
            Dialog::WordCount => {
                let text = self.doc.plain_text();
                let words = self.word_count();
                let chars = text.chars().filter(|c| *c != '\n').count();
                let chars_ns = text.chars().filter(|c| !c.is_whitespace()).count();
                let paras = text.lines().filter(|l| !l.trim().is_empty()).count();
                let pages = self.layout.as_ref().map(|l| l.pages.len()).unwrap_or(1);
                egui::Modal::new(egui::Id::NULL.with("wc")).show(ctx, |ui| {
                    ui.set_width(300.0);
                    ui.heading(tr("Word Count"));
                    ui.add_space(6.0);
                    egui::Grid::new("wcg").num_columns(2).spacing([40.0, 6.0]).show(ui, |ui| {
                        for (k, v) in [("Pages", pages), ("Words", words), ("Characters (no spaces)", chars_ns), ("Characters (with spaces)", chars), ("Paragraphs", paras)] {
                            ui.label(tr(k));
                            ui.label(v.to_string());
                            ui.end_row();
                        }
                    });
                    ui.add_space(10.0);
                    if ui.button(format!("  {}  ", tr("Close"))).clicked() {
                        close = true;
                    }
                });
            }
            Dialog::ConfirmClose { then } => {
                let then = (**then).clone();
                let name = self.doc.title();
                egui::Modal::new(egui::Id::NULL.with("confirm")).show(ctx, |ui| {
                    ui.set_width(380.0);
                    ui.heading("RustyDocumentWriter");
                    ui.add_space(6.0);
                    ui.label(tf("Do you want to save changes to “{}”?", &[&name]));
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.button(format!("  {}  ", tr("Save"))).clicked() && self.save(false) {
                            self.modified = false;
                            close = true;
                            cmds.push(then.clone());
                        }
                        if ui.button(format!("  {}  ", tr("Don't Save"))).clicked() {
                            self.modified = false;
                            close = true;
                            cmds.push(then.clone());
                        }
                        if ui.button(format!("  {}  ", tr("Cancel"))).clicked() {
                            close = true;
                        }
                    });
                });
                if close {
                    if let Some(Cmd::Unsupported("__quit")) = cmds.last() {
                        cmds.pop();
                        if !self.modified {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                }
            }
            Dialog::Find => {
                let mut open = true;
                egui::Window::new(tr("Find and Replace")).open(&mut open).collapsible(false).resizable(false).default_pos([500.0, 220.0]).show(ctx, |ui| {
                    egui::Grid::new("fr").num_columns(2).show(ui, |ui| {
                        ui.label(tr("Find what:"));
                        let r = ui.add(egui::TextEdit::singleline(&mut self.find_text).desired_width(240.0));
                        if self.frame_count % 1000 == 0 {
                            r.request_focus();
                        }
                        ui.end_row();
                        ui.label(tr("Replace with:"));
                        ui.add(egui::TextEdit::singleline(&mut self.replace_text).desired_width(240.0));
                        ui.end_row();
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button(tr("Find Next")).clicked() {
                            self.find_next();
                        }
                        if ui.button(tr("Replace")).clicked() {
                            self.replace_one();
                        }
                        if ui.button(tr("Replace All")).clicked() {
                            let n = self.replace_all();
                            self.dialog = Dialog::Message(tr("Replace All").into(), tf("RustyDocumentWriter made {} replacement(s).", &[&n]));
                        }
                    });
                });
                if !open {
                    close = true;
                }
            }
        }
        if close {
            self.dialog = Dialog::None;
        }
    }

    fn find_next(&mut self) -> bool {
        if self.find_text.is_empty() {
            return false;
        }
        let needle: Vec<char> = self.find_text.to_lowercase().chars().collect();
        let paths = edit::para_paths(&self.doc.body);
        let start = self.sel().1;
        let n = paths.len();
        let si = paths.iter().position(|p| *p == start.path).unwrap_or(0);
        for k in 0..=n {
            let pi = (si + k) % n.max(1);
            let Some(path) = paths.get(pi) else { break };
            let text: Vec<char> = edit::para_in(&self.doc.body, path).map(|p| p.plain_text().to_lowercase()).unwrap_or_default().chars().collect();
            let from = if k == 0 { start.off } else { 0 };
            if text.len() >= needle.len() {
                for i in from..=text.len() - needle.len() {
                    if text[i..i + needle.len()] == needle[..] {
                        self.anchor = Pos::body(path.clone(), i);
                        self.caret = Pos::body(path.clone(), i + needle.len());
                        self.scroll_to_caret = true;
                        return true;
                    }
                }
            }
        }
        false
    }

    fn replace_one(&mut self) {
        let (a, b) = self.sel();
        let sel = edit::selected_text(&self.doc, &a, &b);
        if !sel.is_empty() && sel.to_lowercase() == self.find_text.to_lowercase() {
            let r = self.replace_text.clone();
            self.type_text(&r);
        }
        self.find_next();
    }

    fn replace_all(&mut self) -> usize {
        if self.find_text.is_empty() {
            return 0;
        }
        self.checkpoint("replace");
        let mut count = 0;
        let needle: Vec<char> = self.find_text.to_lowercase().chars().collect();
        for path in edit::para_paths(&self.doc.body) {
            loop {
                let Some(p) = edit::para_in(&self.doc.body, &path) else { break };
                let text: Vec<char> = p.plain_text().to_lowercase().chars().collect();
                let found = (0..=text.len().saturating_sub(needle.len())).find(|&i| text.len() >= needle.len() && text[i..i + needle.len()] == needle[..]);
                let Some(i) = found else { break };
                let a = Pos::body(path.clone(), i);
                let b = Pos::body(path.clone(), i + needle.len());
                let rpr = p.rpr_at(i + 1);
                edit::delete_range(&mut self.doc, &a, &b);
                edit::insert_text(&mut self.doc, &a, &self.replace_text.clone(), rpr);
                count += 1;
                if count > 100_000 || self.replace_text.to_lowercase().contains(&self.find_text.to_lowercase()) {
                    break;
                }
            }
        }
        self.invalidate();
        count
    }
}

#[allow(dead_code)]
fn _assert_paragraph(_: &Paragraph) {}
