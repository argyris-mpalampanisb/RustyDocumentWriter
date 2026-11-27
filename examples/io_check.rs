//! Round-trip checker and I/O benchmark:
//! `cargo run --release --example io_check -- a.docx b.docx ...`
//!
//! For each file: time to read the zip, parse, lay out and save; then reopen the
//! saved copy and check that the text, the set of parts and every part the
//! editor did not touch survived unchanged.

use rusty_document_writer::document::Document;
use rusty_document_writer::fonts::FontDb;
use rusty_document_writer::layout::{layout_document, LayoutOptions};
use rusty_document_writer::opc::Package;
use std::time::Instant;

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn main() {
    let files: Vec<String> = std::env::args().skip(1).collect();
    let fonts = FontDb::load_system();
    let out_dir = std::env::temp_dir().join("rdw-io-check");
    std::fs::create_dir_all(&out_dir).unwrap();
    println!("{:<34} {:>8} {:>7} {:>7} {:>7} {:>7} {:>8}  result", "file", "KiB", "unzip", "parse", "layout", "save", "out KiB");
    let (mut bad, mut n) = (0, 0);
    for f in &files {
        n += 1;
        let path = std::path::Path::new(f);
        let name: String = path.file_name().unwrap().to_string_lossy().chars().take(34).collect();
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let t = Instant::now();
        let pkg = match Package::open(path) {
            Ok(p) => p,
            Err(e) => {
                println!("{name:<34} {:>8} open failed: {e}", size / 1024);
                bad += 1;
                continue;
            }
        };
        let t_zip = ms(t);
        let t = Instant::now();
        let mut doc = match Document::from_package(pkg.clone()) {
            Ok(d) => d,
            Err(e) => {
                println!("{name:<34} {:>8} parse failed: {e}", size / 1024);
                bad += 1;
                continue;
            }
        };
        let t_parse = ms(t);
        let t = Instant::now();
        let layout = layout_document(&doc, &fonts, &LayoutOptions::default());
        let t_layout = ms(t);
        let out = out_dir.join(format!("{n}.docx"));
        let t = Instant::now();
        if let Err(e) = doc.save_as(&out) {
            println!("{name:<34} save failed: {e}");
            bad += 1;
            continue;
        }
        let t_save = ms(t);
        let out_size = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);

        // Verify the round trip.
        let mut problems = Vec::new();
        let back = Package::open(&out).expect("reopen");
        let doc2 = Document::from_package(back.clone()).expect("reparse");
        if doc2.plain_text() != Document::from_package(pkg.clone()).unwrap().plain_text() {
            problems.push("text differs".to_string());
        }
        let names = |p: &Package| {
            let mut v: Vec<String> = p.parts.iter().map(|p| p.name.clone()).collect();
            v.sort();
            v
        };
        if names(&pkg) != names(&back) {
            problems.push(format!("parts {:?} -> {:?}", names(&pkg), names(&back)));
        }
        for p in &pkg.parts {
            if p.name == doc.main_part {
                continue;
            }
            if back.get(&p.name) != Some(p.data.as_slice()) {
                problems.push(format!("{} changed", p.name));
            }
        }
        if doc2.layout_pages_hint(&fonts) != layout.pages.len() {
            problems.push("page count differs".into());
        }
        let result = if problems.is_empty() { "ok".to_string() } else { problems.join("; ") };
        if !problems.is_empty() {
            bad += 1;
        }
        println!(
            "{name:<34} {:>8} {t_zip:>7.1} {t_parse:>7.1} {t_layout:>7.1} {t_save:>7.1} {:>8}  {result} ({} pages)",
            size / 1024,
            out_size / 1024,
            layout.pages.len()
        );
    }
    println!("{n} files, {bad} with problems");
}

trait PagesHint {
    fn layout_pages_hint(&self, fonts: &FontDb) -> usize;
}

impl PagesHint for Document {
    fn layout_pages_hint(&self, fonts: &FontDb) -> usize {
        layout_document(self, fonts, &LayoutOptions::default()).pages.len()
    }
}
