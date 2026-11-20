//! Headless renderer: `rusty-document-writer-render input.docx [out_dir] [--dpi 96] [--roundtrip out.docx]`
//! Writes one PNG per page — handy for comparing against a reference renderer.

use std::path::PathBuf;
use rusty_document_writer::document::Document;
use rusty_document_writer::fonts::FontDb;
use rusty_document_writer::layout::{layout_document, LayoutOptions};
use rusty_document_writer::render::Renderer;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: rusty-document-writer-render <input.docx> [out_dir] [--dpi N] [--roundtrip out.docx] [--marks]");
        std::process::exit(2);
    }
    let input = PathBuf::from(&args[1]);
    let mut out_dir = PathBuf::from("render-out");
    let mut dpi = 96.0f32;
    let mut roundtrip: Option<PathBuf> = None;
    let mut marks = false;
    let mut dump = false;
    let mut lines_mode = false;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--dpi" => {
                dpi = args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(96.0);
                i += 1;
            }
            "--roundtrip" => {
                roundtrip = args.get(i + 1).map(PathBuf::from);
                i += 1;
            }
            "--marks" => marks = true,
            "--dump" => dump = true,
            "--lines" => lines_mode = true,
            other => out_dir = PathBuf::from(other),
        }
        i += 1;
    }
    let t0 = std::time::Instant::now();
    let fonts = FontDb::load_system();
    eprintln!("fonts: {} faces in {:?}", fonts.len(), t0.elapsed());
    let t1 = std::time::Instant::now();
    let mut doc = match Document::open(&input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };
    for w in &doc.warnings {
        eprintln!("warning: {w}");
    }
    eprintln!("parsed in {:?}", t1.elapsed());
    let t2 = std::time::Instant::now();
    let layout = layout_document(&doc, &fonts, &LayoutOptions { show_marks: marks });
    eprintln!("layout: {} pages in {:?}", layout.pages.len(), t2.elapsed());
    if lines_mode {
        let mut counts: std::collections::BTreeMap<Vec<usize>, usize> = Default::default();
        for p in &layout.pages {
            for l in &p.lines {
                if l.story == rusty_document_writer::layout::Story::Body {
                    *counts.entry(l.path.clone()).or_default() += 1;
                }
            }
        }
        for (_, c) in counts {
            println!("{c}");
        }
        return;
    }
    if dump {
        // page<TAB>y_top(pt)<TAB>text — first line of every paragraph, document order.
        let mut rows: Vec<(Vec<usize>, usize, f32)> = Vec::new();
        for (pi, p) in layout.pages.iter().enumerate() {
            for l in &p.lines {
                if l.story == rusty_document_writer::layout::Story::Body && l.start == 0 {
                    rows.push((l.path.clone(), pi + 1, l.top));
                    if std::env::var("DUMPX").is_ok() { eprintln!("p{} x={:.1} y={:.1} {:?}", pi + 1, l.left, l.top, l.path); }
                }
            }
        }
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        rows.dedup_by(|a, b| a.0 == b.0);
        for (path, page, y) in rows {
            let text = rusty_document_writer::edit::para_in(&doc.body, &path).map(|p| p.plain_text()).unwrap_or_default();
            let snippet: String = text.chars().filter(|c| !c.is_control()).take(30).collect();
            println!("{}	{:.1}	{}", page, y, snippet);
        }
        return;
    }
    std::fs::create_dir_all(&out_dir).ok();
    let mut r = Renderer::new();
    let scale = dpi / 72.0;
    for (n, p) in layout.pages.iter().enumerate() {
        let img = r.render(p, scale);
        let path = out_dir.join(format!("page-{:03}.png", n + 1));
        if let Err(e) = img.save_png(&path) {
            eprintln!("cannot write {}: {e}", path.display());
        }
    }
    eprintln!("rendered to {}", out_dir.display());
    if let Some(rt) = roundtrip {
        match doc.save_as(&rt) {
            Ok(()) => eprintln!("saved round-trip copy to {}", rt.display()),
            Err(e) => eprintln!("round-trip save failed: {e}"),
        }
    }
}
