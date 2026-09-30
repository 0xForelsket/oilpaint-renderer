//! The raster grid and shape rasterisation. Semantics follow v1's `scene.py` (same formulas and defaults); the
//! rasterisation itself (pixel-centre polygon fill, hashed noise, exact distance transform) is the engine's.
use crate::spec::{Point, Shape};
use oil_image::{geom, noise};
use std::collections::BTreeMap;

/// A raster `w` x `h` pixels, one canvas width across: pixel (i, j) has its centre at ((i + 0.5) / w,
/// (j + 0.5) / w) in cw.
#[derive(Clone, Copy, Debug)]
pub struct Grid {
    pub w: usize,
    pub h: usize,
    /// Pixels per canvas width (= w).
    pub pw: f64,
}

impl Grid {
    /// Height is round(width * aspectH / aspectW), half up, as for StrokeLists.
    pub fn new(width: u32, aspect: [u32; 2]) -> Grid {
        let [aw, ah] = aspect.map(|v| v as u64);
        let h = ((width as u64 * ah * 2 + aw) / (2 * aw)) as usize;
        Grid { w: width as usize, h, pw: width as f64 }
    }

    #[inline(always)]
    pub fn x(&self, i: usize) -> f64 {
        (i as f64 + 0.5) / self.pw
    }

    #[inline(always)]
    pub fn y(&self, j: usize) -> f64 {
        (j as f64 + 0.5) / self.pw
    }

    pub fn len(&self) -> usize {
        self.w * self.h
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Evaluate `f(x, y)` at every pixel centre.
    pub fn map(&self, f: impl Fn(f64, f64) -> f32) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.len());
        for j in 0..self.h {
            let y = self.y(j);
            for i in 0..self.w {
                out.push(f(self.x(i), y));
            }
        }
        out
    }

    /// Pixel window (x0, y0, x1, y1) covering the cw box [cx - rx, cx + rx] x [cy - ry, cy + ry], clipped.
    pub fn window(&self, c: Point, rx: f64, ry: f64) -> (usize, usize, usize, usize) {
        let clip = |v: f64, n: usize| (v.max(0.0).min(n as f64)) as usize;
        let x0 = clip(((c[0] - rx) * self.pw - 0.5).floor(), self.w);
        let x1 = clip(((c[0] + rx) * self.pw + 0.5).ceil() + 1.0, self.w);
        let y0 = clip(((c[1] - ry) * self.pw - 0.5).floor(), self.h);
        let y1 = clip(((c[1] + ry) * self.pw + 0.5).ceil() + 1.0, self.h);
        (x0, y0, x1.max(x0), y1.max(y0))
    }

    pub fn fbm(&self, scale: f64, octaves: u32, seed: u32) -> Vec<f32> {
        noise::fbm_window(self.pw, (0, 0, self.w, self.h), scale, octaves, seed)
    }

    pub fn to_px(&self, pts: &[Point]) -> Vec<[f64; 2]> {
        pts.iter().map(|p| [p[0] * self.pw, p[1] * self.pw]).collect()
    }
}

#[inline(always)]
pub fn clip01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

/// Sampled fields at their own resolution, by name.
pub type Fields = BTreeMap<String, crate::flows::Sampled>;

pub fn polygon(g: &Grid, pts: &[Point]) -> Vec<f32> {
    geom::fill_polygon(g.w, g.h, &g.to_px(pts))
}

pub fn ellipse(g: &Grid, c: Point, rx: f64, ry: f64, softness: f64) -> Vec<f32> {
    g.map(|x, y| {
        let (ex, ey) = ((x - c[0]) / rx, (y - c[1]) / ry);
        let d = (ex * ex + ey * ey).sqrt();
        if softness <= 0.0 {
            (d < 1.0) as u8 as f32
        } else {
            clip01((1.0 + softness - d) / softness) as f32
        }
    })
}

/// A soft sector from `apex` pointing at `angle` degrees (0 = +x, 90 = down), `spread` degrees either side.
pub fn wedge_at(apex: Point, angle: f64, spread: f64, length: f64, x: f64, y: f64) -> f64 {
    let (dx, dy) = (x - apex[0], y - apex[1]);
    let d = (dx * dx + dy * dy).sqrt();
    let a = oil_math::atan2(dy, dx).to_degrees();
    let da = (a - angle + 180.0).rem_euclid(360.0) - 180.0;
    let ang = clip01(1.0 - da.abs() / spread);
    let rad = clip01((length - d) / (0.5 * length));
    ang * rad
}

pub fn wedge(g: &Grid, apex: Point, angle: f64, spread: f64, length: f64) -> Vec<f32> {
    g.map(|x, y| wedge_at(apex, angle, spread, length, x, y) as f32)
}

/// v1's `noisy`: the boundary band (0.02 < m < 0.98) moves by 4 x amount x noise; everything moves by 0.5 x that,
/// including outside the shape (a faint halo, kept from v1).
pub fn noisy(g: &Grid, m: &[f32], amount: f64, scale: f64, seed: u32) -> Vec<f32> {
    let n = g.fbm(scale, 3, seed);
    m.iter()
        .zip(&n)
        .map(|(&m, &n)| {
            let (m, n) = (m as f64, n as f64 - 0.5);
            let edge = (m > 0.02 && m < 0.98) as u8 as f64;
            clip01(m + amount * n * edge * 4.0 + amount * n * 0.5) as f32
        })
        .collect()
}

/// Rasterise a shape to a mask in [0, 1].
pub fn shape(g: &Grid, s: &Shape, fields: &Fields) -> Vec<f32> {
    match s {
        Shape::All(_) => vec![1.0; g.len()],
        Shape::Above(y0) => g.map(|_, y| (y < *y0) as u8 as f32),
        Shape::Below(y0) => g.map(|_, y| (y >= *y0) as u8 as f32),
        Shape::Ellipse(e) => ellipse(g, e.c, e.r[0], e.r[1], e.softness),
        Shape::Disc(d) => ellipse(g, d.c, d.r, d.r, d.softness),
        Shape::Polygon(pts) => polygon(g, pts),
        Shape::Wedge(w) => wedge(g, w.apex, w.angle, w.spread, w.length),
        Shape::BandAround(b) => {
            let m = polygon(g, &b.polygon);
            let k = ((b.dist * g.pw) as usize).max(1);
            let (dil, ero) = (geom::dilate(&m, g.w, g.h, k), geom::erode(&m, g.w, g.h, k));
            dil.iter().zip(&ero).map(|(a, b)| (a - b).clamp(0.0, 1.0)).collect()
        }
        Shape::Union(v) => {
            let mut acc = vec![0f32; g.len()];
            for s in v {
                for (a, b) in acc.iter_mut().zip(shape(g, s, fields)) {
                    *a += b;
                }
            }
            acc.iter().map(|v| v.clamp(0.0, 1.0)).collect()
        }
        Shape::Intersect(v) => {
            let mut acc = vec![1f32; g.len()];
            for s in v {
                for (a, b) in acc.iter_mut().zip(shape(g, s, fields)) {
                    *a *= b;
                }
            }
            acc
        }
        Shape::Not(s) => shape(g, s, fields).iter().map(|v| (1.0 - v).clamp(0.0, 1.0)).collect(),
        Shape::Noisy(n) => noisy(g, &shape(g, &n.shape, fields), n.amount, n.scale, n.seed),
        Shape::Field(name) => match fields.get(name) {
            Some(f) => g.map(|x, y| f.at(x, y)[0].clamp(0.0, 1.0) as f32),
            None => vec![0.0; g.len()],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_and_shapes() {
        let g = Grid::new(40, [4, 5]);
        assert_eq!((g.w, g.h), (40, 50));
        assert_eq!(Grid::new(602, [4, 5]).h, 753); // 752.5 rounds half up
        let f = Fields::new();
        let above = shape(&g, &Shape::Above(0.5), &f);
        assert_eq!(above.iter().sum::<f32>(), 40.0 * 20.0);
        let not = shape(&g, &Shape::Not(Box::new(Shape::Above(0.5))), &f);
        assert!(above.iter().zip(&not).all(|(a, b)| a + b == 1.0));
        let w = wedge(&g, [0.5, 0.5], 0.0, 10.0, 0.4);
        // pixel (28, 20) is 3.4 degrees off the axis; (28, 25) is 33 degrees off; (12, 20) is behind the apex
        assert!(w[20 * 40 + 28] > 0.5 && w[25 * 40 + 28] == 0.0 && w[20 * 40 + 12] == 0.0);
        let band = shape(&g, &Shape::BandAround(crate::spec::BandAround { polygon: vec![[0.2, 0.2], [0.8, 0.2], [0.8, 0.8], [0.2, 0.8]], dist: 0.05 }), &f);
        assert_eq!(band[20 * 40 + 20], 0.0);
        assert_eq!(band[8 * 40 + 20], 1.0);
    }
}
