//! Page view: renders pages as textures and handles caret, selection and input.

use super::ribbon::STATUS_H;
use super::theme::{self, *};
use super::{App, Cmd};
use egui::{pos2, vec2, Align2, Color32, Event, Key, Rect, Sense, Stroke, Ui};
use std::time::{Duration, Instant};
use rusty_document_writer::edit::{self, Pos};
use rusty_document_writer::layout::{LineInfo, Story};

const PAGE_GAP: f32 = 16.0;
const TOP_PAD: f32 = 20.0;

fn k(app: &App) -> f32 {
    app.zoom * 96.0 / 72.0
}

/// Page rectangles in content coordinates.
fn page_rects(app: &App, view_w: f32) -> (Vec<Rect>, egui::Vec2) {
    let k = k(app);
    let mut rects = Vec::new();
    let mut y = TOP_PAD;
    let mut max_w: f32 = 0.0;
    if let Some(l) = &app.layout {
        for p in &l.pages {
            max_w = max_w.max(p.width * k);
        }
        let content_w = view_w.max(max_w + 40.0);
        for p in &l.pages {
            let w = p.width * k;
            let h = p.height * k;
            rects.push(Rect::from_min_size(pos2((content_w - w) / 2.0, y), vec2(w, h)));
            y += h + PAGE_GAP;
        }
        return (rects, vec2(content_w, y + TOP_PAD));
    }
    (rects, vec2(view_w, 100.0))
}

fn dark_transform(px: &mut [u8]) {
    for c in px.chunks_exact_mut(4) {
        let (r, g, b) = (c[0] as i32, c[1] as i32, c[2] as i32);
        let mx = r.max(g).max(b);
        let mn = r.min(g).min(b);
        let f = |v: i32| -> u8 {
            let inv = v + 255 - mx - mn;
            (0x1E + inv * (0xE6 - 0x1E) / 255).clamp(0, 255) as u8
        };
        c[0] = f(r);
        c[1] = f(g);
        c[2] = f(b);
    }
}

fn ensure_texture(app: &mut App, ctx: &egui::Context, idx: usize, scale: f32) -> Option<egui::TextureId> {
    let skey = (scale * 1000.0) as u32 ^ if app.dark_page { 1 << 31 } else { 0 };
    if let Some((t, s, v)) = app.textures.get(&idx) {
        if *s == skey && *v == app.layout_version {
            return Some(t.id());
        }
    }
    let page = app.layout.as_ref()?.pages.get(idx)?;
    let mut raster = app.renderer.render(page, scale);
    if app.dark_page {
        dark_transform(&mut raster.pixels);
    }
    let img = egui::ColorImage::from_rgba_unmultiplied([raster.width as usize, raster.height as usize], &raster.pixels);
    let opts = egui::TextureOptions::LINEAR;
    match app.textures.get_mut(&idx) {
        Some((t, s, v)) => {
            t.set(img, opts);
            *s = skey;
            *v = app.layout_version;
            Some(t.id())
        }
        None => {
            let t = ctx.load_texture(format!("page-{idx}"), img, opts);
            let id = t.id();
            app.textures.insert(idx, (t, skey, app.layout_version));
            Some(id)
        }
    }
}

// ---------------------------------------------------------------------------
// Hit testing

fn hit_line<'a>(lines: &'a [LineInfo], story_filter: &dyn Fn(&Story) -> bool, x: f32, y: f32) -> Option<&'a LineInfo> {
    let mut best: Option<(f32, &LineInfo)> = None;
    for l in lines.iter().filter(|l| story_filter(&l.story)) {
        let dy = if y < l.top {
            l.top - y
        } else if y > l.bottom {
            y - l.bottom
        } else {
            0.0
        };
        let dx = if x < l.left - 40.0 {
            l.left - 40.0 - x
        } else if x > l.right + 40.0 {
            x - l.right - 40.0
        } else {
            0.0
        };
        let d = dy * 4.0 + dx;
        if best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, l));
        }
    }
    best.map(|b| b.1)
}

fn hit(app: &App, rects: &[Rect], origin: egui::Pos2, p: egui::Pos2, story: Option<&Story>) -> Option<Pos> {
    let layout = app.layout.as_ref()?;
    let k = k(app);
    // Nearest page vertically.
    let mut pi = 0;
    let mut bd = f32::MAX;
    for (i, r) in rects.iter().enumerate() {
        let r = r.translate(origin.to_vec2());
        let d = if p.y < r.top() { r.top() - p.y } else if p.y > r.bottom() { p.y - r.bottom() } else { 0.0 };
        if d < bd {
            bd = d;
            pi = i;
        }
    }
    let r = rects.get(pi)?.translate(origin.to_vec2());
    let x = (p.x - r.left()) / k;
    let y = (p.y - r.top()) / k;
    let page = &layout.pages[pi];
    let filter: Box<dyn Fn(&Story) -> bool> = match story {
        Some(s) => {
            let s = s.clone();
            Box::new(move |t: &Story| *t == s)
        }
        None => Box::new(|t: &Story| *t == Story::Body),
    };
    let line = hit_line(&page.lines, &*filter, x, y)?;
    Some(Pos { story: line.story.clone(), path: line.path.clone(), off: line.pos_at_x(x) })
}

/// Lines adjacent to the caret line for up/down movement.
fn vertical_target(app: &App, dir: i32, x: f32) -> Option<Pos> {
    let layout = app.layout.as_ref()?;
    let (pi, cur) = app.locate(&app.caret)?;
    let story = cur.story.clone();
    let (cur_top, cur_bottom) = (cur.top, cur.bottom);
    let mut pages: Vec<usize> = if dir > 0 { (pi..layout.pages.len()).collect() } else { (0..=pi).rev().collect() };
    if story != Story::Body {
        pages = vec![pi];
    }
    for p in pages {
        let page = &layout.pages[p];
        let mut best: Option<(f32, &LineInfo)> = None;
        for l in page.lines.iter().filter(|l| l.story == story) {
            let ok = if p != pi {
                true
            } else if dir > 0 {
                l.top >= cur_bottom - 0.5 && !(l.path == cur.path && l.start == cur.start)
            } else {
                l.bottom <= cur_top + 0.5 && !(l.path == cur.path && l.start == cur.start)
            };
            if !ok {
                continue;
            }
            let horiz = if x < l.left { l.left - x } else if x > l.right { x - l.right } else { 0.0 };
            let vert = if dir > 0 { l.top } else { -l.bottom };
            let score = vert + horiz * 0.5;
            if best.is_none_or(|(s, _)| score < s) {
                best = Some((score, l));
            }
        }
        if let Some((_, l)) = best {
            return Some(Pos { story: l.story.clone(), path: l.path.clone(), off: l.pos_at_x(x) });
        }
    }
    None
}

fn line_home_end(app: &App, end: bool) -> Option<Pos> {
    let (_, l) = app.locate(&app.caret)?;
    let off = if end {
        if l.end < l.para_len { l.end.saturating_sub(1).max(l.start) } else { l.end }
    } else {
        l.start
    };
    Some(Pos { off, ..app.caret.clone() })
}

// ---------------------------------------------------------------------------
// Keyboard

fn handle_keys(app: &mut App, ui: &mut Ui, cmds: &mut Vec<Cmd>, page_h_pt: f32) {
    let events = ui.input(|i| i.events.clone());
    let mut consumed = false;
    for ev in events {
        match ev {
            Event::Text(t) => {
                let mods = ui.input(|i| i.modifiers);
                if mods.command && !mods.alt {
                    continue;
                }
                let t: String = t.chars().filter(|c| !c.is_control()).collect();
                if !t.is_empty() {
                    app.type_text(&t);
                }
                consumed = true;
            }
            Event::Ime(egui::ImeEvent::Commit(t)) => {
                app.type_text(&t);
                consumed = true;
            }
            Event::Copy => cmds.push(Cmd::Copy),
            Event::Cut => cmds.push(Cmd::Cut),
            Event::Paste(t) => {
                app.type_text(&t.replace("\r\n", "\n").replace('\r', "\n"));
                consumed = true;
            }
            Event::Key { key, pressed: true, modifiers: m, .. } => {
                consumed = true;
                let shift = m.shift;
                let ctrl = m.command;
                match key {
                    Key::ArrowLeft | Key::ArrowRight => {
                        let right = key == Key::ArrowRight;
                        if app.has_sel() && !shift {
                            let (a, b) = app.sel();
                            app.set_caret(if right { b } else { a }, false);
                            continue;
                        }
                        let c = app.caret.clone();
                        let len = edit::para_len(&app.doc, &c.story, &c.path);
                        let np = if ctrl {
                            let p = edit::para(&app.doc, &c);
                            let off = p.map(|p| if right { edit::word_right(p, c.off) } else { edit::word_left(p, c.off) }).unwrap_or(c.off);
                            if off == c.off { None } else { Some(Pos { off, ..c.clone() }) }
                        } else if right && c.off < len {
                            Some(Pos { off: c.off + 1, ..c.clone() })
                        } else if !right && c.off > 0 {
                            Some(Pos { off: c.off - 1, ..c.clone() })
                        } else {
                            None
                        };
                        let np = np.or_else(|| {
                            if right {
                                edit::next_para(&app.doc, &c.story, &c.path).map(|p| Pos { story: c.story.clone(), path: p, off: 0 })
                            } else {
                                edit::prev_para(&app.doc, &c.story, &c.path).map(|p| {
                                    let l = edit::para_len(&app.doc, &c.story, &p);
                                    Pos { story: c.story.clone(), path: p, off: l }
                                })
                            }
                        });
                        if let Some(p) = np {
                            app.set_caret(p, shift);
                        }
                        app.desired_x = None;
                    }
                    Key::ArrowUp | Key::ArrowDown => {
                        let x = app.desired_x.or_else(|| app.locate(&app.caret).map(|(_, l)| l.caret_x(app.caret.off)));
                        if let Some(x) = x {
                            if ctrl {
                                // Paragraph up/down.
                                let c = app.caret.clone();
                                let t = if key == Key::ArrowDown { edit::next_para(&app.doc, &c.story, &c.path) } else if c.off > 0 { Some(c.path.clone()) } else { edit::prev_para(&app.doc, &c.story, &c.path) };
                                if let Some(p) = t {
                                    app.set_caret(Pos { story: c.story.clone(), path: p, off: 0 }, shift);
                                }
                            } else if let Some(p) = vertical_target(app, if key == Key::ArrowDown { 1 } else { -1 }, x) {
                                app.set_caret(p, shift);
                            } else if key == Key::ArrowDown {
                                let e = edit::end_of_doc(&app.doc);
                                if app.caret.story == Story::Body {
                                    app.set_caret(e, shift);
                                }
                            } else if app.caret.story == Story::Body {
                                let s = edit::start_of_doc(&app.doc);
                                app.set_caret(s, shift);
                            }
                            app.desired_x = Some(x);
                        }
                    }
                    Key::Home | Key::End => {
                        let p = if ctrl {
                            Some(if key == Key::Home { edit::start_of_doc(&app.doc) } else { edit::end_of_doc(&app.doc) })
                        } else {
                            line_home_end(app, key == Key::End)
                        };
                        if let Some(p) = p {
                            app.set_caret(p, shift);
                        }
                        app.desired_x = None;
                    }
                    Key::PageUp | Key::PageDown => {
                        let x = app.locate(&app.caret).map(|(_, l)| l.caret_x(app.caret.off)).unwrap_or(0.0);
                        let steps = (page_h_pt / 15.0).max(5.0) as usize;
                        for _ in 0..steps {
                            match vertical_target(app, if key == Key::PageDown { 1 } else { -1 }, x) {
                                Some(p) => {
                                    app.caret = edit::clamp(&app.doc, &p);
                                }
                                None => break,
                            }
                        }
                        let c = app.caret.clone();
                        app.set_caret(c, shift);
                    }
                    Key::Enter => {
                        if ctrl {
                            app.insert_break(Some("page"));
                        } else if shift {
                            app.insert_break(None);
                        } else {
                            app.enter();
                        }
                    }
                    Key::Backspace => app.backspace(ctrl),
                    Key::Delete => app.delete(ctrl),
                    Key::Tab => {
                        let at_start = app.caret.off == 0 && !app.has_sel();
                        if app.tb.list.is_some() && at_start {
                            cmds.push(if shift { Cmd::IndentDec } else { Cmd::IndentInc });
                        } else if !shift {
                            app.type_text("\t");
                        }
                    }
                    Key::Escape => {
                        let c = app.caret.clone();
                        app.set_caret(c, false);
                        if app.caret.story != Story::Body {
                            let s = edit::start_of_doc(&app.doc);
                            app.set_caret(s, false);
                        }
                    }
                    Key::F12 => cmds.push(Cmd::SaveAs),
                    _ if ctrl => {
                        match key {
                            Key::A => cmds.push(Cmd::SelectAll),
                            Key::B => cmds.push(Cmd::Toggle("w:b")),
                            Key::I => cmds.push(Cmd::Toggle("w:i")),
                            Key::U => cmds.push(Cmd::Underline),
                            Key::Z => cmds.push(Cmd::Undo),
                            Key::Y => cmds.push(Cmd::Redo),
                            Key::S => cmds.push(if shift { Cmd::SaveAs } else { Cmd::Save }),
                            Key::O => cmds.push(Cmd::Open),
                            Key::N => cmds.push(Cmd::New),
                            Key::E => cmds.push(Cmd::Align("center")),
                            Key::L => cmds.push(Cmd::Align("left")),
                            Key::R => cmds.push(Cmd::Align("right")),
                            Key::J => cmds.push(Cmd::Align("both")),
                            Key::F | Key::H => cmds.push(Cmd::Find),
                            Key::CloseBracket => cmds.push(Cmd::GrowFont),
                            Key::OpenBracket => cmds.push(Cmd::ShrinkFont),
                            Key::Equals | Key::Plus => cmds.push(Cmd::VertAlign(if shift { "superscript" } else { "subscript" })),
                            Key::Num1 => cmds.push(Cmd::LineSpacing(1.0)),
                            Key::Num2 => cmds.push(Cmd::LineSpacing(2.0)),
                            Key::Num5 => cmds.push(Cmd::LineSpacing(1.5)),
                            Key::Num8 if shift => cmds.push(Cmd::ShowMarks),
                            Key::Space => cmds.push(Cmd::ClearFormat),
                            _ => consumed = false,
                        }
                    }
                    _ => consumed = false,
                }
            }
            _ => {}
        }
    }
    if consumed {
        app.caret_time = Instant::now();
    }
    // Don't let the scroll area or widgets react to keys we've handled.
    ui.input_mut(|i| i.events.retain(|e| !matches!(e, Event::Key { .. } | Event::Text(_) | Event::Paste(_) | Event::Ime(_))));
}

// ---------------------------------------------------------------------------
// Main view

pub fn document_area(app: &mut App, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
    let full = ui.max_rect();
    let top = ui.cursor().top();
    let mut area = Rect::from_min_max(pos2(full.left(), top), pos2(full.right(), full.bottom() - STATUS_H));
    if area.height() < 40.0 {
        return;
    }
    let ctx = ui.ctx().clone();

    // Navigation pane.
    if app.show_nav {
        let nav = Rect::from_min_max(area.left_top(), pos2(area.left() + 260.0, area.bottom()));
        area.min.x = nav.right();
        let mut nui = ui.new_child(egui::UiBuilder::new().max_rect(nav.shrink(8.0)));
        nui.painter().rect_filled(nav, 0.0, Color32::from_rgb(0x20, 0x20, 0x20));
        nui.label(egui::RichText::new("Navigation").size(16.0).color(TEXT));
        nui.add_space(6.0);
        let mut find = app.find_text.clone();
        if nui.add(egui::TextEdit::singleline(&mut find).hint_text("Search document").desired_width(f32::INFINITY)).changed() {
            app.find_text = find;
        }
        nui.add_space(6.0);
        let outline = app.layout.as_ref().map(|l| l.outline.clone()).unwrap_or_default();
        egui::ScrollArea::vertical().id_salt("nav").show(&mut nui, |ui| {
            if outline.is_empty() {
                ui.label(egui::RichText::new("Create an interactive outline of your document by applying Heading styles.").color(TEXT_DIM).size(12.0));
            }
            for (text, lvl, _page, _y, path) in outline {
                ui.horizontal(|ui| {
                    ui.add_space(lvl as f32 * 14.0);
                    let label = if text.chars().count() > 40 { format!("{}…", text.chars().take(40).collect::<String>()) } else { text.clone() };
                    if ui.add(egui::Button::new(egui::RichText::new(label).size(12.5)).frame(false)).clicked() {
                        app.set_caret(Pos::body(path.clone(), 0), false);
                    }
                });
            }
        });
    }

    // Ruler.
    if app.show_ruler {
        let rr = Rect::from_min_max(area.left_top(), pos2(area.right(), area.top() + 22.0));
        area.min.y = rr.bottom();
        draw_ruler(app, ui, rr);
    }

    app.view_width = area.width();
    let doc_focus = !app.text_focus && matches!(app.dialog, super::Dialog::None) && !app.backstage;
    let page_h = app.layout.as_ref().and_then(|l| l.pages.first()).map(|p| p.height * 0.8).unwrap_or(600.0);
    let any_popup = egui::Popup::is_any_open(&ctx);
    if doc_focus && !any_popup {
        handle_keys(app, ui, cmds, page_h);
    }
    if app.layout_dirty {
        app.ensure_layout();
    }

    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area));
    child.set_clip_rect(area);
    let (rects, content) = page_rects(app, area.width());
    let mut sa = egui::ScrollArea::both().auto_shrink([false, false]).id_salt("doc-scroll");
    if let Some(y) = app.pending_scroll.take() {
        sa = sa.vertical_scroll_offset(y);
    }
    // Ctrl + wheel zoom.
    if child.rect_contains_pointer(area) {
        let zd = ctx.input(|i| i.zoom_delta());
        if (zd - 1.0).abs() > 0.001 {
            app.zoom = (app.zoom * zd).clamp(0.1, 5.0);
        }
    }
    let ppp = ctx.pixels_per_point();
    sa.show_viewport(&mut child, |ui, viewport| {
        let (_, resp) = ui.allocate_exact_size(content, Sense::click_and_drag());
        let origin = resp.rect.min;
        let painter = ui.painter().clone();
        let kk = k(app);
        let scale = kk * ppp;
        let mut rendered = 0;
        let skey = (scale * 1000.0) as u32 ^ if app.dark_page { 1 << 31 } else { 0 };
        // While the zoom is still moving (Ctrl+wheel, slider drag) re-rasterising every
        // frame is wasted work: stretch the existing texture and render once it settles.
        if (app.zoom - app.last_zoom).abs() > f32::EPSILON {
            app.last_zoom = app.zoom;
            app.zoom_changed = Instant::now();
        }
        let zoom_settling = app.zoom_changed.elapsed() < Duration::from_millis(150);
        if zoom_settling {
            ctx.request_repaint_after(Duration::from_millis(160));
        }
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        let keep = viewport.expand2(vec2(0.0, viewport.height().max(400.0) * 2.0));
        for (i, r) in rects.iter().enumerate() {
            if !r.intersects(viewport.expand(200.0)) {
                continue;
            }
            let sr = r.translate(origin.to_vec2());
            painter.rect_filled(sr.translate(vec2(0.0, 2.0)).expand(1.5), 0.0, Color32::from_black_alpha(90));
            let cached = app.textures.get(&i).map(|(t, s, v)| (t.id(), *s, *v));
            let up_to_date = cached.is_some_and(|(_, s, v)| v == app.layout_version && s == skey);
            let same_content = cached.is_some_and(|(_, s, v)| v == app.layout_version && (s >> 31) == (skey >> 31));
            if !up_to_date && (rendered >= 3 || (zoom_settling && same_content)) {
                // Defer: show the previous rendering (or a blank page) now, render later.
                match cached {
                    Some((tid, _, _)) => {
                        painter.image(tid, sr, uv, Color32::WHITE);
                    }
                    None => {
                        painter.rect_filled(sr, 0.0, if app.dark_page { Color32::from_gray(0x1E) } else { Color32::WHITE });
                    }
                }
                if rendered >= 3 {
                    ctx.request_repaint();
                }
                continue;
            }
            if !up_to_date {
                rendered += 1;
            }
            if let Some(tid) = ensure_texture(app, &ctx, i, scale) {
                painter.image(tid, sr, uv, Color32::WHITE);
            }
        }
        // Free page textures well outside the view (and those of pages that no longer exist).
        app.textures.retain(|i, _| rects.get(*i).is_some_and(|r| r.intersects(keep)));

        // Selection.
        let layout = app.layout.as_ref();
        if let (Some(layout), true) = (layout, app.has_sel()) {
            let (a, b) = app.sel();
            let sel_color = if app.dark_page { Color32::from_rgba_unmultiplied(0x60, 0x90, 0xD0, 0x60) } else { Color32::from_rgba_unmultiplied(0x30, 0x30, 0x30, 0x40) };
            for (pi, page) in layout.pages.iter().enumerate() {
                let Some(r) = rects.get(pi) else { continue };
                if !r.intersects(viewport) {
                    continue;
                }
                let sr = r.translate(origin.to_vec2());
                for l in page.lines.iter().filter(|l| l.story == a.story) {
                    if l.path < a.path || l.path > b.path {
                        continue;
                    }
                    let from = if l.path == a.path { a.off.max(l.start) } else { l.start };
                    let to = if l.path == b.path { b.off.min(l.end) } else { l.end };
                    if from > to || (from == to && !(l.path < b.path && to == l.end)) {
                        continue;
                    }
                    if l.path == a.path && a.off > l.end {
                        continue;
                    }
                    if l.path == b.path && b.off < l.start {
                        continue;
                    }
                    let x0 = l.caret_x(from);
                    let mut x1 = l.caret_x(to);
                    if l.path < b.path && to >= l.end && l.end == l.para_len {
                        x1 += 5.0; // paragraph mark
                    }
                    if l.end < l.para_len && to >= l.end && (l.path < b.path || b.off > l.end) {
                        x1 = x1.max(l.caret_x(l.end));
                    }
                    let rect = Rect::from_min_max(pos2(sr.left() + x0 * kk, sr.top() + l.top * kk), pos2(sr.left() + x1 * kk, sr.top() + l.bottom * kk));
                    painter.rect_filled(rect, 0.0, sel_color);
                }
            }
        }

        // Header/footer editing indicator.
        if app.caret.story != Story::Body {
            if let Some((pi, _)) = app.locate(&app.caret) {
                if let (Some(r), Some(page)) = (rects.get(pi), layout.and_then(|l| l.pages.get(pi))) {
                    let sr = r.translate(origin.to_vec2());
                    let is_footer = app.locate(&app.caret).is_some_and(|(_, l)| l.top > page.height / 2.0);
                    let y = if is_footer { sr.top() + page.footer_area.0 * kk } else { sr.top() + page.header_area.1 * kk };
                    let mut x = sr.left();
                    while x < sr.right() {
                        painter.hline(x..=(x + 4.0).min(sr.right()), y, Stroke::new(1.0, ACCENT));
                        x += 8.0;
                    }
                    let label = if is_footer { "Footer" } else { "Header" };
                    let lr = Rect::from_min_size(pos2(sr.left() + 4.0, if is_footer { y - 18.0 } else { y }), vec2(52.0, 18.0));
                    painter.rect_filled(lr, 2.0, Color32::from_rgb(0xD6, 0xE4, 0xF5));
                    painter.text(lr.center(), Align2::CENTER_CENTER, label, theme::ui_font(11.0), Color32::from_rgb(0x1F, 0x4E, 0x79));
                }
            }
        }

        // Caret.
        if let Some((pi, l)) = app.locate(&app.caret) {
            if let Some(r) = rects.get(pi) {
                let sr = r.translate(origin.to_vec2());
                let x = sr.left() + l.caret_x(app.caret.off) * kk;
                let font_h = (l.bottom - l.top).min((l.baseline - l.top) * 1.35 + 2.0);
                let y0 = sr.top() + (l.baseline - font_h * 0.78).max(l.top) * kk;
                let y1 = sr.top() + (l.baseline + font_h * 0.24).min(l.bottom) * kk;
                let caret_rect = Rect::from_min_max(pos2(x - 0.5, y0), pos2(x + 1.0, y1));
                let blink = app.caret_time.elapsed().as_millis() / 530 % 2 == 0;
                if blink && doc_focus && !app.has_sel() {
                    painter.rect_filled(caret_rect, 0.0, if app.dark_page { Color32::WHITE } else { Color32::BLACK });
                }
                ctx.request_repaint_after(Duration::from_millis(530));
                if app.scroll_to_caret {
                    ui.scroll_to_rect(caret_rect.expand2(vec2(40.0, 60.0)), None);
                    app.scroll_to_caret = false;
                }
                if doc_focus {
                    ctx.output_mut(|o| o.ime = Some(egui::output::IMEOutput { purpose: Default::default(), rect: area, cursor_rect: caret_rect, should_interrupt_composition: false }));
                }
            }
        }

        // Mouse.
        if resp.hovered() {
            ctx.set_cursor_icon(egui::CursorIcon::Text);
        }
        let shift = ctx.input(|i| i.modifiers.shift);
        if let Some(p) = resp.interact_pointer_pos() {
            if resp.double_clicked() {
                // In the header/footer zone: start editing it.
                let pi = rects.iter().position(|r| r.translate(origin.to_vec2()).contains(p));
                let mut handled = false;
                if let (Some(pi), Some(layout)) = (pi, app.layout.as_ref()) {
                    let page = &layout.pages[pi];
                    let sr = rects[pi].translate(origin.to_vec2());
                    let y = (p.y - sr.top()) / kk;
                    let in_hdr = y < page.header_area.1;
                    let in_ftr = y > page.footer_area.0;
                    if in_hdr || in_ftr {
                        let target = page.lines.iter().find(|l| matches!(l.story, Story::HeaderFooter(_)) && ((in_hdr && l.top < page.header_area.1) || (in_ftr && l.top >= page.footer_area.0 - 1.0))).map(|l| l.story.clone());
                        if let Some(story) = target {
                            if let Some(pos) = hit(app, &rects, origin, p, Some(&story)) {
                                app.set_caret(pos, false);
                                handled = true;
                            }
                        }
                    } else if app.caret.story != Story::Body {
                        if let Some(pos) = hit(app, &rects, origin, p, None) {
                            app.set_caret(pos, false);
                        }
                    }
                }
                if !handled {
                    if let Some(pos) = hit(app, &rects, origin, p, Some(&app.caret.story.clone())) {
                        if let Some(par) = edit::para(&app.doc, &pos) {
                            let (s, e) = edit::word_at(par, pos.off);
                            app.anchor = Pos { off: s, ..pos.clone() };
                            app.caret = Pos { off: e, ..pos };
                        }
                    }
                }
                app.dragging = false;
            } else if resp.triple_clicked() {
                if let Some(pos) = hit(app, &rects, origin, p, Some(&app.caret.story.clone())) {
                    let len = edit::para_len(&app.doc, &pos.story, &pos.path);
                    app.anchor = Pos { off: 0, ..pos.clone() };
                    app.caret = Pos { off: len, ..pos };
                }
            } else if resp.drag_started() || resp.clicked() {
                let story = app.caret.story.clone();
                if let Some(pos) = hit(app, &rects, origin, p, Some(&story)) {
                    app.set_caret(pos, shift);
                    app.scroll_to_caret = false;
                    app.desired_x = None;
                }
                app.dragging = resp.drag_started();
                app.text_focus = false;
            } else if resp.dragged() && app.dragging {
                let story = app.caret.story.clone();
                if let Some(pos) = hit(app, &rects, origin, p, Some(&story)) {
                    app.caret = pos;
                }
                // Auto-scroll when dragging near the edges.
                if p.y > viewport.bottom() + origin.y - 10.0 || p.y < origin.y + viewport.top() + 10.0 {
                    ui.scroll_to_rect(Rect::from_center_size(p, vec2(10.0, 10.0)), None);
                }
            }
        }
        if resp.drag_stopped() {
            app.dragging = false;
            app.update_toolbar();
        }
        if resp.clicked() || resp.double_clicked() || resp.triple_clicked() {
            app.update_toolbar();
        }
    });
    // Any focused text field (font box, find box) owns the keyboard.
    app.text_focus = ctx.memory(|m| m.focused().is_some());
}

fn draw_ruler(app: &App, ui: &Ui, rr: Rect) {
    let p = ui.painter();
    p.rect_filled(rr, 0.0, Color32::from_rgb(0x1A, 0x1A, 0x1A));
    let Some(layout) = &app.layout else { return };
    let Some(page) = layout.pages.first() else { return };
    let k = k(app);
    let w = page.width * k;
    let content_w = rr.width().max(w + 40.0);
    let x0 = rr.left() + (content_w - w) / 2.0;
    let (_, mr, _, ml) = page.margins;
    let band = Rect::from_min_max(pos2(x0, rr.top() + 4.0), pos2(x0 + w, rr.bottom() - 4.0));
    p.rect_filled(band, 0.0, Color32::from_rgb(0x3A, 0x3A, 0x3A));
    p.rect_filled(Rect::from_min_max(pos2(x0 + ml * k, band.top()), pos2(x0 + w - mr * k, band.bottom())), 0.0, Color32::from_rgb(0x58, 0x58, 0x58));
    let mut i = 0;
    while (i as f32) * 9.0 <= page.width {
        let x = x0 + ml * k + (i as f32 * 9.0 - ml.min(page.width)) * k * 0.0 + (i as f32 * 9.0) * k - ml * k;
        let inch = i % 8 == 0;
        let half = i % 4 == 0;
        let rel = (i as f32 * 9.0 - ml) / 72.0;
        if inch && rel.abs() > 0.01 {
            p.text(pos2(x, band.center().y), Align2::CENTER_CENTER, format!("{}", rel.abs().round() as i32), theme::ui_font(9.5), TEXT_DIM);
        } else if half {
            p.vline(x, (band.center().y - 3.0)..=(band.center().y + 3.0), Stroke::new(1.0, TEXT_DIM));
        } else {
            p.vline(x, (band.center().y - 1.0)..=(band.center().y + 1.0), Stroke::new(1.0, TEXT_DIM));
        }
        i += 1;
    }
}
