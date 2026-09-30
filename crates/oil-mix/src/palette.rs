//! Opt-in four-paint recipe states with a prepared Ochrell decoder. Target
//! matching is an authoring operation; mixing and display never solve an inverse.
use crate::{srgb, Mixer};
pub use ::ochrell::palette::{synthetic_four, AmountBasis, Palette, PaletteError, PaletteMetadata};
pub use ::ochrell::palette_lut::{LutError, PaletteLut};
use ::ochrell::{conversion, palette_match::ColorMatcher};

pub struct PaletteMixer {
    table: PaletteLut<'static>,
    matcher: ColorMatcher<'static>,
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
pub struct TargetMatch {
    pub recipe: [f32; 4],
    /// Actual prepared-decoder display, with the engine's portable transfer.
    pub achieved_srgb: [f32; 3],
    /// Error of the f32 recipe through the prepared decoder, OKLab times 100.
    pub error_ok100: f64,
    /// Direct reference solver's error, before f32 storage and LUT approximation.
    pub reference_error_ok100: f64,
    pub evaluations: usize,
}

impl PaletteMixer {
    pub fn new(table: PaletteLut<'static>) -> Result<Self, PaletteError> {
        let matcher = ColorMatcher::with_cbrt(table.palette(), oil_math::cbrt)?.into_owned();
        Ok(Self { table, matcher })
    }
    pub fn from_bytes(palette: &[u8], table: &[u8]) -> Result<Self, PrepareError> {
        let p = Palette::from_bytes(palette).map_err(PrepareError::Palette)?;
        let table = PaletteLut::from_bytes(&p, table)
            .map_err(PrepareError::Table)?
            .into_owned();
        Self::new(table).map_err(PrepareError::Palette)
    }
    pub fn palette(&self) -> &Palette {
        self.table.palette()
    }
    pub fn table(&self) -> &PaletteLut<'static> {
        &self.table
    }

    /// Normalize nonnegative relative amounts once for persistent f32 storage.
    pub fn recipe(&self, amounts: [f64; 4]) -> Result<[f32; 4], PaletteError> {
        Ok(self
            .palette()
            .recipe(amounts)?
            .proportions()
            .map(|v| v as f32))
    }
    pub fn paint(&self, name: &str) -> Result<[f32; 4], PaletteError> {
        Ok(self.palette().paint(name)?.proportions().map(|v| v as f32))
    }
    pub fn match_target(&self, target: [f32; 3]) -> Result<TargetMatch, PaletteError> {
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
        Ok(TargetMatch {
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

impl Mixer for PaletteMixer {
    const ID: &'static str = "ochrell-palette-1";
    type State = [f32; 4];
    fn encode(&self, srgb: [f32; 3]) -> Self::State {
        self.match_target(srgb)
            .expect("finite unit-range target RGB")
            .recipe
    }
    fn decode_linear_rgb(&self, z: &Self::State) -> [f32; 3] {
        let recipe = self
            .palette()
            .recipe(z.map(|v| v as f64))
            .expect("valid palette state");
        conversion::gamut_map(self.table.decode_linear(&recipe).expect("matching palette"))
            .map(|v| v as f32)
    }
    fn decode_srgb(&self, z: &Self::State) -> [f32; 3] {
        self.decode_linear_rgb(z).map(srgb::from_linear)
    }
    fn streak(&self, base: &mut Self::State, delta: &Self::State, amplitude: f32) {
        let previous = *base;
        let mut scale = 1.0f32;
        for k in 0..4 {
            let d = amplitude * delta[k];
            if d < 0.0 {
                scale = scale.min((base[k] / -d).max(0.0));
            }
        }
        for k in 0..4 {
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
