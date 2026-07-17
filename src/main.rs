//! Command-line entry point for wallgen.
//!
//! This layer resolves the configuration (including deriving today's date for
//! the default seed), writes the encoded image, and — only behind the explicit
//! `--set` flag — applies the wallpaper on macOS via `osascript`. All pixel
//! generation happens in the library and never reads the clock.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use image::DynamicImage;

use wallgen::{
    civil_from_unix_days, date_to_seed, default_filename, generate, parse_date_seed, parse_size,
    set_supported_on, styles, wallpaper_applescript, Config, Style, DEFAULT_SIZE,
};

/// Generate deterministic generative-art wallpapers seeded by the date.
#[derive(Parser, Debug)]
#[command(name = "wallgen", version, about, long_about = None)]
struct Cli {
    /// Style: flow, voronoi, waves, bauhaus, or "random" (seed-chosen).
    #[arg(long, default_value = "random")]
    style: String,

    /// Size: "WxH" (e.g. 5120x2880) or a preset (720p, 1080p, 1440p, 4k, 5k, 8k, ...).
    #[arg(long)]
    size: Option<String>,

    /// Palette name (see --list-palettes). Default: chosen from the seed.
    #[arg(long)]
    palette: Option<String>,

    /// Override the seed directly (any integer). Beats --date and the default.
    #[arg(long)]
    seed: Option<u64>,

    /// Seed from a specific date, YYYY-MM-DD. Ignored when --seed is given.
    #[arg(long)]
    date: Option<String>,

    /// Output format / extension.
    #[arg(long, value_parser = ["png", "jpg", "jpeg", "webp"], default_value = "png")]
    format: String,

    /// Output file path. Its extension overrides --format. Default:
    /// wallgen-YYYYMMDD-<style>.<ext> in the current directory.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// List the available palettes and exit.
    #[arg(long)]
    list_palettes: bool,

    /// List the available styles and exit.
    #[arg(long)]
    list_styles: bool,

    /// macOS only: set the generated image as the desktop wallpaper.
    #[arg(long)]
    set: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.list_palettes {
        for p in wallgen::palette::PALETTES {
            let swatches = p
                .colors
                .iter()
                .map(|c| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]))
                .collect::<Vec<_>>()
                .join(" ");
            println!("{:<8} {swatches}", p.name);
        }
        return Ok(());
    }
    if cli.list_styles {
        for s in Style::ALL {
            println!("{:<8} {}", s.name(), s.describe());
        }
        return Ok(());
    }

    // Resolve the seed: --seed beats --date beats today's date.
    let seed = match (cli.seed, &cli.date) {
        (Some(s), _) => s,
        (None, Some(d)) => parse_date_seed(d)?,
        (None, None) => today_seed()?,
    };

    // Resolve the style (may be "random", chosen from the seed).
    let style = Style::parse(&cli.style, seed).ok_or_else(|| {
        anyhow!(
            "unknown style: {:?} (expected one of: {}, random)",
            cli.style,
            styles::names().join(", ")
        )
    })?;

    // Resolve the palette: explicit name, else chosen from the seed.
    let palette = match &cli.palette {
        Some(name) => wallgen::palette::by_name(name).ok_or_else(|| {
            anyhow!(
                "unknown palette: {name:?} (see --list-palettes: {})",
                wallgen::palette::names().join(", ")
            )
        })?,
        None => wallgen::palette::pick(seed),
    };

    let size = match &cli.size {
        Some(s) => parse_size(s)?,
        None => DEFAULT_SIZE,
    };

    let cfg = Config {
        size,
        seed,
        style,
        palette,
    };

    let img = generate(&cfg);

    let (path, ext) = resolve_output(&cli, seed, style);
    save_image(&img, &path, &ext).with_context(|| format!("failed to write {}", path.display()))?;
    eprintln!(
        "wallgen: wrote {} ({}x{}, style={}, palette={}, seed={})",
        path.display(),
        size.width,
        size.height,
        style.name(),
        palette.name,
        seed
    );

    if cli.set {
        set_wallpaper(&path)?;
        eprintln!("wallgen: set as desktop wallpaper");
    }

    Ok(())
}

/// Derive today's `YYYYMMDD` seed from the system clock. This is the only place
/// the clock is read, and `--seed`/`--date` fully override it.
fn today_seed() -> Result<u64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?;
    let days = (now.as_secs() / 86_400) as i64;
    let (y, m, d) = civil_from_unix_days(days);
    date_to_seed(y, m, d)
}

/// Resolve the output path and encoder extension. With --output the extension
/// comes from the given path; otherwise it is the default name plus --format.
fn resolve_output(cli: &Cli, seed: u64, style: Style) -> (PathBuf, String) {
    if let Some(out) = &cli.output {
        let ext = out
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_else(|| cli.format.clone());
        return (out.clone(), ext);
    }
    let ext = cli.format.clone();
    let name = default_filename(seed, style, &ext);
    (PathBuf::from(name), ext)
}

fn save_image(img: &image::RgbaImage, path: &Path, ext: &str) -> Result<()> {
    match ext {
        "jpg" | "jpeg" => {
            // JPEG has no alpha channel; drop it before encoding.
            DynamicImage::ImageRgba8(img.clone()).to_rgb8().save(path)?;
        }
        "png" | "webp" => img.save(path)?,
        other => {
            return Err(anyhow!(
                "unsupported output extension: .{other} (use .png/.jpg/.webp)"
            ))
        }
    }
    Ok(())
}

/// Apply the wallpaper on macOS. Guarded so it only ever runs behind `--set`
/// and only on macOS; on any other OS it errors clearly without touching the
/// desktop.
fn set_wallpaper(path: &Path) -> Result<()> {
    if !set_supported_on(std::env::consts::OS) {
        return Err(anyhow!(
            "--set is only supported on macOS (current OS: {})",
            std::env::consts::OS
        ));
    }
    // Use an absolute path so System Events resolves it regardless of cwd.
    let abs = std::fs::canonicalize(path)
        .with_context(|| format!("failed to resolve {}", path.display()))?;
    let script = wallpaper_applescript(&abs.to_string_lossy());
    let status = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .status()
        .context("failed to run osascript")?;
    if !status.success() {
        return Err(anyhow!("osascript exited with {status}"));
    }
    Ok(())
}
