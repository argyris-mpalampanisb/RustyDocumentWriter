//! PDF export from our own layout: each page is drawn by [`crate::render`] and
//! stored losslessly, so the PDF looks exactly like the editor (WordArt effects,
//! fonts and all) on any machine.
//!
//! No PDF library is needed: a PNG's IDAT stream is zlib data with per-row PNG
//! filters, which PDF's FlateDecode reads directly with `/Predictor 15`. Text is
//! not selectable in these PDFs; that needs embedded fonts (see the README roadmap).

use crate::render::Raster;
use std::io::Write;

pub struct PdfWriter<W: Write> {
    w: W,
    pos: u64,
    /// Byte offset of each object, indexed by object number (0 is unused).
    offsets: Vec<u64>,
    kids: Vec<usize>,
}

const CATALOG: usize = 1;
const PAGES: usize = 2;
const INFO: usize = 3;

impl<W: Write> PdfWriter<W> {
    pub fn new(w: W, title: &str) -> std::io::Result<Self> {
        let mut p = PdfWriter { w, pos: 0, offsets: vec![0; 4], kids: Vec::new() };
        // The binary comment marks the file as binary for transfer tools.
        p.raw(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n")?;
        p.object(CATALOG, format!("<< /Type /Catalog /Pages {PAGES} 0 R >>").as_bytes())?;
        let info = format!("<< /Title {} /Producer (RustyDocumentWriter) /Creator (RustyDocumentWriter) >>", text_string(title));
        p.object(INFO, info.as_bytes())?;
        Ok(p)
    }

    fn raw(&mut self, b: &[u8]) -> std::io::Result<()> {
        self.w.write_all(b)?;
        self.pos += b.len() as u64;
        Ok(())
    }

    fn next_id(&mut self) -> usize {
        self.offsets.push(0);
        self.offsets.len() - 1
    }

    fn object(&mut self, id: usize, body: &[u8]) -> std::io::Result<()> {
        self.offsets[id] = self.pos;
        self.raw(format!("{id} 0 obj\n").as_bytes())?;
        self.raw(body)?;
        self.raw(b"\nendobj\n")
    }

    fn stream(&mut self, id: usize, dict: &str, data: &[u8]) -> std::io::Result<()> {
        self.offsets[id] = self.pos;
        self.raw(format!("{id} 0 obj\n<< {dict} /Length {} >>\nstream\n", data.len()).as_bytes())?;
        self.raw(data)?;
        self.raw(b"\nendstream\nendobj\n")
    }

    /// Add one page, `width` x `height` points, showing `img` stretched over it.
    pub fn add_page(&mut self, img: &Raster, width: f32, height: f32) -> std::io::Result<()> {
        let (w, h) = (img.width, img.height);
        let (idat, colors) = png_idat(img).map_err(std::io::Error::other)?;
        let space = if colors == 1 { "/DeviceGray" } else { "/DeviceRGB" };
        let image = self.next_id();
        self.stream(
            image,
            &format!(
                "/Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace {space} /BitsPerComponent 8 /Filter /FlateDecode /DecodeParms << /Predictor 15 /Colors {colors} /BitsPerComponent 8 /Columns {w} >>"
            ),
            &idat,
        )?;
        let content = self.next_id();
        let ops = format!("q {width:.3} 0 0 {height:.3} 0 0 cm /Im0 Do Q");
        self.stream(content, "", ops.as_bytes())?;
        let page = self.next_id();
        let body = format!(
            "<< /Type /Page /Parent {PAGES} 0 R /MediaBox [0 0 {width:.3} {height:.3}] /Resources << /XObject << /Im0 {image} 0 R >> >> /Contents {content} 0 R >>"
        );
        self.object(page, body.as_bytes())?;
        self.kids.push(page);
        Ok(())
    }

    pub fn finish(mut self) -> std::io::Result<W> {
        let kids: Vec<String> = self.kids.iter().map(|k| format!("{k} 0 R")).collect();
        let pages = format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), self.kids.len());
        self.object(PAGES, pages.as_bytes())?;
        let xref = self.pos;
        let mut table = format!("xref\n0 {}\n0000000000 65535 f \n", self.offsets.len());
        for off in &self.offsets[1..] {
            table.push_str(&format!("{off:010} 00000 n \n"));
        }
        table.push_str(&format!("trailer\n<< /Size {} /Root {CATALOG} 0 R /Info {INFO} 0 R >>\nstartxref\n{xref}\n%%EOF\n", self.offsets.len()));
        self.raw(table.as_bytes())?;
        self.w.flush()?;
        Ok(self.w)
    }
}

/// A PDF text string: plain when printable ASCII, UTF-16BE hex otherwise.
fn text_string(s: &str) -> String {
    if s.chars().all(|c| (' '..='~').contains(&c)) {
        let esc = s.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)");
        format!("({esc})")
    } else {
        let mut hex = String::from("<FEFF");
        for u in s.encode_utf16() {
            hex.push_str(&format!("{u:04X}"));
        }
        hex.push('>');
        hex
    }
}

/// The zlib stream inside a PNG of `img` (alpha composited onto white) and its
/// number of colour channels: pages with no colour are stored as greyscale,
/// a third of the data. Default compression: on a typical text page "best" is
/// only 1% smaller and ten times slower.
fn png_idat(img: &Raster) -> Result<(Vec<u8>, u8), String> {
    use image::ImageEncoder;
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    let mut rgb = Vec::with_capacity((img.width * img.height * 3) as usize);
    for px in img.pixels.as_chunks::<4>().0 {
        let a = px[3] as u32;
        for &c in &px[..3] {
            rgb.push(((c as u32 * a + 255 * (255 - a)) / 255) as u8);
        }
    }
    let grey = rgb.as_chunks::<3>().0.iter().all(|p| p[0] == p[1] && p[1] == p[2]);
    let (data, kind, colors) = if grey {
        (rgb.as_chunks::<3>().0.iter().map(|p| p[0]).collect(), image::ExtendedColorType::L8, 1)
    } else {
        (rgb, image::ExtendedColorType::Rgb8, 3)
    };
    let mut png = Vec::new();
    PngEncoder::new_with_quality(&mut png, CompressionType::Default, FilterType::Adaptive)
        .write_image(&data, img.width, img.height, kind)
        .map_err(|e| e.to_string())?;
    // Concatenate the IDAT chunks: they are one zlib stream split into pieces.
    let mut idat = Vec::new();
    let mut i = 8;
    while i + 8 <= png.len() {
        let len = u32::from_be_bytes([png[i], png[i + 1], png[i + 2], png[i + 3]]) as usize;
        let kind = &png[i + 4..i + 8];
        if kind == b"IDAT" {
            idat.extend_from_slice(&png[i + 8..i + 8 + len]);
        }
        i += 12 + len;
    }
    Ok((idat, colors))
}

/// Render every page of `layout` at `dpi` into a PDF at `path`.
pub fn export(layout: &crate::layout::Layout, title: &str, dpi: f32, path: &std::path::Path) -> Result<(), String> {
    let tmp = path.with_extension("rdw-tmp");
    let result = (|| -> std::io::Result<()> {
        let f = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
        let mut pdf = PdfWriter::new(f, title)?;
        let mut r = crate::render::Renderer::new();
        for page in &layout.pages {
            let img = r.render(page, dpi / 72.0);
            pdf.add_page(&img, page.width, page.height)?;
        }
        pdf.finish()?;
        Ok(())
    })();
    match result {
        Ok(()) => std::fs::rename(&tmp, path).map_err(|e| format!("Cannot write {}: {e}", path.display())),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(format!("Cannot write {}: {e}", path.display()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_valid_skeleton() {
        let img = Raster { width: 3, height: 2, pixels: vec![255; 3 * 2 * 4] };
        let mut pdf = PdfWriter::new(Vec::new(), "Γειά (test)").unwrap();
        pdf.add_page(&img, 595.0, 842.0).unwrap();
        let out = pdf.finish().unwrap();
        let find = |needle: &[u8], from: usize| out[from..].windows(needle.len()).position(|w| w == needle).map(|p| p + from);
        assert!(out.starts_with(b"%PDF-1.4"));
        for needle in [&b"/Count 1"[..], b"/Predictor 15", b"/Colors 1", b"/DeviceGray", b"<FEFF0393"] {
            assert!(find(needle, 0).is_some());
        }
        // Every xref offset must point at its "N 0 obj" line.
        let sx = find(b"startxref\n", 0).unwrap() + 10;
        let xref_at: usize = std::str::from_utf8(&out[sx..]).unwrap().lines().next().unwrap().parse().unwrap();
        let table = std::str::from_utf8(&out[xref_at..]).unwrap();
        let entries: Vec<&str> = table.lines().skip(3).take_while(|l| l.ends_with(" n ")).collect();
        assert_eq!(entries.len(), 6);
        for (n, line) in entries.iter().enumerate() {
            let off: usize = line[..10].parse().unwrap();
            assert!(out[off..].starts_with(format!("{} 0 obj", n + 1).as_bytes()), "object {}", n + 1);
        }
    }
}
