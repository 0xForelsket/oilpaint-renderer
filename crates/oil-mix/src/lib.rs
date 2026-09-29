//! Mixers behind one trait (docs/plans/LIBRARY_PLAN.md, section 6).
//!
//! A mixer defines the per-pixel paint state (a fixed-size f32 vector the kernel mixes linearly), how an authored
//! sRGB colour becomes a state, and how a state is displayed. The kernel is generic over `M: Mixer` and
//! monomorphised, so there is no dispatch per pixel.
//!
//! Mixers in the engine: `ochrell` (the default; added once the Ochrell optimisation work is finished), `rgb` (plain
//! sRGB interpolation, a baseline). Mixbox (CC BY-NC 4.0) lives in the separate opt-in crate `oil-mix-mixbox`.
#![forbid(unsafe_code)]
// v1's clamp01 and range checks are kept verbatim (same results as f32::clamp, including NaN and signed zero).
#![allow(clippy::manual_clamp, clippy::manual_range_contains)]

/// A paint state: a fixed-length f32 vector that mixes by linear interpolation.
pub trait State: Copy + Send + Sync + 'static {
    const LEN: usize;
    fn zero() -> Self;
    fn as_slice(&self) -> &[f32];
    fn as_mut_slice(&mut self) -> &mut [f32];
}

impl<const N: usize> State for [f32; N] {
    const LEN: usize = N;
    #[inline(always)]
    fn zero() -> Self {
        [0.0; N]
    }
    #[inline(always)]
    fn as_slice(&self) -> &[f32] {
        self
    }
    #[inline(always)]
    fn as_mut_slice(&mut self) -> &mut [f32] {
        self
    }
}

pub trait Mixer: Send + Sync + 'static {
    /// Stable identifier, recorded in StrokeList metadata and reports (for example `ochrell-0.2`, `mixbox-2.0`).
    const ID: &'static str;
    type State: State;

    /// Encode an authored sRGB colour (each channel in [0, 1]).
    fn encode(&self, srgb: [f32; 3]) -> Self::State;

    /// Display colour of a state, sRGB in [0, 1].
    fn decode_srgb(&self, z: &Self::State) -> [f32; 3];

    /// `base += amplitude * delta` (bristle colour streaks), keeping the state valid for this mixer.
    #[inline(always)]
    fn streak(&self, base: &mut Self::State, delta: &Self::State, amplitude: f32) {
        for (b, d) in base.as_mut_slice().iter_mut().zip(delta.as_slice()) {
            *b += amplitude * *d;
        }
    }

    /// True if the state may be mixed and decoded (finite, and within the mixer's own domain).
    fn is_valid(&self, z: &Self::State) -> bool {
        z.as_slice().iter().all(|v| v.is_finite())
    }
}

/// Plain sRGB interpolation: the flat baseline (v1's `--mixer rgb`).
#[derive(Clone, Copy, Debug, Default)]
pub struct RgbMixer;

impl Mixer for RgbMixer {
    const ID: &'static str = "rgb";
    type State = [f32; 3];

    #[inline(always)]
    fn encode(&self, srgb: [f32; 3]) -> [f32; 3] {
        srgb.map(clamp01)
    }

    #[inline(always)]
    fn decode_srgb(&self, z: &[f32; 3]) -> [f32; 3] {
        z.map(clamp01)
    }
}

#[inline(always)]
pub fn clamp01(x: f32) -> f32 {
    if x < 0.0 {
        0.0
    } else if x > 1.0 {
        1.0
    } else {
        x
    }
}

/// Composite `new` over `old` (both sRGB) with coverage `a` in linear light, using the engine's sRGB transfer.
#[inline]
pub fn composite_srgb(old: &mut [f32; 3], new: [f32; 3], a: f32) {
    for ch in 0..3 {
        let lo = oil_math::srgb_to_linear(old[ch] as f64);
        let ln = oil_math::srgb_to_linear(new[ch] as f64);
        old[ch] = oil_math::linear_to_srgb(lo + a as f64 * (ln - lo)) as f32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_mixer_round_trips_and_streaks() {
        let m = RgbMixer;
        let z = m.encode([0.2, 0.5, 1.2]);
        assert_eq!(m.decode_srgb(&z), [0.2, 0.5, 1.0]);
        let mut b = z;
        m.streak(&mut b, &[0.1, -0.1, 0.0], 0.5);
        assert_eq!(b, [0.25, 0.45, 1.0]);
        assert!(m.is_valid(&b));
        assert!(!m.is_valid(&[f32::NAN, 0.0, 0.0]));
    }

    #[test]
    fn composite_is_in_linear_light() {
        let mut c = [0.0f32, 1.0, 0.5];
        composite_srgb(&mut c, [1.0, 0.0, 0.5], 0.5);
        // half-way in linear light is brighter than half-way in sRGB
        assert!((c[0] - 0.735_356).abs() < 1e-5 && (c[1] - 0.735_356).abs() < 1e-5, "{c:?}");
        assert!((c[2] - 0.5).abs() < 1e-6);
    }
}
