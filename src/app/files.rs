//! Opening, saving and exporting, including the conversions that run in the
//! background (LibreOffice imports/exports and PDF rendering) behind a
//! "Converting…" dialog instead of freezing the window.

use super::{tf, tr, App, Dialog};
use rusty_document_writer::convert::{self, Format, Kind};
use rusty_document_writer::document::Document;
use rusty_document_writer::layout::{self, LayoutOptions};
use rusty_document_writer::pdf;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Instant;

/// Word package types, opened and saved natively.
pub const WORD_EXTS: &[&str] = &["docx", "docm", "dotx", "dotm"];
/// Other documents, opened by converting them with LibreOffice.
pub const IMPORT_EXTS: &[&str] = &["doc", "odt", "rtf", "html", "htm", "txt"];
/// Resolution of the parts of an exported PDF that stay pictures (text effects).
const PDF_DPI: f32 = 200.0;

pub enum Done {
    Opened(PathBuf, Result<Box<(Document, Format)>, String>),
    Exported(Result<(), String>),
}

pub struct Job {
    label: String,
    rx: mpsc::Receiver<Done>,
    started: Instant,
}

fn ext_of(p: &Path) -> String {
    p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).unwrap_or_default()
}

/// Can File ▸ Open (or a drop onto the window) take this file?
pub fn is_openable(p: &Path) -> bool {
    let e = ext_of(p);
    WORD_EXTS.contains(&e.as_str()) || IMPORT_EXTS.contains(&e.as_str())
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

impl App {
    fn start_job(&mut self, label: String, work: impl FnOnce() -> Done + Send + 'static) {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(work());
        });
        self.job = Some(Job { label, rx, started: Instant::now() });
    }

    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// Open a file of any supported type (Word files at once, others converted in the background).
    pub fn open_path(&mut self, p: &Path) {
        if self.busy() {
            return;
        }
        let head = {
            use std::io::Read;
            let mut buf = vec![0u8; 64 * 1024];
            std::fs::File::open(p).and_then(|mut f| f.read(&mut buf)).map(|n| {
                buf.truncate(n);
                buf
            })
        };
        let format = match &head {
            Ok(h) => convert::detect(h, p.extension().and_then(|e| e.to_str())),
            Err(_) => Format::Docx, // let Document::open report the error
        };
        if format.convertible() {
            if convert::libreoffice().is_none() {
                self.dialog = Dialog::Message(
                    tr("LibreOffice is needed").into(),
                    tf("“{}” is a {}. RustyDocumentWriter converts these with LibreOffice, which isn't installed. Install it from libreoffice.org, or save the file as .docx in another program.", &[&file_name(p), &tr(format.describe())]),
                );
                return;
            }
            let path = p.to_path_buf();
            self.start_job(tf("Converting “{}”…", &[&file_name(p)]), move || Done::Opened(path.clone(), Document::import(&path).map(Box::new)));
            return;
        }
        match Document::open(p) {
            Ok(d) => {
                self.doc = d;
                self.after_load();
                self.push_recent(p);
            }
            Err(e) => {
                // A recent entry whose file has gone away is just noise: drop it.
                if !p.exists() && self.recent.iter().any(|x| x == p) {
                    self.recent.retain(|x| x != p);
                    self.save_recent();
                }
                self.dialog = Dialog::Message(tr("Couldn't open the document").into(), e)
            }
        }
    }

    /// Where to save: the current file, or a file chosen in the Save As dialog.
    pub fn save_target(&self, force_dialog: bool) -> Option<PathBuf> {
        if !force_dialog && let Some(p) = &self.doc.path {
            return Some(p.clone());
        }
        // Keep the document's own type as the default (a .docm stays macro-enabled).
        let current = self.doc.path.as_deref().map(ext_of).filter(|e| WORD_EXTS.contains(&e.as_str()));
        let ext = current.unwrap_or_else(|| if self.doc.has_macros() { "docm".into() } else { "docx".into() });
        let mut filters: Vec<(&str, &[&str])> = vec![
            (tr("Word Document"), &["docx"]),
            (tr("Word Macro-Enabled Document"), &["docm"]),
            (tr("Word Template"), &["dotx"]),
            (tr("Word Macro-Enabled Template"), &["dotm"]),
        ];
        // The dialog's first filter is the default one.
        if let Some(i) = filters.iter().position(|(_, e)| e[0] == ext) {
            let f = filters.remove(i);
            filters.insert(0, f);
        }
        let mut d = rfd::FileDialog::new().set_file_name(format!("{}.{ext}", self.doc.title()));
        for (name, exts) in filters {
            d = d.add_filter(name, exts);
        }
        if let Some(dir) = self.doc.path.as_ref().and_then(|p| p.parent()) {
            d = d.set_directory(dir);
        }
        let p = d.save_file()?;
        Some(if p.extension().is_none() { p.with_extension(&ext) } else { p })
    }

    /// File ▸ Export: PDF (our own rendering), plain text, or other formats via LibreOffice.
    pub fn export(&mut self) {
        if self.busy() {
            return;
        }
        let mut d = rfd::FileDialog::new().set_file_name(format!("{}.pdf", self.doc.title())).add_filter("PDF", &["pdf"]);
        let have_lo = convert::libreoffice().is_some();
        if have_lo {
            for (ext, _, name) in convert::EXPORTS {
                d = d.add_filter(*name, &[*ext]);
            }
        }
        d = d.add_filter(tr("Plain Text"), &["txt"]);
        if let Some(dir) = self.doc.path.as_ref().and_then(|p| p.parent()) {
            d = d.set_directory(dir);
        }
        let Some(path) = d.save_file() else { return };
        let path = if path.extension().is_none() { path.with_extension("pdf") } else { path };
        let ext = ext_of(&path);
        let name = file_name(&path);
        match ext.as_str() {
            "pdf" => {
                let (doc, fonts, title) = (self.doc.clone(), self.fonts.clone(), self.doc.title());
                self.start_job(tf("Exporting “{}”…", &[&name]), move || {
                    let layout = layout::layout_document(&doc, &fonts, &LayoutOptions::default());
                    Done::Exported(pdf::export(&layout, &title, PDF_DPI, &path))
                });
            }
            "txt" => {
                if let Err(e) = std::fs::write(&path, self.doc.plain_text()) {
                    self.dialog = Dialog::Message(tr("Couldn't export").into(), tf("Cannot write {}: {}", &[&path.display(), &e]));
                }
            }
            e if have_lo && convert::EXPORTS.iter().any(|(x, _, _)| *x == e) => {
                let pkg = self.doc.to_package();
                self.start_job(tf("Exporting “{}” with LibreOffice…", &[&name]), move || {
                    let tmp = std::env::temp_dir().join(format!("rdw-export-{}.docx", std::process::id()));
                    let r = pkg.save(&tmp).and_then(|_| convert::from_docx(&tmp, &path));
                    let _ = std::fs::remove_file(&tmp);
                    Done::Exported(r)
                });
            }
            e => self.dialog = Dialog::Message(tr("Couldn't export").into(), tf("RustyDocumentWriter can't export .{} files.", &[&e])),
        }
    }

    /// Finish a background job, or show its progress dialog.
    pub fn poll_job(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.job else { return };
        match job.rx.try_recv() {
            Ok(done) => {
                self.job = None;
                match done {
                    Done::Opened(p, Ok(opened)) => {
                        let (doc, format) = *opened;
                        self.doc = doc;
                        self.after_load();
                        self.push_recent(&p);
                        if format != Format::Docx {
                            self.dialog = Dialog::Message(
                                tf("Converted a {}", &[&tr(format.describe())]),
                                tf("“{}” was converted to a Word document with LibreOffice. Saving creates a new .docx; the original file is not changed.", &[&file_name(&p)]),
                            );
                        }
                    }
                    Done::Opened(_, Err(e)) => self.dialog = Dialog::Message(tr("Couldn't open the document").into(), e),
                    Done::Exported(Ok(())) => {}
                    Done::Exported(Err(e)) => self.dialog = Dialog::Message(tr("Couldn't export").into(), e),
                }
            }
            Err(mpsc::TryRecvError::Empty) => {
                let secs = job.started.elapsed().as_secs();
                let label = job.label.clone();
                egui::Modal::new(egui::Id::NULL.with("job")).show(ctx, |ui| {
                    ui.set_width(340.0);
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(label);
                    });
                    if secs >= 3 {
                        ui.label(egui::RichText::new(format!("{secs} s")).small());
                    }
                });
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.job = None;
                self.dialog = Dialog::Message(tr("Something went wrong").into(), tr("The conversion stopped unexpectedly.").into());
            }
        }
    }
}

/// Does saving to `path` drop the document's macros?
pub fn drops_macros(doc: &Document, path: &Path) -> bool {
    doc.has_macros() && Kind::from_ext(&ext_of(path)).is_some_and(|k| !k.allows_macros())
}
