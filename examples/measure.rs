//! Debug helper: width of a string in a font, with and without pair kerning.
use rusty_document_writer::fonts::FontDb;

fn main() {
    let fam = std::env::args().nth(1).unwrap_or("Calibri".into());
    let size: f32 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(11.0);
    let text = std::env::args().nth(3).unwrap_or("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur.".into());
    if std::env::var("GPOS").is_ok() { debug_gpos("C:/Windows/Fonts/calibri.ttf"); }
    let db = FontDb::load_system();
    let m = db.resolve(&fam, false, false);
    let f = &m.font;
    let mut w = 0.0;
    let mut k = 0.0;
    let chars: Vec<char> = text.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        w += f.advance(*c, size);
        if let Some(n) = chars.get(i + 1) {
            k += f.kern(f.glyph(*c).0, f.glyph(*n).0) * size / f.upm;
        }
    }
    println!("font={} upm={} width={:.2}pt kerning={:.2}pt total={:.2}pt", f.family, f.upm, w, k, w + k);
}

#[allow(dead_code)]
pub fn debug_gpos(path: &str) {
    let data = std::fs::read(path).unwrap();
    let face = ttf_parser::Face::parse(&data, 0).unwrap();
    let gpos = face.tables().gpos.unwrap();
    for f in gpos.features {
        let tag = f.tag.to_string();
        let lookups: Vec<String> = f.lookup_indices.into_iter().map(|li| {
            let l = gpos.lookups.get(li).unwrap();
            format!("{}:k{}x{}", li, l.subtables.len(), {
                let s = l.subtables.get::<ttf_parser::gpos::PositioningSubtable>(0);
                match s { Some(ttf_parser::gpos::PositioningSubtable::Pair(_)) => "pair", Some(_) => "other", None => "none" }
            })
        }).collect();
        println!("{tag}: {:?}", lookups);
    }
}
