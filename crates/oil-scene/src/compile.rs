//! The scene compiler: a ScenePlan at a given width becomes the guide maps the planner paints from.
//!
//! - target: RGB reference image (sRGB, [0, 1]);
//! - region_id: the hard region map (later regions override earlier ones), and one soft mask per region;
//! - flow: unit stroke directions. Authored flows are functions evaluated where needed (`flows.rs`), since a
//!   region's strokes follow its own field wherever they go; elsewhere a pixel takes the flow of the region that
//!   owns it, or the target's structure-tensor flow, the one flow kept as a raster;
//! - light: the light map in [0, 1].
use crate::color;
use crate::flows::{unit, FlowEval, Sampled};
use crate::raster::{self, clip01, Fields, Grid};
use crate::spec::*;
use oil_errors::Error;
use oil_image::{blur, sobel};
use oil_math::{atan2, cos, exp, pow, sin};
use oil_mix::Mixer;
use sha2::{Digest, Sha256};

/// A soft mask cropped to the box where it is non-zero, stored as 16-bit fractions (value = q / 65535).
#[derive(Clone, Debug, PartialEq)]
pub struct Mask {
    pub x0: usize,
    pub y0: usize,
    pub w: usize,
    pub h: usize,
    pub data: Vec<u16>,
}

impl Mask {
    /// Quantise and crop a full-canvas mask.
    pub fn from_full(full: &[f32], gw: usize, gh: usize) -> Mask {
        let q: Vec<u16> = full.iter().map(|v| (v.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16).collect();
        let (mut x0, mut y0, mut x1, mut y1) = (gw, gh, 0, 0);
        for j in 0..gh {
            for i in 0..gw {
                if q[j * gw + i] != 0 {
                    x0 = x0.min(i);
                    x1 = x1.max(i + 1);
                    y0 = y0.min(j);
                    y1 = y1.max(j + 1);
                }
            }
        }
        if x1 <= x0 {
            return Mask { x0: 0, y0: 0, w: 0, h: 0, data: Vec::new() };
        }
        let (w, h) = (x1 - x0, y1 - y0);
        let mut data = Vec::with_capacity(w * h);
        for j in y0..y1 {
            data.extend_from_slice(&q[j * gw + x0..j * gw + x1]);
        }
        Mask { x0, y0, w, h, data }
    }

    /// The mask at pixel (i, j); 0 outside its box.
    #[inline(always)]
    pub fn at(&self, i: usize, j: usize) -> f32 {
        if i < self.x0 || j < self.y0 || i >= self.x0 + self.w || j >= self.y0 + self.h {
            return 0.0;
        }
        self.data[(j - self.y0) * self.w + (i - self.x0)] as f32 * (1.0 / 65535.0)
    }

    /// The whole-canvas plane (for previews and exports).
    pub fn to_full(&self, gw: usize, gh: usize) -> Vec<f32> {
        let mut out = vec![0f32; gw * gh];
        for j in 0..self.h {
            for i in 0..self.w {
                out[(self.y0 + j) * gw + self.x0 + i] = self.data[j * self.w + i] as f32 * (1.0 / 65535.0);
            }
        }
        out
    }
}

pub struct Guides {
    pub w: usize,
    pub h: usize,
    /// Canvas height in cw (aspect height / width).
    pub canvas_h: f64,
    pub ground: [f32; 3],
    /// Row-major sRGB.
    pub target: Vec<[f32; 3]>,
    pub region_id: Vec<u8>,
    /// Region names, in id order.
    pub names: Vec<String>,
    /// Soft masks, in region order.
    pub masks: Vec<Mask>,
    /// The target's structure-tensor flow (the fallback where no authored flow applies).
    pub structure: Vec<[f32; 2]>,
    /// Each region's authored flow (None: the region follows the global flow).
    pub flows: Vec<Option<FlowEval>>,
    pub light: Vec<f32>,
}

/// Wall-clock milliseconds per stage (from the host's clock; not part of the output).
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Timings {
    pub target_ms: f64,
    pub regions_ms: f64,
    pub flow_ms: f64,
    pub light_ms: f64,
    pub total_ms: f64,
}

/// A sampled field supplied next to the spec: little-endian f32 values, channels interleaved, row-major.
pub struct FieldData {
    pub data: Vec<f32>,
}

/// SHA-256 of f32 values as little-endian bytes (lowercase hex).
pub fn sha256_f32(values: impl IntoIterator<Item = f32>) -> String {
    let mut h = Sha256::new();
    let mut buf = Vec::with_capacity(1 << 16);
    for v in values {
        buf.extend_from_slice(&v.to_bits().to_le_bytes());
        if buf.len() >= 1 << 16 {
            h.update(&buf);
            buf.clear();
        }
    }
    h.update(&buf);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn channels(kind: FieldKind) -> usize {
    match kind {
        FieldKind::Mask => 1,
        FieldKind::Flow => 2,
        FieldKind::Rgb => 3,
    }
}

/// Check supplied fields against their declarations; they stay at their own resolution and are sampled where used.
fn resolve_fields(plan: &ScenePlan, supplied: &std::collections::BTreeMap<String, FieldData>) -> Result<Fields, Vec<Error>> {
    let mut out = Fields::new();
    let mut errs = Vec::new();
    for (name, decl) in &plan.fields {
        let p = format!("/fields/{name}");
        let Some(f) = supplied.get(name) else {
            errs.push(Error::new("UNKNOWN_FIELD", format!("field {name} is declared but was not supplied")).path(p));
            continue;
        };
        let (fw, fh, nc) = (decl.width as usize, decl.height as usize, channels(decl.kind));
        if f.data.len() != fw * fh * nc {
            errs.push(Error::new("RANGE", format!("field {name} has the wrong number of values")).path(p).got(f.data.len()).expected(fw * fh * nc));
            continue;
        }
        let hash = sha256_f32(f.data.iter().copied());
        if hash != decl.sha256 {
            errs.push(Error::new("FIELD_HASH_MISMATCH", format!("field {name} does not match its sha256")).path(format!("{p}/sha256")).got(hash).expected(&decl.sha256));
            continue;
        }
        out.insert(name.clone(), Sampled { w: fw, h: fh, channels: nc, data: f.data.clone() });
    }
    if errs.is_empty() {
        Ok(out)
    } else {
        Err(errs)
    }
}

/// Compile `plan` at `width` pixels. `mixer` resolves colour mixes; `clock` returns milliseconds (for timings).
pub fn compile<M: Mixer>(
    plan: &ScenePlan,
    width: u32,
    mixer: &M,
    fields: &std::collections::BTreeMap<String, FieldData>,
    clock: &dyn Fn() -> f64,
) -> Result<(Guides, Timings), Vec<Error>> {
    if !(16..=20_000).contains(&width) {
        return Err(vec![Error::new("RANGE", "guide width is 16..20000 px").path("/width").got(width).expected("16..20000")]);
    }
    let r = crate::validate::validate(plan);
    if !r.errors.is_empty() {
        return Err(r.errors);
    }
    let t0 = clock();
    let g = Grid::new(width, plan.canvas.aspect);
    let canvas_h = plan.canvas.aspect[1] as f64 / plan.canvas.aspect[0] as f64;
    let fields = resolve_fields(plan, fields)?;
    let col = |c: &ColorSpec, p: &str| color::resolve(c, mixer, p).map_err(|e| vec![e]);
    let ground = col(&plan.canvas.ground, "/canvas/ground")?;

    let target = render_target(plan, &g, canvas_h, &fields, &col)?;
    let t1 = clock();

    let n = plan.regions.len();
    let mut region_id = vec![0u8; g.len()];
    let mut masks = Vec::with_capacity(n);
    for (i, r) in plan.regions.iter().enumerate() {
        let m: Vec<f32> = raster::shape(&g, &r.shape, &fields).iter().map(|v| v.clamp(0.0, 1.0)).collect();
        for (id, v) in region_id.iter_mut().zip(&m) {
            if *v > 0.5 {
                *id = i as u8;
            }
        }
        masks.push(Mask::from_full(&blur(&m, g.w, g.h, (r.edge * g.pw).max(0.5)), g.w, g.h));
    }
    let t2 = clock();

    let structure = structure_flow(&target, &g);
    let flows = plan.regions.iter().map(|r| r.flow.as_ref().map(|f| FlowEval::new(f, canvas_h, |n| fields.get(n).cloned()))).collect();
    let t3 = clock();

    let light = light_map(plan, &g);
    let t4 = clock();

    let guides = Guides {
        w: g.w,
        h: g.h,
        canvas_h,
        ground,
        target,
        region_id,
        names: plan.regions.iter().map(|r| r.name.clone()).collect(),
        masks,
        structure,
        flows,
        light,
    };
    let timings = Timings { target_ms: t1 - t0, regions_ms: t2 - t1, flow_ms: t3 - t2, light_ms: t4 - t3, total_ms: t4 - t0 };
    Ok((guides, timings))
}

// ------------------------------------------------------------------ target

type Col<'a> = dyn Fn(&ColorSpec, &str) -> Result<[f32; 3], Vec<Error>> + 'a;

fn blend(img: &mut [[f32; 3]], c: [f32; 3], alpha: &[f32]) {
    for (p, &a) in img.iter_mut().zip(alpha) {
        let a = a.clamp(0.0, 1.0);
        for k in 0..3 {
            p[k] = p[k] * (1.0 - a) + c[k] * a;
        }
    }
}

fn add_light(img: &mut [[f32; 3]], c: [f32; 3], alpha: &[f32]) {
    for (p, &a) in img.iter_mut().zip(alpha) {
        let a = a.clamp(0.0, 2.0);
        for k in 0..3 {
            p[k] += a * (c[k] - p[k] * 0.5);
        }
    }
}

fn gradient(g: &Grid, stops: &[(f64, [f32; 3])]) -> Vec<[f32; 3]> {
    let mut out = Vec::with_capacity(g.len());
    for j in 0..g.h {
        let y = g.y(j);
        let c = match stops.iter().position(|(sy, _)| *sy > y) {
            None => stops[stops.len() - 1].1,
            Some(0) => stops[0].1,
            Some(k) => {
                let ((y0, c0), (y1, c1)) = (stops[k - 1], stops[k]);
                let t = (y - y0) / (y1 - y0);
                [0, 1, 2].map(|i| (c0[i] as f64 + t * (c1[i] as f64 - c0[i] as f64)) as f32)
            }
        };
        out.extend(std::iter::repeat_n(c, g.w));
    }
    out
}

/// Average of a sampled rgb field over the pixel box [u0, u1) x [v0, v1) (field pixels), exact area weights.
fn area_rgb(s: &Sampled, u0: f64, u1: f64, v0: f64, v1: f64) -> [f64; 3] {
    let (mut acc, mut wsum) = ([0.0f64; 3], 0.0f64);
    let (i0, i1) = (u0.floor().max(0.0) as usize, (u1.ceil() as usize).min(s.w));
    let (j0, j1) = (v0.floor().max(0.0) as usize, (v1.ceil() as usize).min(s.h));
    for j in j0..j1 {
        let wy = (v1.min(j as f64 + 1.0) - v0.max(j as f64)).max(0.0);
        for i in i0..i1 {
            let wgt = wy * (u1.min(i as f64 + 1.0) - u0.max(i as f64)).max(0.0);
            let k = (j * s.w + i) * s.channels;
            for (c, a) in acc.iter_mut().enumerate() {
                *a += wgt * s.data[k + c] as f64;
            }
            wsum += wgt;
        }
    }
    if wsum > 0.0 {
        acc.map(|v| v / wsum)
    } else {
        [0.0; 3]
    }
}

/// A picture fitted to the canvas: colour and coverage (0 outside a `contain` picture) per pixel. Downscaling
/// averages the picture's pixels under each canvas pixel; upscaling is bilinear.
fn image_layer(g: &Grid, canvas_h: f64, s: &Sampled, fit: Fit) -> (Vec<[f32; 3]>, Vec<f32>) {
    let pa = s.h as f64 / s.w as f64;
    // displayed picture width in cw, and its offset
    let wd = match fit {
        Fit::Cover => 1f64.max(canvas_h / pa),
        Fit::Contain => 1f64.min(canvas_h / pa),
        Fit::Stretch => 1.0,
    };
    let hd = if fit == Fit::Stretch { canvas_h } else { wd * pa };
    let (ox, oy) = ((1.0 - wd) * 0.5, (canvas_h - hd) * 0.5);
    let (sx, sy) = (s.w as f64 / wd, s.h as f64 / hd); // field pixels per cw
    let footprint = sx / g.pw;
    let mut rgb = Vec::with_capacity(g.len());
    let mut cov = Vec::with_capacity(g.len());
    for j in 0..g.h {
        for i in 0..g.w {
            let (x, y) = (g.x(i), g.y(j));
            let (u, v) = ((x - ox) * sx, (y - oy) * sy);
            if u < 0.0 || v < 0.0 || u > s.w as f64 || v > s.h as f64 {
                rgb.push([0.0; 3]);
                cov.push(0.0);
                continue;
            }
            let c = if footprint >= 1.0 {
                let (hu, hv) = (0.5 * sx / g.pw, 0.5 * sy / g.pw);
                area_rgb(s, u - hu, u + hu, v - hv, v + hv)
            } else {
                s.at((u / s.w as f64) * 1.0, (v / s.w as f64) * 1.0)
            };
            rgb.push(c.map(|v| v.clamp(0.0, 1.0) as f32));
            cov.push(1.0);
        }
    }
    (rgb, cov)
}

fn render_target(plan: &ScenePlan, g: &Grid, canvas_h: f64, fields: &Fields, col: &Col) -> Result<Vec<[f32; 3]>, Vec<Error>> {
    let mut img = vec![[0f32; 3]; g.len()];
    for (i, op) in plan.target.iter().enumerate() {
        let p = format!("/target/{i}");
        match op {
            TargetOp::Fill(f) => {
                let fill: Vec<[f32; 3]> = match (&f.color, &f.gradient_v) {
                    (Some(c), _) => vec![col(c, &format!("{p}/fill/color"))?; g.len()],
                    (None, Some(stops)) => {
                        let mut s = Vec::with_capacity(stops.len());
                        for (k, (y, c)) in stops.iter().enumerate() {
                            s.push((*y, col(c, &format!("{p}/fill/gradientV/{k}/1"))?));
                        }
                        gradient(g, &s)
                    }
                    (None, None) => unreachable!("validated"),
                };
                match &f.mask {
                    None => img = fill,
                    Some(m) => {
                        let a = raster::shape(g, m, fields);
                        for ((px, c), a) in img.iter_mut().zip(&fill).zip(&a) {
                            let a = a.clamp(0.0, 1.0);
                            for k in 0..3 {
                                px[k] = px[k] * (1.0 - a) + c[k] * a;
                            }
                        }
                    }
                }
            }
            TargetOp::Blob(b) => {
                let c = col(&b.color, &format!("{p}/blob/color"))?;
                blend(&mut img, c, &blob_alpha(g, b));
            }
            TargetOp::Polygon(pg) => {
                let c = col(&pg.color, &format!("{p}/polygon/color"))?;
                let mut m = raster::polygon(g, &pg.points);
                if pg.noise > 0.0 {
                    m = raster::noisy(g, &m, pg.noise, 0.05, pg.seed);
                }
                if pg.softness > 0.0 {
                    m = blur(&m, g.w, g.h, (pg.softness * g.pw).max(0.5));
                }
                let a: Vec<f32> = m.iter().map(|v| (pg.strength * *v as f64) as f32).collect();
                blend(&mut img, c, &a);
            }
            TargetOp::Bands(b) => {
                let poly = raster::polygon(g, &b.points);
                let (y0, y1) = b.points.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, z), p| (a.min(p[1]), z.max(p[1])));
                for (k, (f0, f1, c)) in b.bands.iter().enumerate() {
                    let c = col(c, &format!("{p}/bands/bands/{k}/2"))?;
                    let (lo, hi) = (y0 + f0 * (y1 - y0), y0 + f1 * (y1 - y0));
                    let mut m = poly.clone();
                    for j in 0..g.h {
                        let y = g.y(j);
                        if !(y >= lo && y < hi) {
                            m[j * g.w..(j + 1) * g.w].iter_mut().for_each(|v| *v = 0.0);
                        }
                    }
                    blend(&mut img, c, &blur(&m, g.w, g.h, (b.softness * g.pw).max(0.5)));
                }
            }
            TargetOp::Glow(gl) => {
                let c = col(&gl.color, &format!("{p}/glow/color"))?;
                let a = g.map(|x, y| {
                    let d = ((x - gl.c[0]) * (x - gl.c[0]) + (y - gl.c[1]) * (y - gl.c[1])).sqrt() / gl.r;
                    (gl.strength * exp(-pow(d, gl.power))) as f32
                });
                add_light(&mut img, c, &a);
            }
            TargetOp::Image(im) => {
                let Some(s) = fields.get(&im.field) else { continue };
                let (rgb, cov) = image_layer(g, canvas_h, s, im.fit);
                let mask = im.mask.as_ref().map(|m| raster::shape(g, m, fields));
                for (k, px) in img.iter_mut().enumerate() {
                    let a = (im.strength as f32 * cov[k] * mask.as_ref().map_or(1.0, |m| m[k].clamp(0.0, 1.0))).clamp(0.0, 1.0);
                    for c in 0..3 {
                        px[c] = px[c] * (1.0 - a) + rgb[k][c] * a;
                    }
                }
            }
            TargetOp::Beam(b) => {
                let c = col(&b.color, &format!("{p}/beam/color"))?;
                let w = raster::wedge(g, b.apex, b.angle, b.spread, b.length);
                let w = blur(&w, g.w, g.h, (b.softness * b.spread * 0.02 * g.pw).max(1.0));
                let a: Vec<f32> = w.iter().map(|v| (b.strength * *v as f64) as f32).collect();
                add_light(&mut img, c, &a);
            }
        }
    }
    for p in &mut img {
        *p = p.map(|v| v.clamp(0.0, 1.0));
    }
    Ok(img)
}

/// Lowest value of `fbm - 0.5` for any lattice values in [0, 1): bicubic's negative lobes (Keys, a = -0.75) reach
/// 2 x 0.1875 x 1.1875 = 0.4453 below 0 in 2-D.
const NOISE_LOW: f64 = 0.9454;

/// v1's blob: alpha = strength x clip((1 - d') / softness), d' the elliptical distance scaled by (1 + noise x
/// (fbm - 0.5)). Evaluated only on the window where d' < 1 can hold.
fn blob_alpha(g: &Grid, b: &Blob) -> Vec<f32> {
    let mut a = vec![0f32; g.len()];
    let reach = if b.noise > 0.0 { 1.0 - b.noise * NOISE_LOW } else { 1.0 };
    let win = if reach > 0.0 { g.window(b.c, b.r[0] / reach, b.r[1] / reach) } else { (0, 0, g.w, g.h) };
    let (x0, y0, x1, y1) = win;
    let ww = x1 - x0;
    let n = if b.noise > 0.0 { oil_image::noise::fbm_window(g.pw, win, b.r[0] * 0.8, 3, b.seed) } else { Vec::new() };
    let soft = b.softness.max(1e-3);
    for j in y0..y1 {
        let y = g.y(j);
        for i in x0..x1 {
            let (ex, ey) = ((g.x(i) - b.c[0]) / b.r[0], (y - b.c[1]) / b.r[1]);
            let mut d = (ex * ex + ey * ey).sqrt();
            if b.noise > 0.0 {
                d *= 1.0 + b.noise * (n[(j - y0) * ww + (i - x0)] as f64 - 0.5);
            }
            a[j * g.w + i] = (b.strength * clip01((1.0 - d) / soft)) as f32;
        }
    }
    a
}

// ------------------------------------------------------------------ flows

/// Flow along the target's edges: the minor eigenvector of the smoothed structure tensor (Sobel gradients of
/// luma, Gaussian sigma = max(1, 0.02 w)).
pub fn structure_flow(target: &[[f32; 3]], g: &Grid) -> Vec<[f32; 2]> {
    let gray: Vec<f32> = target.iter().map(|p| 0.299 * p[0] + 0.587 * p[1] + 0.114 * p[2]).collect();
    let (gx, gy) = (sobel(&gray, g.w, g.h, true), sobel(&gray, g.w, g.h, false));
    let sigma = (0.02 * g.pw).max(1.0);
    let prod = |a: &[f32], b: &[f32]| -> Vec<f32> { a.iter().zip(b).map(|(x, y)| x * y).collect() };
    let jxx = blur(&prod(&gx, &gx), g.w, g.h, sigma);
    let jxy = blur(&prod(&gx, &gy), g.w, g.h, sigma);
    let jyy = blur(&prod(&gy, &gy), g.w, g.h, sigma);
    (0..g.len())
        .map(|i| {
            let th = 0.5 * atan2(2.0 * jxy[i] as f64, jxx[i] as f64 - jyy[i] as f64);
            [(-sin(th)) as f32, cos(th) as f32]
        })
        .collect()
}

// ------------------------------------------------------------------ light

fn light_map(plan: &ScenePlan, g: &Grid) -> Vec<f32> {
    let mut lm = vec![0f64; g.len()];
    for l in &plan.lights {
        match l {
            Light::Glow(gl) => {
                for (k, v) in g.map(|x, y| {
                    let (dx, dy) = (x - gl.c[0], (y - gl.c[1]) * 2.5);
                    let d = (dx * dx + dy * dy).sqrt() / gl.r;
                    exp(-d * d) as f32
                })
                .iter()
                .enumerate()
                {
                    lm[k] += gl.strength * *v as f64;
                }
            }
            Light::Beam(b) => {
                let w = raster::wedge(g, b.apex, b.angle, b.spread, b.length);
                for (k, v) in blur(&w, g.w, g.h, (0.02 * g.pw).max(1.0)).iter().enumerate() {
                    lm[k] += b.strength * *v as f64;
                }
            }
            Light::Lamp(lp) => {
                for (k, v) in g.map(|x, y| {
                    let d = ((x - lp.c[0]) * (x - lp.c[0]) + (y - lp.c[1]) * (y - lp.c[1])).sqrt() / lp.r;
                    exp(-d * d) as f32
                })
                .iter()
                .enumerate()
                {
                    lm[k] += lp.strength * *v as f64;
                }
            }
        }
    }
    lm.iter().map(|v| clip01(*v) as f32).collect()
}

// ------------------------------------------------------------------ queries and digests

impl Guides {
    /// The flow for a stroke of `region` at (x, y) in cw: the region's authored flow if it has one; otherwise the
    /// authored flow of the region owning that pixel, or the structure-tensor flow (bilinear) where there is none.
    pub fn flow_at(&self, region: Option<usize>, x: f64, y: f64) -> [f32; 2] {
        if let Some(Some(f)) = region.map(|r| &self.flows[r]) {
            return f.at(x, y);
        }
        let (i, j) = self.pixel(x, y);
        if let Some(f) = &self.flows[self.region_id[j * self.w + i] as usize] {
            return f.at(x, y);
        }
        self.structure_at(x, y)
    }

    fn pixel(&self, x: f64, y: f64) -> (usize, usize) {
        let pw = self.w as f64;
        (((x * pw).floor().max(0.0) as usize).min(self.w - 1), ((y * pw).floor().max(0.0) as usize).min(self.h - 1))
    }

    /// Bilinear structure-tensor flow at (x, y) in cw, renormalised.
    pub fn structure_at(&self, x: f64, y: f64) -> [f32; 2] {
        let pw = self.w as f64;
        let (u, v) = ((x * pw - 0.5).clamp(0.0, (self.w - 1) as f64), (y * pw - 0.5).clamp(0.0, (self.h - 1) as f64));
        let (i0, j0) = (u.floor() as usize, v.floor() as usize);
        let (i1, j1) = ((i0 + 1).min(self.w - 1), (j0 + 1).min(self.h - 1));
        let (tu, tv) = (u - i0 as f64, v - j0 as f64);
        let p = |i: usize, j: usize, c: usize| self.structure[j * self.w + i][c] as f64;
        let lerp2 = |c: usize| {
            let top = p(i0, j0, c) + tu * (p(i1, j0, c) - p(i0, j0, c));
            let bot = p(i0, j1, c) + tu * (p(i1, j1, c) - p(i0, j1, c));
            top + tv * (bot - top)
        };
        unit(lerp2(0), lerp2(1))
    }

    /// The global flow at every pixel centre: the owning region's authored flow, or the structure-tensor flow
    /// (for previews, exports and the cross-host digest).
    pub fn flow_raster(&self) -> Vec<[f32; 2]> {
        let pw = self.w as f64;
        let mut out = Vec::with_capacity(self.w * self.h);
        for j in 0..self.h {
            for i in 0..self.w {
                let k = j * self.w + i;
                out.push(match &self.flows[self.region_id[k] as usize] {
                    Some(f) => f.at((i as f64 + 0.5) / pw, (j as f64 + 0.5) / pw),
                    None => {
                        let s = self.structure[k];
                        unit(s[0] as f64, s[1] as f64)
                    }
                });
            }
        }
        out
    }

    /// A region's authored flow over the whole canvas, or None (for exports).
    pub fn region_flow_raster(&self, r: usize) -> Option<Vec<[f32; 2]>> {
        let f = self.flows[r].as_ref()?;
        let pw = self.w as f64;
        Some((0..self.h).flat_map(|j| (0..self.w).map(move |i| (i, j))).map(|(i, j)| f.at((i as f64 + 0.5) / pw, (j as f64 + 0.5) / pw)).collect())
    }

    /// SHA-256 of each plane (f32 little-endian; region ids as bytes; masks as their boxes and 16-bit values): the
    /// determinism check across hosts.
    pub fn hashes(&self) -> std::collections::BTreeMap<&'static str, String> {
        let hex = |d: sha2::digest::Output<Sha256>| d.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let mut m = std::collections::BTreeMap::new();
        m.insert("target", sha256_f32(self.target.iter().flatten().copied()));
        m.insert("regionId", hex(Sha256::digest(&self.region_id)));
        let mut mh = Sha256::new();
        for mk in &self.masks {
            for v in [mk.x0, mk.y0, mk.w, mk.h] {
                mh.update((v as u32).to_le_bytes());
            }
            for v in &mk.data {
                mh.update(v.to_le_bytes());
            }
        }
        m.insert("masks", hex(mh.finalize()));
        m.insert("flow", sha256_f32(self.flow_raster().iter().flatten().copied()));
        m.insert("light", sha256_f32(self.light.iter().copied()));
        m
    }
}
