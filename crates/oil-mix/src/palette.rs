//! Palette recipes with prepared four-paint or direct 1-16-paint decoding. Target
//! matching is an authoring operation; mixing and display never solve an inverse.
use crate::{srgb, Mixer};
pub use ::ochrell::palette::{
    synthetic_four, AmountBasis, Palette, PaletteError, PaletteMetadata, PaletteMetadataN,
    PaletteN, MAX_PAINTS,
};
pub use ::ochrell::palette_lut::{LutError, PaletteLut};
use ::ochrell::{conversion, palette_match::ColorMatcherN};

pub struct PaletteMixerN<const N: usize, const PREPARED: bool = false> {
    table: Option<PaletteLut<'static>>,
    matcher: ColorMatcherN<'static, N>,
}

#[derive(Debug)]
pub enum PrepareError {
    Palette(PaletteError),
    Table(LutError),
}
impl std::fmt::Display for PrepareError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Palette(e) => write!(f, "{e}"),
            Self::Table(e) => write!(f, "{e}"),
        }
    }
}
impl std::error::Error for PrepareError {}

#[derive(Clone, Copy, Debug)]
pub struct TargetMatchN<const N: usize> {
    pub recipe: [f32; N],
    /// Actual selected-decoder display, with the engine's portable transfer.
    pub achieved_srgb: [f32; 3],
    /// Error of the f32 recipe through the selected decoder, OKLab times 100.
    pub error_ok100: f64,
    /// Direct reference solver's error, before f32 storage and LUT approximation.
    pub reference_error_ok100: f64,
    pub evaluations: usize,
}

pub type PaletteMixer = PaletteMixerN<4, true>;
pub type TargetMatch = TargetMatchN<4>;

impl PaletteMixerN<4, true> {
    pub fn new(table: PaletteLut<'static>) -> Result<Self, PaletteError> {
        let matcher = ColorMatcherN::with_cbrt(table.palette(), oil_math::cbrt)?.into_owned();
        Ok(Self {
            table: Some(table),
            matcher,
        })
    }
    /// Prepared four-paint construction always supplies a table.
    pub fn table(&self) -> &PaletteLut<'static> {
        self.table.as_ref().unwrap()
    }
}

impl<const N: usize> PaletteMixerN<N> {
    /// Own the optical model and evaluate mixtures directly. No exponential LUT.
    pub fn direct(palette: PaletteN<N>) -> Result<Self, PaletteError> {
        let matcher = ColorMatcherN::with_cbrt(&palette, oil_math::cbrt)?.into_owned();
        Ok(Self {
            table: None,
            matcher,
        })
    }
    pub fn from_palette_bytes(palette: &[u8]) -> Result<Self, PrepareError> {
        Self::direct(PaletteN::from_bytes(palette).map_err(PrepareError::Palette)?)
            .map_err(PrepareError::Palette)
    }
}

impl<const N: usize, const PREPARED: bool> PaletteMixerN<N, PREPARED> {
    /// Prepared mode requires a four-paint OPL1; direct mode requires an empty
    /// table section, so a saved decoder cannot silently change on reload.
    pub fn from_bytes(palette: &[u8], table: &[u8]) -> Result<Self, PrepareError> {
        if (PREPARED && (N != 4 || table.is_empty())) || (!PREPARED && !table.is_empty()) {
            return Err(PrepareError::Palette(PaletteError::InvalidFormat));
        }
        let p = PaletteN::<N>::from_bytes(palette).map_err(PrepareError::Palette)?;
        let prepared = if PREPARED {
            let four = Palette::from_bytes(palette).map_err(PrepareError::Palette)?;
            Some(
                PaletteLut::from_bytes(&four, table)
                    .map_err(PrepareError::Table)?
                    .into_owned(),
            )
        } else {
            None
        };
        let matcher = ColorMatcherN::with_cbrt(&p, oil_math::cbrt)
            .map_err(PrepareError::Palette)?
            .into_owned();
        Ok(Self {
            table: prepared,
            matcher,
        })
    }
    pub fn palette(&self) -> &PaletteN<N> {
        self.matcher.palette()
    }
    pub fn prepared_table(&self) -> Option<&PaletteLut<'static>> {
        self.table.as_ref()
    }

    /// Normalize nonnegative relative amounts once for persistent f32 storage.
    pub fn recipe(&self, amounts: [f64; N]) -> Result<[f32; N], PaletteError> {
        Ok(self
            .palette()
            .recipe(amounts)?
            .proportions()
            .map(|v| v as f32))
    }
    pub fn paint(&self, name: &str) -> Result<[f32; N], PaletteError> {
        Ok(self.palette().paint(name)?.proportions().map(|v| v as f32))
    }
    pub fn match_target(&self, target: [f32; 3]) -> Result<TargetMatchN<N>, PaletteError> {
        if target
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return Err(PaletteError::InvalidTarget);
        }
        let linear = target.map(|v| oil_math::srgb_to_linear(v as f64));
        let found = self.matcher.match_linear(linear)?;
        let recipe = found.recipe.proportions().map(|v| v as f32);
        let achieved = self.decode_linear_rgb(&recipe);
        Ok(TargetMatchN {
            recipe,
            achieved_srgb: achieved.map(srgb::from_linear),
            error_ok100: distance(achieved.map(|v| v as f64), linear),
            reference_error_ok100: found.error_ok100,
            evaluations: found.evaluations,
        })
    }
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let lab = |v| {
        conversion::mat(
            conversion::LAB,
            conversion::mat(conversion::LMS, v).map(oil_math::cbrt),
        )
    };
    let (a, b) = (lab(a), lab(b));
    100.0
        * a.into_iter()
            .zip(b)
            .map(|(a, b)| (a - b) * (a - b))
            .sum::<f64>()
            .sqrt()
}

impl<const N: usize, const PREPARED: bool> Mixer for PaletteMixerN<N, PREPARED> {
    const ID: &'static str = if PREPARED {
        "ochrell-palette-1"
    } else {
        "ochrell-palette-direct-2"
    };
    type State = [f32; N];
    fn encode(&self, srgb: [f32; 3]) -> Self::State {
        self.match_target(srgb)
            .expect("finite unit-range target RGB")
            .recipe
    }
    fn decode_linear_rgb(&self, z: &Self::State) -> [f32; 3] {
        let linear = if let Some(table) = &self.table {
            // The prepared constructor and decoder enforce N=4 and identity.
            let four = table
                .palette()
                .recipe(std::array::from_fn(|i| z[i] as f64))
                .expect("valid four-paint state");
            table.decode_linear(&four).expect("matching palette")
        } else {
            self.palette()
                .recipe(z.map(|v| v as f64))
                .expect("valid palette state")
                .decode_linear()
        };
        conversion::gamut_map(linear).map(|v| v as f32)
    }
    fn decode_srgb(&self, z: &Self::State) -> [f32; 3] {
        self.decode_linear_rgb(z).map(srgb::from_linear)
    }
    fn streak(&self, base: &mut Self::State, delta: &Self::State, amplitude: f32) {
        let previous = *base;
        let mut scale = 1.0f32;
        for k in 0..N {
            let d = amplitude * delta[k];
            if d < 0.0 {
                scale = scale.min((base[k] / -d).max(0.0));
            }
        }
        for k in 0..N {
            base[k] = (base[k] + amplitude * scale * delta[k]).max(0.0);
        }
        let total: f64 = base.iter().map(|v| *v as f64).sum();
        if total > 0.0 && total.is_finite() {
            for v in base {
                *v = (*v as f64 / total) as f32;
            }
        } else {
            *base = previous;
        }
    }
    fn is_valid(&self, z: &Self::State) -> bool {
        z.iter().all(|v| v.is_finite() && *v >= 0.0) && z.iter().any(|v| *v > 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn target_reports_prepared_error_and_streaks_remain_recipes() {
        let m = PaletteMixer::new(
            PaletteLut::build(synthetic_four(), 65)
                .unwrap()
                .into_owned(),
        )
        .unwrap();
        let found = m.match_target([0.0; 3]).unwrap();
        assert!(found.error_ok100 > 40.0);
        assert_eq!(found.achieved_srgb, m.decode_srgb(&found.recipe));
        assert_eq!(found.recipe, m.match_target([0.0; 3]).unwrap().recipe);
        assert!(m.match_target([f32::NAN, 0.0, 0.0]).is_err());
        let mut base = m.recipe([1.0, 2.0, 3.0, 4.0]).unwrap();
        for i in 0..10000 {
            m.streak(
                &mut base,
                &[0.5, -0.25, 0.25, -0.5],
                if i % 2 == 0 { 40.0 } else { -40.0 },
            );
            assert!(m.is_valid(&base));
            assert!((base.iter().sum::<f32>() - 1.0).abs() < 1e-6);
        }
    }
}
