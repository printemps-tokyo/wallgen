//! `flow` — flow-field streamlines.
//!
//! A vector field is derived from self-generated fBm noise (the angle at each
//! point is `noise * TAU`). Many particles are dropped onto the field and
//! advected along it, leaving soft translucent trails colored from the palette.
//! The result is a layered, silky "flow" texture.

use crate::draw::{blend_px, fill_circle, lerp_rgb};
use crate::noise::ValueNoise;
use crate::palette::Palette;
use crate::styles::{style_rng, Style};
use crate::Size;
use image::RgbaImage;

const TAU: f32 = std::f32::consts::TAU;

pub fn render(size: Size, seed: u64, palette: &Palette) -> RgbaImage {
    let (w, h) = (size.width, size.height);
    let mut img = RgbaImage::new(w, h);
    let mut rng = style_rng(seed, Style::Flow);

    // Background: a soft diagonal gradient across the darker palette colors.
    let top = palette.colors[0];
    let bot = palette.sample(0.45);
    let denom = (w + h).max(1) as f32;
    for (x, y, px) in img.enumerate_pixels_mut() {
        let t = (x + y) as f32 / denom;
        let c = lerp_rgb(top, bot, t);
        *px = image::Rgba([c[0], c[1], c[2], 255]);
    }

    // Field parameters.
    let noise = ValueNoise::new(seed ^ 0x9E37_79B9);
    let scale = rng.range_f32(1.4, 2.6) / (w.min(h) as f32); // noise cells across image
    let curl = rng.range_f32(1.5, 2.8); // how many turns the field can take
    let step = (w.min(h) as f32 / 900.0).max(0.6); // advance per step in px

    // Counts scale with area but keep a healthy minimum so small images (tests)
    // still get plenty of variance.
    let area = w as u64 * h as u64;
    let particles = ((area / 1500) as u32).clamp(120, 60_000);
    let steps = ((w.min(h)) / 3).clamp(60, 900);
    let dot = (w.min(h) as f32 / 600.0).clamp(0.6, 3.0);

    for _ in 0..particles {
        let mut x = rng.range_f32(0.0, w as f32);
        let mut y = rng.range_f32(0.0, h as f32);
        // Color each streamline by a palette gradient position.
        let ct = rng.next_f32();
        let color = palette.sample(0.25 + ct * 0.75);
        let alpha = rng.range_f32(0.02, 0.06);

        for _ in 0..steps {
            if x < 0.0 || y < 0.0 || x >= w as f32 || y >= h as f32 {
                break;
            }
            let n = noise.fbm(x * scale, y * scale, 4);
            let angle = n * TAU * curl;
            x += angle.cos() * step;
            y += angle.sin() * step;
            if dot <= 0.75 {
                blend_px(&mut img, x as i64, y as i64, color, alpha * 3.0);
            } else {
                fill_circle(&mut img, x, y, dot, color, alpha);
            }
        }
    }

    img
}

#[cfg(test)]
mod tests {
    use crate::draw::luma_variance;
    use crate::palette::by_name;
    use crate::Size;

    #[test]
    fn produces_non_flat_image() {
        let img = super::render(Size::new(96, 96), 20260717, by_name("ocean").unwrap());
        assert_eq!((img.width(), img.height()), (96, 96));
        assert!(luma_variance(&img) > 1.0, "flow image looks flat");
    }
}
