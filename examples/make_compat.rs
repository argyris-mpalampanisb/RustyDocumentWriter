//! Compatibility corpus: one document that goes through every editing path the
//! app offers (run and paragraph formatting in scrambled orders, lists, a table,
//! a picture, page setup, columns, page numbers, WordArt), for validating with
//! tools/validate and opening in Word / LibreOffice / OpenOffice:
//! `cargo run --release --example make_compat -- compat.docx`

use rusty_document_writer::document::Document;
use rusty_document_writer::edit::{self, Pos};
use rusty_document_writer::fonts::FontDb;
use rusty_document_writer::model::{Block, RunItem};
use rusty_document_writer::xml::Element;

fn para(doc: &mut Document, at: &Pos, text: &str, run: &dyn Fn(&mut Element), ppr: &dyn Fn(&mut Element)) -> Pos {
    let start = at.clone();
    let end = edit::insert_text(doc, &start, text, None);
    edit::format_runs(doc, &start, &end, run);
    edit::format_paras(doc, &start, &end, ppr);
    edit::split_paragraph(doc, &end)
}

fn none(_: &mut Element) {}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "compat.docx".into());
    let fonts = FontDb::load_system();
    let mut doc = Document::new_blank();
    let mut at = edit::start_of_doc(&doc);

    at = para(&mut doc, &at, "Compatibility check", &none, &|p| edit::set_style(p, "Title"));
    at = para(&mut doc, &at, "Headings and paragraph formatting", &none, &|p| edit::set_style(p, "Heading1"));
    // Paragraph properties set in the reverse of schema order.
    at = para(
        &mut doc,
        &at,
        "Justified, indented, 1.5 lines, 6 pt before and 12 pt after, set in reverse schema order. The quick brown fox jumps over the lazy dog while seven wizards box.",
        &none,
        &|p| {
            edit::set_jc(p, "both");
            edit::set_indent(p, Some(36.0), Some(18.0));
            edit::set_spacing(p, Some(6.0), Some(12.0), Some((1.5, "auto")));
            edit::set_style(p, "Normal");
        },
    );
    at = para(&mut doc, &at, "Centred, exactly 18 pt line spacing.", &none, &|p| {
        edit::set_spacing(p, None, None, Some((18.0, "exact")));
        edit::set_jc(p, "center");
    });

    at = para(&mut doc, &at, "Character formatting", &none, &|p| edit::set_style(p, "Heading1"));
    // Run properties in the reverse of schema order, toggled on and off again.
    at = para(
        &mut doc,
        &at,
        "Bold italic underlined red highlighted 14 pt Georgia superscript-free text",
        &|r| {
            edit::set_highlight(r, Some("yellow"));
            edit::set_underline_style(r, Some("double"));
            edit::set_underline_color(r, Some("0070C0"));
            edit::set_color(r, Some("C00000"));
            edit::set_size(r, 14.0);
            edit::set_font(r, "Georgia");
            edit::set_toggle(r, "w:i", true);
            edit::set_toggle(r, "w:b", true);
            edit::set_toggle(r, "w:strike", true);
            edit::set_toggle(r, "w:strike", false);
        },
        &none,
    );
    for (name, label) in [("w:caps", "All caps"), ("w:smallCaps", "Small caps"), ("w:dstrike", "Double strike"), ("w:outline", "Outline"), ("w:shadow", "Shadow"), ("w:emboss", "Emboss"), ("w:imprint", "Engrave")] {
        at = para(&mut doc, &at, label, &|r| {
            edit::set_size(r, 13.0);
            edit::set_effect(r, name, true);
        }, &none);
    }
    at = para(&mut doc, &at, "H2O and E = mc2", &|r| edit::set_vert_align(r, Some("subscript")), &none);
    for t in rusty_document_writer::texttheme::THEMES.iter().take(4) {
        at = para(&mut doc, &at, &format!("Text theme: {}", t.name), &|r| t.apply(r), &none);
    }

    at = para(&mut doc, &at, "Lists", &none, &|p| edit::set_style(p, "Heading1"));
    let bullets = doc.add_list(true);
    let numbers = doc.add_list(false);
    for (i, t) in ["First bullet", "Second bullet", "Nested bullet"].iter().enumerate() {
        let id = bullets.clone();
        at = para(&mut doc, &at, t, &none, &move |p| {
            edit::set_jc(p, "left");
            edit::set_numbering(p, Some(&id), if i == 2 { 1 } else { 0 });
        });
    }
    for t in ["Step one", "Step two", "Step three"] {
        let id = numbers.clone();
        at = para(&mut doc, &at, t, &none, &move |p| edit::set_numbering(p, Some(&id), 0));
    }

    at = para(&mut doc, &at, "Table and picture", &none, &|p| edit::set_style(p, "Heading1"));
    let table = edit::new_table(3, 3, 9000);
    at = edit::insert_block_at(&mut doc, &at, Block::Table(table));
    let png = {
        let img = image::RgbaImage::from_fn(64, 32, |x, y| image::Rgba([(x * 4) as u8, (y * 8) as u8, 160, 255]));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png).unwrap();
        buf
    };
    let rid = doc.add_image(png, "png");
    let id = doc.next_drawing_id();
    at = edit::insert_item(&mut doc, &at, RunItem::Drawing(edit::image_drawing(&rid, 128.0, 64.0, id, "Gradient.png")), None);
    at = edit::split_paragraph(&mut doc, &at);

    at = para(&mut doc, &at, "WordArt", &none, &|p| edit::set_style(p, "Heading1"));
    let art = rusty_document_writer::wordart::WordArt { style: 1, text: "Compatible".into(), font: "Arial".into(), size: 40.0 };
    let wid = doc.next_drawing_id();
    at = edit::insert_item(&mut doc, &at, rusty_document_writer::wordart::run_item(&art, &fonts, wid, None), None);
    at = edit::split_paragraph(&mut doc, &at);
    let _ = para(&mut doc, &at, "The end.", &none, &none);

    // Page setup: A4 landscape-free portrait, narrow margins, two columns; page numbers.
    let n = doc.body.len() - 1;
    let sect = doc.sectpr_for_block_mut(n);
    edit::set_columns(sect, 2);
    edit::set_margins(sect, 54.0, 54.0, 54.0, 54.0);
    edit::set_page_size(sect, 595.3, 841.9, false);
    if let Some(st) = rusty_document_writer::pagenum::style("brackets") {
        doc.insert_page_number(st, rusty_document_writer::pagenum::Place::Bottom);
    }
    doc.save_as(std::path::Path::new(&out)).expect("save");
    println!("saved {out}");
}
