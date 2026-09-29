//! Transcendentals used by the kernel.
//! Default: the platform libm (on Linux: glibc expf/sinf, the same functions the C kernel calls -> bit parity with C).
//! Feature `detmath`: our own exp/sin built only from IEEE-754 basic operations (+ - * / floor) in f64, rounded to f32.
//! Basic operations are correctly rounded on every IEEE target (x86-64, aarch64, wasm32) and Rust never contracts
//! a*b+c into FMA on its own, so with `detmath` native and WASM builds produce identical bits.

#[cfg(not(feature = "detmath"))]
#[inline(always)]
pub fn expf(x: f32) -> f32 {
    x.exp()
}

#[cfg(not(feature = "detmath"))]
#[inline(always)]
pub fn sinf(x: f32) -> f32 {
    x.sin()
}

#[cfg(feature = "detmath")]
#[inline(always)]
pub fn expf(x: f32) -> f32 {
    det_exp(x as f64) as f32
}

#[cfg(feature = "detmath")]
#[inline(always)]
pub fn sinf(x: f32) -> f32 {
    det_sin(x as f64) as f32
}

#[allow(dead_code)]
#[inline(always)]
fn floor64(x: f64) -> f64 {
    x.floor() // wasm: f64.floor instruction; x86: roundsd -> exact everywhere
}

#[allow(dead_code)]
pub fn det_exp(x: f64) -> f64 {
    if x < -745.0 {
        return 0.0;
    }
    if x > 709.0 {
        return f64::INFINITY;
    }
    const LN2_HI: f64 = 6.93147180369123816490e-01;
    const LN2_LO: f64 = 1.90821492927058770002e-10;
    const INV_LN2: f64 = 1.44269504088896338700e+00;
    let k = floor64(x * INV_LN2 + 0.5);
    let r = (x - k * LN2_HI) - k * LN2_LO; // |r| <= ~0.35
    // Taylor to r^12: error < 1e-17 relative on |r| <= 0.35
    let mut p = 1.0 / 479001600.0;
    p = p * r + 1.0 / 39916800.0;
    p = p * r + 1.0 / 3628800.0;
    p = p * r + 1.0 / 362880.0;
    p = p * r + 1.0 / 40320.0;
    p = p * r + 1.0 / 5040.0;
    p = p * r + 1.0 / 720.0;
    p = p * r + 1.0 / 120.0;
    p = p * r + 1.0 / 24.0;
    p = p * r + 1.0 / 6.0;
    p = p * r + 0.5;
    p = p * r + 1.0;
    p = p * r + 1.0;
    // scale by 2^k with exact multiplications by powers of two
    let mut ki = k as i32;
    let mut s = p;
    while ki > 0 {
        let step = if ki > 1000 { 1000 } else { ki };
        s *= f64::from_bits(((1023 + step as i64) as u64) << 52);
        ki -= step;
    }
    while ki < 0 {
        let step = if ki < -1000 { -1000 } else { ki };
        s *= f64::from_bits(((1023 + step as i64) as u64) << 52);
        ki -= step;
    }
    s
}

#[allow(dead_code)]
pub fn det_sin(x: f64) -> f64 {
    // reduce by pi/2 (Cody-Waite, 3 parts); fine for |x| < ~1e5, which covers the kernel's arguments
    const INV_PIO2: f64 = 6.36619772367581382433e-01;
    const PIO2_1: f64 = 1.57079632673412561417e+00;
    const PIO2_2: f64 = 6.07710050650619224932e-11;
    const PIO2_3: f64 = 2.02226624879595063154e-21;
    let k = floor64(x * INV_PIO2 + 0.5);
    let r = ((x - k * PIO2_1) - k * PIO2_2) - k * PIO2_3; // |r| <= pi/4
    let q = (k as i64).rem_euclid(4);
    let r2 = r * r;
    let sin_r = {
        // sin(r) = r - r^3 (1/3! - r^2/5! + ... - r^14/17!)
        let mut p = -1.0 / 355687428096000.0;
        p = p * r2 + 1.0 / 1307674368000.0;
        p = p * r2 - 1.0 / 6227020800.0;
        p = p * r2 + 1.0 / 39916800.0;
        p = p * r2 - 1.0 / 362880.0;
        p = p * r2 + 1.0 / 5040.0;
        p = p * r2 - 1.0 / 120.0;
        p = p * r2 + 1.0 / 6.0;
        r - r * r2 * p
    };
    let cos_r = {
        let mut p = 1.0 / 20922789888000.0;
        p = p * r2 - 1.0 / 87178291200.0;
        p = p * r2 + 1.0 / 479001600.0;
        p = p * r2 - 1.0 / 3628800.0;
        p = p * r2 + 1.0 / 40320.0;
        p = p * r2 - 1.0 / 720.0;
        p = p * r2 + 1.0 / 24.0;
        p = p * r2 - 0.5;
        1.0 + r2 * p
    };
    match q {
        0 => sin_r,
        1 => cos_r,
        2 => -sin_r,
        _ => -cos_r,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exp_sin_close() {
        let mut maxe: f64 = 0.0;
        let mut maxs: f64 = 0.0;
        let mut x = -80.0f64;
        while x < 20.0 {
            maxe = maxe.max(((det_exp(x) - x.exp()) / x.exp()).abs());
            x += 0.01731;
        }
        let mut x = -400.0f64;
        while x < 400.0 {
            maxs = maxs.max((det_sin(x) - x.sin()).abs());
            x += 0.01379;
        }
        assert!(maxe < 1e-14, "exp rel err {}", maxe);
        assert!(maxs < 1e-13, "sin abs err {}", maxs);
    }
}
