//! Colour for the planner: CIELAB (D65, v1's matrices), mixes made with the active mixer, and palette snapping.
//!
//! Two Lab paths: `lab` (f64, `oil-math`) for the per-stroke colour pipeline, and `lab_fast` (f32, sRGB table and a
//! bit-seeded Newton cube root) for error planes, which convert every pixel of a region per pass. Both use only
//! IEEE basic operations and `oil-math`, so every host gives the same bits.
use oil_mix::{Mixer, State};

const M_RGB2XYZ: [[f64; 3]; 3] = [[0.4124564, 0.3575761, 0.1804375], [0.2126729, 0.7151522, 0.0721750], [0.0193339, 0.1191920, 0.9503041]];
const M_XYZ2RGB: [[f64; 3]; 3] = [[3.2404542, -1.5371385, -0.4985314], [-0.9692660, 1.8760108, 0.0415560], [0.0556434, -0.2040259, 1.0572252]];
const WHITE: [f64; 3] = [0.95047, 1.0, 1.08883];

fn f_lab(t: f64) -> f64 {
    if t > 0.008856 {
        oil_math::cbrt(t)
    } else {
        7.787 * t + 16.0 / 116.0
    }
}

/// sRGB in [0, 1] to CIELAB.
pub fn lab(rgb: [f32; 3]) -> [f64; 3] {
    let lin = rgb.map(|c| oil_math::srgb_to_linear(c.clamp(0.0, 1.0) as f64));
    let xyz: [f64; 3] = [0, 1, 2].map(|r| (M_RGB2XYZ[r][0] * lin[0] + M_RGB2XYZ[r][1] * lin[1] + M_RGB2XYZ[r][2] * lin[2]) / WHITE[r]);
    let f = xyz.map(f_lab);
    [116.0 * f[1] - 16.0, 500.0 * (f[0] - f[1]), 200.0 * (f[1] - f[2])]
}

/// CIELAB to sRGB, clipped to the gamut in linear light.
pub fn lab_to_rgb(lab: [f64; 3]) -> [f32; 3] {
    let fy = (lab[0] + 16.0) / 116.0;
    let fx = fy + lab[1] / 500.0;
    let fz = fy - lab[2] / 200.0;
    let finv = |f: f64| {
        let f3 = f * f * f;
        if f3 > 0.008856 {
            f3
        } else {
            (f - 16.0 / 116.0) / 7.787
        }
    };
    let xyz = [finv(fx) * WHITE[0], finv(fy) * WHITE[1], finv(fz) * WHITE[2]];
    [0, 1, 2].map(|r| {
        let l = M_XYZ2RGB[r][0] * xyz[0] + M_XYZ2RGB[r][1] * xyz[1] + M_XYZ2RGB[r][2] * xyz[2];
        oil_math::linear_to_srgb(l.clamp(0.0, 1.0)) as f32
    })
}

/// Cube root of x > 0 in f32: a bit-level first guess, then three Newton steps (about 1 ulp).
#[inline(always)]
fn cbrt_fast(x: f32) -> f32 {
    let mut y = f32::from_bits(x.to_bits() / 3 + 709_921_077);
    for _ in 0..3 {
        y = (2.0 * y + x / (y * y)) * (1.0 / 3.0);
    }
    y
}

/// Fast sRGB to CIELAB for error planes (f32).
#[inline(always)]
pub fn lab_fast(rgb: [f32; 3]) -> [f32; 3] {
    let lin = rgb.map(oil_mix::srgb::to_linear);
    let f = |r: usize| {
        let t = ((M_RGB2XYZ[r][0] as f32) * lin[0] + (M_RGB2XYZ[r][1] as f32) * lin[1] + (M_RGB2XYZ[r][2] as f32) * lin[2]) / WHITE[r] as f32;
        if t > 0.008856 {
            cbrt_fast(t)
        } else {
            7.787 * t + 16.0 / 116.0
        }
    };
    let (fx, fy, fz) = (f(0), f(1), f(2));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

#[inline(always)]
pub fn dist_fast(a: [f32; 3], b: [f32; 3]) -> f32 {
    let (d0, d1, d2) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    (d0 * d0 + d1 * d1 + d2 * d2).sqrt()
}

fn lerp_state<S: State>(a: &S, b: &S, t: f32) -> S {
    let mut z = S::zero();
    for ((o, x), y) in z.as_mut_slice().iter_mut().zip(a.as_slice()).zip(b.as_slice()) {
        *o = (1.0 - t) * x + t * y;
    }
    z
}

/// Two colours mixed at t with the mixer (v1's `mix_rgb`): encode both, lerp the states, decode.
pub fn mix<M: Mixer>(m: &M, a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    m.decode_srgb(&lerp_state(&m.encode(a), &m.encode(b), t))
}

/// Palette snapping (v1's `Palette`): the candidates are each colour's tints with white, each pair's mixtures, and
/// the pairs lightened with 30% white, all made by the mixer at 9 steps; a colour moves toward the nearest candidate
/// in Lab by `amount`.
pub struct Palette<S> {
    lat: Vec<S>,
    lab: Vec<[f64; 3]>,
}

impl<S: State> Palette<S> {
    pub fn new<M: Mixer<State = S>>(m: &M, colors: &[[f32; 3]], white: [f32; 3]) -> Palette<S> {
        let cols: Vec<[f32; 3]> = if colors.is_empty() { vec![[0.5, 0.5, 0.5]] } else { colors.to_vec() };
        let lats: Vec<S> = cols.iter().map(|c| m.encode(*c)).collect();
        let zw = m.encode(white);
        let ts: Vec<f32> = (0..9).map(|i| i as f32 / 8.0).collect();
        let mut lat = Vec::new();
        for (i, zi) in lats.iter().enumerate() {
            for &t in &ts {
                lat.push(lerp_state(zi, &zw, t));
            }
            for zj in &lats[i + 1..] {
                for &t in &ts {
                    let pair = lerp_state(zi, zj, t);
                    lat.push(pair);
                    lat.push(lerp_state(&pair, &zw, 0.3));
                }
            }
        }
        let lab = lat.iter().map(|z| lab(m.decode_srgb(z))).collect();
        Palette { lat, lab }
    }

    pub fn snap<M: Mixer<State = S>>(&self, m: &M, rgb: [f32; 3], amount: f32) -> [f32; 3] {
        let q = lab(rgb);
        let mut best = (f64::INFINITY, 0usize);
        for (k, c) in self.lab.iter().enumerate() {
            let d = (c[0] - q[0]) * (c[0] - q[0]) + (c[1] - q[1]) * (c[1] - q[1]) + (c[2] - q[2]) * (c[2] - q[2]);
            if d < best.0 {
                best = (d, k);
            }
        }
        let src = m.encode(rgb);
        m.decode_srgb(&lerp_state(&src, &self.lat[best.1], amount))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lab_round_trips_and_fast_path_agrees() {
        for rgb in [[0.0, 0.0, 0.0], [1.0, 1.0, 1.0], [0.8, 0.2, 0.1], [0.1, 0.5, 0.9], [0.23, 0.27, 0.35]] {
            let l = lab(rgb);
            let back = lab_to_rgb(l);
            for k in 0..3 {
                assert!((back[k] - rgb[k]).abs() < 1e-5, "{rgb:?} -> {l:?} -> {back:?}");
            }
            let f = lab_fast(rgb);
            for k in 0..3 {
                assert!((f[k] as f64 - l[k]).abs() < 0.02, "{rgb:?}: {f:?} vs {l:?}");
            }
        }
        assert!((lab([1.0, 1.0, 1.0])[0] - 100.0).abs() < 1e-3);
    }

    #[test]
    fn palette_snaps_toward_its_mixtures() {
        let m = oil_mix::RgbMixer;
        let p = Palette::new(&m, &[[0.8, 0.1, 0.1], [0.1, 0.1, 0.8]], [0.98, 0.97, 0.94]);
        let s = p.snap(&m, [0.7, 0.15, 0.2], 1.0);
        let d = |a: [f32; 3], b: [f32; 3]| dist_fast(lab_fast(a), lab_fast(b));
        assert!(d(s, [0.7, 0.15, 0.2]) < 10.0);
        assert_eq!(p.snap(&m, [0.3, 0.3, 0.3], 0.0), m.decode_srgb(&m.encode([0.3, 0.3, 0.3])));
    }
}
