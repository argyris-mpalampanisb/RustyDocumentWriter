//! The built-in 3D shape library.
//!
//! The shapes are rendered offline with Blender (`tools/shapes/make_shapes.py`, then
//! `tools/shapes/finish.py`) in neutral grey on a transparent background, with a soft
//! contact shadow, and embedded in the binary. [`tinted_png`] recolours one to any
//! colour, and the result is inserted as an ordinary picture, so documents with
//! shapes open anywhere.

use crate::props::Color;

pub struct Shape {
    pub id: &'static str,
    pub name: &'static str,
    pub png: &'static [u8],
}

pub const SHAPES: &[Shape] = &[
    Shape { id: "cube", name: "Cube", png: include_bytes!("../assets/shapes/cube.png") },
    Shape { id: "sphere", name: "Sphere", png: include_bytes!("../assets/shapes/sphere.png") },
    Shape { id: "cylinder", name: "Cylinder", png: include_bytes!("../assets/shapes/cylinder.png") },
    Shape { id: "cone", name: "Cone", png: include_bytes!("../assets/shapes/cone.png") },
    Shape { id: "pyramid", name: "Pyramid", png: include_bytes!("../assets/shapes/pyramid.png") },
    Shape { id: "torus", name: "Torus", png: include_bytes!("../assets/shapes/torus.png") },
    Shape { id: "capsule", name: "Capsule", png: include_bytes!("../assets/shapes/capsule.png") },
    Shape { id: "hexagon", name: "Hexagon", png: include_bytes!("../assets/shapes/hexagon.png") },
    Shape { id: "gem", name: "Gem", png: include_bytes!("../assets/shapes/gem.png") },
    Shape { id: "ring", name: "Ring", png: include_bytes!("../assets/shapes/ring.png") },
    Shape { id: "star", name: "Star", png: include_bytes!("../assets/shapes/star.png") },
    Shape { id: "heart", name: "Heart", png: include_bytes!("../assets/shapes/heart.png") },
    Shape { id: "arrow", name: "Arrow", png: include_bytes!("../assets/shapes/arrow.png") },
    Shape { id: "lightning", name: "Lightning", png: include_bytes!("../assets/shapes/lightning.png") },
    Shape { id: "plus", name: "Plus", png: include_bytes!("../assets/shapes/plus.png") },
    Shape { id: "check", name: "Check Mark", png: include_bytes!("../assets/shapes/check.png") },
    Shape { id: "bubble", name: "Speech Bubble", png: include_bytes!("../assets/shapes/bubble.png") },
    Shape { id: "crown", name: "Crown", png: include_bytes!("../assets/shapes/crown.png") },
];

/// The renders' median grey is about 0.59; this brings it to 0.5, where the tint is exact.
const EXPOSURE: f32 = 0.85;

/// Recolour grey RGBA pixels in place: dark greys go toward black, mid-grey becomes
/// `color` and highlights go toward white, keeping the rendered shading and specular
/// highlights. The shadow (translucent black) stays black.
pub fn tint_rgba(pixels: &mut [u8], color: Color) {
    let c = [color.0 as f32, color.1 as f32, color.2 as f32];
    for px in pixels.as_chunks_mut::<4>().0 {
        let t = (px[0] as f32 / 255.0 * EXPOSURE).min(1.0);
        for k in 0..3 {
            let v = if t < 0.5 { c[k] * 2.0 * t } else { c[k] + (255.0 - c[k]) * (2.0 * t - 1.0) };
            px[k] = v.round().clamp(0.0, 255.0) as u8;
        }
    }
}

/// The shape as PNG bytes, recoloured to `color` (or the neutral render for `None`).
pub fn tinted_png(shape: &Shape, color: Option<Color>) -> Vec<u8> {
    let Some(color) = color else { return shape.png.to_vec() };
    let Ok(img) = image::load_from_memory(shape.png) else { return shape.png.to_vec() };
    let mut rgba = img.to_rgba8();
    tint_rgba(&mut rgba, color);
    let mut out = Vec::new();
    match image::DynamicImage::ImageRgba8(rgba).write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png) {
        Ok(()) => out,
        Err(_) => shape.png.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shape_decodes_and_tints() {
        for s in SHAPES {
            let img = image::load_from_memory(s.png).unwrap_or_else(|e| panic!("{}: {e}", s.id));
            assert!(img.width() > 64 && img.height() > 64, "{} too small", s.id);
            let red = image::load_from_memory(&tinted_png(s, Some(Color(0xC0, 0, 0)))).unwrap().to_rgba8();
            // Mid-tones of the shape body must come out red, not grey.
            let body: Vec<_> = red.pixels().filter(|p| p[3] == 255).collect();
            let reddish = body.iter().filter(|p| p[0] as i32 > p[1] as i32 + 40).count();
            assert!(reddish * 2 > body.len(), "{} didn't tint", s.id);
        }
    }

    #[test]
    fn mid_grey_maps_to_the_colour() {
        let g = (0.5 / EXPOSURE * 255.0).round() as u8;
        let mut px = [g, g, g, 255];
        tint_rgba(&mut px, Color(0x15, 0x60, 0x82));
        assert!((px[0] as i32 - 0x15).abs() <= 2 && (px[1] as i32 - 0x60).abs() <= 2 && (px[2] as i32 - 0x82).abs() <= 2, "{px:?}");
    }
}
