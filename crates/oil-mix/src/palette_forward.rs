//! Versioned evaluation of the same palette optics, without altering recipes.
use crate::palette::{PaletteError, PaletteN};

/// Preferred evaluation for new corrected palettes; saved jobs retain their tag.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ForwardDecoder {
    Reference,
    AlgebraicV1,
    #[default]
    ExpLutV1,
}

impl ForwardDecoder {
    pub fn tag(self) -> u32 {
        match self {
            Self::Reference => 0,
            Self::AlgebraicV1 => 1,
            Self::ExpLutV1 => 2,
        }
    }
    pub fn from_tag(tag: u32) -> Option<Self> {
        match tag {
            0 => Some(Self::Reference),
            1 => Some(Self::AlgebraicV1),
            2 => Some(Self::ExpLutV1),
            _ => None,
        }
    }
}

const TABLE_SAMPLES: usize = 513;
const TABLE_MIN: f64 = -1.6;
const TABLE_MAX: f64 = 1.6;

/// A prepared decoder owns an exact copy of its immutable palette. Both accelerated
/// choices preserve pure endpoints; floating-point output can differ from reference.
pub struct PaletteForwardN<const N: usize, const B: usize = 81> {
    palette: PaletteN<N, B>,
    method: ForwardDecoder,
    basis: [[f64; 4]; B],
    table: Option<Box<[f64; TABLE_SAMPLES]>>,
}

impl<const N: usize, const B: usize> PaletteForwardN<N, B> {
    pub fn new(palette: &PaletteN<N, B>, method: ForwardDecoder) -> Self {
        let basis = std::array::from_fn(|band| {
            let t = band as f64 / (B - 1) as f64;
            let u = 1. - t;
            [u * u * u, 3. * t * (u * u), 3. * t * t * u, t * t * t]
        });
        let table = (method == ForwardDecoder::ExpLutV1).then(|| {
            Box::new(std::array::from_fn(|i| {
                oil_math::exp(
                    TABLE_MIN + (TABLE_MAX - TABLE_MIN) * i as f64 / (TABLE_SAMPLES - 1) as f64,
                )
            }))
        });
        Self {
            palette: palette.clone(),
            method,
            basis,
            table,
        }
    }
    pub fn method(&self) -> ForwardDecoder {
        self.method
    }
    pub fn palette(&self) -> &PaletteN<N, B> {
        &self.palette
    }
    /// Prepared basis/table bytes, excluding the cloned optical model and strings.
    pub fn auxiliary_bytes(&self) -> usize {
        std::mem::size_of_val(&self.basis)
            + if self.table.is_some() {
                TABLE_SAMPLES * 8
            } else {
                0
            }
    }
    fn exponential(&self, x: f64) -> f64 {
        if let Some(table) = &self.table {
            if (TABLE_MIN..=TABLE_MAX).contains(&x) {
                let position =
                    (x - TABLE_MIN) * ((TABLE_SAMPLES - 1) as f64 / (TABLE_MAX - TABLE_MIN));
                let index = (position as usize).min(TABLE_SAMPLES - 2);
                let fraction = position - index as f64;
                return table[index] + fraction * (table[index + 1] - table[index]);
            }
        }
        oil_math::exp(x)
    }
    pub fn reflectance(&self, amounts: [f64; N]) -> Result<[f64; B], PaletteError> {
        let recipe = self.palette.recipe(amounts)?;
        if self.method == ForwardDecoder::Reference {
            return Ok(recipe.reflectance());
        }
        let c = recipe.proportions();
        let mut controls = [0.; 4];
        let has_correction = !self.palette.pair_controls().is_empty();
        if has_correction {
            let mut pair = 0;
            for i in 0..N {
                for j in i + 1..N {
                    let weight = 4. * c[i] * c[j];
                    for (a, value) in controls.iter_mut().zip(self.palette.pair_controls()[pair]) {
                        *a += weight * value;
                    }
                    pair += 1;
                }
            }
        }
        Ok(std::array::from_fn(|band| {
            let mut k = 0.;
            let mut s = 0.;
            for (paint, &fraction) in c.iter().enumerate() {
                k += fraction * self.palette.absorption()[band][paint];
                s += fraction * self.palette.scattering()[band][paint];
            }
            let r = ochrell::kubelka_munk::reflectance(k / s);
            if !has_correction {
                return r;
            }
            let shift: f64 = controls
                .iter()
                .zip(self.basis[band])
                .map(|(a, b)| a * b)
                .sum();
            if shift == 0. || r == 0. || r == 1. {
                r
            } else {
                r / (r + (1. - r) * self.exponential(-shift))
            }
        }))
    }
    pub fn decode_linear(&self, amounts: [f64; N]) -> Result<[f64; 3], PaletteError> {
        let mut rgb = [0.; 3];
        for (r, weights) in self
            .reflectance(amounts)?
            .iter()
            .zip(self.palette.display_projection())
        {
            for (value, weight) in rgb.iter_mut().zip(weights) {
                *value += r * weight;
            }
        }
        Ok(rgb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::synthetic_four;
    #[test]
    fn table_interval_and_fallback_are_bounded_and_continuous() {
        let decoder = PaletteForwardN::new(synthetic_four(), ForwardDecoder::ExpLutV1);
        for i in 0..=10000 {
            let x = TABLE_MIN + (TABLE_MAX - TABLE_MIN) * i as f64 / 10000.;
            let exact = oil_math::exp(x);
            let actual = decoder.exponential(x);
            assert!(((actual - exact) / exact).abs() < 5e-6);
        }
        for x in [-2., 2., -20., 20.] {
            assert_eq!(decoder.exponential(x).to_bits(), oil_math::exp(x).to_bits());
        }
        assert_eq!(decoder.exponential(0.).to_bits(), 1.0f64.to_bits());
    }
}
