//! Relighting (v1's `oilpaint/light.py`, ported via `spikes/oilcore/src/light.rs`): canvas weave, two-scale normals
//! from the paint height, raking diffuse light, a satin highlight that fades on thin paint, a capped cavity term and
//! a relief tint. All maths goes through `oil-math`, and the weave comes from hashed noise instead of numpy's RNG,
//! so a relit image is identical on every host.
#![forbid(unsafe_code)]

use oil_image::{blur, gaussian_blur, sobel};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightParams {
    /// Direction to the light: x right, y down, z toward the viewer.
    pub light_dir: [f32; 3],
    pub bump: f32,
    pub bump_fine: f32,
    pub contrast: f32,
    pub spec: f32,
    pub shininess: f32,
    pub gloss_h0: f32,
    pub gloss_h1: f32,
    pub cavity: f32,
    /// Blurs in canvas widths: height anti-aliasing, broad/fine split, diffuse softening.
    pub hsmooth: f32,
    pub broad: f32,
    pub shade_blur: f32,
    pub tint: f32,
    pub weave_amp: f32,
    pub weave_h0: f32,
    /// Weave threads across the canvas width (v1: a 66 cm canvas at 19.5 threads/cm).
    pub weave_threads: f32,
    pub matte: bool,
}

impl Default for LightParams {
    /// v1's `LIGHT_DEFAULTS` ("default" light in the eval harness): shows every flaw.
    fn default() -> Self {
        LightParams {
            light_dir: [-0.5, -0.6, 0.62],
            bump: 1.4,
            bump_fine: 1.8,
            contrast: 0.45,
            spec: 0.10,
            shininess: 22.0,
            gloss_h0: 0.25,
            gloss_h1: 1.4,
            cavity: 0.06,
            hsmooth: 0.0004,
            broad: 0.004,
            shade_blur: 0.0006,
            tint: 0.06,
            weave_amp: 0.18,
            weave_h0: 1.2,
            weave_threads: 1290.0,
            matte: false,
        }
    }
}

impl LightParams {
    /// The "painting" light of Storm Light (eval harness `--light painting`).
    pub fn painting() -> Self {
        LightParams { bump: 0.95, bump_fine: 1.0, shade_blur: 0.002, contrast: 0.30, spec: 0.10, cavity: 0.02, ..Self::default() }
    }
}

#[inline(always)]
fn hash3(x: i32, y: i32, seed: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^ (h >> 16)
}

#[inline(always)]
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 * (1.0 / 16_777_216.0)
}

/// Catmull-Rom weights for fractional position t.
#[inline(always)]
fn cubic_weights(t: f32) -> [f32; 4] {
    let t2 = t * t;
    let t3 = t2 * t;
    [
        0.5 * (-t3 + 2.0 * t2 - t),
        0.5 * (3.0 * t3 - 5.0 * t2 + 2.0),
        0.5 * (-3.0 * t3 + 4.0 * t2 + t),
        0.5 * (t3 - t2),
    ]
}

/// Smooth value noise in [~0, ~1]: a hashed lattice with `cells` cells across the width, bicubic (Catmull-Rom),
/// `octaves` octaves halving in amplitude. The lattice lives in canvas-width units, so the field is the same at
/// every resolution.
pub fn value_noise(w: usize, h: usize, cells: f32, octaves: u32, seed: u32) -> Vec<f32> {
    let mut out = vec![0f32; w * h];
    let (mut amp, mut tot, mut c) = (1.0f32, 0.0f32, cells);
    for o in 0..octaves {
        let s = seed.wrapping_add(o.wrapping_mul(0x9e37_79b9));
        let scale = c / w as f32;
        let xw: Vec<(i32, [f32; 4])> = (0..w)
            .map(|x| {
                let fx = (x as f32 + 0.5) * scale;
                let ix = fx.floor();
                (ix as i32, cubic_weights(fx - ix))
            })
            .collect();
        for y in 0..h {
            let fy = (y as f32 + 0.5) * scale;
            let iy = fy.floor();
            let wy = cubic_weights(fy - iy);
            let iy = iy as i32;
            for (x, (ix, wx)) in xw.iter().enumerate() {
                let mut v = 0f32;
                for (j, wyj) in wy.iter().enumerate() {
                    let mut row = 0f32;
                    for (i, wxi) in wx.iter().enumerate() {
                        row += wxi * unit(hash3(ix - 1 + i as i32, iy - 1 + j as i32, s));
                    }
                    v += wyj * row;
                }
                out[y * w + x] += amp * v;
            }
        }
        tot += amp;
        amp *= 0.5;
        c *= 2.0;
    }
    out.iter_mut().for_each(|v| *v /= tot);
    out
}

/// Canvas weave height: two sine gratings (plain weave) plus slub noise, band-limited to fine noise where the
/// thread pitch falls below 2.2 px (as v1).
pub fn weave(w: usize, h: usize, amp: f32, threads_across: f32, seed: u32) -> Vec<f32> {
    let pitch = w as f32 / threads_across;
    let slub = value_noise(w, h, 40.0, 3, seed);
    let fine = |x: usize, y: usize| unit(hash3(x as i32, y as i32, seed ^ 0x5bd1_e995)) - 0.5;
    let mut out = vec![0f32; w * h];
    let k = 2.0 * std::f32::consts::PI / pitch;
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let s = slub[i];
            out[i] = if pitch < 2.2 {
                amp * (0.35 * (s - 0.5) + 0.35 * fine(x, y))
            } else {
                let gx = oil_math::sinf(x as f32 * k + 0.8 * s);
                let gy = oil_math::sinf(y as f32 * k + 0.8 * s);
                amp * (0.5 * gx * gy + 0.25 * (gx + gy) * 0.3 + 0.35 * (s - 0.5) + 0.25 * fine(x, y))
            };
        }
    }
    out
}

/// Lit sRGB image from the unlit colour `rgb` (sRGB, 3 per pixel) and the paint height `hgt`.
pub fn relight(rgb: &[[f32; 3]], hgt: &[f32], w: usize, h: usize, p: &LightParams) -> Vec<[f32; 3]> {
    let n = w * h;
    let wf = w as f64;
    let mut h_total: Vec<f32> = if p.weave_amp > 0.0 {
        let wv = weave(w, h, p.weave_amp, p.weave_threads, 7);
        let h0 = p.weave_h0.max(1e-6);
        (0..n).map(|i| hgt[i] + wv[i] * oil_math::expf(-hgt[i] / h0)).collect()
    } else {
        hgt.to_vec()
    };
    let scale = (w as f64 / 600.0) as f32;
    if p.hsmooth > 0.0 {
        h_total = gaussian_blur(&h_total, w, h, (p.hsmooth as f64 * wf).max(0.3));
    }
    let h_broad = blur(&h_total, w, h, (p.broad as f64 * wf).max(1.0));
    let h_fine: Vec<f32> = (0..n).map(|i| h_total[i] - h_broad[i]).collect();
    let (gxb, gyb) = (sobel(&h_broad, w, h, true), sobel(&h_broad, w, h, false));
    let (gxf, gyf) = (sobel(&h_fine, w, h, true), sobel(&h_fine, w, h, false));
    let mut l = p.light_dir;
    let ln = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
    l.iter_mut().for_each(|v| *v /= ln);
    let mut hv = [l[0], l[1], l[2] + 1.0];
    let hn = (hv[0] * hv[0] + hv[1] * hv[1] + hv[2] * hv[2]).sqrt();
    hv.iter_mut().for_each(|v| *v /= hn);
    let gd = (p.gloss_h1 - p.gloss_h0).max(1e-6);
    let spec_k = if p.matte { 0.03 } else { p.spec };
    let mut shade = vec![0f32; n];
    let mut spec = vec![0f32; n];
    for i in 0..n {
        let gx = p.bump * (gxb[i] / 8.0 * scale) + p.bump_fine * (gxf[i] / 8.0 * scale);
        let gy = p.bump * (gyb[i] / 8.0 * scale) + p.bump_fine * (gyf[i] / 8.0 * scale);
        let nz = 1.0 / (1.0 + gx * gx + gy * gy).sqrt();
        let (nx, ny) = (-gx * nz, -gy * nz);
        let ndotl = (nx * l[0] + ny * l[1] + nz * l[2]).clamp(0.0, 1.0);
        let ndoth = (nx * hv[0] + ny * hv[1] + nz * hv[2]).clamp(0.0, 1.0);
        let mut gl = ((hgt[i] - p.gloss_h0) / gd).clamp(0.0, 1.0);
        gl = gl * gl * (3.0 - 2.0 * gl);
        spec[i] = spec_k * gl * oil_math::pow(ndoth as f64, p.shininess as f64) as f32;
        shade[i] = (1.0 + p.contrast * (ndotl - l[2]) / l[2]).clamp(0.55, 1.35);
    }
    let shade = gaussian_blur(&shade, w, h, (p.shade_blur as f64 * wf).max(0.5));
    let hb = blur(&h_total, w, h, 6.0 * scale as f64);
    let cav_raw: Vec<f32> = (0..n).map(|i| ((hb[i] - h_total[i]) / 1.5).clamp(0.0, 1.0)).collect();
    let cav_raw = blur(&cav_raw, w, h, 2.0 * scale as f64);
    (0..n)
        .map(|i| {
            let cav = 1.0 - p.cavity * cav_raw[i];
            let rel = (h_fine[i] / 0.5).clamp(-1.0, 1.0);
            let [r, g, b] = rgb[i];
            let mean = (r + g + b) / 3.0;
            let (k1, k2) = (1.0 + p.tint * rel, 0.6 * p.tint * rel);
            let sc = shade[i] * cav;
            [r, g, b].map(|v| ((v * k1 + k2 * (v - mean)).clamp(0.0, 1.0) * sc + spec[i]).clamp(0.0, 1.0))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_paint_keeps_its_colour_and_slopes_shade() {
        let (w, h) = (64, 48);
        let rgb = vec![[0.5f32, 0.4, 0.3]; w * h];
        let p = LightParams { weave_amp: 0.0, ..LightParams::default() };
        let lit = relight(&rgb, &vec![0.0; w * h], w, h, &p);
        assert!(lit.iter().all(|c| (c[0] - 0.5).abs() < 1e-5 && (c[2] - 0.3).abs() < 1e-5));
        // a ridge: the side facing the light (left, x < 32) is brighter than the side away from it
        let hgt: Vec<f32> = (0..w * h).map(|i| (3.0 - ((i % w) as f32 - 32.0).abs() * 0.2).max(0.0)).collect();
        let lit = relight(&rgb, &hgt, w, h, &p);
        let row = 24 * w;
        assert!(lit[row + 26][1] > lit[row + 38][1], "{:?} vs {:?}", lit[row + 26], lit[row + 38]);
    }

    #[test]
    fn weave_is_resolution_independent_noise() {
        let a = value_noise(100, 80, 10.0, 2, 3);
        assert!(a.iter().all(|v| (-0.2..=1.2).contains(v)));
        let wv = weave(200, 150, 0.18, 1290.0, 7);
        let mean = wv.iter().sum::<f32>() / wv.len() as f32;
        assert!(mean.abs() < 0.02, "{mean}");
    }
}
