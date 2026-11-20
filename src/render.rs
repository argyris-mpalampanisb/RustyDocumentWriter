//! Software rasterizer for laid-out pages.
//!
//! We rasterize glyphs ourselves (instead of letting the GUI toolkit lay out
//! text) so that what you see is exactly what the layout engine measured.

use crate::layout::{Item, Page};
use crate::props::Color;
use ab_glyph::{Font as _, Glyph, PxScale};
use std::collections::HashMap;
use std::sync::Arc;

struct GlyphBitmap {
    left: i32,
    top: i32,
    w: u32,
    h: u32,
    data: Vec<u8>,
}

#[derive(Default)]
pub struct Renderer {
    glyphs: HashMap<(usize, u16, u32, u8), Option<Arc<GlyphBitmap>>>,
    images: HashMap<String, Option<Arc<image::RgbaImage>>>,
    scaled: HashMap<(String, u32, u32, [u32; 4]), Arc<image::RgbaImage>>,
}

pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>, // RGBA
}

impl Raster {
    fn blend(&mut self, x: i32, y: i32, c: Color, a: f32) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 || a <= 0.0 {
            return;
        }
        let i = ((y as u32 * self.width + x as u32) * 4) as usize;
        let a = a.min(1.0);
        let p = &mut self.pixels[i..i + 4];
        p[0] = (p[0] as f32 * (1.0 - a) + c.0 as f32 * a) as u8;
        p[1] = (p[1] as f32 * (1.0 - a) + c.1 as f32 * a) as u8;
        p[2] = (p[2] as f32 * (1.0 - a) + c.2 as f32 * a) as u8;
    }

    fn fill_rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, c: Color) {
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        // Hairlines stay visible at low zoom.
        let (x0, x1) = if x1 - x0 < 1.0 { let m = (x0 + x1) / 2.0; (m - 0.5, m + 0.5) } else { (x0, x1) };
        let (y0, y1) = if y1 - y0 < 1.0 { let m = (y0 + y1) / 2.0; (m - 0.5, m + 0.5) } else { (y0, y1) };
        let ix0 = x0.floor().max(0.0) as i32;
        let iy0 = y0.floor().max(0.0) as i32;
        let ix1 = (x1.ceil() as i32).min(self.width as i32);
        let iy1 = (y1.ceil() as i32).min(self.height as i32);
        for y in iy0..iy1 {
            let cy = (y1.min(y as f32 + 1.0) - y0.max(y as f32)).clamp(0.0, 1.0);
            for x in ix0..ix1 {
                let cx = (x1.min(x as f32 + 1.0) - x0.max(x as f32)).clamp(0.0, 1.0);
                self.blend(x, y, c, cx * cy);
            }
        }
    }

    pub fn save_png(&self, path: &std::path::Path) -> Result<(), String> {
        image::save_buffer(path, &self.pixels, self.width, self.height, image::ExtendedColorType::Rgba8).map_err(|e| e.to_string())
    }
}

impl Renderer {
    pub fn new() -> Self {
        Self::default()
    }

    fn glyph(&mut self, font: &crate::fonts::LoadedFont, gid: ab_glyph::GlyphId, px: f32, subpx: u8) -> Option<Arc<GlyphBitmap>> {
        let key = (font.id, gid.0, (px * 16.0).round() as u32, subpx);
        if let Some(g) = self.glyphs.get(&key) {
            return g.clone();
        }
        let h_unscaled = font.font.height_unscaled();
        let scale = PxScale::from(px * h_unscaled / font.upm);
        let glyph: Glyph = gid.with_scale_and_position(scale, ab_glyph::point(subpx as f32 / 4.0, 0.0));
        let result = font.font.outline_glyph(glyph).map(|og| {
            let b = og.px_bounds();
            let w = b.width().ceil() as u32 + 1;
            let h = b.height().ceil() as u32 + 1;
            let mut data = vec![0u8; (w * h) as usize];
            og.draw(|x, y, c| {
                if x < w && y < h {
                    let v = (c.clamp(0.0, 1.0).powf(0.85) * 255.0) as u8;
                    let i = (y * w + x) as usize;
                    data[i] = data[i].max(v);
                }
            });
            Arc::new(GlyphBitmap { left: b.min.x.floor() as i32, top: b.min.y.floor() as i32, w, h, data })
        });
        if self.glyphs.len() > 60_000 {
            self.glyphs.clear();
        }
        self.glyphs.insert(key, result.clone());
        result
    }

    fn image(&mut self, key: &str, data: &[u8]) -> Option<Arc<image::RgbaImage>> {
        if let Some(i) = self.images.get(key) {
            return i.clone();
        }
        let img = image::load_from_memory(data).ok().map(|i| Arc::new(i.to_rgba8()));
        self.images.insert(key.to_string(), img.clone());
        img
    }

    /// Render a page at `scale` pixels per point.
    pub fn render(&mut self, page: &Page, scale: f32) -> Raster {
        let width = (page.width * scale).ceil().max(1.0) as u32;
        let height = (page.height * scale).ceil().max(1.0) as u32;
        let bg = page.background.unwrap_or(Color::WHITE);
        let mut pixels = vec![255u8; (width * height * 4) as usize];
        for px in pixels.chunks_exact_mut(4) {
            px[0] = bg.0;
            px[1] = bg.1;
            px[2] = bg.2;
            px[3] = 255;
        }
        let mut r = Raster { width, height, pixels };
        for it in &page.items {
            match it {
                Item::Rect { x, y, w, h, color } => {
                    r.fill_rect(x * scale, y * scale, (x + w) * scale, (y + h) * scale, *color);
                }
                Item::Glyphs { font, size, color, y, glyphs, synth_bold, synth_italic, .. } => {
                    let px = size * scale;
                    if px < 0.5 {
                        continue;
                    }
                    let by = y * scale;
                    let iby = by.round() as i32;
                    let bold_off = if *synth_bold { (px / 28.0).max(0.6) } else { 0.0 };
                    for (gid, gx) in glyphs {
                        let fx = gx * scale;
                        let ix = fx.floor();
                        let sub = ((fx - ix) * 4.0).floor() as u8;
                        let Some(g) = self.glyph(font, *gid, px, sub.min(3)) else { continue };
                        let passes: &[f32] = if bold_off > 0.0 { &[0.0, 1.0] } else { &[0.0] };
                        for pass in passes {
                            let ox = ix as i32 + g.left + (pass * bold_off).round() as i32;
                            for gy in 0..g.h {
                                let rel_y = g.top + gy as i32;
                                let shear = if *synth_italic { (-(rel_y as f32) * 0.2).round() as i32 } else { 0 };
                                let row = &g.data[(gy * g.w) as usize..((gy + 1) * g.w) as usize];
                                for (gxp, &cov) in row.iter().enumerate() {
                                    if cov > 0 {
                                        r.blend(ox + gxp as i32 + shear, iby + rel_y, *color, cov as f32 / 255.0);
                                    }
                                }
                            }
                        }
                    }
                }
                Item::Image { key, data, x, y, w, h, crop } => {
                    let dw = (w * scale).round().max(1.0) as u32;
                    let dh = (h * scale).round().max(1.0) as u32;
                    if dw > 8000 || dh > 8000 {
                        continue;
                    }
                    let ckey = (key.clone(), dw, dh, crop.map(|c| (c * 1000.0) as u32));
                    let scaled = if let Some(s) = self.scaled.get(&ckey) {
                        Some(s.clone())
                    } else if let Some(src) = self.image(key, data) {
                        let (sw, sh) = (src.width() as f32, src.height() as f32);
                        let cx = (crop[0] * sw).max(0.0) as u32;
                        let cy = (crop[1] * sh).max(0.0) as u32;
                        let cw = ((1.0 - crop[0] - crop[2]) * sw).max(1.0) as u32;
                        let ch = ((1.0 - crop[1] - crop[3]) * sh).max(1.0) as u32;
                        let view = image::imageops::crop_imm(src.as_ref(), cx.min(src.width() - 1), cy.min(src.height() - 1), cw.min(src.width()), ch.min(src.height())).to_image();
                        let s = Arc::new(image::imageops::resize(&view, dw, dh, image::imageops::FilterType::Triangle));
                        if self.scaled.len() > 64 {
                            self.scaled.clear();
                        }
                        self.scaled.insert(ckey, s.clone());
                        Some(s)
                    } else {
                        None
                    };
                    let ox = (x * scale).round() as i32;
                    let oy = (y * scale).round() as i32;
                    match scaled {
                        Some(img) => {
                            for (ix, iy, p) in img.enumerate_pixels() {
                                let a = p[3] as f32 / 255.0;
                                r.blend(ox + ix as i32, oy + iy as i32, Color(p[0], p[1], p[2]), a);
                            }
                        }
                        None => draw_placeholder(&mut r, x * scale, y * scale, w * scale, h * scale),
                    }
                }
                Item::Placeholder { x, y, w, h, .. } => draw_placeholder(&mut r, x * scale, y * scale, w * scale, h * scale),
            }
        }
        r
    }
}

fn draw_placeholder(r: &mut Raster, x: f32, y: f32, w: f32, h: f32) {
    let c = Color(0xF2, 0xF2, 0xF2);
    r.fill_rect(x, y, x + w, y + h, c);
    let e = Color(0xA6, 0xA6, 0xA6);
    r.fill_rect(x, y, x + w, y + 1.0, e);
    r.fill_rect(x, y + h - 1.0, x + w, y + h, e);
    r.fill_rect(x, y, x + 1.0, y + h, e);
    r.fill_rect(x + w - 1.0, y, x + w, y + h, e);
    // Diagonal cross.
    let n = (w.max(h)) as i32;
    for i in 0..n {
        let t = i as f32 / n as f32;
        r.blend((x + t * w) as i32, (y + t * h) as i32, e, 0.7);
        r.blend((x + t * w) as i32, (y + (1.0 - t) * h) as i32, e, 0.7);
    }
}
