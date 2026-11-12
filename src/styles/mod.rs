//! Pluggable generative styles.
//!
//! Each variant of [`Style`] lives in its own submodule and exposes a
//! `render(size, seed, palette) -> RgbaImage` function. Adding a style means
//! adding a module, an enum variant (at the end, so earlier styles keep their
//! RNG streams), and a match arm; the CLI and library pick it up through
//! [`Style::ALL`] and [`Style::render`].
//!
//! `--style random` draws only from [`Style::RANDOM_POOL`], the styles of the
//! first release, so a date's wallpaper stays the same across versions.
//! `--style any` draws from every style and changes when styles are added.

pub mod bauhaus;
pub mod circles;
pub mod flow;
pub mod mesh;
pub mod topo;
pub mod truchet;
pub mod voronoi;
pub mod waves;

use crate::palette::Palette;
use crate::rng::{mix, Rng};
use crate::Size;
use image::RgbaImage;

/// Domain constant so `--style random` draws from its own RNG stream.
const DOMAIN_STYLE: u64 = 0x5354_594C_4552_4E44; // "STYLERND"

/// A generative wallpaper style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Style {
    /// Flow-field streamlines over self-generated fBm noise.
    Flow,
    /// Colored Voronoi cells from self-implemented nearest-site search.
    Voronoi,
    /// Interference of several sine wave fields.
    Waves,
    /// Bauhaus-inspired grid of bold geometric shapes.
    Bauhaus,
    /// Topographic map: elevation bands and contour lines.
    Topo,
    /// Truchet tiles: quarter-circle arcs that link into winding paths.
    Truchet,
    /// Soft mesh gradient of blended color blobs.
    Mesh,
    /// Circle packing.
    Circles,
}

impl Style {
    /// Every concrete style, in a stable order.
    pub const ALL: &'static [Style] = &[
        Style::Flow,
        Style::Voronoi,
        Style::Waves,
        Style::Bauhaus,
        Style::Topo,
        Style::Truchet,
        Style::Mesh,
        Style::Circles,
    ];

    /// The styles `--style random` chooses from. Frozen at the first release
    /// so a given seed keeps its wallpaper as styles are added.
    pub const RANDOM_POOL: &'static [Style] =
        &[Style::Flow, Style::Voronoi, Style::Waves, Style::Bauhaus];

    /// The style's canonical lowercase name.
    pub fn name(self) -> &'static str {
        match self {
            Style::Flow => "flow",
            Style::Voronoi => "voronoi",
            Style::Waves => "waves",
            Style::Bauhaus => "bauhaus",
            Style::Topo => "topo",
            Style::Truchet => "truchet",
            Style::Mesh => "mesh",
            Style::Circles => "circles",
        }
    }

    /// A one-line description (used in `--help`-adjacent docs).
    pub fn describe(self) -> &'static str {
        match self {
            Style::Flow => "flow-field streamlines over layered noise",
            Style::Voronoi => "colored Voronoi cells with soft shading",
            Style::Waves => "interference of several sine wave fields",
            Style::Bauhaus => "grid of bold geometric shapes",
            Style::Topo => "topographic map with contour lines",
            Style::Truchet => "Truchet tiles of linked quarter-circle arcs",
            Style::Mesh => "soft mesh gradient of blended color blobs",
            Style::Circles => "circle packing with concentric accents",
        }
    }

    /// Parse a style name. `"random"` picks one of [`Style::RANDOM_POOL`]
    /// from `seed`; `"any"` picks one of [`Style::ALL`].
    pub fn parse(name: &str, seed: u64) -> Option<Style> {
        let n = name.trim().to_ascii_lowercase();
        match n.as_str() {
            "random" => Some(Style::pick_random(seed)),
            "any" => Some(Style::pick_any(seed)),
            _ => Style::ALL.iter().copied().find(|s| s.name() == n),
        }
    }

    /// Deterministically choose a style from [`Style::RANDOM_POOL`].
    pub fn pick_random(seed: u64) -> Style {
        let mut rng = Rng::new(mix(seed, DOMAIN_STYLE));
        Style::RANDOM_POOL[rng.index(Style::RANDOM_POOL.len())]
    }

    /// Deterministically choose a style from every style.
    pub fn pick_any(seed: u64) -> Style {
        let mut rng = Rng::new(mix(seed, DOMAIN_STYLE));
        Style::ALL[rng.index(Style::ALL.len())]
    }

    /// Render this style into an image.
    pub fn render(self, size: Size, seed: u64, palette: &Palette) -> RgbaImage {
        match self {
            Style::Flow => flow::render(size, seed, palette),
            Style::Voronoi => voronoi::render(size, seed, palette),
            Style::Waves => waves::render(size, seed, palette),
            Style::Bauhaus => bauhaus::render(size, seed, palette),
            Style::Topo => topo::render(size, seed, palette),
            Style::Truchet => truchet::render(size, seed, palette),
            Style::Mesh => mesh::render(size, seed, palette),
            Style::Circles => circles::render(size, seed, palette),
        }
    }
}

/// The list of style names, for help text and `--style` validation.
pub fn names() -> Vec<&'static str> {
    Style::ALL.iter().map(|s| s.name()).collect()
}

/// A per-style RNG seed, mixed so different styles at the same base seed do not
/// share a sequence.
pub(crate) fn style_rng(seed: u64, style: Style) -> Rng {
    Rng::new(mix(seed, 0xC0FFEE00 ^ style as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_known_names() {
        for s in Style::ALL {
            assert_eq!(Style::parse(s.name(), 0), Some(*s));
            assert_eq!(Style::parse(&s.name().to_uppercase(), 0), Some(*s));
        }
        assert_eq!(Style::parse("nope", 0), None);
    }

    #[test]
    fn random_is_deterministic_and_valid() {
        for seed in [0u64, 20260717, 42] {
            let a = Style::parse("random", seed).unwrap();
            let b = Style::parse("random", seed).unwrap();
            assert_eq!(a, b);
            assert!(Style::ALL.contains(&a));
        }
    }

    #[test]
    fn random_pool_is_frozen_and_any_reaches_new_styles() {
        assert_eq!(Style::RANDOM_POOL, &Style::ALL[..4]);
        // Known picks from the first release must not change.
        for (seed, style) in [
            (20261101u64, Style::Waves),
            (20261102, Style::Flow),
            (20261104, Style::Voronoi),
            (20261108, Style::Bauhaus),
        ] {
            assert_eq!(Style::parse("random", seed), Some(style), "seed {seed}");
        }
        let mut seen = std::collections::HashSet::new();
        for seed in 0..500u64 {
            seen.insert(Style::pick_any(seed));
        }
        assert_eq!(seen.len(), Style::ALL.len());
    }

    #[test]
    fn random_covers_multiple_styles() {
        let mut seen = std::collections::HashSet::new();
        for seed in 0..500u64 {
            seen.insert(Style::pick_random(seed));
        }
        assert!(seen.len() > 1);
    }
}
