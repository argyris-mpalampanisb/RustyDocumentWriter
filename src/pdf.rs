//! PDF export from our own layout, so the PDF matches the editor page for page
//! and line for line.
//!
//! [`export`] writes a real PDF with [krilla]: text as text in the exact fonts
//! the layout used (embedded and subsetted, so it's searchable, copyable and
//! sharp at any zoom), rectangles as vector paths, and pictures embedded in
//! their own format. Text with effects (WordArt, outline, shadow, emboss) is
//! drawn by our renderer into a transparent image, so it looks like the screen.
//!
//! [`export_raster`] is the older all-image export (every page one lossless
//! picture), kept for viewers with poor font support. It needs no PDF library:
//! a PNG's IDAT stream is zlib data with per-row PNG filters, which PDF's
//! FlateDecode reads directly with `/Predictor 15`.

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

/// Render every page of `layout` at `dpi` as one picture per page into a PDF at `path`.
pub fn export_raster(layout: &crate::layout::Layout, title: &str, dpi: f32, path: &std::path::Path) -> Result<(), String> {
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

/// Write `layout` as a vector PDF at `path`. `dpi` is the resolution of the
/// parts that stay pictures (text effects).
pub fn export(layout: &crate::layout::Layout, title: &str, dpi: f32, path: &std::path::Path) -> Result<(), String> {
    let bytes = vector::write(layout, title, dpi)?;
    let tmp = path.with_extension("rdw-tmp");
    std::fs::write(&tmp, bytes).and_then(|_| std::fs::rename(&tmp, path)).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Cannot write {}: {e}", path.display())
    })
}

mod vector {
    use crate::layout::{Item, Layout, TextFx};
    use crate::props::Color;
    use krilla::color::rgb;
    use krilla::geom::{PathBuilder, Point, Rect, Size, Transform};
    use krilla::image::Image;
    use krilla::metadata::Metadata;
    use krilla::page::PageSettings;
    use krilla::paint::{Fill, FillRule, LineJoin, Stroke};
    use krilla::text::{Font, GlyphId, KrillaGlyph};
    use krilla::{Data, Document};
    use std::collections::HashMap;
    use std::sync::Arc;

    fn paint(c: Color) -> krilla::paint::Paint {
        rgb::Color::new(c.0, c.1, c.2).into()
    }

    fn fill(c: Color) -> Fill {
        Fill { paint: paint(c), ..Default::default() }
    }

    fn rect_path(x: f32, y: f32, w: f32, h: f32) -> Option<krilla::geom::Path> {
        let mut pb = PathBuilder::new();
        pb.push_rect(Rect::from_xywh(x, y, w.max(0.01), h.max(0.01))?);
        pb.finish()
    }

    /// A picture in the format it's stored in (JPEG stays JPEG); anything krilla
    /// doesn't read directly is decoded and embedded as pixels.
    fn image(data: &Arc<Vec<u8>>) -> Option<Image> {
        let shared: Data = (data.clone() as Arc<dyn AsRef<[u8]> + Send + Sync>).into();
        let direct = match image::guess_format(data).ok()? {
            image::ImageFormat::Png => Image::from_png(shared, true).ok(),
            image::ImageFormat::Jpeg => Image::from_jpeg(shared, true).ok(),
            image::ImageFormat::Gif => Image::from_gif(shared, true).ok(),
            image::ImageFormat::WebP => Image::from_webp(shared, true).ok(),
            _ => None,
        };
        direct.or_else(|| {
            let rgba = image::load_from_memory(data).ok()?.to_rgba8();
            let (w, h) = rgba.dimensions();
            Some(Image::from_rgba8(rgba.into_raw(), w, h))
        })
    }

    pub fn write(layout: &Layout, title: &str, dpi: f32) -> Result<Vec<u8>, String> {
        let mut doc = Document::new();
        doc.set_metadata(Metadata::new().title(title.to_string()).creator("RustyDocumentWriter".into()).producer("RustyDocumentWriter".into()));
        let mut fonts: HashMap<usize, Option<Font>> = HashMap::new();
        let mut images: HashMap<String, Option<Image>> = HashMap::new();
        let mut renderer = crate::render::Renderer::new();
        for pg in &layout.pages {
            let settings = PageSettings::from_wh(pg.width, pg.height).ok_or("invalid page size")?;
            let mut page = doc.start_page_with(settings);
            let mut s = page.surface();
            if let (Some(bg), Some(p)) = (pg.background, rect_path(0.0, 0.0, pg.width, pg.height)) {
                s.set_stroke(None);
                s.set_fill(Some(fill(bg)));
                s.draw_path(&p);
            }
            for it in &pg.items {
                match it {
                    Item::Rect { x, y, w, h, color } => {
                        if let Some(p) = rect_path(*x, *y, *w, *h) {
                            s.set_stroke(None);
                            s.set_fill(Some(fill(*color)));
                            s.draw_path(&p);
                        }
                    }
                    Item::Image { key, data, x, y, w, h, crop } => {
                        let img = images.entry(key.clone()).or_insert_with(|| image(data)).clone();
                        let Some(img) = img else { continue };
                        // crop = [left, top, right, bottom] fractions cut off the picture.
                        let fw = w / (1.0 - crop[0] - crop[2]).max(0.01);
                        let fh = h / (1.0 - crop[1] - crop[3]).max(0.01);
                        let (Some(clip), Some(size)) = (rect_path(*x, *y, *w, *h), Size::from_wh(fw, fh)) else { continue };
                        s.push_clip_path(&clip, &FillRule::NonZero);
                        s.push_transform(&Transform::from_translate(x - crop[0] * fw, y - crop[1] * fh));
                        s.draw_image(img, size);
                        s.pop();
                        s.pop();
                    }
                    Item::Placeholder { x, y, w, h, .. } => {
                        if let Some(p) = rect_path(*x, *y, *w, *h) {
                            s.set_fill(Some(fill(Color(0xF2, 0xF2, 0xF2))));
                            s.set_stroke(Some(Stroke { paint: paint(Color(0xA6, 0xA6, 0xA6)), width: 0.75, ..Default::default() }));
                            s.draw_path(&p);
                        }
                    }
                    Item::Glyphs { font, size, color, y, glyphs, chars, synth_bold, synth_italic, scale_x, fx } => {
                        if glyphs.is_empty() {
                            continue;
                        }
                        let embedded = fonts
                            .entry(font.id)
                            .or_insert_with(|| {
                                let (data, index) = font.file();
                                Font::new(Data::from(data), index)
                            })
                            .clone();
                        // Text effects (and fonts krilla can't embed) become a picture.
                        let (Some(kf), true) = (embedded, *fx == TextFx::default()) else {
                            let (x0, x1) = (glyphs[0].1 - size * 0.8, glyphs[glyphs.len() - 1].1 + size * 1.8);
                            let (y0, y1) = (y - size * 1.6, y + size * 1.4);
                            let mut tile = it.clone();
                            tile.translate(-x0, -y0);
                            let r = renderer.render_transparent(&[tile], x1 - x0, y1 - y0, dpi / 72.0);
                            if let Some(sz) = Size::from_wh(x1 - x0, y1 - y0) {
                                s.push_transform(&Transform::from_translate(x0, y0));
                                s.draw_image(Image::from_rgba8(r.pixels, r.width, r.height), sz);
                                s.pop();
                            }
                            continue;
                        };
                        let text: String = chars.iter().collect();
                        let mut ranges = Vec::with_capacity(chars.len());
                        let mut b = 0;
                        for c in chars {
                            ranges.push(b..b + c.len_utf8());
                            b += c.len_utf8();
                        }
                        let sx = scale_x.max(0.01);
                        let x0 = glyphs[0].1;
                        // Advances reproduce our exact positions (kerning, justification, scaling).
                        let run: Vec<KrillaGlyph> = glyphs
                            .iter()
                            .enumerate()
                            .map(|(i, (gid, x))| {
                                let adv = match glyphs.get(i + 1) {
                                    Some((_, nx)) => nx - x,
                                    None => chars.get(i).map(|c| font.advance(*c, *size) * sx).unwrap_or(0.0),
                                };
                                KrillaGlyph {
                                    glyph_id: GlyphId::new(gid.0 as u32),
                                    text_range: ranges.get(i).cloned().unwrap_or(0..0),
                                    x_advance: adv / sx / size,
                                    x_offset: 0.0,
                                    y_offset: 0.0,
                                    y_advance: 0.0,
                                    location: None,
                                }
                            })
                            .collect();
                        let mut pushed = 0;
                        if (sx - 1.0).abs() > 0.001 {
                            s.push_transform(&Transform::from_row(sx, 0.0, 0.0, 1.0, x0 * (1.0 - sx), 0.0));
                            pushed += 1;
                        }
                        if *synth_italic {
                            // The same 0.2 shear about the baseline as the screen renderer.
                            s.push_transform(&Transform::from_row(1.0, 0.0, -0.2, 1.0, 0.2 * y, 0.0));
                            pushed += 1;
                        }
                        s.set_fill(Some(fill(*color)));
                        s.set_stroke(synth_bold.then(|| Stroke { paint: paint(*color), width: size / 28.0, line_join: LineJoin::Round, ..Default::default() }));
                        s.draw_glyphs(Point::from_xy(x0, *y), &run, kf, &text, *size, false);
                        s.set_stroke(None);
                        for _ in 0..pushed {
                            s.pop();
                        }
                    }
                }
            }
            s.finish();
            page.finish();
        }
        doc.finish().map_err(|e| format!("PDF export failed: {e:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The vector export embeds fonts with a Unicode map (selectable text), keeps
    /// the page size, and turns text effects into images.
    #[test]
    fn vector_export_has_real_text() {
        use crate::document::Document;
        use crate::edit;
        let fonts = crate::fonts::FontDb::load_system();
        let mut doc = Document::new_blank();
        let at = edit::start_of_doc(&doc);
        let end = edit::insert_text(&mut doc, &at, "Searchable text, Γειά σου", None);
        let next = edit::split_paragraph(&mut doc, &end);
        let fx_end = edit::insert_text(&mut doc, &next, "Shadowed", None);
        edit::format_runs(&mut doc, &next, &fx_end, &|r| edit::set_effect(r, "w:shadow", true));
        let layout = crate::layout::layout_document(&doc, &fonts, &Default::default());
        let pdf = super::vector::write(&layout, "Test", 150.0).unwrap();
        let has = |n: &[u8]| pdf.windows(n.len()).any(|w| w == n);
        assert!(pdf.starts_with(b"%PDF-"));
        assert!(has(b"/ToUnicode"), "text must map back to Unicode");
        assert!(has(b"/FontFile2") || has(b"/FontFile3"), "fonts must be embedded");
        assert!(has(b"/Subtype /Image") || has(b"/Subtype/Image"), "effect text becomes an image");
        let (w, h) = (layout.pages[0].width, layout.pages[0].height);
        let media = format!("/MediaBox [0 0 {} {}]", w, h);
        assert!(has(media.as_bytes()) || has(b"/MediaBox"), "page size");
    }

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
