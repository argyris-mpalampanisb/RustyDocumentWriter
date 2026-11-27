//! Software rasterizer for laid-out pages.
//!
//! We rasterize glyphs ourselves (instead of letting the GUI toolkit lay out
//! text) so that what you see is exactly what the layout engine measured.

use crate::layout::{Item, Page};
use crate::props::{Color, Paint, RichFx};
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

/// Blend one glyph bitmap with its origin at (`ox`, `oy`), applying synthetic bold
/// (a second pass `bold_off` pixels right) and synthetic italic (a shear).
#[allow(clippy::too_many_arguments)]
fn stamp(r: &mut Raster, g: &GlyphBitmap, ox: i32, oy: i32, bold_off: f32, italic: bool, c: Color, alpha: f32) {
    let passes: &[f32] = if bold_off > 0.0 { &[0.0, 1.0] } else { &[0.0] };
    for pass in passes {
        let x0 = ox + g.left + (pass * bold_off).round() as i32;
        for gy in 0..g.h {
            let rel_y = g.top + gy as i32;
            let shear = if italic { (-(rel_y as f32) * 0.2).round() as i32 } else { 0 };
            let row = &g.data[(gy * g.w) as usize..((gy + 1) * g.w) as usize];
            for (gxp, &cov) in row.iter().enumerate() {
                if cov > 0 {
                    r.blend(x0 + gxp as i32 + shear, oy + rel_y, c, cov as f32 / 255.0 * alpha);
                }
            }
        }
    }
}

/// The outer stroke of a glyph: its coverage dilated by `rw` pixels minus the glyph itself.
/// `bold` (pixels) widens the glyph first, standing in for synthetic bold.
fn ring(g: &GlyphBitmap, rw: i32, bold: i32) -> GlyphBitmap {
    let (gw, gh) = (g.w as i32, g.h as i32);
    let at = |x: i32, y: i32| -> u8 {
        if x < 0 || y < 0 || x >= gw || y >= gh { 0 } else { g.data[(y * gw + x) as usize] }
    };
    // The glyph body, with synthetic bold folded in.
    let body = |x: i32, y: i32| -> u8 { (0..=bold).map(|b| at(x - b, y)).max().unwrap_or(0) };
    let pad = rw + 1;
    let (w, h) = (gw + bold + pad * 2, gh + pad * 2);
    let mut data = vec![0u8; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let (sx, sy) = (x - pad, y - pad);
            let mut dil = 0u8;
            for dy in -rw..=rw {
                for dx in -rw..=rw {
                    if dx * dx + dy * dy <= rw * rw + rw {
                        dil = dil.max(body(sx + dx, sy + dy));
                    }
                }
            }
            data[(y * w + x) as usize] = dil.saturating_sub(body(sx, sy));
        }
    }
    GlyphBitmap { left: g.left - pad, top: g.top - pad, w: w as u32, h: h as u32, data }
}

// ---------------------------------------------------------------------------
// Word 2010 text effects (WordArt)

/// A coverage mask in device pixels, origin at (`x0`, `y0`).
#[derive(Clone)]
struct Mask {
    x0: i32,
    y0: i32,
    w: i32,
    h: i32,
    a: Vec<f32>,
}

impl Mask {
    fn new(x0: i32, y0: i32, w: i32, h: i32) -> Mask {
        Mask { x0, y0, w, h, a: vec![0.0; (w.max(1) * h.max(1)) as usize] }
    }
    /// Coverage at device pixel (x, y).
    fn at(&self, x: i32, y: i32) -> f32 {
        let (lx, ly) = (x - self.x0, y - self.y0);
        if lx < 0 || ly < 0 || lx >= self.w || ly >= self.h { 0.0 } else { self.a[(ly * self.w + lx) as usize] }
    }
    fn add_glyph(&mut self, g: &GlyphBitmap, ox: i32, oy: i32, bold_off: f32, italic: bool) {
        let passes: &[f32] = if bold_off > 0.0 { &[0.0, 1.0] } else { &[0.0] };
        for pass in passes {
            let gx0 = ox + g.left + (pass * bold_off).round() as i32;
            for gy in 0..g.h {
                let rel_y = g.top + gy as i32;
                let shear = if italic { (-(rel_y as f32) * 0.2).round() as i32 } else { 0 };
                let ly = oy + rel_y - self.y0;
                if ly < 0 || ly >= self.h {
                    continue;
                }
                for gxp in 0..g.w {
                    let cov = g.data[(gy * g.w + gxp) as usize];
                    let lx = gx0 + gxp as i32 + shear - self.x0;
                    if cov > 0 && lx >= 0 && lx < self.w {
                        let i = (ly * self.w + lx) as usize;
                        self.a[i] = self.a[i].max(cov as f32 / 255.0);
                    }
                }
            }
        }
    }
    /// Separable max (dilate) or min (erode) filter with a square window of radius `r`.
    fn morph(&self, r: i32, max: bool) -> Mask {
        if r <= 0 {
            return self.clone();
        }
        let pick = |a: f32, b: f32| if max { a.max(b) } else { a.min(b) };
        // Outside the mask counts as empty for both, so erosion eats in from the edges.
        let outside = 0.0f32;
        let mut tmp = self.clone();
        for y in 0..self.h {
            for x in 0..self.w {
                let mut v = if max { 0.0f32 } else { 1.0f32 };
                for dx in -r..=r {
                    let xx = x + dx;
                    let s = if xx < 0 || xx >= self.w { outside } else { self.a[(y * self.w + xx) as usize] };
                    v = pick(v, s);
                }
                tmp.a[(y * self.w + x) as usize] = v;
            }
        }
        let mut out = tmp.clone();
        for y in 0..self.h {
            for x in 0..self.w {
                let mut v = if max { 0.0f32 } else { 1.0f32 };
                for dy in -r..=r {
                    let yy = y + dy;
                    let s = if yy < 0 || yy >= self.h { outside } else { tmp.a[(yy * self.w + x) as usize] };
                    v = pick(v, s);
                }
                out.a[(y * self.w + x) as usize] = v;
            }
        }
        out
    }
    /// Approximate Gaussian blur: two separable box-blur passes of radius `r`.
    fn blur(&self, r: i32) -> Mask {
        if r <= 0 {
            return self.clone();
        }
        let mut m = self.clone();
        let n = (2 * r + 1) as f32;
        for _ in 0..2 {
            let mut tmp = m.clone();
            for y in 0..m.h {
                let row = (y * m.w) as usize;
                let mut acc = 0.0;
                for x in -r..m.w + r {
                    if x + r < m.w && x + r >= 0 {
                        acc += m.a[row + (x + r) as usize];
                    }
                    if x - r > 0 && x - r - 1 < m.w {
                        acc -= m.a[row + (x - r - 1) as usize];
                    }
                    if x >= 0 && x < m.w {
                        tmp.a[row + x as usize] = acc / n;
                    }
                }
            }
            for x in 0..m.w {
                let mut acc = 0.0;
                for y in -r..m.h + r {
                    if y + r < m.h && y + r >= 0 {
                        acc += tmp.a[((y + r) * m.w + x) as usize];
                    }
                    if y - r > 0 && y - r - 1 < m.h {
                        acc -= tmp.a[((y - r - 1) * m.w + x) as usize];
                    }
                    if y >= 0 && y < m.h {
                        m.a[(y * m.w + x) as usize] = acc / n;
                    }
                }
            }
        }
        m
    }
}

/// Blend `f(x, y, coverage)` → (colour, opacity) over every covered pixel of `m`, shifted by (dx, dy).
fn paint_mask(r: &mut Raster, m: &Mask, dx: i32, dy: i32, f: impl Fn(i32, i32, f32) -> (Color, f32)) {
    for ly in 0..m.h {
        for lx in 0..m.w {
            let v = m.a[(ly * m.w + lx) as usize];
            if v > 0.003 {
                let (x, y) = (m.x0 + lx, m.y0 + ly);
                let (c, a) = f(x, y, v);
                r.blend(x + dx, y + dy, c, a * v);
            }
        }
    }
}

/// The direction (per pixel of depth) in which an extrusion shows for a `w14:scene3d` camera preset.
/// A straight-on camera hides the extrusion, as in the reference.
fn extrusion_dir(camera: Option<&str>) -> Option<(f32, f32)> {
    let cam = camera?;
    if cam == "orthographicFront" || cam == "perspectiveFront" {
        return None;
    }
    let dx = if cam.contains("Right") { 1.0 } else if cam.contains("Left") { -1.0 } else { 0.0 };
    let dy = if cam.contains("Bottom") || cam.contains("Below") { 1.0 } else if cam.contains("Top") || cam.contains("Above") { -1.0 } else { 0.0 };
    let (dx, dy) = if dx == 0.0 && dy == 0.0 { (0.55, 0.85) } else { (dx, dy) };
    let len = f32::hypot(dx, dy);
    Some((dx / len, dy / len))
}

/// Render a glyph run with Word 2010 text effects. `px` is the font size and `k` the scale, in device pixels.
#[allow(clippy::too_many_arguments)]
fn render_rich(r: &mut Raster, run: &[(i32, Arc<GlyphBitmap>)], iby: i32, bold_off: f32, italic: bool, color: Color, px: f32, k: f32, fx: &RichFx) {
    // Glyph bounds.
    let (mut gx0, mut gy0, mut gx1, mut gy1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for (ix, g) in run {
        let slant = if italic { (px * 0.25).ceil() as i32 } else { 0 };
        gx0 = gx0.min(ix + g.left - slant);
        gx1 = gx1.max(ix + g.left + g.w as i32 + bold_off.ceil() as i32 + slant);
        gy0 = gy0.min(iby + g.top);
        gy1 = gy1.max(iby + g.top + g.h as i32);
    }
    if gx0 >= gx1 || gy0 >= gy1 {
        return;
    }
    let text_h = (gy1 - gy0) as f32;
    let ext_dir = fx.extrusion.and_then(|_| extrusion_dir(fx.camera.as_deref()));
    let depth_px = match (fx.extrusion, ext_dir) {
        (Some((d, _)), Some(_)) => (d * k * 0.6).round().clamp(1.0, 300.0) as i32,
        _ => 0,
    };
    let mut margin = 2.0f32;
    if let Some((rad, ..)) = fx.glow {
        margin = margin.max(rad * k * 1.3);
    }
    if let Some(s) = &fx.shadow {
        margin = margin.max(s.dist * k + s.blur * k);
    }
    if let Some((w, _)) = &fx.outline {
        margin = margin.max(w * k);
    }
    margin = margin.max(depth_px as f32);
    let m_px = margin.ceil() as i32 + 2;
    let refl_extra = fx.reflection.as_ref().map(|rf| (text_h * rf.end_pos + rf.dist * k).ceil() as i32 + 2).unwrap_or(0);
    let mut mask = Mask::new(gx0 - m_px, gy0 - m_px, gx1 - gx0 + 2 * m_px, gy1 - gy0 + 2 * m_px + refl_extra);
    for (ix, g) in run {
        mask.add_glyph(g, *ix, iby, bold_off, italic);
    }

    // The fill paint, evaluated over the glyph bounds.
    let fill = fx.fill.clone().unwrap_or(Paint::Solid(color, 1.0));
    let (cx, cy) = ((gx0 + gx1) as f32 / 2.0, (gy0 + gy1) as f32 / 2.0);
    let (bw, bh) = ((gx1 - gx0) as f32, text_h);
    let eval = |p: &Paint, x: i32, y: i32| -> (Color, f32) {
        match p {
            Paint::Solid(c, a) => (*c, *a),
            Paint::Linear { angle, .. } => {
                let (sn, cs) = angle.to_radians().sin_cos();
                let extent = (bw * cs).abs() + (bh * sn).abs();
                let t = ((x as f32 + 0.5 - cx) * cs + (y as f32 + 0.5 - cy) * sn) / extent.max(1.0) + 0.5;
                p.at(t.clamp(0.0, 1.0))
            }
        }
    };
    let main = fill.at(0.5).0;
    // The stroke, centred on the glyph edge.
    let ring = fx.outline.as_ref().map(|(w, _)| {
        let w_px = (w * k).max(0.7);
        let rad = (w_px / 2.0).ceil().max(1.0) as i32;
        let d = mask.morph(rad, true);
        let e = mask.morph(rad, false);
        let mut ring = d;
        let strength = (w_px / (2 * rad) as f32).min(1.0);
        for (i, v) in ring.a.iter_mut().enumerate() {
            *v = (*v - e.a[i]).clamp(0.0, 1.0) * strength;
        }
        ring
    });
    // What casts glow, shadow and reflection: the letters, or only their stroke when hollow.
    let caster = match (&ring, fx.no_fill) {
        (Some(r), true) => r,
        _ => &mask,
    };

    if let Some((rad, c, a)) = fx.glow {
        let rr = (rad * k).max(1.0);
        let g = caster.morph((rr * 0.45) as i32, true).blur((rr * 0.5).ceil() as i32);
        paint_mask(r, &g, 0, 0, |_, _, _| (c, a));
    }
    if let Some(s) = &fx.shadow {
        let sm = caster.blur((s.blur * k * 0.5).round() as i32);
        let (sn, cs) = s.dir.to_radians().sin_cos();
        let (dx, dy) = ((s.dist * k * cs).round() as i32, (s.dist * k * sn).round() as i32);
        paint_mask(r, &sm, dx, dy, |_, _, _| (s.color, s.alpha));
    }
    if let Some(rf) = &fx.reflection {
        // Mirror the text below its bottom edge, fading out.
        let dist = (rf.dist * k).round() as i32;
        let rows = (text_h * rf.end_pos).ceil() as i32;
        for dyr in 0..rows {
            let y = gy1 + dist + dyr;
            let sy = gy1 - 1 - dyr;
            let f = dyr as f32 / rows.max(1) as f32;
            let alpha = rf.start_alpha + (rf.end_alpha - rf.start_alpha) * f;
            if alpha <= 0.003 {
                continue;
            }
            for x in mask.x0..mask.x0 + mask.w {
                let v = caster.at(x, sy);
                if v > 0.003 {
                    let (c, a) = if fx.no_fill { fx.outline.as_ref().map(|(_, p)| eval(p, x, sy)).unwrap_or((main, 1.0)) } else { eval(&fill, x, sy) };
                    r.blend(x, y, c, a * v * alpha);
                }
            }
        }
    }
    if let (Some((_, ecol)), Some((ux, uy))) = (fx.extrusion, ext_dir) {
        // Oblique extrusion: stack shaded copies from the back to the front.
        let side = ecol.unwrap_or_else(|| main.shade(0.55));
        for s in (1..=depth_px).rev() {
            let f = s as f32 / depth_px as f32;
            let c = mix(side, Color::BLACK, 0.35 * f);
            paint_mask(r, &mask, (ux * s as f32).round() as i32, (uy * s as f32).round() as i32, |_, _, _| (c, 1.0));
        }
    }
    if !fx.no_fill {
        paint_mask(r, &mask, 0, 0, |x, y, _| eval(&fill, x, y));
        if let Some((bev_w, _)) = fx.bevel {
            // Light from the top left: brighten edges facing it, darken the others.
            let bw_px = (bev_w * k).max(1.5);
            let o = (bw_px * 0.5).round().max(1.0) as i32;
            let b = mask.blur(o);
            for ly in 0..mask.h {
                for lx in 0..mask.w {
                    let v = mask.a[(ly * mask.w + lx) as usize];
                    if v <= 0.003 {
                        continue;
                    }
                    let (x, y) = (mask.x0 + lx, mask.y0 + ly);
                    let d = b.at(x + o, y + o) - b.at(x - o, y - o);
                    if d > 0.0 {
                        r.blend(x, y, Color::WHITE, (d * 1.3).min(1.0) * 0.65 * v);
                    } else if d < 0.0 {
                        r.blend(x, y, Color::BLACK, (-d * 1.3).min(1.0) * 0.45 * v);
                    }
                }
            }
        }
    }
    if let (Some(ring), Some((_, paint))) = (&ring, &fx.outline) {
        paint_mask(r, ring, 0, 0, |x, y, _| eval(paint, x, y));
    }
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color(m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
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
        if p[3] == 255 {
            // Opaque destination (every page): plain "over".
            p[0] = (p[0] as f32 * (1.0 - a) + c.0 as f32 * a) as u8;
            p[1] = (p[1] as f32 * (1.0 - a) + c.1 as f32 * a) as u8;
            p[2] = (p[2] as f32 * (1.0 - a) + c.2 as f32 * a) as u8;
        } else {
            // Transparent tiles (PDF export of text effects): full alpha compositing.
            let da = p[3] as f32 / 255.0;
            let oa = a + da * (1.0 - a);
            if oa <= 0.0 {
                return;
            }
            for (k, cc) in [c.0, c.1, c.2].into_iter().enumerate() {
                p[k] = ((cc as f32 * a + p[k] as f32 * da * (1.0 - a)) / oa).round().min(255.0) as u8;
            }
            p[3] = (oa * 255.0).round() as u8;
        }
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
        self.paint(&mut r, &page.items, scale);
        r
    }

    /// Draw `items` onto a fully transparent `width` x `height` pt raster (used by
    /// the PDF export for text with effects, so what's underneath still shows).
    pub fn render_transparent(&mut self, items: &[Item], width: f32, height: f32, scale: f32) -> Raster {
        let w = (width * scale).ceil().max(1.0) as u32;
        let h = (height * scale).ceil().max(1.0) as u32;
        let mut r = Raster { width: w, height: h, pixels: vec![0u8; (w * h * 4) as usize] };
        self.paint(&mut r, items, scale);
        r
    }

    fn paint(&mut self, r: &mut Raster, items: &[Item], scale: f32) {
        for it in items {
            match it {
                Item::Rect { x, y, w, h, color } => {
                    r.fill_rect(x * scale, y * scale, (x + w) * scale, (y + h) * scale, *color);
                }
                Item::Glyphs { font, size, color, y, glyphs, synth_bold, synth_italic, fx, .. } => {
                    let px = size * scale;
                    if px < 0.5 {
                        continue;
                    }
                    let iby = (y * scale).round() as i32;
                    let bold_off = if *synth_bold { (px / 28.0).max(0.6) } else { 0.0 };
                    let italic = *synth_italic;
                    // Rasterize the run once, then stamp it for each effect layer.
                    let mut run: Vec<(i32, Arc<GlyphBitmap>)> = Vec::with_capacity(glyphs.len());
                    for (gid, gx) in glyphs {
                        let fx_ = gx * scale;
                        let ix = fx_.floor();
                        let sub = ((fx_ - ix) * 4.0).floor() as u8;
                        if let Some(g) = self.glyph(font, *gid, px, sub.min(3)) {
                            run.push((ix as i32, g));
                        }
                    }
                    if let Some(rich) = &fx.rich {
                        render_rich(r, &run, iby, bold_off, italic, *color, px, scale, rich);
                        continue;
                    }
                    // Effect offset in device pixels: about 1/22 em, never under a pixel.
                    let d = (px / 26.0).round().max(1.0) as i32;
                    let stamp_all = |r: &mut Raster, run: &[(i32, Arc<GlyphBitmap>)], dx: i32, dy: i32, c: Color, a: f32| {
                        for (ix, g) in run {
                            stamp(r, g, ix + dx, iby + dy, bold_off, italic, c, a);
                        }
                    };
                    if fx.emboss || fx.imprint {
                        // Relief: a light fill with a dark edge below-right (raised) or above-left (pressed in).
                        let dark = if color.luminance() < 0.45 { Color(0x50, 0x50, 0x50) } else { mix(*color, Color::BLACK, 0.55) };
                        let fill = mix(*color, Color::WHITE, 0.72);
                        let s = if fx.emboss { 1 } else { -1 };
                        stamp_all(r, &run, s * d, s * d, dark, 0.6);
                        stamp_all(r, &run, -s * d, -s * d, Color::WHITE, 0.9);
                        stamp_all(r, &run, 0, 0, fill, 1.0);
                        continue;
                    }
                    if fx.shadow {
                        stamp_all(r, &run, d, d, Color(0x80, 0x80, 0x80), 0.75);
                    }
                    if fx.outline {
                        // Hollow letters: an outer stroke in the text colour, interior left open.
                        let rw = (px / 40.0).round().max(1.0) as i32;
                        if fx.shadow {
                            // Keep the shadow from showing through the hollow interior.
                            stamp_all(r, &run, 0, 0, Color::WHITE, 1.0);
                        }
                        for (ix, g) in &run {
                            let ring = ring(g, rw, bold_off.round() as i32);
                            stamp(r, &ring, *ix, iby, 0.0, italic, *color, 1.0);
                        }
                        continue;
                    }
                    stamp_all(r, &run, 0, 0, *color, 1.0);
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
                        let mut view = image::imageops::crop_imm(src.as_ref(), cx.min(src.width() - 1), cy.min(src.height() - 1), cw.min(src.width()), ch.min(src.height())).to_image();
                        // Resize with premultiplied alpha: otherwise the colour of fully transparent
                        // pixels (often black) bleeds into the edges as a dark fringe.
                        let has_alpha = view.pixels().any(|p| p[3] < 255);
                        if has_alpha {
                            for p in view.pixels_mut() {
                                let a = p[3] as u32;
                                for c in 0..3 {
                                    p[c] = ((p[c] as u32 * a + 127) / 255) as u8;
                                }
                            }
                        }
                        let mut s = image::imageops::resize(&view, dw, dh, image::imageops::FilterType::Triangle);
                        if has_alpha {
                            for p in s.pixels_mut() {
                                let a = p[3] as u32;
                                if a > 0 {
                                    for c in 0..3 {
                                        p[c] = ((p[c] as u32 * 255 + a / 2) / a).min(255) as u8;
                                    }
                                }
                            }
                        }
                        let s = Arc::new(s);
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
                        None => draw_placeholder(r, x * scale, y * scale, w * scale, h * scale),
                    }
                }
                Item::Placeholder { x, y, w, h, .. } => draw_placeholder(r, x * scale, y * scale, w * scale, h * scale),
            }
        }
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
