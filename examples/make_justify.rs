//! Calibration document for justified-text line breaking.
use rusty_document_writer::document::Document;
use rusty_document_writer::{model, xml};
fn main() {
    let out = std::env::args().nth(1).unwrap();
    let mut doc = Document::new_blank();
    let words = "the quick brown fox jumps over a lazy dog while seven wizards quietly box frogs near hazy riverbanks at dawn and every mountain listens to distant thunder";
    let w: Vec<&str> = words.split(' ').collect();
    let mut seed = 99u32;
    let mut b = String::new();
    for i in 0..80 {
        let n = 25 + (i * 7) % 30;
        let mut t = String::new();
        for k in 0..n {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            if k > 0 { t.push(' '); }
            t.push_str(w[(seed >> 16) as usize % w.len()]);
        }
        let font = if i % 2 == 0 { r#"<w:rFonts w:ascii="Times New Roman" w:hAnsi="Times New Roman"/><w:sz w:val="24"/>"# } else { r#"<w:rFonts w:ascii="Calibri" w:hAnsi="Calibri"/><w:sz w:val="22"/>"# };
        b.push_str(&format!(r#"<w:p><w:pPr><w:jc w:val="both"/><w:rPr>{font}</w:rPr></w:pPr><w:r><w:rPr>{font}</w:rPr><w:t>P{i} {t}.</w:t></w:r></w:p>"#));
    }
    let x = format!(r#"<w:body xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">{b}</w:body>"#);
    doc.body = model::parse_blocks(&xml::parse_str(&x).unwrap().root);
    rusty_document_writer::edit::set_margins(&mut doc.final_sectpr, 72.0, 198.0, 72.0, 198.0);
    doc.save_as(std::path::Path::new(&out)).unwrap();
}
