//! Formats other than DOCX: recognising them, and converting through LibreOffice.
//!
//! RustyDocumentWriter edits WordprocessingML only. Legacy Word (.doc), OpenDocument,
//! RTF, HTML and plain text are imported by asking a headless LibreOffice to
//! produce a .docx, and exported the same way in reverse. PDF export does not need
//! LibreOffice: it uses our own layout and renderer (see [`crate::pdf`]), so the
//! PDF matches the screen.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// What a file really is, judged by its first bytes (extensions lie).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// Any WordprocessingML package: .docx, .docm, .dotx, .dotm.
    Docx,
    /// Word 97-2003 binary (an OLE compound file).
    Doc,
    /// A password-protected Office document (OOXML wrapped in an encrypted compound file).
    Encrypted,
    Odt,
    Rtf,
    Html,
    Pdf,
    Text,
    Unknown,
}

impl Format {
    pub fn describe(self) -> &'static str {
        match self {
            Format::Docx => "Word document",
            Format::Doc => "Word 97-2003 document (.doc)",
            Format::Encrypted => "password-protected document",
            Format::Odt => "OpenDocument text (.odt)",
            Format::Rtf => "Rich Text Format (.rtf)",
            Format::Html => "web page (.html)",
            Format::Pdf => "PDF",
            Format::Text => "plain text",
            Format::Unknown => "unrecognised file",
        }
    }

    /// Can LibreOffice turn this into a .docx for us?
    pub fn convertible(self) -> bool {
        matches!(self, Format::Doc | Format::Odt | Format::Rtf | Format::Html | Format::Text)
    }
}

/// Recognise a document from its bytes; `ext` only breaks ties (text vs HTML).
pub fn detect(bytes: &[u8], ext: Option<&str>) -> Format {
    const CFB: &[u8] = &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
    let ext = ext.map(|e| e.to_ascii_lowercase());
    if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06") {
        // OpenDocument stores its "mimetype" file first and uncompressed.
        if bytes.len() > 38 && &bytes[30..38] == b"mimetype" && find(&bytes[..bytes.len().min(120)], b"opendocument.text") {
            return Format::Odt;
        }
        return Format::Docx;
    }
    if bytes.starts_with(CFB) {
        // Encrypted OOXML keeps an "EncryptionInfo" stream (names are UTF-16).
        let name: Vec<u8> = "EncryptionInfo".bytes().flat_map(|b| [b, 0]).collect();
        return if find(bytes, &name) { Format::Encrypted } else { Format::Doc };
    }
    if bytes.starts_with(b"{\\rtf") {
        return Format::Rtf;
    }
    if bytes.starts_with(b"%PDF") {
        return Format::Pdf;
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).to_ascii_lowercase();
    let head = head.trim_start_matches('\u{feff}').trim_start();
    if head.starts_with("<!doctype html") || head.starts_with("<html") || matches!(ext.as_deref(), Some("htm" | "html")) {
        return Format::Html;
    }
    let texty = !bytes.is_empty() && std::str::from_utf8(bytes).is_ok() && !bytes.contains(&0);
    if texty || matches!(ext.as_deref(), Some("txt" | "md" | "text")) {
        return Format::Text;
    }
    Format::Unknown
}

fn find(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// The LibreOffice executable, if one is installed.
pub fn libreoffice() -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) { &["soffice.exe", "soffice.com"] } else { &["soffice", "libreoffice"] };
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    if cfg!(windows) {
        for var in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(pf) = std::env::var_os(var) {
                dirs.push(PathBuf::from(pf).join("LibreOffice").join("program"));
            }
        }
    } else if cfg!(target_os = "macos") {
        dirs.push(PathBuf::from("/Applications/LibreOffice.app/Contents/MacOS"));
    }
    dirs.iter().flat_map(|d| names.iter().map(move |n| d.join(n))).find(|p| p.is_file())
}

/// A fresh, empty scratch directory.
fn scratch_dir(tag: &str) -> Result<PathBuf, String> {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("rdw-{tag}-{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create {}: {e}", dir.display()))?;
    Ok(dir)
}

fn file_url(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let mut enc = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~/:".contains(&b) {
            enc.push(b as char);
        } else {
            enc.push_str(&format!("%{b:02X}"));
        }
    }
    let s = enc;
    if s.starts_with('/') { format!("file://{s}") } else { format!("file:///{s}") }
}

/// Run `soffice --headless --convert-to <filter> <input>` and return the produced file.
///
/// A private user profile keeps this working while the user has LibreOffice open
/// (a second instance sharing the normal profile would just hand the job to the
/// running one and exit without converting).
fn run_libreoffice(input: &Path, filter: &str, out_ext: &str) -> Result<Vec<u8>, String> {
    let soffice = libreoffice().ok_or("LibreOffice is needed for this format but was not found. Install it from libreoffice.org and try again.")?;
    let input = std::fs::canonicalize(input).map_err(|e| format!("Cannot read {}: {e}", input.display()))?;
    let out_dir = scratch_dir("convert")?;
    let profile = std::env::temp_dir().join("rdw-libreoffice-profile");
    let mut child = std::process::Command::new(&soffice)
        .arg(format!("-env:UserInstallation={}", file_url(&profile)))
        .args(["--headless", "--norestore", "--nolockcheck", "--convert-to", filter, "--outdir"])
        .arg(&out_dir)
        .arg(&input)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Cannot start LibreOffice ({}): {e}", soffice.display()))?;
    let deadline = Instant::now() + Duration::from_secs(120);
    let status = loop {
        if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
            break s;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = std::fs::remove_dir_all(&out_dir);
            return Err("LibreOffice took more than two minutes and was stopped.".into());
        }
        std::thread::sleep(Duration::from_millis(40));
    };
    let mut err = String::new();
    if let Some(mut e) = child.stderr.take() {
        use std::io::Read;
        let _ = e.read_to_string(&mut err);
    }
    let stem = input.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let produced = out_dir.join(format!("{stem}.{out_ext}"));
    let result = std::fs::read(&produced).map_err(|_| {
        let detail = err.lines().find(|l| l.to_lowercase().contains("error")).unwrap_or("no output was produced");
        format!("LibreOffice could not convert {} ({}; exit status {status}).", input.display(), detail.trim())
    });
    let _ = std::fs::remove_dir_all(&out_dir);
    result
}

/// Convert a non-DOCX document to DOCX bytes.
pub fn to_docx(path: &Path) -> Result<Vec<u8>, String> {
    run_libreoffice(path, "docx:MS Word 2007 XML", "docx")
}

/// Formats [`from_docx`] can write, as (extension, LibreOffice filter, description).
pub const EXPORTS: &[(&str, &str, &str)] = &[
    ("odt", "odt:writer8", "OpenDocument Text"),
    ("doc", "doc:MS Word 97", "Word 97-2003 Document"),
    ("rtf", "rtf:Rich Text Format", "Rich Text Format"),
    ("html", "html:HTML (StarWriter)", "Web Page"),
];

/// Convert a saved .docx to another format through LibreOffice (see [`EXPORTS`]).
pub fn from_docx(docx: &Path, out: &Path) -> Result<(), String> {
    let ext = out.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).unwrap_or_default();
    let (ext, filter, _) = EXPORTS.iter().find(|(e, _, _)| *e == ext).ok_or_else(|| format!("Can't export to .{ext}"))?;
    // LibreOffice names the output after the input, so give the input the wanted stem.
    let dir = scratch_dir("export")?;
    let stem = out.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let tmp = dir.join(format!("{stem}.docx"));
    std::fs::copy(docx, &tmp).map_err(|e| e.to_string())?;
    let bytes = run_libreoffice(&tmp, filter, ext);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::write(out, bytes?).map_err(|e| format!("Cannot write {}: {e}", out.display()))
}

/// The four WordprocessingML package kinds, which differ only in the main part's content type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Document,
    Template,
    MacroDocument,
    MacroTemplate,
}

impl Kind {
    pub fn from_ext(ext: &str) -> Option<Kind> {
        Some(match ext.to_ascii_lowercase().as_str() {
            "docx" => Kind::Document,
            "dotx" => Kind::Template,
            "docm" => Kind::MacroDocument,
            "dotm" => Kind::MacroTemplate,
            _ => return None,
        })
    }

    pub fn content_type(self) -> &'static str {
        match self {
            Kind::Document => "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
            Kind::Template => "application/vnd.openxmlformats-officedocument.wordprocessingml.template.main+xml",
            Kind::MacroDocument => "application/vnd.ms-word.document.macroEnabled.main+xml",
            Kind::MacroTemplate => "application/vnd.ms-word.template.macroEnabledTemplate.main+xml",
        }
    }

    pub fn allows_macros(self) -> bool {
        matches!(self, Kind::MacroDocument | Kind::MacroTemplate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_formats() {
        assert_eq!(detect(b"PK\x03\x04rest", Some("docx")), Format::Docx);
        let mut odt = b"PK\x03\x04".to_vec();
        odt.resize(30, 0);
        odt.extend_from_slice(b"mimetypeapplication/vnd.oasis.opendocument.text");
        assert_eq!(detect(&odt, None), Format::Odt);
        let mut cfb = vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
        assert_eq!(detect(&cfb, Some("doc")), Format::Doc);
        cfb.extend("EncryptionInfo".bytes().flat_map(|b| [b, 0]));
        assert_eq!(detect(&cfb, Some("docx")), Format::Encrypted);
        assert_eq!(detect(b"{\\rtf1\\ansi", None), Format::Rtf);
        assert_eq!(detect(b"%PDF-1.7", None), Format::Pdf);
        assert_eq!(detect(b"\xEF\xBB\xBF<!DOCTYPE html><p>x", None), Format::Html);
        assert_eq!(detect("Γειά σου\nκόσμε".as_bytes(), Some("txt")), Format::Text);
        assert_eq!(detect(&[0, 159, 146, 150], None), Format::Unknown);
    }

    #[test]
    fn kinds() {
        assert_eq!(Kind::from_ext("DOTM"), Some(Kind::MacroTemplate));
        assert!(Kind::from_ext("pdf").is_none());
        assert!(!Kind::Document.allows_macros());
    }
}
