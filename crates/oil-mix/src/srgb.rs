//! The engine's sRGB transfer for per-pixel work: table interpolation instead of `pow`.
//!
//! The kernel converts display colours to linear light and back for every painted pixel (coverage compositing).
//! Evaluating `oil_math::pow` there costs about 40 ns a call, nine calls a pixel. These tables are built once from
//! `oil_math` (so they are identical on every host) and interpolated linearly in f32:
//! - sRGB to linear: exact `c / 12.92` on the linear segment, exact `oil_math` up to the first table knot past
//!   the kink at 0.04045, then 4096 equal steps over [0, 1].
//! - linear to sRGB: exact `12.92 l` on the linear segment, exact `oil_math` up to the first knot past the kink at
//!   0.0031308, then 4096 steps in sqrt(l) (steep near 0, smooth in that variable).
//!
//! Measured error against `oil_math` over 100,001 points: below 1e-7 both ways.
//!
//! Both are defined on [0, 1] and clamp outside it.
use std::sync::OnceLock;

const N: usize = 4096;

struct Tables {
    to_lin: Vec<f32>,
    from_lin: Vec<f32>,
}

fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| Tables {
        to_lin: (0..=N).map(|i| oil_math::srgb_to_linear(i as f64 / N as f64) as f32).collect(),
        from_lin: (0..=N)
            .map(|i| {
                let t = i as f64 / N as f64;
                oil_math::linear_to_srgb(t * t) as f32
            })
            .collect(),
    })
}

#[inline(always)]
fn lerp_table(t: &[f32], x: f32) -> f32 {
    if x >= 1.0 || x.is_nan() {
        return t[N]; // exact at the top end: a + 1 * (b - a) need not round to b
    }
    let x = if x > 0.0 { x } else { 0.0 };
    let f = x * N as f32;
    let i = (f as usize).min(N - 1);
    let frac = f - i as f32;
    t[i] + frac * (t[i + 1] - t[i])
}

/// First table knots past the transfer function's kink: interpolating across the kink would mix both branches.
const TO_LIN_KNOT: f32 = 166.0 / N as f32; // 0.04045 * 4096 = 165.7
const FROM_LIN_KNOT: f32 = (230.0 / N as f32) * (230.0 / N as f32); // sqrt(0.0031308) * 4096 = 229.2

/// sRGB-encoded value in [0, 1] to linear light.
#[inline]
pub fn to_linear(c: f32) -> f32 {
    if c <= 0.040_45 {
        return if c > 0.0 { c / 12.92 } else { 0.0 };
    }
    if c < TO_LIN_KNOT {
        return oil_math::srgb_to_linear(c as f64) as f32;
    }
    lerp_table(&tables().to_lin, c)
}

/// Linear light in [0, 1] to the sRGB-encoded value.
#[inline]
pub fn from_linear(l: f32) -> f32 {
    if l <= 0.003_130_8 {
        return if l > 0.0 { 12.92 * l } else { 0.0 };
    }
    if l < FROM_LIN_KNOT {
        return oil_math::linear_to_srgb(l as f64) as f32;
    }
    lerp_table(&tables().from_lin, l.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_match_the_exact_transfer() {
        let (mut e1, mut e2) = (0.0f64, 0.0f64);
        for i in 0..=100_000 {
            let x = (i as f64 / 100_000.0) as f32; // both sides see the same f32 input
            e1 = e1.max((to_linear(x) as f64 - oil_math::srgb_to_linear(x as f64)).abs());
            e2 = e2.max((from_linear(x) as f64 - oil_math::linear_to_srgb(x as f64)).abs());
        }
        assert!(e1 < 1e-7, "to_linear error {e1}");
        assert!(e2 < 1e-7, "from_linear error {e2}");
        assert_eq!(to_linear(0.0), 0.0);
        assert_eq!(to_linear(1.0), 1.0);
        assert_eq!(from_linear(0.0), 0.0);
        assert!((from_linear(1.0) - 1.0).abs() < 1e-6);
        assert_eq!(to_linear(-0.5), 0.0);
        assert_eq!(from_linear(2.0), from_linear(1.0));
    }
}
