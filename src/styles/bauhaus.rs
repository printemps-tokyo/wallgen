//! `bauhaus` — a grid of bold geometric shapes.
//!
//! The canvas is divided into a grid of cells. Each cell gets one bold shape
//! drawn from a small vocabulary (full circle, half circle, quarter circle,
//! rectangle, triangle, diagonal split) in a palette color over a contrasting
//! cell background. Clean, primary, poster-like.

use crate::draw::{fill_rect, fill_triangle};
use crate::palette::Palette;
use crate::rng::Rng;
use crate::styles::{style_rng, Style};
use crate::Size;
use image::{Rgba, RgbaImage};

/// A pixel rectangle in image space, used to clip shapes to a cell (or half).
#[derive(Clone, Copy)]
struct Rect {
    x0: i64,
    y0: i64,
    x1: i64,
    y1: i64,
}

impl Rect {
    fn from_cell(x0: f32, y0: f32, cw: f32, ch: f32) -> Self {
        Rect {
            x0: x0.floor() as i64,
            y0: y0.floor() as i64,
            x1: (x0 + cw).ceil() as i64,
            y1: (y0 + ch).ceil() as i64,
        }
    }
}

pub fn render(size: Size, seed: u64, palette: &Palette) -> RgbaImage {
    let (w, h) = (size.width, size.height);
    let mut rng = style_rng(seed, Style::Bauhaus);

    // Overall background: the base palette color.
    let bg = palette.base();
    let mut img = RgbaImage::from_pixel(w, h, Rgba([bg[0], bg[1], bg[2], 255]));

    // Grid dimensions: aim for roughly square cells.
    let cols = rng.range_u32(3, 7).max(1);
    let aspect = h as f32 / w as f32;
    let rows = ((cols as f32 * aspect).round() as u32).clamp(2, 8);
    let cell_w = w as f32 / cols as f32;
    let cell_h = h as f32 / rows as f32;

    let accents = &palette.colors[1..];

    for gy in 0..rows {
        for gx in 0..cols {
            let x0 = gx as f32 * cell_w;
            let y0 = gy as f32 * cell_h;
            let cell = Rect::from_cell(x0, y0, cell_w, cell_h);

            // Optional cell background (a different palette tone) so shapes have
            // contrast; sometimes leave the global background showing.
            if rng.chance(0.6) {
                let c = palette.colors[rng.index(palette.colors.len())];
                fill_rect(&mut img, cell.x0, cell.y0, cell.x1, cell.y1, c);
            }

            let color = accents[rng.index(accents.len())];
            draw_shape(&mut img, &mut rng, x0, y0, cell_w, cell_h, cell, color);
        }
    }

    img
}

/// Fill the part of a disc that lies within `clip`, alpha-blended.
fn fill_disc_clipped(img: &mut RgbaImage, cx: f32, cy: f32, r: f32, clip: Rect, color: [u8; 3]) {
    if r <= 0.0 {
        return;
    }
    let r2 = r * r;
    let x0 = clip.x0.max((cx - r).floor() as i64).max(0);
    let x1 = clip.x1.min((cx + r).ceil() as i64).min(img.width() as i64);
    let y0 = clip.y0.max((cy - r).floor() as i64).max(0);
    let y1 = clip.y1.min((cy + r).ceil() as i64).min(img.height() as i64);
    let px = Rgba([color[0], color[1], color[2], 255]);
    for y in y0..y1 {
        for x in x0..x1 {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            if dx * dx + dy * dy <= r2 {
                img.put_pixel(x as u32, y as u32, px);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_shape(
    img: &mut RgbaImage,
    rng: &mut Rng,
    x0: f32,
    y0: f32,
    cw: f32,
    ch: f32,
    cell: Rect,
    color: [u8; 3],
) {
    let cx = x0 + cw * 0.5;
    let cy = y0 + ch * 0.5;
    let r = cw.min(ch) * 0.5 * rng.range_f32(0.72, 0.94);

    match rng.range_u32(0, 6) {
        0 => {
            // Full circle.
            fill_disc_clipped(img, cx, cy, r, cell, color);
        }
        1 => {
            // Half circle: a disc clipped to one half of the cell.
            let half = match rng.range_u32(0, 4) {
                0 => Rect {
                    y1: (y0 + ch * 0.5) as i64,
                    ..cell
                }, // top
                1 => Rect {
                    y0: (y0 + ch * 0.5) as i64,
                    ..cell
                }, // bottom
                2 => Rect {
                    x1: (x0 + cw * 0.5) as i64,
                    ..cell
                }, // left
                _ => Rect {
                    x0: (x0 + cw * 0.5) as i64,
                    ..cell
                }, // right
            };
            let rr = cw.min(ch) * 0.5 * rng.range_f32(0.85, 0.98);
            fill_disc_clipped(img, cx, cy, rr, half, color);
        }
        2 => {
            // Quarter circle: a disc centered on a cell corner, clipped to cell.
            let (ax, ay) = corner(rng, x0, y0, cw, ch);
            let rr = cw.min(ch) * rng.range_f32(0.82, 1.0);
            fill_disc_clipped(img, ax, ay, rr, cell, color);
        }
        3 => {
            // Centered rectangle / bar.
            let bw = cw * rng.range_f32(0.4, 0.85);
            let bh = ch * rng.range_f32(0.4, 0.85);
            fill_rect(
                img,
                (cx - bw * 0.5) as i64,
                (cy - bh * 0.5) as i64,
                (cx + bw * 0.5) as i64,
                (cy + bh * 0.5) as i64,
                color,
            );
        }
        4 => {
            // Triangle: either pointing up or down within the cell.
            let pad = cw.min(ch) * 0.08;
            if rng.chance(0.5) {
                fill_triangle(
                    img,
                    (x0 + pad, y0 + ch - pad),
                    (x0 + cw - pad, y0 + ch - pad),
                    (x0 + cw * 0.5, y0 + pad),
                    color,
                    1.0,
                );
            } else {
                fill_triangle(
                    img,
                    (x0 + pad, y0 + pad),
                    (x0 + cw - pad, y0 + pad),
                    (x0 + cw * 0.5, y0 + ch - pad),
                    color,
                    1.0,
                );
            }
        }
        _ => {
            // Diagonal split: a triangle covering half the cell.
            let corners = [(x0, y0), (x0 + cw, y0), (x0 + cw, y0 + ch), (x0, y0 + ch)];
            let k = rng.index(4);
            fill_triangle(
                img,
                corners[k],
                corners[(k + 1) % 4],
                corners[(k + 2) % 4],
                color,
                1.0,
            );
        }
    }
}

/// Pick one of the four cell corners.
fn corner(rng: &mut Rng, x0: f32, y0: f32, cw: f32, ch: f32) -> (f32, f32) {
    match rng.range_u32(0, 4) {
        0 => (x0, y0),
        1 => (x0 + cw, y0),
        2 => (x0 + cw, y0 + ch),
        _ => (x0, y0 + ch),
    }
}

#[cfg(test)]
mod tests {
    use crate::draw::luma_variance;
    use crate::palette::by_name;
    use crate::Size;

    #[test]
    fn produces_non_flat_image() {
        let img = super::render(Size::new(128, 96), 20260717, by_name("nordic").unwrap());
        assert_eq!((img.width(), img.height()), (128, 96));
        assert!(luma_variance(&img) > 1.0, "bauhaus image looks flat");
    }
}
