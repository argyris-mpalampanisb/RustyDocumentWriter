//! Builds a feature-rich sample DOCX for testing the layout engine:
//! `cargo run --example make_sample -- sample.docx`

use rusty_document_writer::document::Document;
use rusty_document_writer::model::{self, Block};
use rusty_document_writer::opc;
use rusty_document_writer::xml;

const W: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#;

fn blocks(xml_body: &str) -> Vec<Block> {
    let x = format!("<w:body {W}>{xml_body}</w:body>");
    let root = xml::parse_str(&x).unwrap().root;
    model::parse_blocks(&root)
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "sample.docx".into());
    let mut doc = Document::new_blank();
    let bullets = doc.add_list(true);
    let numbers = doc.add_list(false);

    // A generated image (gradient) to exercise inline pictures.
    let mut img = image::RgbImage::new(240, 120);
    for (x, y, p) in img.enumerate_pixels_mut() {
        *p = image::Rgb([(x as f32 / 240.0 * 255.0) as u8, (y as f32 / 120.0 * 180.0) as u8, 160]);
    }
    let mut png = Vec::new();
    image::DynamicImage::ImageRgb8(img).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    let rid = doc.add_image(png, "png");
    let drawing = rusty_document_writer::edit::image_drawing(&rid, 180.0, 90.0, 1, "Gradient");

    // Header & footer parts with a PAGE field.
    let hdr = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:hdr {W}><w:p><w:pPr><w:pStyle w:val="Header"/></w:pPr><w:r><w:t>RustyDocumentWriter layout test</w:t></w:r><w:r><w:tab/></w:r><w:r><w:tab/></w:r><w:r><w:t>Confidential</w:t></w:r></w:p></w:hdr>"#);
    let ftr = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:ftr {W}><w:p><w:pPr><w:pStyle w:val="Footer"/><w:jc w:val="center"/></w:pPr><w:r><w:t xml:space="preserve">Page </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> PAGE </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>1</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t xml:space="preserve"> of </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> NUMPAGES </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>1</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p></w:ftr>"#);
    doc.package.set("word/header1.xml", hdr.into_bytes());
    doc.package.set("word/footer1.xml", ftr.into_bytes());
    doc.package.ensure_override_content_type("word/header1.xml", "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml");
    doc.package.ensure_override_content_type("word/footer1.xml", "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml");
    let main = doc.main_part.clone();
    let hid = doc.package.add_rel(&main, opc::REL_HEADER, "header1.xml", false);
    let fid = doc.package.add_rel(&main, opc::REL_FOOTER, "footer1.xml", false);
    let sect = &mut doc.final_sectpr;
    sect.children.insert(0, xml::Node::Element(xml::Element::new("w:footerReference").with_attr("w:type", "default").with_attr("r:id", fid.as_str())));
    sect.children.insert(0, xml::Node::Element(xml::Element::new("w:headerReference").with_attr("w:type", "default").with_attr("r:id", hid.as_str())));

    let lorem = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur.";
    let greek = "Η γρήγορη καφέ αλεπού πηδά πάνω από τον τεμπέλη σκύλο. Ελέγχουμε ότι οι ελληνικοί χαρακτήρες μετρώνται και αναδιπλώνονται σωστά.";
    let mut body = String::new();
    body.push_str(r#"<w:p><w:pPr><w:pStyle w:val="Title"/></w:pPr><w:r><w:t>RustyDocumentWriter Fidelity Sample</w:t></w:r></w:p>"#);
    body.push_str(r#"<w:p><w:pPr><w:pStyle w:val="Subtitle"/></w:pPr><w:r><w:t>A test of faithful DOCX layout</w:t></w:r></w:p>"#);
    body.push_str(r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>1. Character formatting</w:t></w:r></w:p>"#);
    body.push_str(r#"<w:p><w:r><w:t xml:space="preserve">Plain, </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>bold</w:t></w:r><w:r><w:t xml:space="preserve">, </w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>italic</w:t></w:r><w:r><w:t xml:space="preserve">, </w:t></w:r><w:r><w:rPr><w:u w:val="single"/></w:rPr><w:t>underlined</w:t></w:r><w:r><w:t xml:space="preserve">, </w:t></w:r><w:r><w:rPr><w:strike/></w:rPr><w:t>struck</w:t></w:r><w:r><w:t xml:space="preserve">, </w:t></w:r><w:r><w:rPr><w:color w:val="C00000"/></w:rPr><w:t>red</w:t></w:r><w:r><w:t xml:space="preserve">, </w:t></w:r><w:r><w:rPr><w:highlight w:val="yellow"/></w:rPr><w:t>highlighted</w:t></w:r><w:r><w:t xml:space="preserve">, E=mc</w:t></w:r><w:r><w:rPr><w:vertAlign w:val="superscript"/></w:rPr><w:t>2</w:t></w:r><w:r><w:t xml:space="preserve">, H</w:t></w:r><w:r><w:rPr><w:vertAlign w:val="subscript"/></w:rPr><w:t>2</w:t></w:r><w:r><w:t xml:space="preserve">O, </w:t></w:r><w:r><w:rPr><w:smallCaps/></w:rPr><w:t>Small Caps</w:t></w:r><w:r><w:t xml:space="preserve"> and </w:t></w:r><w:r><w:rPr><w:rFonts w:ascii="Times New Roman" w:hAnsi="Times New Roman"/><w:sz w:val="28"/></w:rPr><w:t>Times 14pt</w:t></w:r><w:r><w:t>.</w:t></w:r></w:p>"#);
    body.push_str(&format!(r#"<w:p><w:pPr><w:jc w:val="both"/></w:pPr><w:r><w:t>{lorem} {lorem}</w:t></w:r></w:p>"#));
    body.push_str(&format!(r#"<w:p><w:r><w:t>{greek}</w:t></w:r></w:p>"#));
    body.push_str(r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>2. Lists and tabs</w:t></w:r></w:p>"#);
    for (lvl, t) in [(0, "First bullet"), (0, "Second bullet"), (1, "Nested bullet"), (0, "Third bullet")] {
        body.push_str(&format!(r#"<w:p><w:pPr><w:pStyle w:val="ListParagraph"/><w:numPr><w:ilvl w:val="{lvl}"/><w:numId w:val="{bullets}"/></w:numPr></w:pPr><w:r><w:t>{t}</w:t></w:r></w:p>"#));
    }
    for (lvl, t) in [(0, "Step one"), (0, "Step two"), (1, "Sub-step a"), (1, "Sub-step b"), (0, "Step three")] {
        body.push_str(&format!(r#"<w:p><w:pPr><w:pStyle w:val="ListParagraph"/><w:numPr><w:ilvl w:val="{lvl}"/><w:numId w:val="{numbers}"/></w:numPr></w:pPr><w:r><w:t>{t}</w:t></w:r></w:p>"#));
    }
    body.push_str(r#"<w:p><w:pPr><w:tabs><w:tab w:val="right" w:leader="dot" w:pos="9360"/></w:tabs></w:pPr><w:r><w:t>Introduction</w:t></w:r><w:r><w:tab/></w:r><w:r><w:t>1</w:t></w:r></w:p>"#);
    body.push_str(r#"<w:p><w:pPr><w:tabs><w:tab w:val="center" w:pos="4680"/><w:tab w:val="decimal" w:pos="8000"/></w:tabs></w:pPr><w:r><w:t>Left</w:t></w:r><w:r><w:tab/></w:r><w:r><w:t>Centered</w:t></w:r><w:r><w:tab/></w:r><w:r><w:t>1234.56</w:t></w:r></w:p>"#);
    body.push_str(r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>3. Tables</w:t></w:r></w:p>"#);
    body.push_str(r#"<w:tbl><w:tblPr><w:tblStyle w:val="TableGrid"/><w:tblW w:w="0" w:type="auto"/><w:tblLook w:val="04A0" w:firstRow="1" w:lastRow="0" w:firstColumn="1" w:lastColumn="0" w:noHBand="0" w:noVBand="1"/></w:tblPr><w:tblGrid><w:gridCol w:w="3120"/><w:gridCol w:w="3120"/><w:gridCol w:w="3120"/></w:tblGrid>"#);
    body.push_str(r#"<w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:tcPr><w:tcW w:w="3120" w:type="dxa"/><w:shd w:val="clear" w:color="auto" w:fill="156082" w:themeFill="accent1"/></w:tcPr><w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Feature</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="3120" w:type="dxa"/><w:shd w:val="clear" w:color="auto" w:fill="156082" w:themeFill="accent1"/></w:tcPr><w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Status</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="3120" w:type="dxa"/><w:shd w:val="clear" w:color="auto" w:fill="156082" w:themeFill="accent1"/></w:tcPr><w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Notes</w:t></w:r></w:p></w:tc></w:tr>"#);
    body.push_str(r#"<w:tr><w:tc><w:tcPr><w:tcW w:w="3120" w:type="dxa"/><w:vMerge w:val="restart"/><w:vAlign w:val="center"/></w:tcPr><w:p><w:r><w:t>Merged vertically</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="3120" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>Done</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="3120" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>Cell text wraps inside its column when it is long enough to need it.</w:t></w:r></w:p></w:tc></w:tr>"#);
    body.push_str(r#"<w:tr><w:tc><w:tcPr><w:tcW w:w="3120" w:type="dxa"/><w:vMerge/></w:tcPr><w:p/></w:tc><w:tc><w:tcPr><w:tcW w:w="6240" w:type="dxa"/><w:gridSpan w:val="2"/></w:tcPr><w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:t>Spans two columns</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#);
    body.push_str(r#"<w:p/>"#);
    body.push_str(r#"<w:p><w:pPr><w:pStyle w:val="Heading2"/></w:pPr><w:r><w:t>3.1 An inline picture</w:t></w:r></w:p>"#);
    body.push_str(r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:t>PLACEHOLDER_IMAGE</w:t></w:r></w:p>"#);
    body.push_str(r#"<w:p><w:pPr><w:pStyle w:val="Quote"/></w:pPr><w:r><w:t>“Quotes use their own style, centered and italic.”</w:t></w:r></w:p>"#);
    body.push_str(r#"<w:p><w:r><w:br w:type="page"/></w:r></w:p>"#);
    body.push_str(r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>4. Second page</w:t></w:r></w:p>"#);
    for _ in 0..8 {
        body.push_str(&format!(r#"<w:p><w:r><w:t>{lorem}</w:t></w:r></w:p>"#));
    }
    doc.body = blocks(&body);
    // Swap the placeholder run for the drawing.
    for b in doc.body.iter_mut() {
        if let Block::Paragraph(p) = b {
            if p.plain_text() == "PLACEHOLDER_IMAGE" {
                p.inlines = vec![model::Inline::Run(model::Run { rpr: None, items: vec![model::RunItem::Drawing(drawing.clone())], attrs: vec![] })];
            }
        }
    }
    // Reload so header/footer parts are picked up.
    let pkg = doc.to_package();
    let mut pkg = pkg;
    // Optional: swap theme fonts (e.g. RDW_FONTS="Calibri Light,Calibri") for engine-vs-reference comparisons
    // on machines without the Aptos cloud font.
    if let Ok(f) = std::env::var("RDW_FONTS") {
        let mut parts = f.split(',');
        let (major, minor) = (parts.next().unwrap_or("Calibri Light").to_string(), parts.next().unwrap_or("Calibri").to_string());
        let theme = String::from_utf8(pkg.get("word/theme/theme1.xml").unwrap().to_vec()).unwrap();
        let theme = theme.replace("typeface=\"Aptos Display\"", &format!("typeface=\"{major}\"")).replace("typeface=\"Aptos\"", &format!("typeface=\"{minor}\""));
        pkg.set("word/theme/theme1.xml", theme.into_bytes());
    }
    let mut doc = Document::from_package(pkg).unwrap();
    doc.save_as(std::path::Path::new(&out)).unwrap();
    println!("wrote {out}");
}
