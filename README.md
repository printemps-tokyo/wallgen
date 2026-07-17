# wallgen

> Same day, same wallpaper, everywhere. Deterministic generative-art desktop wallpapers, seeded by the date.

[![CI](https://github.com/printemps-tokyo/wallgen/actions/workflows/ci.yml/badge.svg)](https://github.com/printemps-tokyo/wallgen/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)

`wallgen` renders a fresh generative-art wallpaper from a single seed. By
default the seed is today's date as an integer (`YYYYMMDD`), so everyone who
runs it on the same day gets the byte-identical wallpaper worldwide. Point it at
`cron` and wake up to a new wallpaper every morning that the whole world shares.

Everything is rendered in pure Rust with a self-implemented RNG and noise — no
`rand`, no noise crate, no external service. The same `(seed, style, size,
palette)` always produces the exact same PNG bytes.

## Why

A wallpaper you did not choose, but that is not random either: it is a shared
daily artifact. Because generation is fully deterministic and seeded by the
date, your machine, your colleague's laptop, and a stranger's desktop on the
other side of the planet all show the same art today — and a different one
tomorrow. No network, no telemetry, no surprises.

## Install

```bash
cargo install --git https://github.com/printemps-tokyo/wallgen
```

Or build from a clone:

```bash
cargo build --release
./target/release/wallgen --help
```

Prebuilt binaries will be attached to the [Releases](https://github.com/printemps-tokyo/wallgen/releases) page once a version is tagged.

## Quickstart

```bash
wallgen                                  # today's wallpaper at 2560x1440
wallgen --size 4k                        # today's wallpaper at 3840x2160
wallgen --style flow --palette ocean     # pick a style and palette
wallgen --date 2026-07-17                # reproduce a specific day
wallgen --seed 1234 --style voronoi      # any integer seed
wallgen --size 5120x2880 -o today.png    # explicit size and output path
wallgen --list-styles                    # show styles
wallgen --list-palettes                  # show palettes and their colors
```

By default the file is named `wallgen-YYYYMMDD-<style>.png` in the current
directory. The run summary (size, style, palette, seed) is printed to stderr.

## Styles

Pass `--style <name>`, or `--style random` to let the seed choose one.

| Style | Description |
| --- | --- |
| `flow` | Flow-field streamlines advected over self-generated fBm noise |
| `voronoi` | Colored Voronoi cells (self-implemented nearest-site) with soft shading |
| `waves` | Interference of several sine wave fields, mapped through the palette |
| `bauhaus` | A grid of bold geometric shapes (circles, arcs, triangles, bars) |

Each style lives in its own module, so more can be added over time.

## Palettes

A handful of curated palettes ship built in. Without `--palette` the seed picks
one deterministically; run `wallgen --list-palettes` to see them with their hex
colors.

`sunset`, `ocean`, `forest`, `ember`, `nordic`, `pastel`, `neon`, `mono`,
`sand`, `aurora`

## Size

`--size` accepts either explicit `WxH` (e.g. `5120x2880`) or a named preset.
The default is `1440p` (2560x1440).

| Preset | Pixels |
| --- | --- |
| `720p` | 1280x720 |
| `1080p` | 1920x1080 |
| `wuxga` | 1920x1200 |
| `1440p` | 2560x1440 |
| `wqxga` | 2560x1600 |
| `4k` | 3840x2160 |
| `5k` | 5120x2880 |
| `8k` | 7680x4320 |

Absurd sizes are rejected: each side is capped at 20000 px, and the total is
capped at 100 megapixels.

## Output format

PNG by default. Use `--format png|jpg|webp`, or give `-o` a path whose extension
selects the encoder. JPEG has no alpha channel and is encoded from flattened
RGB.

## Daily wallpaper via cron

Generate a fresh wallpaper every morning at 07:00 and set it (macOS):

```cron
0 7 * * * /usr/local/bin/wallgen --size 5k --output ~/Pictures/wallgen-today.png --set
```

On Linux, drop `--set` and point your desktop environment at the generated file
(for example with `feh`, `gsettings`, or your compositor's wallpaper tool).

## Setting the wallpaper (macOS)

```bash
wallgen --size 5k --set
```

`--set` is macOS-only: after writing the file, wallgen asks System Events (via
`osascript`) to set it as the wallpaper of every desktop. On any other OS the
flag errors clearly and changes nothing. wallgen only ever touches your desktop
when you pass `--set` explicitly.

## Library

The pure generation logic is a library; file writing and the `--set` side
effect live in the binary.

```rust
use wallgen::{generate, palette, parse_size, Config, Style};

let cfg = Config {
    size: parse_size("2560x1440")?,
    seed: 20_260_717,               // or wallgen::parse_date_seed("2026-07-17")?
    style: Style::Flow,
    palette: palette::by_name("ocean").unwrap(),
};
let img = generate(&cfg);           // deterministic RgbaImage
img.save("wallpaper.png")?;
# Ok::<(), anyhow::Error>(())
```

## Determinism

All randomness flows from the seed through a self-implemented SplitMix64 RNG and
value-noise/fBm — no system time and no OS randomness ever touch the pixel path.
The current date is only read to derive the default seed, and `--seed` / `--date`
fully override it, so runs are perfectly reproducible.

## License

[MIT](./LICENSE) (c) printemps.tokyo
