//! Integration tests: end-to-end determinism, per-style validity, and encoding.
//!
//! SAFETY: these tests never touch the desktop wallpaper. The `--set` side
//! effect is covered only by unit tests of the pure helpers (see the library's
//! `set_supported_on` / `wallpaper_applescript` tests); `osascript` is never
//! executed here.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use image::GenericImageView;
use wallgen::{
    draw::luma_variance, generate, palette, parse_size, Config, Size, Style, DEFAULT_SIZE,
};

fn cfg(style: Style, seed: u64, size: Size) -> Config {
    Config {
        size,
        seed,
        style,
        palette: palette::pick(seed),
    }
}

fn hash_image(img: &image::RgbaImage) -> u64 {
    let mut h = DefaultHasher::new();
    img.as_raw().hash(&mut h);
    h.finish()
}

#[test]
fn same_seed_is_byte_identical() {
    let size = Size::new(128, 96);
    for style in Style::ALL {
        let a = generate(&cfg(*style, 20_260_717, size));
        let b = generate(&cfg(*style, 20_260_717, size));
        assert_eq!(
            a.as_raw(),
            b.as_raw(),
            "style {} not deterministic",
            style.name()
        );
    }
}

#[test]
fn different_seeds_differ() {
    let size = Size::new(128, 96);
    for style in Style::ALL {
        // Hold the palette fixed so we isolate the seed's effect on pixels.
        let pal = palette::by_name("ocean").unwrap();
        let a = generate(&Config {
            size,
            seed: 20_260_717,
            style: *style,
            palette: pal,
        });
        let b = generate(&Config {
            size,
            seed: 20_260_718,
            style: *style,
            palette: pal,
        });
        assert_ne!(
            hash_image(&a),
            hash_image(&b),
            "style {} produced identical output for different seeds",
            style.name()
        );
    }
}

#[test]
fn every_style_is_non_degenerate() {
    let size = Size::new(128, 96);
    for style in Style::ALL {
        let img = generate(&cfg(*style, 20_260_717, size));
        assert_eq!(img.dimensions(), (128, 96));
        assert!(
            luma_variance(&img) > 1.0,
            "style {} produced a flat image",
            style.name()
        );
    }
}

#[test]
fn all_styles_all_palettes_are_valid() {
    let size = Size::new(64, 64);
    for style in Style::ALL {
        for pal in palette::PALETTES {
            let img = generate(&Config {
                size,
                seed: 20_260_717,
                style: *style,
                palette: pal,
            });
            assert!(
                luma_variance(&img) > 0.5,
                "style {} / palette {} produced a flat image",
                style.name(),
                pal.name
            );
        }
    }
}

#[test]
fn renders_and_encodes_each_format() {
    let dir = tempdir();
    let size = Size::new(96, 64);
    let img = generate(&cfg(Style::Waves, 20_260_717, size));

    for ext in ["png", "webp"] {
        let path = dir.join(format!("out.{ext}"));
        img.save(&path).unwrap();
        let decoded = image::open(&path).unwrap();
        assert_eq!(decoded.dimensions(), (96, 64), "{ext} dimensions differ");
    }

    // JPEG path (drop alpha) via DynamicImage, mirroring the binary.
    let jpg = dir.join("out.jpg");
    image::DynamicImage::ImageRgba8(img.clone())
        .to_rgb8()
        .save(&jpg)
        .unwrap();
    let decoded = image::open(&jpg).unwrap();
    assert_eq!(decoded.dimensions(), (96, 64));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn png_bytes_are_reproducible_on_disk() {
    // Encoding to PNG twice yields identical files (byte-for-byte).
    let dir = tempdir();
    let size = Size::new(80, 80);
    let img = generate(&cfg(Style::Flow, 20_260_717, size));
    let p1 = dir.join("a.png");
    let p2 = dir.join("b.png");
    img.save(&p1).unwrap();
    img.save(&p2).unwrap();
    let a = std::fs::read(&p1).unwrap();
    let b = std::fs::read(&p2).unwrap();
    assert_eq!(a, b, "PNG encoding is not reproducible");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn size_presets_and_default() {
    assert_eq!(parse_size("4k").unwrap(), Size::new(3840, 2160));
    assert_eq!(parse_size("1080p").unwrap(), Size::new(1920, 1080));
    assert_eq!(DEFAULT_SIZE, Size::new(2560, 1440));
    // Reject an oversized request.
    assert!(parse_size("30000x30000").is_err());
}

fn tempdir() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let base = std::env::temp_dir();
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut dir = base.join(format!("wallgen-test-{}-{n}", std::process::id()));
    let mut extra = 0;
    while dir.exists() {
        extra += 1;
        dir = base.join(format!("wallgen-test-{}-{n}-{extra}", std::process::id()));
    }
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
