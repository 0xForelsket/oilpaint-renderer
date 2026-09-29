//! Ochrell 0.2 (spectral Kubelka-Munk on synthetic pigments), the engine's default mixer.
//!
//! State: 41 absorption bands, 41 scattering bands, 3 residual channels (85 f32, the Ochrell bridge's wire format).
//! Ochrell's own sRGB conversions use the platform `powf`, whose last bits differ between native targets and wasm32
//! (measured in L0), so this mixer never calls them: encoding applies `oil_math`'s sRGB transfer and calls
//! `encode_linear`; decoding calls `decode_linear` and Ochrell's gamut map (arithmetic only) and converts with the
//! engine's sRGB tables.
//!
//! Licence: Ochrell's code is MIT OR Apache-2.0; its generated optical tables are CC BY-SA 4.0 (CIE-derived). A build
//! with this mixer is "MIT AND CC-BY-SA-4.0" (docs/plans/LIBRARY_PLAN.md, section 10, option A).
use crate::{clamp01, srgb, Mixer};
use ochrell::{FastPigmentMixer, Latent};

pub const BANDS: usize = 41;
pub const LAT: usize = 2 * BANDS + 3;
/// Smallest scattering coefficient a streak may leave (Ochrell requires S > 0).
const S_FLOOR: f32 = 1e-10;

#[derive(Clone, Copy, Debug, Default)]
pub struct OchrellMixer;

fn unpack(z: &[f32; LAT]) -> Latent {
    Latent::try_from_parts(
        z[..BANDS].try_into().unwrap(),
        z[BANDS..2 * BANDS].try_into().unwrap(),
        z[2 * BANDS..].try_into().unwrap(),
    )
    // The kernel only forms convex mixtures of encoded states and limits streaks to K >= 0, S > 0, so an invalid
    // state is a programming error, not an input to recover from.
    .expect("invalid Ochrell state")
}

impl Mixer for OchrellMixer {
    const ID: &'static str = "ochrell-0.2";
    type State = [f32; LAT];

    fn encode(&self, srgb: [f32; 3]) -> [f32; LAT] {
        let lin = srgb.map(|c| oil_math::srgb_to_linear(clamp01(c) as f64));
        let z = FastPigmentMixer.encode_linear(lin).expect("linear RGB in [0, 1]");
        let mut out = [0.0f32; LAT];
        out[..BANDS].copy_from_slice(z.absorption());
        out[BANDS..2 * BANDS].copy_from_slice(z.scattering());
        out[2 * BANDS..].copy_from_slice(&z.residual());
        out
    }

    #[inline]
    fn decode_linear_rgb(&self, z: &[f32; LAT]) -> [f32; 3] {
        ochrell::conversion::gamut_map(FastPigmentMixer.decode_linear(unpack(z))).map(|v| v as f32)
    }

    #[inline]
    fn decode_srgb(&self, z: &[f32; LAT]) -> [f32; 3] {
        self.decode_linear_rgb(z).map(srgb::from_linear)
    }

    /// `base += amplitude * delta`, scaled down along its direction just enough that no K turns negative and no S
    /// falls below `S_FLOOR` (the Ochrell bridge's rule, `native/ochrell-brush/src/material.rs`).
    fn streak(&self, base: &mut [f32; LAT], delta: &[f32; LAT], amplitude: f32) {
        let mut scale = 1.0f32;
        for k in 0..2 * BANDS {
            let d = amplitude * delta[k];
            let floor = if k < BANDS { 0.0 } else { S_FLOOR };
            if d < 0.0 {
                scale = scale.min(((base[k] - floor) / -d).max(0.0));
            }
        }
        for k in 0..LAT {
            base[k] += amplitude * scale * delta[k];
        }
        // the limiting subtraction can round just below its boundary in f32
        for v in &mut base[..BANDS] {
            *v = v.max(0.0);
        }
        for v in &mut base[BANDS..2 * BANDS] {
            *v = v.max(S_FLOOR);
        }
    }

    fn is_valid(&self, z: &[f32; LAT]) -> bool {
        z.iter().all(|v| v.is_finite()) && z[..BANDS].iter().all(|k| *k >= 0.0) && z[BANDS..2 * BANDS].iter().all(|s| *s > 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::State;

    #[test]
    fn round_trip_and_blue_yellow_green() {
        let m = OchrellMixer;
        for c in [[0.9f32, 0.2, 0.1], [0.1, 0.3, 0.8], [1.0, 1.0, 1.0], [0.0, 0.0, 0.0], [0.5, 0.5, 0.5]] {
            let back = m.decode_srgb(&m.encode(c));
            assert!(back.iter().zip(c).all(|(a, b)| (a - b).abs() < 2e-4), "{c:?} -> {back:?}");
        }
        let (y, b) = (m.encode([1.0, 220.0 / 255.0, 0.0]), m.encode([20.0 / 255.0, 70.0 / 255.0, 1.0]));
        let mut z = [0.0f32; LAT];
        for k in 0..LAT {
            z[k] = 0.5 * y[k] + 0.5 * b[k];
        }
        let g = m.decode_srgb(&z);
        assert!(g[1] > g[0] && g[1] > g[2], "{g:?}");
        assert_eq!(<[f32; LAT] as State>::LEN, 85);
    }

    #[test]
    fn streak_keeps_optics_valid() {
        let m = OchrellMixer;
        let (a, b) = (m.encode([0.9, 0.9, 0.2]), m.encode([0.05, 0.05, 0.4]));
        let mut delta = [0.0f32; LAT];
        for k in 0..LAT {
            delta[k] = b[k] - a[k];
        }
        for amp in [-50.0f32, -3.0, 0.5, 40.0] {
            let mut z = a;
            m.streak(&mut z, &delta, amp);
            assert!(m.is_valid(&z), "amplitude {amp}");
        }
    }
}
