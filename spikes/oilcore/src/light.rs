//! Port of light.relight(): two-scale normals from the height field, raking diffuse, satin specular fading on thin
//! paint, capped cavity, relief tint.  The canvas weave is passed in (the spike reuses Python's numpy weave).
use crate::imgops::{blur, gaussian_blur, sobel};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct LightParams {
    pub light_dir: [f32; 3],
    pub bump: f32,
    pub bump_fine: f32,
    pub contrast: f32,
    pub spec: f32,
    pub shininess: f32,
    pub gloss_h0: f32,
    pub gloss_h1: f32,
    pub cavity: f32,
    pub hsmooth: f32,
    pub broad: f32,
    pub shade_blur: f32,
    pub tint: f32,
    pub weave_amp: f32,
    pub weave_h0: f32,
    pub matte: i32,
}

impl Default for LightParams {
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
            matte: 0,
        }
    }
}

pub fn relight(rgb: &[f32], hgt: &[f32], weave: Option<&[f32]>, w: usize, h: usize, p: &LightParams, out: &mut [f32]) {
    let n = w * h;
    let wf = w as f64;
    let mut h_total: Vec<f32> = match weave {
        Some(wv) if p.weave_amp > 0.0 => {
            let h0 = if p.weave_h0 > 1e-6 { p.weave_h0 } else { 1e-6 };
            (0..n).map(|i| hgt[i] + wv[i] * (-hgt[i] / h0).exp()).collect()
        }
        _ => hgt.to_vec(),
    };
    let scale = (w as f64 / 600.0) as f32;
    if p.hsmooth > 0.0 {
        h_total = gaussian_blur(&h_total, w, h, (p.hsmooth as f64 * wf).max(0.3));
    }
    let h_broad = blur(&h_total, w, h, (p.broad as f64 * wf).max(1.0));
    let h_fine: Vec<f32> = (0..n).map(|i| h_total[i] - h_broad[i]).collect();
    let gxb = sobel(&h_broad, w, h, true);
    let gyb = sobel(&h_broad, w, h, false);
    let gxf = sobel(&h_fine, w, h, true);
    let gyf = sobel(&h_fine, w, h, false);
    let mut l = p.light_dir;
    let ln = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
    for v in l.iter_mut() {
        *v /= ln;
    }
    let mut hv = [l[0], l[1], l[2] + 1.0];
    let hn = (hv[0] * hv[0] + hv[1] * hv[1] + hv[2] * hv[2]).sqrt();
    for v in hv.iter_mut() {
        *v /= hn;
    }
    let (g0, g1) = (p.gloss_h0, p.gloss_h1);
    let gd = if g1 - g0 > 1e-6 { g1 - g0 } else { 1e-6 };
    let spec_k = if p.matte != 0 { 0.03 } else { p.spec };
    let mut shade = vec![0f32; n];
    let mut spec = vec![0f32; n];
    for i in 0..n {
        let gx = p.bump * (gxb[i] / 8.0 * scale) + p.bump_fine * (gxf[i] / 8.0 * scale);
        let gy = p.bump * (gyb[i] / 8.0 * scale) + p.bump_fine * (gyf[i] / 8.0 * scale);
        let nz = 1.0 / (1.0 + gx * gx + gy * gy).sqrt();
        let nx = -gx * nz;
        let ny = -gy * nz;
        let ndotl = (nx * l[0] + ny * l[1] + nz * l[2]).clamp(0.0, 1.0);
        let ndoth = (nx * hv[0] + ny * hv[1] + nz * hv[2]).clamp(0.0, 1.0);
        let mut gl = ((hgt[i] - g0) / gd).clamp(0.0, 1.0);
        gl = gl * gl * (3.0 - 2.0 * gl);
        spec[i] = spec_k * gl * ndoth.powf(p.shininess);
        shade[i] = (1.0 + p.contrast * (ndotl - l[2]) / l[2]).clamp(0.55, 1.35);
    }
    let shade = gaussian_blur(&shade, w, h, (p.shade_blur as f64 * wf).max(0.5));
    let hb = blur(&h_total, w, h, 6.0 * scale as f64);
    let cav_raw: Vec<f32> = (0..n).map(|i| ((hb[i] - h_total[i]) / 1.5).clamp(0.0, 1.0)).collect();
    let cav_raw = blur(&cav_raw, w, h, 2.0 * scale as f64);
    for i in 0..n {
        let cav = 1.0 - p.cavity * cav_raw[i];
        let rel = (h_fine[i] / 0.5).clamp(-1.0, 1.0);
        let (r, g, b) = (rgb[i * 3], rgb[i * 3 + 1], rgb[i * 3 + 2]);
        let mean = (r + g + b) / 3.0;
        let k1 = 1.0 + p.tint * rel;
        let k2 = 0.6 * p.tint * rel;
        let sc = shade[i] * cav;
        for (c, v) in [r, g, b].iter().enumerate() {
            let alb = (v * k1 + k2 * (v - mean)).clamp(0.0, 1.0);
            out[i * 3 + c] = (alb * sc + spec[i]).clamp(0.0, 1.0);
        }
    }
}
