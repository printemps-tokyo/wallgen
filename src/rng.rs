//! A small, self-contained deterministic pseudo-random number generator.
//!
//! wallgen deliberately implements its own RNG (SplitMix64) instead of pulling
//! in the `rand` crate: the whole point of the tool is that a given seed
//! produces byte-identical output everywhere, so the generation path must not
//! touch OS randomness. SplitMix64 is tiny, fast, and has good statistical
//! quality for procedural art.

/// SplitMix64 pseudo-random number generator.
///
/// Reference: Sebastiano Vigna, "Fast splittable pseudorandom number
/// generators" (the `splitmix64` finalizer). It advances a 64-bit state by a
/// fixed odd increment (the golden-ratio constant) and applies an avalanche
/// mixing function.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// Create a generator from a 64-bit seed. Every seed is valid.
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Draw the next raw 64-bit value.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Draw a uniform `f32` in the half-open range `[0, 1)`.
    pub fn next_f32(&mut self) -> f32 {
        // Use the top 24 bits for a full-precision f32 mantissa.
        (self.next_u64() >> 40) as f32 / (1u32 << 24) as f32
    }

    /// Draw a uniform `f32` in `[lo, hi)`.
    pub fn range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }

    /// Draw a `u32` in the half-open range `[lo, hi)`. Returns `lo` when the
    /// range is empty.
    pub fn range_u32(&mut self, lo: u32, hi: u32) -> u32 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next_u64() % (hi - lo) as u64) as u32
    }

    /// Draw `true` with the given probability (clamped to `[0, 1]`).
    pub fn chance(&mut self, p: f32) -> bool {
        self.next_f32() < p.clamp(0.0, 1.0)
    }

    /// Pick a random element index for a slice of length `len` (`0` when empty).
    pub fn index(&mut self, len: usize) -> usize {
        if len == 0 {
            0
        } else {
            (self.next_u64() % len as u64) as usize
        }
    }
}

/// Deterministically mix a seed with a domain constant so independent decisions
/// (which style, which palette, per-style streams) do not share a sequence.
pub fn mix(seed: u64, domain: u64) -> u64 {
    let mut z = seed ^ domain.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_sequence() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_differ() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn f32_in_unit_range() {
        let mut r = Rng::new(7);
        for _ in 0..10_000 {
            let v = r.next_f32();
            assert!((0.0..1.0).contains(&v));
        }
    }

    #[test]
    fn range_u32_within_bounds() {
        let mut r = Rng::new(9);
        for _ in 0..10_000 {
            let v = r.range_u32(10, 20);
            assert!((10..20).contains(&v));
        }
        // Empty range yields the low bound.
        assert_eq!(r.range_u32(5, 5), 5);
    }

    #[test]
    fn mix_is_stable_and_domain_separated() {
        assert_eq!(mix(123, 1), mix(123, 1));
        assert_ne!(mix(123, 1), mix(123, 2));
    }
}
