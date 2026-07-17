//! Curated color palettes and deterministic selection.
//!
//! Each palette is a small, ordered set of tasteful colors. Styles treat the
//! first entry as the natural background/base and the rest as accents, and may
//! also sample the whole set as a smooth gradient.

use crate::rng::{mix, Rng};

/// Domain constant so palette choice has its own RNG stream, independent of the
/// pixel path (choosing a palette must not shift the generated art).
const DOMAIN_PALETTE: u64 = 0x5041_4C45_5454_4521; // "PALETTE!"

/// A named, ordered list of RGB colors.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub name: &'static str,
    pub colors: &'static [[u8; 3]],
}

impl Palette {
    /// The base/background color (first entry).
    pub fn base(&self) -> [u8; 3] {
        self.colors[0]
    }

    /// Sample the palette as a smooth gradient at `t` in `[0, 1]`.
    pub fn sample(&self, t: f32) -> [u8; 3] {
        let cols = self.colors;
        if cols.len() == 1 {
            return cols[0];
        }
        let t = t.clamp(0.0, 1.0);
        let scaled = t * (cols.len() - 1) as f32;
        let i = scaled.floor() as usize;
        let i = i.min(cols.len() - 2);
        let f = scaled - i as f32;
        let a = cols[i];
        let b = cols[i + 1];
        [
            lerp_u8(a[0], b[0], f),
            lerp_u8(a[1], b[1], f),
            lerp_u8(a[2], b[2], f),
        ]
    }
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t)
        .round()
        .clamp(0.0, 255.0) as u8
}

/// All built-in palettes, in a stable order.
pub const PALETTES: &[Palette] = &[
    Palette {
        name: "sunset",
        colors: &[
            [26, 20, 35],
            [61, 33, 71],
            [155, 62, 90],
            [224, 122, 95],
            [242, 204, 143],
        ],
    },
    Palette {
        name: "ocean",
        colors: &[
            [8, 24, 43],
            [18, 63, 92],
            [32, 116, 138],
            [86, 179, 176],
            [200, 234, 214],
        ],
    },
    Palette {
        name: "forest",
        colors: &[
            [18, 33, 26],
            [40, 71, 52],
            [88, 121, 79],
            [154, 168, 104],
            [222, 213, 160],
        ],
    },
    Palette {
        name: "ember",
        colors: &[
            [20, 12, 12],
            [77, 22, 22],
            [158, 42, 43],
            [224, 93, 45],
            [244, 187, 68],
        ],
    },
    Palette {
        name: "nordic",
        colors: &[
            [46, 52, 64],
            [59, 66, 82],
            [76, 86, 106],
            [136, 192, 208],
            [236, 239, 244],
        ],
    },
    Palette {
        name: "pastel",
        colors: &[
            [45, 43, 66],
            [116, 106, 173],
            [224, 155, 179],
            [246, 199, 187],
            [250, 237, 205],
        ],
    },
    Palette {
        name: "neon",
        colors: &[
            [13, 2, 33],
            [78, 13, 96],
            [200, 26, 122],
            [246, 65, 108],
            [45, 226, 230],
        ],
    },
    Palette {
        name: "mono",
        colors: &[
            [17, 17, 19],
            [58, 60, 66],
            [116, 120, 128],
            [176, 180, 188],
            [233, 235, 240],
        ],
    },
    Palette {
        name: "sand",
        colors: &[
            [38, 30, 24],
            [92, 71, 52],
            [163, 128, 90],
            [214, 183, 138],
            [240, 224, 194],
        ],
    },
    Palette {
        name: "aurora",
        colors: &[
            [11, 19, 43],
            [28, 55, 90],
            [67, 138, 124],
            [122, 201, 138],
            [214, 245, 178],
        ],
    },
];

/// Look up a palette by (case-insensitive) name.
pub fn by_name(name: &str) -> Option<&'static Palette> {
    let want = name.trim().to_ascii_lowercase();
    PALETTES.iter().find(|p| p.name == want)
}

/// Deterministically choose a palette from the seed.
pub fn pick(seed: u64) -> &'static Palette {
    let mut rng = Rng::new(mix(seed, DOMAIN_PALETTE));
    &PALETTES[rng.index(PALETTES.len())]
}

/// The list of palette names, for `--list-palettes`.
pub fn names() -> Vec<&'static str> {
    PALETTES.iter().map(|p| p.name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique_and_nonempty() {
        let mut seen = std::collections::HashSet::new();
        for p in PALETTES {
            assert!(!p.name.is_empty());
            assert!(p.colors.len() >= 2, "{} needs >=2 colors", p.name);
            assert!(seen.insert(p.name), "duplicate palette: {}", p.name);
        }
    }

    #[test]
    fn lookup_round_trips() {
        for p in PALETTES {
            assert_eq!(by_name(p.name).unwrap().name, p.name);
            assert_eq!(by_name(&p.name.to_uppercase()).unwrap().name, p.name);
        }
        assert!(by_name("does-not-exist").is_none());
    }

    #[test]
    fn pick_is_deterministic() {
        for seed in [1u64, 20260717, 999999] {
            assert_eq!(pick(seed).name, pick(seed).name);
        }
    }

    #[test]
    fn sample_endpoints_and_bounds() {
        let p = by_name("ocean").unwrap();
        assert_eq!(p.sample(0.0), p.colors[0]);
        assert_eq!(p.sample(1.0), *p.colors.last().unwrap());
        // Interior sample is a real color (bounds are trivially satisfied for u8).
        let _ = p.sample(0.5);
    }

    #[test]
    fn pick_covers_multiple_palettes() {
        // Across many seeds we should see more than one palette selected.
        let mut seen = std::collections::HashSet::new();
        for seed in 0..500u64 {
            seen.insert(pick(seed).name);
        }
        assert!(seen.len() > 1);
    }
}
