//! The engine's only maths library (docs/plans/LIBRARY_PLAN.md, section 5).
//!
//! Every function is built from IEEE-754 basic operations (+ - * /, floor, exact casts), which are correctly
//! rounded on x86-64, aarch64 and wasm32, and Rust never contracts a*b+c into an FMA on its own. So the same
//! input gives the same bits on every host, which the platform libm does not: glibc's libm natively vs Rust's own
//! libm in WASM already differed on 0.23% of Storm Light pixels (docs/plans/SPIKE_LOG.txt).
//!
//! L0 carries exp and sin/cos from the kernel spike (`spikes/oilcore/src/math.rs`); L1 adds pow, cbrt, atan2 and
//! the sRGB transfer functions.
#![forbid(unsafe_code)]
// fdlibm's published decimal constants, kept verbatim so the bits match the kernel spike.
#![allow(clippy::excessive_precision, clippy::approx_constant)]

/// exp(x) in f64. Relative error below 1e-14 against a correctly rounded exp over [-80, 20]; 0 below -745
/// and +inf above 709.
pub fn exp(x: f64) -> f64 {
    if x < -745.0 {
        return 0.0;
    }
    if x > 709.0 {
        return f64::INFINITY;
    }
    const LN2_HI: f64 = 6.931_471_803_691_238_164_90e-01;
    const LN2_LO: f64 = 1.908_214_929_270_587_700_02e-10;
    const INV_LN2: f64 = 1.442_695_040_888_963_387_00e+00;
    let k = (x * INV_LN2 + 0.5).floor();
    let r = (x - k * LN2_HI) - k * LN2_LO; // |r| <= ~0.35
    // Taylor series to r^12: relative error < 1e-17 on |r| <= 0.35.
    let mut p = 1.0 / 479_001_600.0;
    p = p * r + 1.0 / 39_916_800.0;
    p = p * r + 1.0 / 3_628_800.0;
    p = p * r + 1.0 / 362_880.0;
    p = p * r + 1.0 / 40_320.0;
    p = p * r + 1.0 / 5_040.0;
    p = p * r + 1.0 / 720.0;
    p = p * r + 1.0 / 120.0;
    p = p * r + 1.0 / 24.0;
    p = p * r + 1.0 / 6.0;
    p = p * r + 0.5;
    p = p * r + 1.0;
    p = p * r + 1.0;
    // Scale by 2^k with exact multiplications by powers of two.
    let mut ki = k as i32;
    let mut s = p;
    while ki > 0 {
        let step = ki.min(1000);
        s *= f64::from_bits(((1023 + step as i64) as u64) << 52);
        ki -= step;
    }
    while ki < 0 {
        let step = ki.max(-1000);
        s *= f64::from_bits(((1023 + step as i64) as u64) << 52);
        ki -= step;
    }
    s
}

/// sin(x) in f64, for |x| below about 1e5 (the engine's arguments). Absolute error below 1e-13.
pub fn sin(x: f64) -> f64 {
    let (q, r) = reduce_half_pi(x);
    match q {
        0 => sin_kernel(r),
        1 => cos_kernel(r),
        2 => -sin_kernel(r),
        _ => -cos_kernel(r),
    }
}

/// cos(x) in f64, for |x| below about 1e5. Absolute error below 1e-13.
pub fn cos(x: f64) -> f64 {
    let (q, r) = reduce_half_pi(x);
    match q {
        0 => cos_kernel(r),
        1 => -sin_kernel(r),
        2 => -cos_kernel(r),
        _ => sin_kernel(r),
    }
}

/// exp for f32 callers: evaluated in f64, rounded once.
#[inline]
pub fn expf(x: f32) -> f32 {
    exp(x as f64) as f32
}

/// sin for f32 callers: evaluated in f64, rounded once.
#[inline]
pub fn sinf(x: f32) -> f32 {
    sin(x as f64) as f32
}

/// cos for f32 callers: evaluated in f64, rounded once.
#[inline]
pub fn cosf(x: f32) -> f32 {
    cos(x as f64) as f32
}

/// x = q*pi/2 + r with |r| <= pi/4, q in 0..4 (three-part Cody-Waite reduction).
#[inline]
fn reduce_half_pi(x: f64) -> (i64, f64) {
    const INV_PIO2: f64 = 6.366_197_723_675_813_824_33e-01;
    const PIO2_1: f64 = 1.570_796_326_734_125_614_17e+00;
    const PIO2_2: f64 = 6.077_100_506_506_192_249_32e-11;
    const PIO2_3: f64 = 2.022_266_248_795_950_631_54e-21;
    let k = (x * INV_PIO2 + 0.5).floor();
    let r = ((x - k * PIO2_1) - k * PIO2_2) - k * PIO2_3;
    ((k as i64).rem_euclid(4), r)
}

#[inline]
fn sin_kernel(r: f64) -> f64 {
    // sin(r) = r - r^3 (1/3! - r^2/5! + ... - r^14/17!)
    let r2 = r * r;
    let mut p = -1.0 / 355_687_428_096_000.0;
    p = p * r2 + 1.0 / 1_307_674_368_000.0;
    p = p * r2 - 1.0 / 6_227_020_800.0;
    p = p * r2 + 1.0 / 39_916_800.0;
    p = p * r2 - 1.0 / 362_880.0;
    p = p * r2 + 1.0 / 5_040.0;
    p = p * r2 - 1.0 / 120.0;
    p = p * r2 + 1.0 / 6.0;
    r - r * r2 * p
}

#[inline]
fn cos_kernel(r: f64) -> f64 {
    let r2 = r * r;
    let mut p = 1.0 / 20_922_789_888_000.0;
    p = p * r2 - 1.0 / 87_178_291_200.0;
    p = p * r2 + 1.0 / 479_001_600.0;
    p = p * r2 - 1.0 / 3_628_800.0;
    p = p * r2 + 1.0 / 40_320.0;
    p = p * r2 - 1.0 / 720.0;
    p = p * r2 + 1.0 / 24.0;
    p = p * r2 - 0.5;
    1.0 + r2 * p
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // the platform libm is the accuracy reference here, never an output
mod tests {
    use super::*;

    #[test]
    fn exp_close_to_libm() {
        let mut worst: f64 = 0.0;
        let mut x = -80.0f64;
        while x < 20.0 {
            worst = worst.max(((exp(x) - x.exp()) / x.exp()).abs());
            x += 0.017_31;
        }
        assert!(worst < 1e-14, "exp relative error {worst}");
        assert_eq!(exp(-800.0), 0.0);
        assert_eq!(exp(800.0), f64::INFINITY);
        assert_eq!(exp(0.0), 1.0);
    }

    #[test]
    fn sin_cos_close_to_libm() {
        let mut worst: f64 = 0.0;
        let mut x = -400.0f64;
        while x < 400.0 {
            worst = worst.max((sin(x) - x.sin()).abs()).max((cos(x) - x.cos()).abs());
            x += 0.013_79;
        }
        assert!(worst < 1e-13, "sin/cos absolute error {worst}");
    }
}
