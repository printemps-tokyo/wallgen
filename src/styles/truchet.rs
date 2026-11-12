//! `truchet` — Truchet tiles.
//!
//! The canvas is a grid of square tiles. Each tile holds two quarter-circle
//! arcs joining the midpoints of its edges, in one of two orientations chosen
//! by the seed, so the arcs link up across tiles into long winding paths.
//! Arcs are antialiased and colored by a slow noise field over the palette.

use crate::draw::lerp_rgb;
use crate::noise::ValueNoise;
use crate::palette::Palette;
use crate::rng::mix;
use crate::styles::{style_rng, Style};
use crate::Size;
use image::{Rgba, RgbaImage};

pub fn render(size: Size, seed: u64, palette: &Palette) -> RgbaImage {
    let (w, h) = (size.width, size.height);
    let mut rng = style_rng(seed, Style::Truchet);

    let tile = (w.min(h) as f32 / rng.range_f32(7.0, 15.0)).max(4.0);
    let half_thick = tile * rng.range_f32(0.06, 0.14);
    let tile_seed = rng.next_u64();
    let noise = ValueNoise::new(seed ^ 0x7275_6368);
    let color_scale = rng.range_f32(1.0, 2.2) / (w.max(h) as f32);
    let bg = palette.base();

    let mut img = RgbaImage::new(w, h);
    for (px, py, pixel) in img.enumerate_pixels_mut() {
        let (x, y) = (px as f32 + 0.5, py as f32 + 0.5);
        let (tx, ty) = ((x / tile).floor(), (y / tile).floor());
        let (u, v) = (x - tx * tile, y - ty * tile);
        let flip = mix(tile_seed, ((tx as i64 as u64) << 32) ^ (ty as i64 as u64)) & 1 == 1;
        // Arc centers at two opposite corners, radius half a tile.
        let (c1, c2) = if flip {
            ((tile, 0.0), (0.0, tile))
        } else {
            ((0.0, 0.0), (tile, tile))
        };
        let r = tile * 0.5;
        let d1 = (((u - c1.0).powi(2) + (v - c1.1).powi(2)).sqrt() - r).abs();
        let d2 = (((u - c2.0).powi(2) + (v - c2.1).powi(2)).sqrt() - r).abs();
        let d = d1.min(d2);
        let coverage = (half_thick - d + 0.5).clamp(0.0, 1.0);

        let t = noise.fbm(x * color_scale, y * color_scale, 3);
        let arc = palette.sample(0.3 + t * 0.7);
        let c = lerp_rgb(bg, arc, coverage);
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
        let img = super::render(Size::new(96, 96), 20260717, by_name("nordic").unwrap());
        assert_eq!((img.width(), img.height()), (96, 96));
        assert!(luma_variance(&img) > 1.0, "truchet image looks flat");
    }
}
