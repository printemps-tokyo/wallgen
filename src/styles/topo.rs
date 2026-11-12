//! `topo` — a topographic map.
//!
//! A self-generated fBm height field is cut into elevation bands, each filled
//! from the palette gradient, and every band edge is drawn as an antialiased
//! contour line of constant pixel width (the field's gradient converts the
//! distance to the nearest level into pixels). Every fifth contour is drawn
//! heavier, like the index contours on a survey map.

use crate::draw::{lerp_rgb, scale_rgb};
use crate::noise::ValueNoise;
use crate::palette::Palette;
use crate::styles::{style_rng, Style};
use crate::Size;
use image::{Rgba, RgbaImage};

pub fn render(size: Size, seed: u64, palette: &Palette) -> RgbaImage {
    let (w, h) = (size.width, size.height);
    let mut rng = style_rng(seed, Style::Topo);

    let noise = ValueNoise::new(seed ^ 0x7070_0000);
    let scale = rng.range_f32(2.5, 4.5) / (w.min(h) as f32);
    let levels = rng.range_u32(9, 15) as f32;
    let off_x = rng.range_f32(0.0, 64.0);
    let off_y = rng.range_f32(0.0, 64.0);
    let line_w = (w.min(h) as f32 / 900.0).clamp(0.6, 2.4);
    let line = scale_rgb(palette.base(), 0.55);

    let height = |x: f32, y: f32| noise.fbm(x * scale + off_x, y * scale + off_y, 5) * levels;

    let mut img = RgbaImage::new(w, h);
    for (px, py, pixel) in img.enumerate_pixels_mut() {
        let (x, y) = (px as f32 + 0.5, py as f32 + 0.5);
        let v = height(x, y);
        let gx = height(x + 1.0, y) - v;
        let gy = height(x, y + 1.0) - v;
        let grad = (gx * gx + gy * gy).sqrt().max(1e-4);

        let band = v.floor();
        let fill = palette.sample((band + 0.5) / levels);
        // Distance (in pixels) to the nearest contour level.
        let nearest = v.round();
        let dist = (v - nearest).abs() / grad;
        let width = if (nearest as i64).rem_euclid(5) == 0 {
            line_w * 1.8
        } else {
            line_w
        };
        let coverage = (width * 0.5 - dist + 0.5).clamp(0.0, 1.0) * 0.85;
        let c = lerp_rgb(fill, line, coverage);
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
        let img = super::render(Size::new(96, 96), 20260717, by_name("forest").unwrap());
        assert_eq!((img.width(), img.height()), (96, 96));
        assert!(luma_variance(&img) > 1.0, "topo image looks flat");
    }
}
