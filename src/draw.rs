//! Low-level drawing helpers shared by the styles: color math, alpha blending,
//! and simple filled primitives on an `RgbaImage`.

use image::{Rgba, RgbaImage};

/// Convert an opaque RGB triple into an `Rgba<u8>`.
pub fn rgb(c: [u8; 3]) -> Rgba<u8> {
    Rgba([c[0], c[1], c[2], 255])
}

/// Linearly interpolate two RGB colors.
pub fn lerp_rgb(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t).round() as u8,
        (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t).round() as u8,
        (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t).round() as u8,
    ]
}

/// Scale an RGB color's brightness by `f` (values above 1 lighten via clamping).
pub fn scale_rgb(c: [u8; 3], f: f32) -> [u8; 3] {
    [
        (c[0] as f32 * f).round().clamp(0.0, 255.0) as u8,
        (c[1] as f32 * f).round().clamp(0.0, 255.0) as u8,
        (c[2] as f32 * f).round().clamp(0.0, 255.0) as u8,
    ]
}

/// Alpha-blend `src` (with coverage `alpha` in `[0, 1]`) over the existing
/// pixel at `(x, y)`. Out-of-bounds coordinates are ignored.
pub fn blend_px(img: &mut RgbaImage, x: i64, y: i64, src: [u8; 3], alpha: f32) {
    if x < 0 || y < 0 || x as u32 >= img.width() || y as u32 >= img.height() {
        return;
    }
    let a = alpha.clamp(0.0, 1.0);
    if a <= 0.0 {
        return;
    }
    let px = img.get_pixel_mut(x as u32, y as u32);
    let inv = 1.0 - a;
    px.0[0] = (src[0] as f32 * a + px.0[0] as f32 * inv).round() as u8;
    px.0[1] = (src[1] as f32 * a + px.0[1] as f32 * inv).round() as u8;
    px.0[2] = (src[2] as f32 * a + px.0[2] as f32 * inv).round() as u8;
    px.0[3] = 255;
}

/// Fill a filled disc centered at `(cx, cy)` with radius `r`, alpha-blended.
pub fn fill_circle(img: &mut RgbaImage, cx: f32, cy: f32, r: f32, color: [u8; 3], alpha: f32) {
    if r <= 0.0 {
        return;
    }
    let r2 = r * r;
    let x0 = (cx - r).floor() as i64;
    let x1 = (cx + r).ceil() as i64;
    let y0 = (cy - r).floor() as i64;
    let y1 = (cy + r).ceil() as i64;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            if dx * dx + dy * dy <= r2 {
                blend_px(img, x, y, color, alpha);
            }
        }
    }
}

/// Fill an antialiased disc: edge pixels are blended by their coverage.
pub fn fill_disc_aa(img: &mut RgbaImage, cx: f32, cy: f32, r: f32, color: [u8; 3], alpha: f32) {
    if r <= 0.0 {
        return;
    }
    let x0 = (cx - r - 1.0).floor() as i64;
    let x1 = (cx + r + 1.0).ceil() as i64;
    let y0 = (cy - r - 1.0).floor() as i64;
    let y1 = (cy + r + 1.0).ceil() as i64;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let coverage = (r - (dx * dx + dy * dy).sqrt() + 0.5).clamp(0.0, 1.0);
            if coverage > 0.0 {
                blend_px(img, x, y, color, alpha * coverage);
            }
        }
    }
}

/// Fill an axis-aligned rectangle `[x0, x1) x [y0, y1)` with a solid color.
pub fn fill_rect(img: &mut RgbaImage, x0: i64, y0: i64, x1: i64, y1: i64, color: [u8; 3]) {
    let (w, h) = (img.width() as i64, img.height() as i64);
    let cx0 = x0.clamp(0, w);
    let cx1 = x1.clamp(0, w);
    let cy0 = y0.clamp(0, h);
    let cy1 = y1.clamp(0, h);
    let px = rgb(color);
    for y in cy0..cy1 {
        for x in cx0..cx1 {
            img.put_pixel(x as u32, y as u32, px);
        }
    }
}

/// Fill a filled triangle from its three vertices with a solid color.
pub fn fill_triangle(
    img: &mut RgbaImage,
    p0: (f32, f32),
    p1: (f32, f32),
    p2: (f32, f32),
    color: [u8; 3],
    alpha: f32,
) {
    let min_x = p0.0.min(p1.0).min(p2.0).floor() as i64;
    let max_x = p0.0.max(p1.0).max(p2.0).ceil() as i64;
    let min_y = p0.1.min(p1.1).min(p2.1).floor() as i64;
    let max_y = p0.1.max(p1.1).max(p2.1).ceil() as i64;
    let area = edge(p0, p1, p2);
    if area.abs() < 1e-6 {
        return;
    }
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let p = (x as f32 + 0.5, y as f32 + 0.5);
            let w0 = edge(p1, p2, p);
            let w1 = edge(p2, p0, p);
            let w2 = edge(p0, p1, p);
            // Inside if all edge functions share the winding sign of `area`.
            let inside = if area > 0.0 {
                w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0
            } else {
                w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0
            };
            if inside {
                blend_px(img, x, y, color, alpha);
            }
        }
    }
}

/// Signed area (twice) of the triangle `(a, b, c)`; the 2D cross product.
fn edge(a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> f32 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

/// Compute the variance of pixel luminance across the image, in `[0, ~]`.
/// Used by tests to assert an image is not a single flat color.
pub fn luma_variance(img: &RgbaImage) -> f64 {
    let n = (img.width() as u64 * img.height() as u64).max(1) as f64;
    let mut sum = 0.0f64;
    let mut sum_sq = 0.0f64;
    for p in img.pixels() {
        let l = 0.299 * p.0[0] as f64 + 0.587 * p.0[1] as f64 + 0.114 * p.0[2] as f64;
        sum += l;
        sum_sq += l * l;
    }
    let mean = sum / n;
    (sum_sq / n) - mean * mean
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_full_alpha_replaces() {
        let mut img = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 255]));
        blend_px(&mut img, 1, 1, [200, 100, 50], 1.0);
        assert_eq!(*img.get_pixel(1, 1), Rgba([200, 100, 50, 255]));
    }

    #[test]
    fn blend_half_alpha_mixes() {
        let mut img = RgbaImage::from_pixel(2, 2, Rgba([0, 0, 0, 255]));
        blend_px(&mut img, 0, 0, [100, 100, 100], 0.5);
        let p = img.get_pixel(0, 0).0;
        assert!((45..=55).contains(&p[0]));
    }

    #[test]
    fn blend_out_of_bounds_is_noop() {
        let mut img = RgbaImage::from_pixel(2, 2, Rgba([0, 0, 0, 255]));
        blend_px(&mut img, -1, 0, [255, 255, 255], 1.0);
        blend_px(&mut img, 5, 5, [255, 255, 255], 1.0);
        assert!(img.pixels().all(|p| *p == Rgba([0, 0, 0, 255])));
    }

    #[test]
    fn fill_rect_clips_to_bounds() {
        let mut img = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 255]));
        fill_rect(&mut img, -2, -2, 2, 2, [255, 0, 0]);
        assert_eq!(*img.get_pixel(0, 0), Rgba([255, 0, 0, 255]));
        assert_eq!(*img.get_pixel(3, 3), Rgba([0, 0, 0, 255]));
    }

    #[test]
    fn fill_circle_center_and_outside() {
        let mut img = RgbaImage::from_pixel(20, 20, Rgba([0, 0, 0, 255]));
        fill_circle(&mut img, 10.0, 10.0, 4.0, [255, 255, 255], 1.0);
        assert_eq!(*img.get_pixel(10, 10), Rgba([255, 255, 255, 255]));
        assert_eq!(*img.get_pixel(0, 0), Rgba([0, 0, 0, 255]));
    }

    #[test]
    fn fill_triangle_covers_centroid() {
        let mut img = RgbaImage::from_pixel(20, 20, Rgba([0, 0, 0, 255]));
        fill_triangle(
            &mut img,
            (2.0, 2.0),
            (18.0, 4.0),
            (6.0, 18.0),
            [0, 255, 0],
            1.0,
        );
        // The centroid is well inside the triangle.
        let cx = ((2.0 + 18.0 + 6.0) / 3.0) as u32;
        let cy = ((2.0 + 4.0 + 18.0) / 3.0) as u32;
        assert_eq!(img.get_pixel(cx, cy).0[1], 255);
    }

    #[test]
    fn variance_zero_for_flat_image() {
        let img = RgbaImage::from_pixel(8, 8, Rgba([120, 120, 120, 255]));
        assert!(luma_variance(&img) < 1e-9);
    }
}
