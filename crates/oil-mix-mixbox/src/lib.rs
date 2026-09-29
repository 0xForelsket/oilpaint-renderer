//! Opt-in Mixbox mixer. Colour mixing: Mixbox (c) 2022 Secret Weapons (Sochorova & Jamriska), CC BY-NC 4.0,
//! https://scrtwpns.com/mixbox (non-commercial use only). Used for comparisons and the port checks P1 and E
//! (docs/plans/LIBRARY_PLAN.md, section 5). Never a default dependency of the engine.
//!
//! Mixbox's encoder (trilinear lookup in its 64^3 table) and decoder (a cubic polynomial) use only IEEE basic
//! operations, so they are deterministic on every host.
#![forbid(unsafe_code)]

use oil_mix::{clamp01, Mixer};

pub const ATTRIBUTION: &str =
    "Colour mixing: Mixbox (c) 2022 Secret Weapons, CC BY-NC 4.0, https://scrtwpns.com/mixbox (non-commercial use)";

#[derive(Clone, Copy, Debug, Default)]
pub struct MixboxMixer;

impl Mixer for MixboxMixer {
    const ID: &'static str = "mixbox-2.0";
    type State = [f32; 7];

    #[inline]
    fn encode(&self, srgb: [f32; 3]) -> [f32; 7] {
        mixbox::float_rgb_to_latent(&srgb.map(clamp01))
    }

    #[inline(always)]
    fn decode_srgb(&self, z: &[f32; 7]) -> [f32; 3] {
        mixbox::latent_to_float_rgb(z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blue_and_yellow_make_green() {
        let m = MixboxMixer;
        let b = m.encode([0.0, 33.0 / 255.0, 133.0 / 255.0]);
        let y = m.encode([254.0 / 255.0, 236.0 / 255.0, 0.0]);
        let mut z = [0.0f32; 7];
        for k in 0..7 {
            z[k] = 0.5 * b[k] + 0.5 * y[k];
        }
        let g = m.decode_srgb(&z);
        assert!(g[1] > g[0] && g[1] > g[2], "{g:?}");
        let back = m.decode_srgb(&m.encode([0.3, 0.6, 0.9]));
        assert!(back.iter().zip([0.3, 0.6, 0.9]).all(|(a, b)| (a - b).abs() < 1e-5), "{back:?}");
    }
}
