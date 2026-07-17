//! `voronoi` — colored Voronoi cells.
//!
//! A set of sites is scattered across the canvas, each assigned a palette color.
//! Every pixel takes the color of its nearest site (nearest-site search is
//! implemented here by hand). Cells are softly shaded by distance to their site
//! and given a thin darker seam where two cells meet, for a stained-glass look.

use crate::draw::{luma_variance, scale_rgb};
use crate::palette::Palette;
use crate::styles::{style_rng, Style};
use crate::Size;
use image::{Rgba, RgbaImage};

struct Site {
    x: f32,
    y: f32,
    color: [u8; 3],
}

pub fn render(size: Size, seed: u64, palette: &Palette) -> RgbaImage {
    let (w, h) = (size.width, size.height);
    let mut rng = style_rng(seed, Style::Voronoi);

    // Site count scales gently with area.
    let area = w as u64 * h as u64;
    let n_sites = ((area / 90_000) as u32).clamp(14, 90) as usize;

    let sites: Vec<Site> = (0..n_sites)
        .map(|_| {
            let x = rng.range_f32(0.0, w as f32);
            let y = rng.range_f32(0.0, h as f32);
            // Bias color selection toward the brighter accents.
            let t = rng.next_f32().powf(0.7);
            let base = palette.sample(0.2 + t * 0.8);
            // Slight per-site brightness jitter so equal colors still separate.
            let color = scale_rgb(base, rng.range_f32(0.85, 1.12));
            Site { x, y, color }
        })
        .collect();

    let mut img = RgbaImage::new(w, h);
    let max_d = ((w * w + h * h) as f32).sqrt();

    for (px, py, pixel) in img.enumerate_pixels_mut() {
        let (fx, fy) = (px as f32 + 0.5, py as f32 + 0.5);
        // Track the nearest and second-nearest site distances.
        let mut best = f32::INFINITY;
        let mut second = f32::INFINITY;
        let mut best_color = [0u8, 0, 0];
        for s in &sites {
            let dx = fx - s.x;
            let dy = fy - s.y;
            let d = dx * dx + dy * dy;
            if d < best {
                second = best;
                best = d;
                best_color = s.color;
            } else if d < second {
                second = d;
            }
        }
        let d1 = best.sqrt();
        let d2 = second.sqrt();

        // Radial shading inside the cell (darker away from the site).
        let shade = 1.0 - 0.35 * (d1 / (max_d * 0.18)).min(1.0);
        let mut color = scale_rgb(best_color, shade);

        // Thin darker seam where the two nearest sites are almost equidistant.
        let seam = (d2 - d1).max(0.0);
        if seam < 1.6 {
            let k = 0.45 + 0.55 * (seam / 1.6);
            color = scale_rgb(color, k);
        }

        *pixel = Rgba([color[0], color[1], color[2], 255]);
    }

    // Guard against a degenerate flat result (e.g. all sites collapsing): if the
    // variance is negligible, nudge with a faint gradient. In practice never
    // triggers, but keeps the "never flat" contract even for odd seeds.
    if luma_variance(&img) < 0.5 {
        for (x, _y, px) in img.enumerate_pixels_mut() {
            let t = x as f32 / w.max(1) as f32;
            let c = scale_rgb(palette.sample(t), 1.0);
            *px = Rgba([c[0], c[1], c[2], 255]);
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
        let img = super::render(Size::new(96, 96), 20260717, by_name("sunset").unwrap());
        assert_eq!((img.width(), img.height()), (96, 96));
        assert!(luma_variance(&img) > 1.0, "voronoi image looks flat");
    }
}
