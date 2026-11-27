//! Builds a showcase of every text theme, effect and underline style through the
//! same editing API the ribbon uses: `cargo run --example make_texteffects -- out.docx`

use rusty_document_writer::document::Document;
use rusty_document_writer::edit::{self, Pos};
use rusty_document_writer::texttheme::THEMES;
use rusty_document_writer::xml::Element;

/// Append a paragraph holding `text`, formatted by `f`, and return the position after it.
fn line(doc: &mut Document, at: &Pos, text: &str, size: f32, f: &dyn Fn(&mut Element)) -> Pos {
    let start = at.clone();
    let end = edit::insert_text(doc, &start, text, None);
    edit::format_runs(doc, &start, &end, &|r| {
        edit::set_size(r, size);
        f(r);
    });
    edit::split_paragraph(doc, &end)
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "texteffects.docx".into());
    let mut doc = Document::new_blank();
    let mut at = edit::start_of_doc(&doc);
    at = line(&mut doc, &at, "Text themes", 20.0, &|r| edit::set_toggle(r, "w:b", true));
    for t in THEMES {
        at = line(&mut doc, &at, &format!("{} — The quick brown fox", t.name), 22.0, &|r| t.apply(r));
    }
    at = line(&mut doc, &at, "Effects", 20.0, &|r| edit::set_toggle(r, "w:b", true));
    for (name, label) in [
        ("w:outline", "Outline"),
        ("w:shadow", "Shadow"),
        ("w:emboss", "Emboss"),
        ("w:imprint", "Engrave"),
        ("w:smallCaps", "Small Caps"),
        ("w:caps", "All caps"),
        ("w:dstrike", "Double strikethrough"),
    ] {
        at = line(&mut doc, &at, label, 18.0, &|r| edit::set_effect(r, name, true));
    }
    at = line(&mut doc, &at, "Underlines", 20.0, &|r| edit::set_toggle(r, "w:b", true));
    for u in ["single", "words", "double", "thick", "dotted", "dash", "dotDash", "dotDotDash", "wave", "wavyHeavy", "wavyDouble"] {
        at = line(&mut doc, &at, &format!("Underline style {u}"), 14.0, &|r| {
            edit::set_underline_style(r, Some(u));
            edit::set_underline_color(r, Some("C00000"));
        });
    }
    let _ = at;
    doc.save_as(std::path::Path::new(&out)).expect("save");
    println!("saved {out}");
}
