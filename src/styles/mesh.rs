//! `mesh` — a soft mesh gradient.
//!
//! A handful of colored control points float over the canvas. Each pixel is a
//! blend of their colors weighted by inverse distance, sampled through a gentle
//! noise warp so the blobs melt into each other. A tiny seed-derived dither
//! keeps the smooth gradient free of banding once quantized to 8 bits.

use crate::noise::ValueNoise;
use crate::palette::Palette;
use crate::rng::mix;
use crate::styles::{style_rng, Style};
use crate::Size;
use image::{Rgba, RgbaImage};

struct Point {
    x: f32,
    y: f32,
    radius: f32,
    color: [f32; 3],
}

pub fn render(size: Size, seed: u64, palette: &Palette) -> RgbaImage {
    let (w, h) = (size.width, size.height);
    let mut rng = style_rng(seed, Style::Mesh);
    let diag = ((w as f32).powi(2) + (h as f32).powi(2)).sqrt().max(1.0);

    let n = rng.range_u32(5, 9) as usize;
    let points: Vec<Point> = (0..n)
        .map(|i| {
            // The first point uses the base color so every palette keeps its mood.
            let c = if i == 0 {
                palette.base()
            } else {
                palette.sample(rng.range_f32(0.2, 1.0))
            };
            Point {
                x: rng.range_f32(-0.1, 1.1) * w as f32,
                y: rng.range_f32(-0.1, 1.1) * h as f32,
                radius: diag * rng.range_f32(0.6, 1.4),
                color: [c[0] as f32, c[1] as f32, c[2] as f32],
            }
        })
        .collect();

    let noise = ValueNoise::new(seed ^ 0x6D65_7368);
    let warp_scale = rng.range_f32(1.0, 2.0) / diag;
    let warp_amp = diag * rng.range_f32(0.03, 0.08);
    let dither_seed = rng.next_u64();

    let mut img = RgbaImage::new(w, h);
    for (px, py, pixel) in img.enumerate_pixels_mut() {
        let (fx, fy) = (px as f32, py as f32);
        let wx = fx + (noise.sample(fx * warp_scale, fy * warp_scale) - 0.5) * 2.0 * warp_amp;
        let wy =
            fy + (noise.sample(fy * warp_scale + 17.0, fx * warp_scale) - 0.5) * 2.0 * warp_amp;

        let mut acc = [0.0f32; 3];
        let mut total = 0.0f32;
        for p in &points {
            // Shepard (inverse-distance) weights: each point owns its
            // neighborhood and hands over smoothly to the next.
            let d2 = ((wx - p.x).powi(2) + (wy - p.y).powi(2)) / (p.radius * p.radius);
            let weight = 1.0 / (d2 * d2.sqrt() + 1e-4);
            total += weight;
            for (a, c) in acc.iter_mut().zip(p.color) {
                *a += c * weight;
            }
        }
        let hash = mix(dither_seed, ((px as u64) << 32) | py as u64);
        let dither = (hash >> 40) as f32 / (1u64 << 24) as f32 - 0.5;
        let ch = |i: usize| (acc[i] / total + dither).round().clamp(0.0, 255.0) as u8;
        *pixel = Rgba([ch(0), ch(1), ch(2), 255]);
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
        let img = super::render(Size::new(96, 96), 20260717, by_name("aurora").unwrap());
        assert_eq!((img.width(), img.height()), (96, 96));
        assert!(luma_variance(&img) > 1.0, "mesh image looks flat");
    }
}
