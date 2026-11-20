//! Exercises the editing API and saves the result, so the output can be
//! validated in a reference word processor: `cargo run --example edit_roundtrip -- in.docx out.docx`

use rusty_document_writer::document::Document;
use rusty_document_writer::edit::{self, Pos};
use rusty_document_writer::model::Block;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut doc = Document::open(std::path::Path::new(&args[1])).expect("open");
    // Type into the first paragraph, split it, format a range bold+red.
    let start = edit::start_of_doc(&doc);
    let p = edit::insert_text(&mut doc, &start, "Edited by RustyDocumentWriter: ", None);
    let after = edit::split_paragraph(&mut doc, &p);
    edit::format_runs(&mut doc, &start, &Pos { off: 6, ..start.clone() }, &|r| {
        edit::set_toggle(r, "w:b", true);
        edit::set_color(r, Some("C00000"));
    });
    // New bulleted list on the following paragraph.
    let id = doc.add_list(true);
    edit::format_paras(&mut doc, &after, &after, &|pp| edit::set_numbering(pp, Some(&id), 0));
    // Insert a 3x4 table after it.
    let t = edit::new_table(3, 4, 9360);
    let cell = edit::insert_block_at(&mut doc, &Pos { off: 0, ..after.clone() }, Block::Table(t));
    edit::insert_text(&mut doc, &cell, "cell text", None);
    // Page setup changes.
    edit::set_margins(&mut doc.final_sectpr, 54.0, 54.0, 54.0, 54.0);
    // Delete across paragraphs at the end of the document.
    let end = edit::end_of_doc(&doc);
    let prev = edit::prev_para(&doc, &end.story, &end.path).unwrap();
    edit::delete_range(&mut doc, &Pos::body(prev, 5), &end);
    doc.save_as(std::path::Path::new(&args[2])).expect("save");
    println!("saved {}", args[2]);
}
