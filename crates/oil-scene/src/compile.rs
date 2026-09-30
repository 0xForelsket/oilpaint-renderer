//! The scene compiler: a ScenePlan at a given width becomes the guide maps the planner paints from.
//!
//! - target: RGB reference image (sRGB, [0, 1]);
//! - region_id: the hard region map (later regions override earlier ones), and one soft mask per region;
//! - flow: unit stroke directions: each pixel takes the authored flow of the region that owns it, or the target's
//!   structure-tensor flow; each authored flow is also kept whole (`region_flows`), since a region's strokes
//!   follow its own field wherever they go;
//! - light: the light map in [0, 1].
use crate::color;
use crate::raster::{self, clip01, Fields, Grid};
use crate::spec::*;
use oil_errors::Error;
use oil_image::{blur, gaussian_blur, geom, sobel};
use oil_math::{atan2, cos, exp, pow, sin};
use oil_mix::Mixer;
use sha2::{Digest, Sha256};

pub struct Guides {
    pub w: usize,
    pub h: usize,
    pub ground: [f32; 3],
    /// Row-major sRGB.
    pub target: Vec<[f32; 3]>,
    pub region_id: Vec<u8>,
    /// Region names, in id order.
    pub names: Vec<String>,
    /// Soft masks, in region order.
    pub masks: Vec<Vec<f32>>,
    pub flow: Vec<[f32; 2]>,
    /// The authored flow of each region, whole-canvas (None: the region follows `flow`).
    pub region_flows: Vec<Option<Vec<[f32; 2]>>>,
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

/// Check supplied fields against their declarations and resample them to the grid (bilinear).
fn resolve_fields(plan: &ScenePlan, g: &Grid, supplied: &std::collections::BTreeMap<String, FieldData>) -> Result<Fields, Vec<Error>> {
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
        let mut res = vec![0f32; g.len() * nc];
        for c in 0..nc {
            let plane: Vec<f32> = f.data.iter().skip(c).step_by(nc).copied().collect();
            let r = oil_image::resize_linear(&plane, fw, fh, g.w, g.h);
            for (i, v) in r.into_iter().enumerate() {
                res[i * nc + c] = v;
            }
        }
        if decl.kind == FieldKind::Flow {
            for v in res.chunks_exact_mut(2) {
                let n = ((v[0] * v[0] + v[1] * v[1]) as f64).sqrt() + 1e-6;
                v[0] = (v[0] as f64 / n) as f32;
                v[1] = (v[1] as f64 / n) as f32;
            }
        }
        out.insert(name.clone(), res);
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
    let fields = resolve_fields(plan, &g, fields)?;
    let col = |c: &ColorSpec, p: &str| color::resolve(c, mixer, p).map_err(|e| vec![e]);
    let ground = col(&plan.canvas.ground, "/canvas/ground")?;

    let target = render_target(plan, &g, &fields, &col)?;
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
        masks.push(blur(&m, g.w, g.h, (r.edge * g.pw).max(0.5)));
    }
    let t2 = clock();

    let st = structure_flow(&target, &g);
    let mut region_flows = Vec::with_capacity(n);
    for r in &plan.regions {
        region_flows.push(r.flow.as_ref().map(|f| flow_field(&g, f, &fields)));
    }
    let mut flow = st;
    for (k, (f, id)) in flow.iter_mut().zip(&region_id).enumerate() {
        if let Some(rf) = &region_flows[*id as usize] {
            *f = rf[k];
        }
        *f = unit(f[0] as f64, f[1] as f64);
    }
    let t3 = clock();

    let light = light_map(plan, &g);
    let t4 = clock();

    let guides = Guides {
        w: g.w,
        h: g.h,
        ground,
        target,
        region_id,
        names: plan.regions.iter().map(|r| r.name.clone()).collect(),
        masks,
        flow,
        region_flows,
        light,
    };
    let timings = Timings { target_ms: t1 - t0, regions_ms: t2 - t1, flow_ms: t3 - t2, light_ms: t4 - t3, total_ms: t4 - t0 };
    Ok((guides, timings))
}

#[inline(always)]
fn unit(dx: f64, dy: f64) -> [f32; 2] {
    let n = (dx * dx + dy * dy).sqrt() + 1e-6;
    [(dx / n) as f32, (dy / n) as f32]
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

fn render_target(plan: &ScenePlan, g: &Grid, fields: &Fields, col: &Col) -> Result<Vec<[f32; 3]>, Vec<Error>> {
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

fn angles_to_unit(a: Vec<f64>) -> Vec<[f32; 2]> {
    a.into_iter().map(|a| unit(cos(a), sin(a))).collect()
}

/// (fbm - 0.5) x 2 x amount, as angles in radians (v1's flow noise term).
fn noise_term(g: &Grid, scale: f64, octaves: u32, seed: u32, amount: f64) -> Vec<f64> {
    g.fbm(scale, octaves, seed).iter().map(|n| (*n as f64 - 0.5) * 2.0 * amount).collect()
}

fn constant(g: &Grid, angle: f64, noise: f64, seed: u32) -> Vec<[f32; 2]> {
    let a0 = angle.to_radians();
    if noise > 0.0 {
        angles_to_unit(noise_term(g, 0.15, 3, seed, noise).into_iter().map(|n| a0 + n).collect())
    } else {
        vec![unit(cos(a0), sin(a0)); g.len()]
    }
}

pub fn flow_field(g: &Grid, f: &Flow, fields: &Fields) -> Vec<[f32; 2]> {
    match f {
        Flow::Constant(c) => constant(g, c.angle, c.noise, c.seed),
        Flow::Sweep(s) => {
            let curl = noise_term(g, s.scale, 2, s.seed, s.curl);
            let nz = noise_term(g, s.scale * 0.35, 3, s.seed.wrapping_add(1), s.noise);
            let a0 = s.angle.to_radians();
            angles_to_unit(curl.iter().zip(&nz).map(|(c, n)| a0 + c + n).collect())
        }
        Flow::Waves(w) => {
            // depth runs from the horizon (0) to the bottom edge of the canvas (1)
            let bottom = g.h as f64 / g.pw;
            let phase = g.fbm(0.3, 2, w.seed);
            let nz = noise_term(g, 0.08, 3, w.seed.wrapping_add(1), w.noise);
            let (a0, amp) = (w.angle.to_radians(), w.amplitude.to_radians());
            let tau = 2.0 * std::f64::consts::PI;
            let mut a = Vec::with_capacity(g.len());
            for j in 0..g.h {
                let y = g.y(j);
                let depth = if w.perspective { clip01((y - w.horizon) / (bottom - w.horizon).max(1e-3)) } else { 1.0 };
                let wl = w.wavelength * (0.3 + 0.7 * depth);
                for i in 0..g.w {
                    let k = j * g.w + i;
                    a.push(a0 + amp * sin(tau * g.x(i) / wl + 6.0 * phase[k] as f64) + nz[k] * (0.3 + 0.7 * depth));
                }
            }
            angles_to_unit(a)
        }
        Flow::SwirlAround(s) => {
            let base = constant(g, -8.0, s.noise, s.seed);
            let mut out = Vec::with_capacity(g.len());
            for j in 0..g.h {
                let y = g.y(j);
                for i in 0..g.w {
                    let x = g.x(i);
                    let (mut dx, mut dy) = (0.0, 0.0);
                    for c in &s.centres {
                        let (ex, ey) = ((x - c[0]) / c[2], (y - c[1]) / c[3]);
                        let d = (ex * ex + ey * ey).sqrt() + 1e-6;
                        let wgt = exp(-d * d * 0.7);
                        dx += wgt * (-ey / d);
                        dy += wgt * (ex / d);
                    }
                    let b = base[j * g.w + i];
                    out.push(unit(s.strength * dx + (1.0 - s.strength) * b[0] as f64, s.strength * dy + (1.0 - s.strength) * b[1] as f64));
                }
            }
            out
        }
        Flow::RadialFrom(r) => {
            let nz = noise_term(g, 0.1, 2, r.seed, r.noise);
            let mut a = Vec::with_capacity(g.len());
            for j in 0..g.h {
                let y = g.y(j);
                for i in 0..g.w {
                    a.push(atan2(y - r.c[1], g.x(i) - r.c[0]) + nz[j * g.w + i]);
                }
            }
            angles_to_unit(a)
        }
        Flow::Upward(u) => constant(g, -90.0, u.noise * 1.2, u.seed),
        Flow::Contour(c) => {
            // tangent of the signed distance to the outline (positive inside), smoothed
            let m = raster::polygon(g, &c.polygon);
            let inside: Vec<bool> = m.iter().map(|v| *v > 0.5).collect();
            let outside: Vec<bool> = inside.iter().map(|b| !b).collect();
            let (di, dout) = (geom::edt(&inside, g.w, g.h), geom::edt(&outside, g.w, g.h));
            let d: Vec<f32> = di.iter().zip(&dout).map(|(a, b)| a - b).collect();
            let d = gaussian_blur(&d, g.w, g.h, (0.01 * g.pw).max(1.0));
            let (gx, gy) = (sobel(&d, g.w, g.h, true), sobel(&d, g.w, g.h, false));
            let nz = noise_term(g, 0.1, 3, c.seed, c.noise);
            let half_pi = 0.5 * std::f64::consts::PI;
            angles_to_unit((0..g.len()).map(|i| atan2(gy[i] as f64, gx[i] as f64) + half_pi + nz[i]).collect())
        }
        Flow::Field(name) => fields.get(name).map(|f| f.chunks_exact(2).map(|v| [v[0], v[1]]).collect()).unwrap_or_else(|| vec![[1.0, 0.0]; g.len()]),
    }
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

// ------------------------------------------------------------------ digests

impl Guides {
    /// SHA-256 of each plane (f32 little-endian; region ids as bytes): the determinism check across hosts.
    pub fn hashes(&self) -> std::collections::BTreeMap<&'static str, String> {
        let mut m = std::collections::BTreeMap::new();
        m.insert("target", sha256_f32(self.target.iter().flatten().copied()));
        m.insert("regionId", Sha256::digest(&self.region_id).iter().map(|b| format!("{b:02x}")).collect());
        m.insert("masks", sha256_f32(self.masks.iter().flatten().copied()));
        m.insert("flow", sha256_f32(self.flow.iter().flatten().copied()));
        m.insert("regionFlows", sha256_f32(self.region_flows.iter().flatten().flatten().flatten().copied()));
        m.insert("light", sha256_f32(self.light.iter().copied()));
        m
    }
}
