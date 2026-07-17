//! Self-implemented value noise and fractal Brownian motion (fBm).
//!
//! No external noise crate: the lattice is hashed directly from the seed, and
//! values are smoothly interpolated. fBm layers several octaves for the organic
//! look the flow-field style relies on.

/// A seeded 2D value-noise field. Cheap to construct and copy.
#[derive(Debug, Clone, Copy)]
pub struct ValueNoise {
    seed: u64,
}

impl ValueNoise {
    /// Create a noise field bound to `seed`.
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    /// Hash an integer lattice point to a value in `[0, 1)`.
    fn lattice(&self, xi: i64, yi: i64) -> f32 {
        let mut h = self.seed;
        h ^= (xi as u64).wrapping_mul(0xA0761D6478BD642F);
        h = (h ^ (h >> 32)).wrapping_mul(0xE7037ED1A0B428DB);
        h ^= (yi as u64).wrapping_mul(0x8EBC6AF09C88C6E3);
        h = (h ^ (h >> 29)).wrapping_mul(0x589965CC75374CC3);
        h ^= h >> 32;
        // Top 24 bits -> [0, 1).
        (h >> 40) as f32 / (1u32 << 24) as f32
    }

    /// Sample the noise field at `(x, y)` in `[0, 1]`, bilinearly interpolated
    /// with a smootherstep fade for continuous first and second derivatives.
    pub fn sample(&self, x: f32, y: f32) -> f32 {
        let x0 = x.floor();
        let y0 = y.floor();
        let xi = x0 as i64;
        let yi = y0 as i64;
        let fx = x - x0;
        let fy = y - y0;
        let ux = smootherstep(fx);
        let uy = smootherstep(fy);

        let n00 = self.lattice(xi, yi);
        let n10 = self.lattice(xi + 1, yi);
        let n01 = self.lattice(xi, yi + 1);
        let n11 = self.lattice(xi + 1, yi + 1);

        let top = lerp(n00, n10, ux);
        let bot = lerp(n01, n11, ux);
        lerp(top, bot, uy)
    }

    /// Fractal Brownian motion: sum `octaves` layers of noise at doubling
    /// frequency and halving amplitude, normalized to `[0, 1]`.
    pub fn fbm(&self, x: f32, y: f32, octaves: u32) -> f32 {
        let mut freq = 1.0f32;
        let mut amp = 1.0f32;
        let mut sum = 0.0f32;
        let mut norm = 0.0f32;
        for _ in 0..octaves.max(1) {
            sum += amp * self.sample(x * freq, y * freq);
            norm += amp;
            freq *= 2.0;
            amp *= 0.5;
        }
        if norm > 0.0 {
            sum / norm
        } else {
            0.0
        }
    }
}

/// Linear interpolation between `a` and `b`.
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Ken Perlin's smootherstep: `6t^5 - 15t^4 + 10t^3`.
pub fn smootherstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_in_unit_range() {
        let n = ValueNoise::new(20260717);
        for i in 0..200 {
            let v = n.sample(i as f32 * 0.37, i as f32 * 0.71);
            assert!((0.0..=1.0).contains(&v), "value out of range: {v}");
        }
    }

    #[test]
    fn deterministic() {
        let a = ValueNoise::new(5);
        let b = ValueNoise::new(5);
        assert_eq!(a.sample(3.2, 9.8), b.sample(3.2, 9.8));
    }

    #[test]
    fn integer_points_are_continuous() {
        // Values just left and right of a lattice line stay close (no seams).
        let n = ValueNoise::new(11);
        let a = n.sample(4.0 - 1e-3, 2.5);
        let b = n.sample(4.0 + 1e-3, 2.5);
        assert!((a - b).abs() < 0.05);
    }

    #[test]
    fn fbm_varies_across_space() {
        let n = ValueNoise::new(99);
        let a = n.fbm(0.0, 0.0, 5);
        let b = n.fbm(10.0, 10.0, 5);
        assert!((a - b).abs() > 1e-4);
        assert!((0.0..=1.0).contains(&a));
    }

    #[test]
    fn smootherstep_endpoints() {
        assert_eq!(smootherstep(0.0), 0.0);
        assert_eq!(smootherstep(1.0), 1.0);
        assert!((smootherstep(0.5) - 0.5).abs() < 1e-6);
    }
}
