//! Test documents for Insert → Page Number: a blank 3-page document per style and position.
//!
//! cargo run --release --example make_pagenum -- <out_dir>
use rusty_document_writer::document::Document;
use rusty_document_writer::pagenum::{self, Place};
use rusty_document_writer::{model, xml};

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).expect("usage: make_pagenum <out_dir>"));
    std::fs::create_dir_all(&out).unwrap();
    for st in pagenum::STYLES {
        for place in [Place::Bottom, Place::Top] {
            let mut doc = Document::new_blank();
            let mut b = String::new();
            for n in 1..=3 {
                let brk = if n < 3 { r#"<w:r><w:br w:type="page"/></w:r>"# } else { "" };
                b.push_str(&format!(
                    r#"<w:p><w:r><w:t>{} at the {} of the page. This is page {n} of 3.</w:t></w:r>{brk}</w:p>"#,
                    st.name,
                    if place == Place::Bottom { "bottom" } else { "top" }
                ));
            }
            let x = format!(r#"<w:body xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">{b}</w:body>"#);
            doc.body = model::parse_blocks(&xml::parse_str(&x).unwrap().root);
            doc.insert_page_number(st, place);
            let name = format!("{}-{}.docx", st.id.replace('_', "-"), if place == Place::Bottom { "bottom" } else { "top" });
            doc.save_as(&out.join(&name)).unwrap();
            println!("{}", out.join(name).display());
        }
    }
}
