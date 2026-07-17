//! `waves` — interference of sine wave fields.
//!
//! Several plane waves with random direction, frequency, and phase are summed.
//! The combined field is mapped through the palette gradient, producing smooth
//! moire/interference bands reminiscent of rippling silk or water caustics.

use crate::palette::Palette;
use crate::styles::{style_rng, Style};
use crate::Size;
use image::{Rgba, RgbaImage};

const TAU: f32 = std::f32::consts::TAU;

struct Wave {
    // Unit direction of propagation.
    dx: f32,
    dy: f32,
    freq: f32,
    phase: f32,
    amp: f32,
}

pub fn render(size: Size, seed: u64, palette: &Palette) -> RgbaImage {
    let (w, h) = (size.width, size.height);
    let mut rng = style_rng(seed, Style::Waves);
    let diag = ((w * w + h * h) as f32).sqrt().max(1.0);

    let n_waves = rng.range_u32(4, 7) as usize;
    let mut total_amp = 0.0f32;
    let waves: Vec<Wave> = (0..n_waves)
        .map(|_| {
            let ang = rng.range_f32(0.0, TAU);
            // Frequency in cycles across the diagonal.
            let cycles = rng.range_f32(2.0, 9.0);
            let freq = cycles * TAU / diag;
            let amp = rng.range_f32(0.5, 1.0);
            total_amp += amp;
            Wave {
                dx: ang.cos(),
                dy: ang.sin(),
                freq,
                phase: rng.range_f32(0.0, TAU),
                amp,
            }
        })
        .collect();

    // A slow warp so bands are not perfectly straight.
    let warp_ang = rng.range_f32(0.0, TAU);
    let warp_freq = rng.range_f32(1.0, 2.5) * TAU / diag;
    let warp_amp = diag * rng.range_f32(0.02, 0.06);

    let mut img = RgbaImage::new(w, h);
    for (px, py, pixel) in img.enumerate_pixels_mut() {
        let fx = px as f32;
        let fy = py as f32;
        // Domain warp.
        let warp = (fx * warp_ang.cos() + fy * warp_ang.sin()) * warp_freq;
        let wx = fx + warp.sin() * warp_amp;
        let wy = fy + warp.cos() * warp_amp;

        let mut acc = 0.0f32;
        for wv in &waves {
            let proj = wx * wv.dx + wy * wv.dy;
            acc += wv.amp * (proj * wv.freq + wv.phase).sin();
        }
        // Normalize the sum from [-total_amp, total_amp] to [0, 1].
        let t = (acc / total_amp) * 0.5 + 0.5;
        let c = palette.sample(t.clamp(0.0, 1.0));
        *pixel = Rgba([c[0], c[1], c[2], 255]);
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
        let img = super::render(Size::new(96, 96), 20260717, by_name("neon").unwrap());
        assert_eq!((img.width(), img.height()), (96, 96));
        assert!(luma_variance(&img) > 1.0, "waves image looks flat");
    }
}
