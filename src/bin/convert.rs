//! File converter built on the editor's own import, layout and export:
//!
//! ```text
//! rusty-document-writer-convert <input> <output> [--dpi N]
//! rusty-document-writer-convert --to <ext> [--dpi N] <input>...   (writes next to each input)
//! ```
//!
//! Inputs: .docx/.docm/.dotx/.dotm natively; .doc, .odt, .rtf, .html and .txt through LibreOffice.
//! Outputs: .docx/.dotx/.docm/.dotm, .pdf (our layout, exactly as on screen), .txt, .png (one per
//! page) natively; .odt, .doc, .rtf and .html through LibreOffice.

use rusty_document_writer::convert;
use rusty_document_writer::document::Document;
use rusty_document_writer::fonts::FontDb;
use rusty_document_writer::layout::{layout_document, LayoutOptions};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const USAGE: &str = "usage: rusty-document-writer-convert <input> <output> [--dpi N]
       rusty-document-writer-convert --to <ext> [--dpi N] <input>...
outputs: docx dotx docm dotm pdf txt png (built in); odt doc rtf html (LibreOffice)";

fn fonts() -> &'static FontDb {
    static FONTS: OnceLock<FontDb> = OnceLock::new();
    FONTS.get_or_init(FontDb::load_system)
}

fn convert_one(input: &Path, output: &Path, dpi: f32) -> Result<(), String> {
    let ext = output.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).unwrap_or_default();
    let (mut doc, from) = Document::import(input)?;
    if from != convert::Format::Docx {
        eprintln!("  imported {} with LibreOffice", from.describe());
    }
    match ext.as_str() {
        "docx" | "dotx" | "docm" | "dotm" => doc.save_as(output),
        "pdf" => {
            let layout = layout_document(&doc, fonts(), &LayoutOptions::default());
            rusty_document_writer::pdf::export(&layout, &doc.title(), dpi, output)
        }
        "txt" => std::fs::write(output, doc.plain_text()).map_err(|e| e.to_string()),
        "png" => {
            let layout = layout_document(&doc, fonts(), &LayoutOptions::default());
            let mut r = rusty_document_writer::render::Renderer::new();
            let stem = output.with_extension("");
            for (n, page) in layout.pages.iter().enumerate() {
                let p = PathBuf::from(format!("{}-{:03}.png", stem.display(), n + 1));
                r.render(page, dpi / 72.0).save_png(&p)?;
            }
            Ok(())
        }
        e if convert::EXPORTS.iter().any(|(x, _, _)| *x == e) => {
            // Save our (possibly edited/normalised) DOCX, then let LibreOffice convert it.
            let tmp = std::env::temp_dir().join(format!("rdw-cli-{}.docx", std::process::id()));
            doc.save_as(&tmp)?;
            let r = convert::from_docx(&tmp, output);
            let _ = std::fs::remove_file(&tmp);
            r
        }
        "" => Err("the output needs an extension (e.g. .pdf)".into()),
        e => Err(format!("can't write .{e} files\n{USAGE}")),
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut dpi = 150.0f32;
    let mut to: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--dpi" if i + 1 < args.len() => {
                dpi = args[i + 1].parse::<f32>().unwrap_or(150.0).clamp(36.0, 1200.0);
                args.drain(i..i + 2);
            }
            "--to" if i + 1 < args.len() => {
                to = Some(args[i + 1].trim_start_matches('.').to_ascii_lowercase());
                args.drain(i..i + 2);
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                return;
            }
            _ => i += 1,
        }
    }
    let jobs: Vec<(PathBuf, PathBuf)> = match (&to, args.as_slice()) {
        (Some(ext), inputs) if !inputs.is_empty() => inputs.iter().map(|p| (PathBuf::from(p), Path::new(p).with_extension(ext))).collect(),
        (None, [input, output]) => vec![(PathBuf::from(input), PathBuf::from(output))],
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    };
    let mut failed = 0;
    for (input, output) in &jobs {
        if input == output {
            eprintln!("{}: input and output are the same file; skipped", input.display());
            failed += 1;
            continue;
        }
        let t = std::time::Instant::now();
        match convert_one(input, output, dpi) {
            Ok(()) => eprintln!("{} -> {} ({:.0} ms)", input.display(), output.display(), t.elapsed().as_secs_f64() * 1000.0),
            Err(e) => {
                eprintln!("{}: {e}", input.display());
                failed += 1;
            }
        }
    }
    if failed > 0 {
        std::process::exit(1);
    }
}
