//! Inserts every built-in 3D shape, each in a different colour, the way the Shapes
//! gallery does: `cargo run --example make_shapes -- out.docx`

use rusty_document_writer::document::Document;
use rusty_document_writer::edit;
use rusty_document_writer::model::RunItem;
use rusty_document_writer::props::Color;
use rusty_document_writer::shapes::{tinted_png, SHAPES};

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "shapes.docx".into());
    let colors = [None, Some("156082"), Some("C00000"), Some("E97132"), Some("196B24"), Some("7030A0"), Some("FFC000"), Some("0F9ED5")];
    let mut doc = Document::new_blank();
    let mut at = edit::start_of_doc(&doc);
    for (i, s) in SHAPES.iter().enumerate() {
        let png = tinted_png(s, colors[i % colors.len()].and_then(Color::from_hex));
        let img = image::load_from_memory(&png).unwrap();
        let k = 100.0 / img.width().max(img.height()) as f32;
        let rid = doc.add_image(png, "png");
        let d = edit::image_drawing(&rid, img.width() as f32 * k, img.height() as f32 * k, 100 + i as u32, s.name);
        at = edit::insert_item(&mut doc, &at, RunItem::Drawing(d), None);
        at = edit::insert_text(&mut doc, &at, " ", None);
    }
    doc.save_as(std::path::Path::new(&out)).expect("save");
    println!("saved {out}");
}
