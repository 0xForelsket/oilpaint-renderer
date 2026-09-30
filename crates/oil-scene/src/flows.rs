//! Authored flows as functions of (x, y) in canvas widths, evaluated where they are needed.
//!
//! The planner asks for a region's flow at each step of a stroke path (about 146,000 lookups for Storm Light's
//! 12,433 strokes), so flows are not rasterised: a raster per region cost 8 B/px and computed 60 times more values
//! than were read (plan section 8, decided after L2). The formulas are v1's (`scene.py`). The contour flow follows a
//! smooth minimum of the distances to the polygon's edges instead of a blurred distance raster.
use crate::spec::{Flow, Point};
use oil_image::noise::fbm_at;
use oil_math::{atan2, cos, exp, sin};

/// A sampled field at its own resolution: `channels` f32 values per pixel, row-major, one canvas width across.
#[derive(Clone, Debug)]
pub struct Sampled {
    pub w: usize,
    pub h: usize,
    pub channels: usize,
    pub data: Vec<f32>,
}

impl Sampled {
    /// Bilinear sample at (x, y) in cw (pixel centres at ((i + 0.5) / w, (j + 0.5) / w)), clamped at the edges.
    pub fn at(&self, x: f64, y: f64) -> [f64; 3] {
        let (u, v) = (x * self.w as f64 - 0.5, y * self.w as f64 - 0.5);
        let clamp = |t: f64, n: usize| t.max(0.0).min((n - 1) as f64);
        let (u, v) = (clamp(u, self.w), clamp(v, self.h));
        let (i0, j0) = (u.floor() as usize, v.floor() as usize);
        let (i1, j1) = ((i0 + 1).min(self.w - 1), (j0 + 1).min(self.h - 1));
        let (tu, tv) = (u - i0 as f64, v - j0 as f64);
        let mut out = [0.0; 3];
        for (c, o) in out.iter_mut().enumerate().take(self.channels.min(3)) {
            let p = |i: usize, j: usize| self.data[(j * self.w + i) * self.channels + c] as f64;
            let top = p(i0, j0) + tu * (p(i1, j0) - p(i0, j0));
            let bot = p(i0, j1) + tu * (p(i1, j1) - p(i0, j1));
            *o = top + tv * (bot - top);
        }
        out
    }
}

#[inline(always)]
pub fn unit(dx: f64, dy: f64) -> [f32; 2] {
    let n = (dx * dx + dy * dy).sqrt() + 1e-6;
    [(dx / n) as f32, (dy / n) as f32]
}

/// (fbm - 0.5) x 2 x amount: v1's flow noise term, in radians.
#[inline(always)]
fn noise_term(x: f64, y: f64, scale: f64, octaves: u32, seed: u32, amount: f64) -> f64 {
    (fbm_at(x, y, scale, octaves, seed) - 0.5) * 2.0 * amount
}

/// Smoothing radius of the contour flow, in cw (v1 blurred its distance field by 0.01 cw).
const CONTOUR_SMOOTH: f64 = 0.01;

#[derive(Clone, Debug)]
enum Kind {
    Constant { a0: f64, noise: f64, seed: u32 },
    Sweep { a0: f64, curl: f64, noise: f64, scale: f64, seed: u32 },
    Waves { a0: f64, amp: f64, wavelength: f64, noise: f64, perspective: bool, horizon: f64, bottom: f64, seed: u32 },
    Swirl { centres: Vec<[f64; 4]>, strength: f64, noise: f64, seed: u32 },
    Radial { c: Point, noise: f64, seed: u32 },
    Contour { poly: Vec<Point>, noise: f64, seed: u32 },
    Field(Sampled),
}

/// One authored flow, ready to evaluate.
#[derive(Clone, Debug)]
pub struct FlowEval(Kind);

fn constant_at(a0: f64, noise: f64, seed: u32, x: f64, y: f64) -> [f32; 2] {
    let a = if noise > 0.0 { a0 + noise_term(x, y, 0.15, 3, seed, noise) } else { a0 };
    unit(cos(a), sin(a))
}

/// Even-odd point-in-polygon.
fn inside(poly: &[Point], x: f64, y: f64) -> bool {
    let mut c = false;
    let n = poly.len();
    for k in 0..n {
        let (a, b) = (poly[k], poly[(k + 1) % n]);
        if (a[1] <= y) != (b[1] <= y) && x < a[0] + (y - a[1]) * (b[0] - a[0]) / (b[1] - a[1]) {
            c = !c;
        }
    }
    c
}

/// Gradient of a smooth signed distance to the polygon's outline (positive inside): each edge contributes the unit
/// vector from its nearest point, weighted by exp(-(d - d_min) / k).
fn contour_gradient(poly: &[Point], x: f64, y: f64) -> (f64, f64) {
    let n = poly.len();
    let mut near = Vec::with_capacity(n);
    let mut dmin = f64::INFINITY;
    for k in 0..n {
        let (a, b) = (poly[k], poly[(k + 1) % n]);
        let (ex, ey) = (b[0] - a[0], b[1] - a[1]);
        let len2 = ex * ex + ey * ey;
        let t = if len2 > 0.0 { (((x - a[0]) * ex + (y - a[1]) * ey) / len2).clamp(0.0, 1.0) } else { 0.0 };
        let (dx, dy) = (x - (a[0] + t * ex), y - (a[1] + t * ey));
        let d = (dx * dx + dy * dy).sqrt();
        dmin = dmin.min(d);
        near.push((dx, dy, d));
    }
    let sign = if inside(poly, x, y) { 1.0 } else { -1.0 };
    let (mut gx, mut gy) = (0.0, 0.0);
    for (dx, dy, d) in near {
        if d > 1e-9 {
            let wgt = exp(-(d - dmin) / CONTOUR_SMOOTH);
            gx += wgt * sign * dx / d;
            gy += wgt * sign * dy / d;
        }
    }
    (gx, gy)
}

impl FlowEval {
    /// `canvas_h` is the canvas height in cw (aspect height / width); `field` resolves `{"field": name}` flows.
    pub fn new(f: &Flow, canvas_h: f64, field: impl Fn(&str) -> Option<Sampled>) -> FlowEval {
        FlowEval(match f {
            Flow::Constant(c) => Kind::Constant { a0: c.angle.to_radians(), noise: c.noise, seed: c.seed },
            Flow::Sweep(s) => Kind::Sweep { a0: s.angle.to_radians(), curl: s.curl, noise: s.noise, scale: s.scale, seed: s.seed },
            Flow::Waves(w) => Kind::Waves {
                a0: w.angle.to_radians(),
                amp: w.amplitude.to_radians(),
                wavelength: w.wavelength,
                noise: w.noise,
                perspective: w.perspective,
                horizon: w.horizon,
                bottom: canvas_h,
                seed: w.seed,
            },
            Flow::SwirlAround(s) => Kind::Swirl { centres: s.centres.clone(), strength: s.strength, noise: s.noise, seed: s.seed },
            Flow::RadialFrom(r) => Kind::Radial { c: r.c, noise: r.noise, seed: r.seed },
            Flow::Upward(u) => Kind::Constant { a0: (-90f64).to_radians(), noise: u.noise * 1.2, seed: u.seed },
            Flow::Contour(c) => Kind::Contour { poly: c.polygon.clone(), noise: c.noise, seed: c.seed },
            Flow::Field(name) => match field(name) {
                Some(s) => Kind::Field(s),
                None => Kind::Constant { a0: 0.0, noise: 0.0, seed: 0 },
            },
        })
    }

    /// The unit flow direction at (x, y) in cw.
    pub fn at(&self, x: f64, y: f64) -> [f32; 2] {
        match &self.0 {
            Kind::Constant { a0, noise, seed } => constant_at(*a0, *noise, *seed, x, y),
            Kind::Sweep { a0, curl, noise, scale, seed } => {
                let a = a0 + noise_term(x, y, *scale, 2, *seed, *curl) + noise_term(x, y, scale * 0.35, 3, seed.wrapping_add(1), *noise);
                unit(cos(a), sin(a))
            }
            Kind::Waves { a0, amp, wavelength, noise, perspective, horizon, bottom, seed } => {
                // depth runs from the horizon (0) to the bottom edge of the canvas (1)
                let depth = if *perspective { ((y - horizon) / (bottom - horizon).max(1e-3)).clamp(0.0, 1.0) } else { 1.0 };
                let wl = wavelength * (0.3 + 0.7 * depth);
                let tau = 2.0 * std::f64::consts::PI;
                let a = a0
                    + amp * sin(tau * x / wl + 6.0 * fbm_at(x, y, 0.3, 2, *seed))
                    + noise_term(x, y, 0.08, 3, seed.wrapping_add(1), *noise) * (0.3 + 0.7 * depth);
                unit(cos(a), sin(a))
            }
            Kind::Swirl { centres, strength, noise, seed } => {
                let (mut dx, mut dy) = (0.0, 0.0);
                for c in centres {
                    let (ex, ey) = ((x - c[0]) / c[2], (y - c[1]) / c[3]);
                    let d = (ex * ex + ey * ey).sqrt() + 1e-6;
                    let wgt = exp(-d * d * 0.7);
                    dx += wgt * (-ey / d);
                    dy += wgt * (ex / d);
                }
                let b = constant_at((-8f64).to_radians(), *noise, *seed, x, y);
                unit(strength * dx + (1.0 - strength) * b[0] as f64, strength * dy + (1.0 - strength) * b[1] as f64)
            }
            Kind::Radial { c, noise, seed } => {
                let a = atan2(y - c[1], x - c[0]) + noise_term(x, y, 0.1, 2, *seed, *noise);
                unit(cos(a), sin(a))
            }
            Kind::Contour { poly, noise, seed } => {
                let (gx, gy) = contour_gradient(poly, x, y);
                let a = atan2(gy, gx) + 0.5 * std::f64::consts::PI + noise_term(x, y, 0.1, 3, *seed, *noise);
                unit(cos(a), sin(a))
            }
            Kind::Field(s) => {
                let v = s.at(x, y);
                unit(v[0], v[1])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{ConstantFlow, ContourFlow};

    #[test]
    fn contour_follows_the_outline() {
        let sq = vec![[0.3, 0.3], [0.7, 0.3], [0.7, 0.7], [0.3, 0.7]];
        let f = FlowEval::new(&Flow::Contour(ContourFlow { polygon: sq, noise: 0.0, seed: 7 }), 1.0, |_| None);
        // just inside and just outside the bottom edge the flow runs along it (horizontal), in opposite senses
        // of travel relative to the outline's normal, as v1's tangent-of-distance did
        for (x, y) in [(0.5, 0.68), (0.5, 0.72), (0.5, 0.95)] {
            let v = f.at(x, y);
            assert!(v[0].abs() > 0.99 && v[1].abs() < 0.1, "({x}, {y}) -> {v:?}");
        }
        // along the left edge it runs vertically
        let v = f.at(0.32, 0.5);
        assert!(v[1].abs() > 0.99, "{v:?}");
    }

    #[test]
    fn constant_without_noise_is_exact() {
        let f = FlowEval::new(&Flow::Constant(ConstantFlow { angle: 90.0, noise: 0.0, seed: 1 }), 1.25, |_| None);
        let v = f.at(0.2, 0.4);
        assert!(v[0].abs() < 1e-6 && (v[1] - 1.0).abs() < 1e-5, "{v:?}");
    }
}
