//! Command-line entry point for wallgen.
//!
//! This layer resolves the configuration (including deriving today's date for
//! the default seed), writes the encoded image, and — only behind the explicit
//! `--set` flag — runs the commands that apply the wallpaper (planned by
//! [`wallgen::desktop`]). All pixel generation happens in the library and
//! never reads the clock.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use image::DynamicImage;

use wallgen::desktop::{self, Desktop, Plan, Step};
use wallgen::{
    civil_from_unix_days, date_to_seed, default_filename, generate, parse_date_seed, parse_size,
    set_supported_on, styles, Config, Style, DEFAULT_SIZE,
};

/// Generate deterministic generative-art wallpapers seeded by the date.
#[derive(Parser, Debug)]
#[command(name = "wallgen", version, about, long_about = None)]
struct Cli {
    /// Style: flow, voronoi, waves, bauhaus, topo, truchet, mesh, circles,
    /// "random" (seed-chosen from the first four, stable across versions), or
    /// "any" (seed-chosen from every style).
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

    /// Set the generated image as the desktop wallpaper (macOS, Windows,
    /// GNOME, Cinnamon, MATE, KDE Plasma, Xfce, sway).
    #[arg(long)]
    set: bool,

    /// With --set: the desktop to set instead of detecting it (macos,
    /// windows, gnome, cinnamon, mate, kde, xfce, sway).
    #[arg(long, value_name = "NAME")]
    desktop: Option<String>,

    /// Print the commands --set would run, without running them.
    #[arg(long)]
    show_set: bool,
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
            let note = if Style::RANDOM_POOL.contains(s) {
                ""
            } else {
                "  (not picked by --style random; use --style any)"
            };
            println!("{:<8} {}{note}", s.name(), s.describe());
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

    // Resolve the desktop before rendering so a bad --desktop fails fast.
    let target = if cli.set || cli.show_set {
        Some(resolve_desktop(cli.desktop.as_deref())?)
    } else {
        if cli.desktop.is_some() {
            return Err(anyhow!("--desktop only applies with --set or --show-set"));
        }
        None
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

    if let Some(target) = target {
        // Use an absolute path so the desktop resolves it regardless of cwd.
        let abs = std::fs::canonicalize(&path)
            .with_context(|| format!("failed to resolve {}", path.display()))?;
        let abs = display_path(&abs)?;
        let plan = desktop::plan(target, &abs).map_err(|e| anyhow!(e))?;
        if cli.show_set {
            show_plan(&plan, &abs);
        }
        if cli.set {
            run_plan(&plan, &abs)?;
            eprintln!("wallgen: set as the {target} desktop wallpaper");
        }
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

/// Pick the desktop: `--desktop` when given, else detect it from the OS and
/// the session's environment.
fn resolve_desktop(name: Option<&str>) -> Result<Desktop> {
    let os = std::env::consts::OS;
    if !set_supported_on(os) {
        return Err(anyhow!("--set is not supported on {os}"));
    }
    if let Some(name) = name {
        return Desktop::from_name(name).ok_or_else(|| {
            anyhow!(
                "unknown desktop: {name:?} (expected one of: {})",
                desktop::names()
            )
        });
    }
    let xdg = std::env::var("XDG_CURRENT_DESKTOP").ok();
    let swaysock = std::env::var_os("SWAYSOCK").is_some();
    desktop::detect(os, xdg.as_deref(), swaysock).map_err(|e| anyhow!(e))
}

/// The path as the desktop should see it. On Windows, `canonicalize` returns
/// a `\\?\` verbatim path, which the wallpaper API does not need. A path that
/// is not valid Unicode is refused rather than passed on altered.
fn display_path(path: &Path) -> Result<String> {
    let s = path.to_str().ok_or_else(|| {
        anyhow!(
            "the output path {} is not valid Unicode; write the image elsewhere with -o",
            path.display()
        )
    })?;
    Ok(match s.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => rest.to_string(),
        _ => s.to_string(),
    })
}

fn show_plan(plan: &Plan, path: &str) {
    let show = |step: &Step| {
        let env: String = step
            .env
            .iter()
            .map(|(k, v)| format!("{k}={v:?} "))
            .collect();
        let optional = if step.required { "" } else { "  (optional)" };
        println!(
            "{env}{} {}{optional}",
            step.program,
            shell_words(&step.args)
        );
    };
    match plan {
        Plan::Run(steps) => steps.iter().for_each(show),
        Plan::Xfce => {
            show(&desktop::xfce_list_step());
            println!("# then, for each /backdrop/.../last-image property:\n{}", {
                let step = desktop::xfce_set_step("<property>", path);
                format!("{} {}", step.program, shell_words(&step.args))
            });
        }
    }
}

fn shell_words(args: &[String]) -> String {
    args.iter()
        .map(|a| {
            if !a.is_empty()
                && a.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_./:=+,@%".contains(&b))
            {
                a.clone()
            } else {
                format!("'{}'", a.replace('\'', "'\\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Run a plan. Optional steps may fail; required ones may not.
fn run_plan(plan: &Plan, path: &str) -> Result<()> {
    match plan {
        Plan::Run(steps) => {
            for step in steps {
                run_step(step)?;
            }
        }
        Plan::Xfce => {
            let list = desktop::xfce_list_step();
            let output = command(&list)
                .output()
                .with_context(|| format!("failed to run {}", list.program))?;
            if !output.status.success() {
                return Err(anyhow!("{} exited with {}", list.program, output.status));
            }
            let props = desktop::xfce_image_properties(&String::from_utf8_lossy(&output.stdout));
            if props.is_empty() {
                return Err(anyhow!(
                    "xfce4-desktop has no /backdrop/.../last-image property yet; \
                     pick any wallpaper once in Desktop settings, then retry"
                ));
            }
            for prop in props {
                run_step(&desktop::xfce_set_step(&prop, path))?;
            }
        }
    }
    Ok(())
}

fn command(step: &Step) -> Command {
    let mut cmd = Command::new(&step.program);
    cmd.args(&step.args);
    for (key, value) in &step.env {
        cmd.env(key, value);
    }
    cmd
}

fn run_step(step: &Step) -> Result<()> {
    let status = command(step).status();
    match status {
        Ok(status) if status.success() => Ok(()),
        _ if !step.required => {
            eprintln!(
                "wallgen: optional step `{} {}` did not succeed; continuing",
                step.program,
                shell_words(&step.args)
            );
            Ok(())
        }
        Ok(status) => Err(anyhow!("{} exited with {status}", step.program)),
        Err(e) => Err(anyhow!("failed to run {}: {e}", step.program)),
    }
}
