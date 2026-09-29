//! The engine's only maths library (docs/plans/LIBRARY_PLAN.md, section 5).
//!
//! Every function is built from IEEE-754 basic operations (+ - * /, floor, exact casts), which are correctly
//! rounded on x86-64, aarch64 and wasm32, and Rust never contracts a*b+c into an FMA on its own. So the same
//! input gives the same bits on every host, which the platform libm does not: glibc's libm natively vs Rust's own
//! libm in WASM already differed on 0.23% of Storm Light pixels (docs/plans/SPIKE_LOG.txt).
//!
//! exp and sin/cos come from the kernel spike (`spikes/oilcore/src/math.rs`); ln, pow, cbrt, atan2 and the sRGB
//! transfer functions were added in L1. `sqrt`, `floor` and `ceil` are exact IEEE operations and may be used
//! directly from `std`.
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

const LN2_HI: f64 = 6.931_471_803_691_238_164_90e-01; // 32 significant bits: k * LN2_HI is exact
const LN2_LO: f64 = 1.908_214_929_270_587_700_02e-10;
const SQRT2: f64 = 1.414_213_562_373_095_048_80;
const PI: f64 = 3.141_592_653_589_793_115_997_96;
const PI_2: f64 = 1.570_796_326_794_896_557_998_98;

/// Natural logarithm. ln(0) = -inf, ln(x < 0) = NaN, ln(inf) = inf. Error a few ulp.
pub fn ln(x: f64) -> f64 {
    if x.is_nan() || x < 0.0 {
        return f64::NAN;
    }
    if x == 0.0 {
        return f64::NEG_INFINITY;
    }
    if x == f64::INFINITY {
        return x;
    }
    // x = m * 2^k with m in [sqrt(1/2), sqrt(2)); subnormals are scaled into the normal range first.
    let (x, mut k) = if x < f64::MIN_POSITIVE { (x * 18_014_398_509_481_984.0, -54_i64) } else { (x, 0) };
    let bits = x.to_bits();
    k += ((bits >> 52) & 0x7ff) as i64 - 1023;
    let mut m = f64::from_bits((bits & 0x000f_ffff_ffff_ffff) | 0x3ff0_0000_0000_0000);
    if m > SQRT2 {
        m *= 0.5;
        k += 1;
    }
    // ln m = 2 atanh(s), s = (m - 1) / (m + 1), |s| <= 0.1716: odd series to s^23 (relative error < 1e-17).
    let s = (m - 1.0) / (m + 1.0);
    let s2 = s * s;
    let mut p = 1.0 / 23.0;
    p = p * s2 + 1.0 / 21.0;
    p = p * s2 + 1.0 / 19.0;
    p = p * s2 + 1.0 / 17.0;
    p = p * s2 + 1.0 / 15.0;
    p = p * s2 + 1.0 / 13.0;
    p = p * s2 + 1.0 / 11.0;
    p = p * s2 + 1.0 / 9.0;
    p = p * s2 + 1.0 / 7.0;
    p = p * s2 + 1.0 / 5.0;
    p = p * s2 + 1.0 / 3.0;
    p = p * s2 + 1.0;
    let kf = k as f64;
    kf * LN2_HI + (kf * LN2_LO + 2.0 * s * p)
}

/// x^y for x >= 0 (exp(y ln x)); pow(0, y > 0) = 0, pow(x, 0) = 1, x < 0 gives NaN.
pub fn pow(x: f64, y: f64) -> f64 {
    if y == 0.0 {
        return 1.0;
    }
    if x == 0.0 {
        return if y > 0.0 { 0.0 } else { f64::INFINITY };
    }
    if x == 1.0 {
        return 1.0;
    }
    exp(y * ln(x))
}

/// Cube root, any sign. One Newton step on exp(ln|x| / 3): about 1 ulp.
pub fn cbrt(x: f64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        return x;
    }
    let a = x.abs();
    let mut r = exp(ln(a) / 3.0);
    r -= (r * r * r - a) / (3.0 * r * r);
    if x < 0.0 {
        -r
    } else {
        r
    }
}

/// Arc tangent. Two half-angle reductions to |t| < 0.11, then an odd series.
pub fn atan(x: f64) -> f64 {
    if x.is_nan() {
        return x;
    }
    if x.is_infinite() {
        return if x > 0.0 { PI_2 } else { -PI_2 };
    }
    let (sign, a) = if x < 0.0 { (-1.0, -x) } else { (1.0, x) };
    let (base, t) = if a > 1.0 { (PI_2, -1.0 / a) } else { (0.0, a) };
    // atan(t) = 2 atan(t / (1 + sqrt(1 + t^2))), applied twice: |t| <= tan(pi/16) ~ 0.199.
    let t = t / (1.0 + (1.0 + t * t).sqrt());
    let t = t / (1.0 + (1.0 + t * t).sqrt());
    let t2 = t * t;
    let mut p = -1.0 / 23.0;
    p = p * t2 + 1.0 / 21.0;
    p = p * t2 - 1.0 / 19.0;
    p = p * t2 + 1.0 / 17.0;
    p = p * t2 - 1.0 / 15.0;
    p = p * t2 + 1.0 / 13.0;
    p = p * t2 - 1.0 / 11.0;
    p = p * t2 + 1.0 / 9.0;
    p = p * t2 - 1.0 / 7.0;
    p = p * t2 + 1.0 / 5.0;
    p = p * t2 - 1.0 / 3.0;
    p = p * t2 + 1.0;
    sign * (base + 4.0 * t * p)
}

/// atan2(y, x) in (-pi, pi], with atan2(0, 0) = 0.
pub fn atan2(y: f64, x: f64) -> f64 {
    if x.is_nan() || y.is_nan() {
        return f64::NAN;
    }
    if x == 0.0 {
        return if y > 0.0 {
            PI_2
        } else if y < 0.0 {
            -PI_2
        } else {
            0.0
        };
    }
    let a = atan(y / x);
    if x > 0.0 {
        a
    } else if y >= 0.0 {
        a + PI
    } else {
        a - PI
    }
}

/// sRGB transfer (IEC 61966-2-1), encoded value in [0, 1] to linear light.
pub fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.040_45 {
        c / 12.92
    } else {
        pow((c + 0.055) / 1.055, 2.4)
    }
}

/// Inverse sRGB transfer, linear light to the encoded value.
pub fn linear_to_srgb(l: f64) -> f64 {
    if l <= 0.003_130_8 {
        12.92 * l
    } else {
        1.055 * pow(l, 1.0 / 2.4) - 0.055
    }
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

    fn rel(a: f64, b: f64) -> f64 {
        if a == b {
            0.0
        } else {
            ((a - b) / b).abs()
        }
    }

    #[test]
    fn ln_pow_cbrt_close_to_libm() {
        let (mut wl, mut wp, mut wc) = (0.0f64, 0.0f64, 0.0f64);
        let mut x = 1e-300f64;
        while x < 1e300 {
            wl = wl.max((ln(x) - x.ln()).abs() / x.ln().abs().max(1.0));
            wc = wc.max(rel(cbrt(x), x.cbrt())).max(rel(cbrt(-x), (-x).cbrt()));
            x *= 1.137;
        }
        let mut x = 0.001f64;
        while x < 1.0 {
            for y in [2.4, 1.0 / 2.4, 0.5, 3.0, 0.1] {
                wp = wp.max(rel(pow(x, y), x.powf(y)));
            }
            x += 0.000_731;
        }
        assert!(wl < 4e-16, "ln error {wl}");
        // exp(y ln x) loses about |y ln x| ulp: ~10 ulp at |y ln x| ~ 20, far below f32 resolution.
        assert!(wp < 5e-15, "pow relative error {wp}");
        assert!(wc < 4e-16, "cbrt relative error {wc}");
        assert_eq!(ln(1.0), 0.0);
        assert_eq!(ln(0.0), f64::NEG_INFINITY);
        assert!(ln(-1.0).is_nan());
        assert!((ln(5e-320) - 5e-320f64.ln()).abs() < 1e-12);
        assert_eq!(pow(0.0, 2.4), 0.0);
        assert_eq!(pow(0.3, 0.0), 1.0);
    }

    #[test]
    fn atan2_close_to_libm() {
        let mut worst = 0.0f64;
        for i in -200..=200 {
            for j in -200..=200 {
                let (y, x) = (i as f64 * 0.37, j as f64 * 0.53);
                worst = worst.max((atan2(y, x) - y.atan2(x)).abs());
            }
        }
        assert!(worst < 1e-15, "atan2 error {worst}");
        assert_eq!(atan2(0.0, 0.0), 0.0);
        assert_eq!(atan2(1.0, 0.0), PI_2);
        assert_eq!(atan(f64::INFINITY), PI_2);
    }

    #[test]
    fn srgb_round_trip() {
        for i in 0..=1000 {
            let c = i as f64 / 1000.0;
            let back = linear_to_srgb(srgb_to_linear(c));
            assert!((back - c).abs() < 1e-14, "{c} -> {back}");
            let lib = if c <= 0.040_45 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
            assert!((srgb_to_linear(c) - lib).abs() < 1e-15);
        }
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
