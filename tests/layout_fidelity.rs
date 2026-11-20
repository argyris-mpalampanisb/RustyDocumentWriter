//! Regression tests against measurements taken from a reference word processor.
//!
//! `testdata/reference-*.tsv` were produced by `tools/reference-positions.ps1` (COM automation:
//! page number and vertical position of every paragraph). These tests lay out
//! the same documents and require RustyDocumentWriter to put every paragraph on the same
//! page as the reference, within a small vertical tolerance.
//!
//! The documents use Calibri / Cambria / Times New Roman; the tests are skipped
//! when those fonts aren't installed.

use std::path::Path;
use rusty_document_writer::document::Document;
use rusty_document_writer::fonts::FontDb;
use rusty_document_writer::layout::{layout_document, LayoutOptions, Story};

fn fonts_available(db: &FontDb) -> bool {
    ["calibri", "calibri light", "times new roman"].iter().all(|f| db.has_family(f))
}

fn positions(doc: &Document, db: &FontDb) -> Vec<(usize, f32, String)> {
    let layout = layout_document(doc, db, &LayoutOptions::default());
    let mut rows: Vec<(Vec<usize>, usize, f32)> = Vec::new();
    for (pi, p) in layout.pages.iter().enumerate() {
        for l in &p.lines {
            if l.story == Story::Body && l.start == 0 {
                rows.push((l.path.clone(), pi + 1, l.top));
            }
        }
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    rows.dedup_by(|a, b| a.0 == b.0);
    rows.into_iter()
        .map(|(path, page, y)| {
            let t: String = rusty_document_writer::edit::para_in(&doc.body, &path).map(|p| p.plain_text()).unwrap_or_default().chars().filter(|c| !c.is_control()).collect();
            (page, y, t)
        })
        .collect()
}

/// Returns (matched, page mismatches, mean |dy|, worst |dy|, worst label).
fn compare(name: &str, skip: &[&str]) -> Option<(usize, usize, f32, f32, String)> {
    let db = FontDb::load_system();
    if !fonts_available(&db) {
        eprintln!("skipping {name}: reference fonts not installed");
        return None;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata");
    let doc = Document::open(&root.join(format!("{name}.docx"))).unwrap();
    let ours = positions(&doc, &db);
    let raw = std::fs::read(root.join(format!("reference-{name}.tsv"))).unwrap();
    // PowerShell writes UTF-16LE with BOM.
    let text = if raw.starts_with(&[0xFF, 0xFE]) {
        String::from_utf16_lossy(&raw[2..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>())
    } else {
        String::from_utf8_lossy(&raw).to_string()
    };
    let word: Vec<(usize, f32, String)> = text
        .lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() != 3 || f[2].trim().is_empty() {
                return None;
            }
            Some((f[0].trim().parse().ok()?, f[1].trim().replace(',', ".").parse().ok()?, f[2].to_string()))
        })
        .collect();
    let mut used = vec![false; ours.len()];
    let (mut n, mut bad, mut sum, mut worst, mut worst_label) = (0, 0, 0.0f32, 0.0f32, String::new());
    for (wp, wy, wt) in &word {
        let key: String = wt.chars().take(12).collect();
        if skip.iter().any(|s| wt.starts_with(s)) {
            continue;
        }
        let Some(i) = ours.iter().enumerate().position(|(i, (_, _, t))| !used[i] && t.chars().take(12).collect::<String>() == key) else { continue };
        used[i] = true;
        let (vp, vy, _) = &ours[i];
        n += 1;
        if vp != wp {
            bad += 1;
            eprintln!("{name}: '{key}' on page {vp}, reference page {wp}");
            continue;
        }
        let d = (vy - wy).abs();
        sum += d;
        if d > worst {
            worst = d;
            worst_label = key;
        }
    }
    Some((n, bad, sum / n.max(1) as f32, worst, worst_label))
}

fn check(name: &str, skip: &[&str], max_mean: f32, max_worst: f32) {
    let Some((n, bad, mean, worst, label)) = compare(name, skip) else { return };
    eprintln!("{name}: {n} paragraphs, {bad} page mismatches, mean |dy| {mean:.2}pt, worst {worst:.2}pt ('{label}')");
    assert!(n > 20, "too few paragraphs matched");
    assert_eq!(bad, 0, "paragraphs on a different page than the reference");
    assert!(mean <= max_mean, "mean deviation {mean} > {max_mean}");
    assert!(worst <= max_worst, "worst deviation {worst} > {max_worst} at '{label}'");
}

#[test]
fn sample_matches_word() {
    // Vertically centred merged cell differs by ~1.5pt (border offset).
    check("sample-calibri", &[], 1.0, 2.0);
}

#[test]
fn edited_document_matches_word() {
    check("edited", &[], 1.0, 2.0);
}

#[test]
fn stress_matches_word() {
    // The reference reports the *text* top for at-least spacing (extra space sits above the text).
    check("stress", &["[3] at least"], 1.0, 2.0);
}

#[test]
fn justified_line_breaks_match_word() {
    let db = FontDb::load_system();
    if !fonts_available(&db) {
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata");
    let doc = Document::open(&root.join("justify.docx")).unwrap();
    let layout = layout_document(&doc, &db, &LayoutOptions::default());
    let mut counts: std::collections::BTreeMap<Vec<usize>, usize> = Default::default();
    for p in &layout.pages {
        for l in &p.lines {
            *counts.entry(l.path.clone()).or_default() += 1;
        }
    }
    let raw = std::fs::read(root.join("reference-justify-lines.txt")).unwrap();
    let text = if raw.starts_with(&[0xFF, 0xFE]) {
        String::from_utf16_lossy(&raw[2..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>())
    } else {
        String::from_utf8_lossy(&raw).to_string()
    };
    let word: Vec<usize> = text.lines().filter_map(|l| l.trim().parse().ok()).collect();
    let ours: Vec<usize> = counts.values().copied().collect();
    assert_eq!(word.len(), ours.len());
    let diff = word.iter().zip(&ours).filter(|(a, b)| a != b).count();
    assert_eq!(diff, 0, "justified paragraphs with a different line count than the reference");
}

#[test]
fn roundtrip_preserves_unknown_parts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata");
    let mut doc = Document::open(&root.join("sample.docx")).unwrap();
    // Add an unknown part + custom element; both must survive a save.
    doc.package.set("customXml/item1.xml", b"<root xmlns=\"urn:x\"><v>42</v></root>".to_vec());
    let before: Vec<String> = doc.package.parts.iter().map(|p| p.name.clone()).collect();
    let tmp = std::env::temp_dir().join("rdw-roundtrip-test.docx");
    doc.save_as(&tmp).unwrap();
    let again = Document::open(&tmp).unwrap();
    for name in before {
        assert!(again.package.get(&name).is_some(), "lost part {name}");
    }
    assert_eq!(again.package.get("customXml/item1.xml").unwrap(), b"<root xmlns=\"urn:x\"><v>42</v></root>");
    assert_eq!(again.plain_text(), doc.plain_text());
    let _ = std::fs::remove_file(tmp);
}

#[test]
fn bundled_arial_substitute_is_metric_compatible() {
    // The embedded Liberation Sans must have Arial's advance widths so documents
    // set in Arial paginate identically on machines without Arial.
    let db = FontDb::load_system();
    let lib = db.resolve("Liberation Sans", false, false);
    assert!(lib.font.family.contains("liberation sans"), "bundled font missing: {}", lib.font.family);
    // "Arial" always resolves to something with Arial metrics.
    let arial = db.resolve("Arial", false, false);
    let sample = "The quick brown fox jumps over the lazy dog 0123456789 ΑΒΓΔ αβγδ ÀÉÎÕÜ àéîõü .,;:!?()[]";
    for c in sample.chars() {
        let a = arial.font.advance(c, 1000.0);
        let l = lib.font.advance(c, 1000.0);
        assert!((a - l).abs() < 0.5, "advance mismatch for {c:?}: Arial {a} vs Liberation {l}");
    }
    if db.has_family("arial") {
        // Vertical metrics drive line heights (and so page breaks).
        assert!((arial.font.line_height(11.0) - lib.font.line_height(11.0)).abs() < 0.01, "line height differs: Arial {} vs Liberation {}", arial.font.line_height(11.0), lib.font.line_height(11.0));
        assert!((arial.font.ascent(11.0) - lib.font.ascent(11.0)).abs() < 0.01);
        let real = db.resolve("Arial", true, false);
        let libb = db.resolve("Liberation Sans", true, false);
        for c in sample.chars() {
            assert!((real.font.advance(c, 1000.0) - libb.font.advance(c, 1000.0)).abs() < 0.5, "bold advance mismatch for {c:?}");
        }
    }
}
