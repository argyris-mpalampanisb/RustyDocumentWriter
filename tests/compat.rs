//! Interoperability with Word, LibreOffice and OpenOffice: things that other
//! programs rely on but a schema validator doesn't check.

use rusty_document_writer::document::Document;
use rusty_document_writer::edit;
use rusty_document_writer::fonts::FontDb;
use rusty_document_writer::layout::{layout_document, LayoutOptions};
use rusty_document_writer::model::{Block, RunItem};

/// LibreOffice substitutes a theme-only font (`w:asciiTheme` without `w:ascii`)
/// with a serif, and OpenOffice ignores theme fonts entirely, so the template
/// names the font explicitly too (Word gives the theme reference priority).
#[test]
fn template_names_theme_fonts_explicitly() {
    let doc = Document::new_blank();
    let styles = String::from_utf8(doc.package.get("word/styles.xml").unwrap().to_vec()).unwrap();
    let theme_refs = styles.matches("w:asciiTheme=").count();
    assert!(theme_refs > 0);
    assert_eq!(styles.matches("<w:rFonts w:ascii=\"Aptos").count(), theme_refs, "every theme reference needs an explicit name");
    // And our own resolver still prefers the theme, like Word.
    let rp = doc.resolver().run("Heading1", None, None);
    assert_eq!(rp.font_ascii, "Aptos Display");
}

/// Drawing ids must be unique in the whole document, including ids already
/// used by drawings we only keep as raw XML.
#[test]
fn new_drawing_ids_never_collide() {
    let mut doc = Document::new_blank();
    let rid = doc.add_image(vec![0x89, b'P', b'N', b'G'], "png");
    let first = doc.next_drawing_id();
    let at = edit::start_of_doc(&doc);
    let at = edit::insert_item(&mut doc, &at, RunItem::Drawing(edit::image_drawing(&rid, 10.0, 10.0, 101, "a.png")), None);
    assert!(doc.next_drawing_id() > 101);
    let id = doc.next_drawing_id();
    edit::insert_item(&mut doc, &at, RunItem::Drawing(edit::image_drawing(&rid, 10.0, 10.0, id, "b.png")), None);
    let dir = std::env::temp_dir().join(format!("rdw-compat-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ids.docx");
    doc.save_as(&path).unwrap();
    let xml = String::from_utf8(Document::open(&path).unwrap().package.get("word/document.xml").unwrap().to_vec()).unwrap();
    let mut ids: Vec<&str> = xml.match_indices("docPr id=\"").map(|(i, m)| xml[i + m.len()..].split('"').next().unwrap()).collect();
    let n = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), n, "duplicate docPr ids: {xml}");
    assert!(first >= 1);
}

/// Pictures are U+FFFC placeholders while editing; exported text and the clipboard leave them out.
#[test]
fn plain_text_has_no_object_placeholders() {
    let mut doc = Document::new_blank();
    let rid = doc.add_image(vec![0x89, b'P', b'N', b'G'], "png");
    let at = edit::start_of_doc(&doc);
    let at = edit::insert_text(&mut doc, &at, "before ", None);
    let at = edit::insert_item(&mut doc, &at, RunItem::Drawing(edit::image_drawing(&rid, 10.0, 10.0, 7, "p.png")), None);
    let end = edit::insert_text(&mut doc, &at, " after", None);
    assert_eq!(doc.plain_text(), "before  after\n");
    assert_eq!(edit::selected_text(&doc, &edit::start_of_doc(&doc), &end), "before  after");
}

/// A WordArt with top-and-bottom wrapping inside a table cell pushes the cell's
/// text below it, and the cell grows (LibreOffice and Word both do this).
#[test]
fn floats_in_table_cells_take_up_room() {
    let fonts = FontDb::load_system();
    let mut doc = Document::new_blank();
    let at = edit::start_of_doc(&doc);
    let cell = edit::insert_block_at(&mut doc, &at, Block::Table(edit::new_table(1, 1, 6000)));
    let art = rusty_document_writer::wordart::WordArt { style: 0, text: "Big".into(), font: "Arial".into(), size: 48.0 };
    let item = rusty_document_writer::wordart::run_item(&art, &fonts, doc.next_drawing_id(), None);
    let after_art = edit::insert_item(&mut doc, &cell, item, None);
    let next = edit::split_paragraph(&mut doc, &after_art);
    edit::insert_text(&mut doc, &next, "After", None);
    let layout = layout_document(&doc, &fonts, &LayoutOptions::default());
    let page = &layout.pages[0];
    let first = page.lines.iter().find(|l| l.path == cell.path).expect("anchor paragraph");
    let after = page.lines.iter().find(|l| l.path == next.path).expect("text after the WordArt");
    // 48 pt WordArt is well over 40 pt tall: the cell's text (even the anchor
    // paragraph's own) starts below it, and the following line comes after that.
    let top = doc.sections()[0].1.margin_top;
    assert!(first.top - top > 40.0, "text overlaps the WordArt: cell top {top}, text at {}", first.top);
    assert!(after.top > first.top);
}
