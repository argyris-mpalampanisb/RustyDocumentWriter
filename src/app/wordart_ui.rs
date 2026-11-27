//! WordArt gallery and the "Edit WordArt" dialog.
//!
//! Previews are drawn by the document renderer itself, so what the gallery
//! shows is exactly what lands on the page.

use super::theme::*;
use super::{App, Cmd};
use egui::{vec2, Color32, Sense, Stroke, StrokeKind, Ui};
use rusty_document_writer::document::Document;
use rusty_document_writer::fonts::FontDb;
use rusty_document_writer::layout::{Item, Page, TextFx};
use rusty_document_writer::props::Color;
use rusty_document_writer::render::Renderer;
use rusty_document_writer::wordart::{WordArt, STYLES};
use rusty_document_writer::xml;

/// State of the open WordArt dialog.
pub struct WordArtEdit {
    /// The drawing being edited (body paragraph path, character offset), or `None` for a new one.
    pub target: Option<(Vec<usize>, usize)>,
    pub art: WordArt,
    preview: Option<(WordArt, egui::TextureHandle)>,
    first_frame: bool,
}

impl WordArtEdit {
    pub fn new(target: Option<(Vec<usize>, usize)>, art: WordArt) -> Self {
        WordArtEdit { target, art, preview: None, first_frame: true }
    }
}

/// Render WordArt text with the page renderer into an egui image (`scale` = pixels per point).
fn render(doc: &Document, fonts: &FontDb, art: &WordArt, scale: f32) -> egui::ColorImage {
    let style = STYLES.get(art.style).copied().unwrap_or(STYLES[0]);
    let x = format!(
        r#"<r xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml">{}</r>"#,
        style.rpr_xml(&art.font, art.size)
    );
    let root = xml::parse_str(&x).expect("WordArt rPr").root;
    let rpr = root.elements().next().cloned();
    let rp = doc.resolver().run("Normal", rpr.as_ref(), None);
    let fm = fonts.resolve(&rp.font_ascii, rp.bold, rp.italic);
    let size = rp.size;
    let line_h = fm.font.ascent(size) + fm.font.descent(size);
    let lines: Vec<&str> = if art.text.trim().is_empty() { vec!["Your text here"] } else { art.text.lines().take(6).collect() };
    let widths: Vec<f32> = lines.iter().map(|l| l.chars().map(|c| fm.font.advance(c, size)).sum()).collect();
    let text_w = widths.iter().cloned().fold(0.0, f32::max);
    // Room for glow, shadow, extrusion and reflection.
    let pad = size * 0.45;
    let refl = if style.reflection { size * 0.62 } else { 0.0 };
    let (page_w, page_h) = (text_w + pad * 2.0, line_h * lines.len() as f32 + pad * 2.0 + refl);
    let mut items = Vec::new();
    for (i, (l, w)) in lines.iter().zip(&widths).enumerate() {
        let mut x = (page_w - w) / 2.0;
        let mut glyphs = Vec::new();
        for c in l.chars() {
            glyphs.push((fm.font.glyph(c).0, x));
            x += fm.font.advance(c, size);
        }
        items.push(Item::Glyphs {
            font: fm.font.clone(),
            size,
            color: rp.color.unwrap_or(Color::BLACK),
            y: pad + line_h * i as f32 + fm.font.ascent(size),
            glyphs,
            synth_bold: fm.synth_bold,
            synth_italic: fm.synth_italic,
            scale_x: 1.0,
            fx: TextFx::of(&rp),
        });
    }
    let page = Page {
        width: page_w,
        height: page_h,
        items,
        lines: Vec::new(),
        number: 1,
        section: 0,
        background: None,
        margins: (0.0, 0.0, 0.0, 0.0),
        header_area: (0.0, 0.0),
        footer_area: (page_h, page_h),
        objects: Vec::new(),
    };
    let raster = Renderer::new().render(&page, scale);
    egui::ColorImage::from_rgba_unmultiplied([raster.width as usize, raster.height as usize], &raster.pixels)
}

/// Gallery tiles ("Aa" in every style), built once.
fn ensure_tiles(app: &mut App, ctx: &egui::Context) {
    if app.wordart_tiles.is_some() {
        return;
    }
    let tiles = (0..STYLES.len())
        .map(|i| {
            let art = WordArt { style: i, text: "Aa".into(), font: "Arial".into(), size: 30.0 };
            let img = render(&app.doc, &app.fonts, &art, 1.5);
            ctx.load_texture(format!("wordart-{i}"), img, egui::TextureOptions::LINEAR)
        })
        .collect();
    app.wordart_tiles = Some(tiles);
}

/// One gallery tile; returns true when clicked.
fn tile(ui: &mut Ui, tex: &egui::TextureHandle, name: &str, selected: bool) -> bool {
    let (rect, r) = ui.allocate_exact_size(vec2(66.0, 56.0), Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, 4.0, Color32::WHITE);
    if selected || r.hovered() {
        p.rect_stroke(rect, 4.0, Stroke::new(2.0, if selected { pal().accent_text } else { pal().accent }), StrokeKind::Outside);
    }
    let sz = tex.size_vec2();
    let k = (60.0 / sz.x).min(50.0 / sz.y).min(1.0);
    p.image(tex.id(), egui::Rect::from_center_size(rect.center(), sz * k), egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
    r.on_hover_text(name).clicked()
}

fn gallery(ui: &mut Ui, tiles: &[egui::TextureHandle], selected: Option<usize>, mut on_pick: impl FnMut(usize)) {
    egui::Grid::new(ui.next_auto_id()).spacing([4.0, 4.0]).show(ui, |ui| {
        for (i, tex) in tiles.iter().enumerate() {
            if tile(ui, tex, STYLES[i].name, selected == Some(i)) {
                on_pick(i);
            }
            if i % 4 == 3 {
                ui.end_row();
            }
        }
    });
}

/// The Insert ▸ WordArt dropdown: pick a style, then type the text in the dialog.
pub fn menu(app: &mut App, ui: &mut Ui, r: &egui::Response, cmds: &mut Vec<Cmd>) {
    if r.clicked() {
        ensure_tiles(app, ui.ctx());
    }
    let Some(tiles) = app.wordart_tiles.as_ref() else { return };
    egui::Popup::from_toggle_button_response(r).show(|ui| {
        ui.label(egui::RichText::new("WordArt").small().color(pal().text_dim));
        let mut pick = None;
        gallery(ui, tiles, None, |i| pick = Some(i));
        if let Some(i) = pick {
            cmds.push(Cmd::NewWordArt(i));
            ui.close();
        }
    });
}

/// The Edit WordArt dialog, when open.
pub fn dialog(app: &mut App, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
    if app.wordart_edit.is_none() {
        return;
    }
    ensure_tiles(app, ctx);
    let (doc, fonts) = (&app.doc, app.fonts.clone());
    let font_names = &app.font_names;
    let tiles = app.wordart_tiles.as_deref().unwrap_or_default();
    let ed = app.wordart_edit.as_mut().unwrap();
    // Live preview of the actual text, re-rendered only when something changes.
    if ed.preview.as_ref().is_none_or(|(a, _)| *a != ed.art) {
        let img = render(doc, &fonts, &ed.art, 1.0);
        ed.preview = Some((ed.art.clone(), ctx.load_texture("wordart-preview", img, egui::TextureOptions::LINEAR)));
    }
    let (mut ok, mut cancel) = (false, false);
    let title = if ed.target.is_some() { "Edit WordArt" } else { "Insert WordArt" };
    egui::Modal::new(egui::Id::NULL.with("wordart-dialog")).show(ctx, |ui| {
        ui.set_width(560.0);
        ui.heading(title);
        ui.add_space(6.0);
        ui.label("Text");
        let te = ui.add(egui::TextEdit::multiline(&mut ed.art.text).desired_rows(2).desired_width(f32::INFINITY).font(egui::TextStyle::Heading));
        if ed.first_frame {
            te.request_focus();
            // Select the placeholder so typing replaces it.
            if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), te.id) {
                let n = ed.art.text.chars().count();
                state.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(n))));
                state.store(ui.ctx(), te.id);
            }
            ed.first_frame = false;
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Font");
            egui::ComboBox::from_id_salt("wordart-font").width(180.0).height(300.0).selected_text(ed.art.font.clone()).show_ui(ui, |ui| {
                for f in font_names {
                    if ui.selectable_label(*f == ed.art.font, f).clicked() {
                        ed.art.font = f.clone();
                    }
                }
            });
            ui.add_space(12.0);
            ui.label("Size");
            ui.add(egui::DragValue::new(&mut ed.art.size).range(8.0..=200.0).speed(0.5).suffix(" pt"));
        });
        ui.add_space(6.0);
        ui.label("Style");
        let mut pick = None;
        gallery(ui, tiles, Some(ed.art.style), |i| pick = Some(i));
        if let Some(i) = pick {
            ed.art.style = i;
        }
        ui.add_space(6.0);
        ui.label("Preview");
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 150.0), Sense::hover());
        ui.painter().rect_filled(rect, 4.0, Color32::WHITE);
        if let Some((_, tex)) = &ed.preview {
            let sz = tex.size_vec2();
            let k = ((rect.width() - 16.0) / sz.x).min((rect.height() - 12.0) / sz.y).min(1.0);
            ui.painter().image(tex.id(), egui::Rect::from_center_size(rect.center(), sz * k), egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button("  OK  ").clicked() {
                ok = true;
            }
            if ui.button("  Cancel  ").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                cancel = true;
            }
        });
    });
    if ok && !ed.art.text.trim().is_empty() {
        cmds.push(Cmd::ApplyWordArt);
    } else if cancel {
        app.wordart_edit = None;
    }
}
