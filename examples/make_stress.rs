//! Multi-page stress document for comparing pagination with a reference:
//! `cargo run --example make_stress -- stress.docx`

use rusty_document_writer::document::Document;
use rusty_document_writer::model;
use rusty_document_writer::xml;

const W: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#;

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "stress.docx".into());
    let mut doc = Document::new_blank();
    let words = "The quick brown fox jumps over the lazy dog while seven wizards quietly box jumping frogs near a hazy riverbank at dawn";
    let w: Vec<&str> = words.split(' ').collect();
    let mut seed = 7u32;
    let mut sentence = |n: usize| -> String {
        let mut s = String::new();
        for i in 0..n {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let word = w[(seed >> 16) as usize % w.len()];
            if i > 0 {
                s.push(' ');
            }
            s.push_str(word);
        }
        s.push('.');
        s
    };
    let tnr = r#"<w:rFonts w:ascii="Times New Roman" w:hAnsi="Times New Roman"/><w:sz w:val="24"/>"#;
    let mut b = String::new();
    let para = |ppr: &str, rpr: &str, text: &str| format!(r#"<w:p><w:pPr>{ppr}<w:rPr>{rpr}</w:rPr></w:pPr><w:r><w:rPr>{rpr}</w:rPr><w:t xml:space="preserve">{text}</w:t></w:r></w:p>"#);
    b.push_str(&para(r#"<w:pStyle w:val="Heading1"/>"#, r#"<w:rFonts w:ascii="Cambria" w:hAnsi="Cambria"/>"#, "Stress test: spacing rules"));
    for (i, (ppr, label)) in [
        (r#"<w:spacing w:line="360" w:lineRule="auto"/><w:jc w:val="both"/>"#, "1.5 lines justified"),
        (r#"<w:spacing w:line="300" w:lineRule="exact"/>"#, "exactly 15pt"),
        (r#"<w:spacing w:line="400" w:lineRule="atLeast"/>"#, "at least 20pt"),
        (r#"<w:ind w:firstLine="720"/><w:spacing w:before="240" w:after="240"/>"#, "first line indent, 12pt before/after"),
        (r#"<w:ind w:left="720" w:hanging="720"/><w:spacing w:after="0"/>"#, "hanging indent, no space after"),
        (r#"<w:pBdr><w:top w:val="single" w:sz="8" w:space="4" w:color="auto"/><w:bottom w:val="single" w:sz="8" w:space="4" w:color="auto"/></w:pBdr><w:shd w:val="clear" w:color="auto" w:fill="DDEEFF"/>"#, "bordered and shaded"),
        (r#"<w:jc w:val="center"/><w:spacing w:line="480" w:lineRule="auto"/>"#, "double spaced centered"),
        (r#"<w:ind w:left="1440" w:right="1440"/>"#, "indented both sides"),
    ]
    .iter()
    .enumerate()
    {
        let text = format!("[{}] {} {} {}", i + 1, label, sentence(40), sentence(30));
        b.push_str(&para(ppr, tnr, &text));
    }
    b.push_str(&para(r#"<w:pStyle w:val="Heading1"/>"#, "", "Widows, orphans and keeps"));
    for i in 0..14 {
        let ppr = if i == 6 { r#"<w:keepLines/>"# } else { "" };
        let text = format!("({}) {} {} {}", i + 1, sentence(25), sentence(35), sentence(20));
        b.push_str(&para(ppr, tnr, &text));
    }
    // Continuous section break: two columns follow.
    b.push_str(&format!(
        r#"<w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/><w:cols w:space="720"/><w:docGrid w:linePitch="360"/></w:sectPr></w:pPr><w:r><w:rPr>{tnr}</w:rPr><w:t>End of single-column section.</w:t></w:r></w:p>"#
    ));
    b.push_str(&para(r#"<w:pStyle w:val="Heading1"/>"#, "", "Two columns"));
    for i in 0..10 {
        let text = format!("#{} {} {}", i + 1, sentence(30), sentence(25));
        b.push_str(&para(r#"<w:jc w:val="both"/>"#, tnr, &text));
    }
    b.push_str(&format!(
        r#"<w:p><w:pPr><w:sectPr><w:type w:val="continuous"/><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/><w:cols w:num="2" w:space="720"/><w:docGrid w:linePitch="360"/></w:sectPr></w:pPr><w:r><w:rPr>{tnr}</w:rPr><w:t>Last paragraph of the two-column section.</w:t></w:r></w:p>"#
    ));
    b.push_str(&para(r#"<w:pStyle w:val="Heading1"/>"#, "", "Landscape section"));
    for i in 0..6 {
        let text = format!("{{{}}} {} {}", i + 1, sentence(40), sentence(40));
        b.push_str(&para("", tnr, &text));
    }
    let x = format!("<w:body {W}>{b}</w:body>");
    doc.body = model::parse_blocks(&xml::parse_str(&x).unwrap().root);
    // Final section: landscape.
    let mut sp = xml::parse_str(&format!(r#"<w:sectPr {W}><w:type w:val="nextPage"/><w:pgSz w:w="15840" w:h="12240" w:orient="landscape"/><w:pgMar w:top="1080" w:right="1080" w:bottom="1080" w:left="1080" w:header="720" w:footer="720" w:gutter="0"/><w:cols w:space="720"/><w:docGrid w:linePitch="360"/></w:sectPr>"#)).unwrap().root;
    sp.attrs.retain(|(k, _)| !k.starts_with("xmlns"));
    doc.final_sectpr = sp;
    // Use Calibri/Cambria theme fonts so the reference and RustyDocumentWriter share fonts on this machine.
    let mut pkg = doc.to_package();
    let theme = String::from_utf8(pkg.get("word/theme/theme1.xml").unwrap().to_vec()).unwrap();
    pkg.set("word/theme/theme1.xml", theme.replace("typeface=\"Aptos Display\"", "typeface=\"Calibri Light\"").replace("typeface=\"Aptos\"", "typeface=\"Calibri\"").into_bytes());
    let mut doc = Document::from_package(pkg).unwrap();
    doc.save_as(std::path::Path::new(&out)).unwrap();
    println!("wrote {out}");
}
