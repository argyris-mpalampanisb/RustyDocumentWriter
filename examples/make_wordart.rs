//! Inserts one WordArt per built-in style, each followed by body text so the
//! top-and-bottom wrapping is visible: `cargo run --example make_wordart -- out.docx`

use rusty_document_writer::document::Document;
use rusty_document_writer::edit;
use rusty_document_writer::fonts::FontDb;
use rusty_document_writer::wordart::{self, WordArt, STYLES};

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "wordart.docx".into());
    let fonts = FontDb::load_system();
    let mut doc = Document::new_blank();
    let mut at = edit::start_of_doc(&doc);
    for (i, st) in STYLES.iter().enumerate() {
        let art = WordArt { style: i, text: st.name.to_string(), font: "Arial".into(), size: 40.0 };
        let id = doc.next_drawing_id();
        at = edit::insert_item(&mut doc, &at, wordart::run_item(&art, &fonts, id, None), None);
        at = edit::insert_text(&mut doc, &at, "Body text flows below the WordArt and never runs underneath it.", None);
        at = edit::split_paragraph(&mut doc, &at);
    }
    doc.save_as(std::path::Path::new(&out)).expect("save");
    println!("saved {out}");
}
