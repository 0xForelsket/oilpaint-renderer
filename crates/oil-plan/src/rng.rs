//! The planner's random numbers: PCG32 (O'Neill's XSH-RR variant), one stream per (plan seed, layer, seed offset,
//! region), so editing one region does not reshuffle any other. Floats come from integers by exact operations;
//! normal draws use Box-Muller on `oil-math`, so every host gives the same sequence.

/// SplitMix64 finaliser, to spread the stream key.
fn mix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
    inc: u64,
}

impl Rng {
    /// A stream keyed by any number of integers.
    pub fn new(key: &[u64]) -> Rng {
        let mut h = 0x243f_6a88_85a3_08d3u64;
        for &k in key {
            h = mix64(h ^ k);
        }
        let mut r = Rng { state: 0, inc: (mix64(h ^ 0x5851_f42d_4c95_7f2d) << 1) | 1 };
        r.next_u32();
        r.state = r.state.wrapping_add(h);
        r.next_u32();
        r
    }

    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Uniform in [0, 1) with 53 random bits.
    pub fn uniform(&mut self) -> f64 {
        let hi = (self.next_u32() >> 5) as u64; // 27 bits
        let lo = (self.next_u32() >> 6) as u64; // 26 bits
        ((hi << 26) | lo) as f64 * (1.0 / 9_007_199_254_740_992.0)
    }

    /// Uniform in [a, b).
    pub fn range(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.uniform()
    }

    /// Uniform integer in [0, n), n >= 1 (Lemire's method without bias).
    pub fn below(&mut self, n: u32) -> u32 {
        let n = n.max(1);
        loop {
            let m = self.next_u32() as u64 * n as u64;
            let l = m as u32;
            if l >= n || l >= n.wrapping_neg() % n {
                return (m >> 32) as u32;
            }
        }
    }

    /// Standard normal (Box-Muller, one value per call).
    pub fn normal(&mut self) -> f64 {
        let u1 = 1.0 - self.uniform(); // (0, 1]
        let u2 = self.uniform();
        (-2.0 * oil_math::ln(u1)).sqrt() * oil_math::cos(2.0 * std::f64::consts::PI * u2)
    }

    /// Fisher-Yates shuffle.
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i as u32 + 1) as usize;
            v.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Rng;

    #[test]
    fn streams_are_repeatable_distinct_and_uniform() {
        let (mut a, mut b, mut c) = (Rng::new(&[1, 2, 3]), Rng::new(&[1, 2, 3]), Rng::new(&[1, 2, 4]));
        let va: Vec<u32> = (0..8).map(|_| a.next_u32()).collect();
        assert_eq!(va, (0..8).map(|_| b.next_u32()).collect::<Vec<_>>());
        assert_ne!(va, (0..8).map(|_| c.next_u32()).collect::<Vec<_>>());
        let mut r = Rng::new(&[7]);
        let n = 20_000;
        let (mut s, mut s2, mut cnt) = (0.0, 0.0, [0u32; 5]);
        for _ in 0..n {
            let u = r.uniform();
            assert!((0.0..1.0).contains(&u));
            let z = r.normal();
            s += z;
            s2 += z * z;
            cnt[r.below(5) as usize] += 1;
        }
        assert!((s / n as f64).abs() < 0.03 && (s2 / n as f64 - 1.0).abs() < 0.05);
        assert!(cnt.iter().all(|&c| (3700..4300).contains(&c)), "{cnt:?}");
    }
}
