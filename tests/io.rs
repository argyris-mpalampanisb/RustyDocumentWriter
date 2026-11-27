//! Import/export behaviour of the package layer: raw-copied parts, package
//! kinds (.docx/.dotx/.docm) and macro removal.

use rusty_document_writer::convert::Kind;
use rusty_document_writer::document::Document;
use rusty_document_writer::opc::Package;

fn tmp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("rdw-io-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn content_type(pkg: &Package, part: &str) -> Option<String> {
    let ct = pkg.get_xml("[Content_Types].xml")?;
    ct.root
        .elements()
        .find(|e| e.attr("PartName").is_some_and(|p| p.trim_start_matches('/') == part))
        .and_then(|e| e.attr("ContentType").map(str::to_string))
}

/// A macro-enabled document: the blank template plus a fake VBA project.
fn macro_document() -> Document {
    let mut d = Document::new_blank();
    let main = d.main_part.clone();
    d.package.set("word/vbaProject.bin", vec![1, 2, 3, 4]);
    d.package.set("word/vbaData.xml", b"<wne:vbaSuppData xmlns:wne=\"http://schemas.microsoft.com/office/word/2006/wordml\"/>".to_vec());
    d.package.ensure_default_content_type("bin", "application/vnd.ms-office.vbaProject");
    d.package.ensure_override_content_type("word/vbaData.xml", "application/vnd.ms-word.vbaData+xml");
    d.package.add_rel(&main, "http://schemas.microsoft.com/office/2006/relationships/vbaProject", "vbaProject.bin", false);
    d.package.add_rel("word/vbaProject.bin", "http://schemas.microsoft.com/office/2006/relationships/wordVbaData", "vbaData.xml", false);
    d
}

#[test]
fn extension_sets_the_package_kind() {
    let mut d = Document::new_blank();
    let dotx = tmp("kind.dotx");
    d.save_as(&dotx).unwrap();
    let pkg = Package::open(&dotx).unwrap();
    assert_eq!(content_type(&pkg, &d.main_part).as_deref(), Some(Kind::Template.content_type()));
    // Template opened and saved as .docx: no longer a template.
    let mut t = Document::open(&dotx).unwrap();
    let docx = tmp("kind.docx");
    t.save_as(&docx).unwrap();
    let pkg = Package::open(&docx).unwrap();
    assert_eq!(content_type(&pkg, &t.main_part).as_deref(), Some(Kind::Document.content_type()));
}

#[test]
fn saving_as_docx_removes_macros_and_docm_keeps_them() {
    let mut d = macro_document();
    let docm = tmp("macros.docm");
    d.save_as(&docm).unwrap();
    let pkg = Package::open(&docm).unwrap();
    assert!(pkg.get("word/vbaProject.bin").is_some());
    assert_eq!(content_type(&pkg, "word/document.xml").as_deref(), Some(Kind::MacroDocument.content_type()));

    let mut d = Document::open(&docm).unwrap();
    let docx = tmp("macros.docx");
    d.save_as(&docx).unwrap();
    let pkg = Package::open(&docx).unwrap();
    for gone in ["word/vbaProject.bin", "word/vbaData.xml", "word/_rels/vbaProject.bin.rels"] {
        assert!(pkg.get(gone).is_none(), "{gone} should be removed");
    }
    assert!(pkg.rels("word/document.xml").iter().all(|r| !r.rel_type.ends_with("/vbaProject")));
    assert!(content_type(&pkg, "word/vbaData.xml").is_none());
    // Still a valid document.
    Document::open(&docx).unwrap();
}

#[test]
fn unchanged_parts_are_copied_without_recompression() {
    let mut d = Document::new_blank();
    // A new picture exercises the "stored" (already compressed) path too.
    let png = {
        let mut buf = Vec::new();
        let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([200, 30, 30, 255]));
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png).unwrap();
        buf
    };
    d.add_image(png, "png");
    let a = tmp("raw-a.docx");
    d.save_as(&a).unwrap();
    let mut d = Document::open(&a).unwrap();
    let b = tmp("raw-b.docx");
    d.save_as(&b).unwrap();
    // Every entry other than the main document keeps its exact compressed bytes.
    let raw = |p: &std::path::Path| {
        let mut z = zip::ZipArchive::new(std::fs::File::open(p).unwrap()).unwrap();
        (0..z.len())
            .map(|i| {
                let f = z.by_index_raw(i).unwrap();
                (f.name().unwrap().to_string(), f.compressed_size(), f.crc32())
            })
            .collect::<Vec<_>>()
    };
    let (ra, rb) = (raw(&a), raw(&b));
    assert_eq!(ra.len(), rb.len());
    for (x, y) in ra.iter().zip(&rb) {
        assert_eq!(x.0, y.0);
        assert_eq!(x.2, y.2, "{} content changed", x.0);
        assert_eq!(x.1, y.1, "{} was recompressed", x.0);
    }
}
