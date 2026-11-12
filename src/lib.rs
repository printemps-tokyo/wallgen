//! wallgen — deterministic generative-art desktop wallpapers.
//!
//! wallgen renders a wallpaper from a single 64-bit seed. The default seed is
//! today's date as an integer (`YYYYMMDD`), so everyone who runs it on the same
//! day gets the byte-identical wallpaper worldwide. All randomness flows from
//! the seed through a self-implemented RNG ([`rng`]) and noise ([`noise`]) — no
//! system time and no OS randomness ever touch the pixel path.
//!
//! The pure generation logic lives here in the library: [`generate`] takes a
//! fully-resolved [`Config`] and returns an [`image::RgbaImage`]. File writing,
//! argument parsing, and the macOS `--set` side effect live in the binary.

pub mod desktop;
pub mod draw;
pub mod noise;
pub mod palette;
pub mod rng;
pub mod styles;

use anyhow::{anyhow, Result};
use image::RgbaImage;

pub use palette::Palette;
pub use styles::Style;

/// A parsed image size in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    /// Construct a size (no validation).
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// Upper bound on a single dimension, to reject absurd inputs early.
pub const MAX_DIM: u32 = 20_000;

/// Upper bound on total pixels, to avoid huge single allocations. An RGBA buffer
/// is 4 bytes per pixel, so 100 MP is ~400 MB. This still allows 8K wallpapers
/// (7680x4320 ~= 33 MP).
pub const MAX_PIXELS: u64 = 100_000_000;

/// The default wallpaper size when `--size` is not given (2560x1440, QHD).
pub const DEFAULT_SIZE: Size = Size {
    width: 2560,
    height: 1440,
};

/// A fully-resolved generation request. Everything here is explicit and
/// deterministic; [`generate`] performs no I/O and reads no clock.
#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub size: Size,
    pub seed: u64,
    pub style: Style,
    pub palette: &'static Palette,
}

/// Render a wallpaper for a fully-resolved config. Deterministic: the same
/// `(size, seed, style, palette)` always produces the identical buffer.
pub fn generate(cfg: &Config) -> RgbaImage {
    cfg.style.render(cfg.size, cfg.seed, cfg.palette)
}

/// Resolve a named size preset (case-insensitive) to pixels, or `None`.
pub fn size_preset(name: &str) -> Option<Size> {
    let key = name.trim().to_ascii_lowercase();
    let s = |w, h| Some(Size::new(w, h));
    match key.as_str() {
        "720p" | "hd" => s(1280, 720),
        "1080p" | "fhd" | "1080" => s(1920, 1080),
        "wuxga" => s(1920, 1200),
        "1440p" | "2k" | "qhd" | "1440" => s(2560, 1440),
        "wqxga" | "1600p" => s(2560, 1600),
        "4k" | "2160p" | "uhd" => s(3840, 2160),
        "5k" => s(5120, 2880),
        "8k" | "4320p" => s(7680, 4320),
        _ => None,
    }
}

/// The list of preset names, for `--help` and docs.
pub fn size_preset_names() -> &'static [&'static str] {
    &["720p", "1080p", "wuxga", "1440p", "wqxga", "4k", "5k", "8k"]
}

/// Parse a size argument: a named preset (e.g. `"4k"`) or `"WxH"` (e.g.
/// `"5120x2880"`). Applies the [`MAX_DIM`] and [`MAX_PIXELS`] guards.
pub fn parse_size(input: &str) -> Result<Size> {
    let raw = input.trim();
    if let Some(preset) = size_preset(raw) {
        return Ok(preset);
    }
    let s = raw.to_ascii_lowercase();
    let (w, h) = s
        .split_once('x')
        .ok_or_else(|| anyhow!("invalid size: {input:?} (expected WxH or a preset like 4k)"))?;
    let width: u32 = w
        .trim()
        .parse()
        .map_err(|_| anyhow!("invalid size: {input:?} (expected WxH or a preset like 4k)"))?;
    let height: u32 = h
        .trim()
        .parse()
        .map_err(|_| anyhow!("invalid size: {input:?} (expected WxH or a preset like 4k)"))?;
    if width == 0 || height == 0 {
        return Err(anyhow!("size must be positive: {input:?}"));
    }
    if width > MAX_DIM || height > MAX_DIM {
        return Err(anyhow!(
            "size too large (max {MAX_DIM} per side): {input:?}"
        ));
    }
    if width as u64 * height as u64 > MAX_PIXELS {
        return Err(anyhow!(
            "size too large (max {MAX_PIXELS} total pixels): {input:?}"
        ));
    }
    Ok(Size { width, height })
}

/// Compute the integer seed `YYYYMMDD` for a calendar date. Validates ranges.
pub fn date_to_seed(year: i64, month: u32, day: u32) -> Result<u64> {
    if !(1..=12).contains(&month) {
        return Err(anyhow!("invalid month: {month} (expected 1..=12)"));
    }
    if !(1..=31).contains(&day) {
        return Err(anyhow!("invalid day: {day} (expected 1..=31)"));
    }
    if !(0..=9999).contains(&year) {
        return Err(anyhow!("invalid year: {year} (expected 0..=9999)"));
    }
    Ok(year as u64 * 10_000 + month as u64 * 100 + day as u64)
}

/// Parse a `YYYY-MM-DD` date string into its `YYYYMMDD` seed.
pub fn parse_date_seed(input: &str) -> Result<u64> {
    let s = input.trim();
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return Err(anyhow!("invalid date: {input:?} (expected YYYY-MM-DD)"));
    }
    let year: i64 = parts[0]
        .parse()
        .map_err(|_| anyhow!("invalid date: {input:?} (expected YYYY-MM-DD)"))?;
    let month: u32 = parts[1]
        .parse()
        .map_err(|_| anyhow!("invalid date: {input:?} (expected YYYY-MM-DD)"))?;
    let day: u32 = parts[2]
        .parse()
        .map_err(|_| anyhow!("invalid date: {input:?} (expected YYYY-MM-DD)"))?;
    date_to_seed(year, month, day)
}

/// Convert a Unix day count (days since 1970-01-01) to a `(year, month, day)`
/// civil date, using Howard Hinnant's `civil_from_days` algorithm. Pure and
/// testable; the binary feeds it the current day derived from `SystemTime`.
pub fn civil_from_unix_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Build the default output file name: `wallgen-YYYYMMDD-<style>.<ext>`.
pub fn default_filename(seed: u64, style: Style, ext: &str) -> String {
    format!("wallgen-{seed:08}-{}.{ext}", style.name())
}

/// Whether `--set` can work on the given OS string (as from
/// [`std::env::consts::OS`]): macOS, Windows, and Linux or BSD desktops that
/// [`desktop::detect`] recognizes. Split out so the guard is unit-testable
/// without executing anything.
pub fn set_supported_on(os: &str) -> bool {
    matches!(
        os,
        "macos" | "windows" | "linux" | "freebsd" | "openbsd" | "netbsd" | "dragonfly"
    )
}

/// Build the AppleScript that sets `path` as the wallpaper of every desktop.
/// The path is escaped for an AppleScript string literal. This only builds the
/// script text; running it is the binary's job and only ever behind `--set`.
pub fn wallpaper_applescript(path: &str) -> String {
    let escaped = path.replace('\\', "\\\\").replace('"', "\\\"");
    format!("tell application \"System Events\" to set picture of every desktop to \"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_size_forms_and_presets() {
        assert_eq!(parse_size("5120x2880").unwrap(), Size::new(5120, 2880));
        assert_eq!(parse_size("4k").unwrap(), Size::new(3840, 2160));
        assert_eq!(parse_size("4K").unwrap(), Size::new(3840, 2160));
        assert_eq!(parse_size("1440p").unwrap(), Size::new(2560, 1440));
        assert_eq!(parse_size("1080p").unwrap(), Size::new(1920, 1080));
        assert_eq!(parse_size("8k").unwrap(), Size::new(7680, 4320));
        assert!(parse_size("0x10").is_err());
        assert!(parse_size("axb").is_err());
        assert!(parse_size("nope").is_err());
    }

    #[test]
    fn parse_size_rejects_too_large() {
        // Within per-side cap but over the total-pixel cap.
        assert!(parse_size("20000x20000").is_err());
        // Over the per-side cap.
        assert!(parse_size("20001x10").is_err());
        // A large-but-legal wallpaper is accepted.
        assert!(parse_size("7680x4320").is_ok());
    }

    #[test]
    fn date_seed_mapping() {
        assert_eq!(parse_date_seed("2026-07-17").unwrap(), 20_260_717);
        assert_eq!(parse_date_seed("2000-01-01").unwrap(), 20_000_101);
        assert_eq!(date_to_seed(2026, 12, 31).unwrap(), 20_261_231);
        assert!(parse_date_seed("2026-13-01").is_err());
        assert!(parse_date_seed("2026-00-10").is_err());
        assert!(parse_date_seed("2026-07").is_err());
        assert!(parse_date_seed("bad").is_err());
    }

    #[test]
    fn civil_from_days_known_dates() {
        assert_eq!(civil_from_unix_days(0), (1970, 1, 1));
        // 2026-07-17: computed independently below.
        let (y, m, d) = civil_from_unix_days(20_651);
        assert_eq!((y, m, d), (2026, 7, 17));
    }

    #[test]
    fn default_filename_shape() {
        assert_eq!(
            default_filename(20_260_717, Style::Flow, "png"),
            "wallgen-20260717-flow.png"
        );
        assert_eq!(
            default_filename(42, Style::Voronoi, "jpg"),
            "wallgen-00000042-voronoi.jpg"
        );
    }

    #[test]
    fn set_supported_on_desktop_systems() {
        assert!(set_supported_on("macos"));
        assert!(set_supported_on("linux"));
        assert!(set_supported_on("windows"));
        assert!(!set_supported_on("ios"));
        assert!(!set_supported_on("android"));
    }

    #[test]
    fn applescript_contains_and_escapes_path() {
        let s = wallpaper_applescript("/tmp/a b.png");
        assert!(s.contains("/tmp/a b.png"));
        assert!(s.contains("every desktop"));
        // Quotes and backslashes are escaped for the AppleScript literal.
        let e = wallpaper_applescript("/x/\"q\"\\z.png");
        assert!(e.contains("\\\""));
        assert!(e.contains("\\\\"));
    }

    #[test]
    fn generate_is_deterministic() {
        let cfg = Config {
            size: Size::new(80, 64),
            seed: 20_260_717,
            style: Style::Waves,
            palette: palette::by_name("ocean").unwrap(),
        };
        let a = generate(&cfg);
        let b = generate(&cfg);
        assert_eq!(a.into_raw(), b.into_raw());
    }
}
