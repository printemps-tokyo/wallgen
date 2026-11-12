//! `circles` — circle packing.
//!
//! Candidate centers are drawn from the seed; each grows until it would touch
//! an existing circle (or reaches the maximum radius) and is kept only if it
//! is at least the minimum radius. A uniform grid makes the neighbor search
//! cheap. Circles are antialiased, colored from the palette accents, and some
//! carry a concentric inner disc.

use crate::draw::{fill_disc_aa, lerp_rgb};
use crate::palette::Palette;
use crate::rng::Rng;
use crate::styles::{style_rng, Style};
use crate::Size;
use image::{Rgba, RgbaImage};

struct Circle {
    x: f32,
    y: f32,
    r: f32,
}

pub fn render(size: Size, seed: u64, palette: &Palette) -> RgbaImage {
    let (w, h) = (size.width, size.height);
    let mut rng = style_rng(seed, Style::Circles);
    let short = w.min(h) as f32;

    let max_r = short * rng.range_f32(0.10, 0.18);
    let min_r = (short * 0.006).max(1.5);
    let gap = (short * 0.003).max(0.75);

    // Background: a quiet vertical gradient between the two darkest colors.
    let top = palette.base();
    let bottom = palette.sample(0.2);
    let mut img = RgbaImage::new(w, h);
    for (_, y, px) in img.enumerate_pixels_mut() {
        let c = lerp_rgb(top, bottom, y as f32 / h.max(1) as f32);
        *px = Rgba([c[0], c[1], c[2], 255]);
    }

    let circles = pack(&mut rng, w as f32, h as f32, max_r, min_r, gap);

    for c in &circles {
        let color = palette.sample(rng.range_f32(0.35, 1.0));
        fill_disc_aa(&mut img, c.x, c.y, c.r, color, 0.95);
        if c.r > min_r * 4.0 && rng.chance(0.45) {
            let inner = palette.sample(rng.range_f32(0.0, 1.0));
            fill_disc_aa(
                &mut img,
                c.x,
                c.y,
                c.r * rng.range_f32(0.35, 0.7),
                inner,
                0.9,
            );
        }
    }
    img
}

/// Pack circles of radius `min_r..=max_r` into a `w` x `h` canvas, keeping at
/// least `gap` between neighbors.
fn pack(rng: &mut Rng, w: f32, h: f32, max_r: f32, min_r: f32, gap: f32) -> Vec<Circle> {
    // Grid with cells as wide as the largest circle's diameter: a circle that
    // can constrain a candidate lies in the 5x5 block of cells around it.
    let cell = (max_r * 2.0).max(1.0);
    let cols = (w / cell).ceil() as usize + 1;
    let rows = (h / cell).ceil() as usize + 1;
    let mut grid: Vec<Vec<usize>> = vec![Vec::new(); cols * rows];
    let mut circles: Vec<Circle> = Vec::new();

    let attempts = ((w as f64 * h as f64) / (min_r as f64 * min_r as f64) * 0.35)
        .clamp(500.0, 400_000.0) as u32;
    for _ in 0..attempts {
        let x = rng.range_f32(0.0, w);
        let y = rng.range_f32(0.0, h);
        let (cx, cy) = ((x / cell) as usize, (y / cell) as usize);
        let mut r = max_r;
        'search: for gy in cy.saturating_sub(2)..=(cy + 2).min(rows - 1) {
            for gx in cx.saturating_sub(2)..=(cx + 2).min(cols - 1) {
                for &i in &grid[gy * cols + gx] {
                    let c = &circles[i];
                    let room = ((x - c.x).powi(2) + (y - c.y).powi(2)).sqrt() - c.r - gap;
                    r = r.min(room);
                    if r < min_r {
                        break 'search;
                    }
                }
            }
        }
        if r < min_r {
            continue;
        }
        grid[cy * cols + cx].push(circles.len());
        circles.push(Circle { x, y, r });
    }
    circles
}

#[cfg(test)]
mod tests {
    use crate::draw::luma_variance;
    use crate::palette::by_name;
    use crate::Size;

    #[test]
    fn produces_non_flat_image() {
        let img = super::render(Size::new(96, 96), 20260717, by_name("pastel").unwrap());
        assert_eq!((img.width(), img.height()), (96, 96));
        assert!(luma_variance(&img) > 1.0, "circles image looks flat");
    }

    #[test]
    fn packed_circles_keep_their_gap() {
        let mut rng = crate::rng::Rng::new(7);
        let circles = super::pack(&mut rng, 320.0, 180.0, 30.0, 2.0, 1.0);
        assert!(circles.len() > 20, "only {} circles", circles.len());
        for (i, a) in circles.iter().enumerate() {
            assert!((2.0..=30.0).contains(&a.r));
            for b in &circles[i + 1..] {
                let d = ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt();
                assert!(
                    d >= a.r + b.r + 1.0 - 1e-3,
                    "overlap: {d} < {} + {}",
                    a.r,
                    b.r
                );
            }
        }
    }
}
