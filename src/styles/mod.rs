//! Pluggable generative styles.
//!
//! Each variant of [`Style`] lives in its own submodule and exposes a
//! `render(size, seed, palette) -> RgbaImage` function. Adding a style means
//! adding a module, an enum variant, and a match arm; the CLI and library pick
//! it up through [`Style::ALL`] and [`Style::render`].

pub mod bauhaus;
pub mod flow;
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
}

impl Style {
    /// Every concrete style, in a stable order.
    pub const ALL: &'static [Style] = &[Style::Flow, Style::Voronoi, Style::Waves, Style::Bauhaus];

    /// The style's canonical lowercase name.
    pub fn name(self) -> &'static str {
        match self {
            Style::Flow => "flow",
            Style::Voronoi => "voronoi",
            Style::Waves => "waves",
            Style::Bauhaus => "bauhaus",
        }
    }

    /// A one-line description (used in `--help`-adjacent docs).
    pub fn describe(self) -> &'static str {
        match self {
            Style::Flow => "flow-field streamlines over layered noise",
            Style::Voronoi => "colored Voronoi cells with soft shading",
            Style::Waves => "interference of several sine wave fields",
            Style::Bauhaus => "grid of bold geometric shapes",
        }
    }

    /// Parse a style name. `"random"` picks one deterministically from `seed`.
    pub fn parse(name: &str, seed: u64) -> Option<Style> {
        let n = name.trim().to_ascii_lowercase();
        if n == "random" {
            return Some(Style::pick_random(seed));
        }
        Style::ALL.iter().copied().find(|s| s.name() == n)
    }

    /// Deterministically choose a style from the seed.
    pub fn pick_random(seed: u64) -> Style {
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
    fn random_covers_multiple_styles() {
        let mut seen = std::collections::HashSet::new();
        for seed in 0..500u64 {
            seen.insert(Style::pick_random(seed));
        }
        assert!(seen.len() > 1);
    }
}
