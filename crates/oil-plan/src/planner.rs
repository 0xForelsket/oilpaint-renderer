//! The planner (the L3 cutover from v1's `oilpaint/planner.py`). Per layer and per region, it proposes sites
//! (where the canvas differs from the reference, on an even grid, or along curves), orders them in painterly
//! sweeps, and traces a flow-following path for each. It picks a colour (reference sample, palette snap, jitter,
//! flecks, warmth) and paints the stroke on a proxy canvas, so later sites see what earlier strokes did.
//!
//! The proxy is painted exactly as `oil_paint::paint` replays a StrokeList: the same cw points scaled by the same
//! f32 width, the same loads, height blur and drying. A StrokeList painted at the plan width is therefore
//! bit-identical to the proxy (tested).
use crate::boundary::outer_contours;
use crate::color::{self, dist_fast, lab_fast, Palette};
use crate::rng::Rng;
use crate::style::{mode_of, resolve, St};
use oil_kernel::brush::{length_in_widths, render_stroke_len, BrushParams, MODE_SCUMBLE};
use oil_kernel::{Canvas, Load};
use oil_mix::Mixer;
use oil_scene::spec::{ColorFrom, CurveSpec, Layer, NumOrMap, Order, Placement, RegionSel, ScenePlan, Style};
use oil_scene::{Guides, Mask};
use oil_strokes::Stroke;
use std::collections::BTreeMap;

/// Tile size of the proxy's Lab cache.
const TILE: usize = 32;

/// CIELAB of the proxy canvas, refreshed tile by tile where strokes have painted since the last read (the
/// incremental error planes of plan section 3).
struct LabCache {
    w: usize,
    h: usize,
    tw: usize,
    lab: Vec<[f32; 3]>,
    dirty: Vec<bool>,
}

impl LabCache {
    fn new(w: usize, h: usize) -> LabCache {
        let (tw, th) = (w.div_ceil(TILE), h.div_ceil(TILE));
        LabCache { w, h, tw, lab: vec![[0.0; 3]; w * h], dirty: vec![true; tw * th] }
    }

    fn mark(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) {
        let clampx = |v: f64| (v.max(0.0) as usize).min(self.w - 1) / TILE;
        let clampy = |v: f64| (v.max(0.0) as usize).min(self.h - 1) / TILE;
        if x1 < 0.0 || y1 < 0.0 || x0 >= self.w as f64 || y0 >= self.h as f64 {
            return;
        }
        for ty in clampy(y0)..=clampy(y1) {
            for tx in clampx(x0)..=clampx(x1) {
                self.dirty[ty * self.tw + tx] = true;
            }
        }
    }

    /// Bring the box `x0..x1, y0..y1` up to date from the proxy's display colours.
    fn refresh(&mut self, rgb: &[[f32; 3]], x0: usize, y0: usize, x1: usize, y1: usize) {
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        for ty in y0 / TILE..=(y1 - 1) / TILE {
            for tx in x0 / TILE..=(x1 - 1) / TILE {
                let k = ty * self.tw + tx;
                if !self.dirty[k] {
                    continue;
                }
                for j in ty * TILE..((ty + 1) * TILE).min(self.h) {
                    for i in tx * TILE..((tx + 1) * TILE).min(self.w) {
                        self.lab[j * self.w + i] = lab_fast(rgb[j * self.w + i]);
                    }
                }
                self.dirty[k] = false;
            }
        }
    }
}

/// Blurred copies of the target (v1's `GuideMaps.reference`), keyed by sigma in hundredths of a pixel; a few are
/// kept, recomputed on demand (the values do not depend on the cache).
struct References {
    keep: Vec<(i64, Vec<[f32; 3]>)>,
}

impl References {
    fn get(&mut self, g: &Guides, sigma: f64) -> &[[f32; 3]] {
        let key = (sigma * 100.0).round() as i64;
        if let Some(i) = self.keep.iter().position(|(k, _)| *k == key) {
            let e = self.keep.remove(i);
            self.keep.push(e);
        } else {
            let img = if sigma > 0.3 {
                let s = key as f64 / 100.0;
                let ch: Vec<Vec<f32>> = (0..3).map(|c| oil_image::blur(&g.target.iter().map(|p| p[c]).collect::<Vec<_>>(), g.w, g.h, s)).collect();
                (0..g.w * g.h).map(|k| [ch[0][k], ch[1][k], ch[2][k]]).collect()
            } else {
                g.target.clone()
            };
            if self.keep.len() >= 4 {
                self.keep.remove(0);
            }
            self.keep.push((key, img));
        }
        &self.keep.last().expect("just pushed").1
    }
}

/// What a layer did, per region (the planning report).
#[derive(Clone, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionReport {
    pub region: String,
    pub sites: usize,
    pub strokes: usize,
    pub gap_strokes: usize,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerReport {
    pub name: String,
    pub strokes: usize,
    pub regions: Vec<RegionReport>,
    pub ms: f64,
}

pub struct Planner<'a, M: Mixer> {
    m: &'a M,
    pub g: Guides,
    plan: &'a ScenePlan,
    seed: u32,
    pub cv: Canvas<M>,
    wf: f32,
    pw: f64,
    lab: LabCache,
    refs: References,
    palettes: BTreeMap<Vec<u32>, Palette<M::State>>,
    detail: Option<Vec<f32>>,
    preset: Option<Style>,
    white: [f32; 3],
    pub points: Vec<[f32; 4]>,
    pub offsets: Vec<u32>,
    pub strokes: Vec<Stroke>,
    pub layers: Vec<oil_strokes::Layer>,
    pub report: Vec<LayerReport>,
    /// Per region: drawn stroke widths (cw, after sizeByY) min and max, and strokes whose path stopped after one
    /// step (two points).
    pub drawn: Vec<([f64; 2], usize)>,
}

/// v1's stroke profile: width factor and pressure along t in [0, 1].
fn profile(t: f64, end_width: f64, end_pressure: f64) -> (f64, f64) {
    let ss = |e0: f64, e1: f64, x: f64| {
        let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let (start, end) = (0.12, 0.20);
    let wf = (0.35 + 0.65 * ss(0.0, start, t)) * (1.0 - (1.0 - end_width) * ss(1.0 - end, 1.0, t));
    let pr = ss(0.0, 0.05, t) * (1.0 - (1.0 - end_pressure) * ss(1.0 - end, 1.0, t));
    (wf, pr)
}

fn rotate(d: (f64, f64), a: f64) -> (f64, f64) {
    let (c, s) = (oil_math::cos(a), oil_math::sin(a));
    (c * d.0 - s * d.1, s * d.0 + c * d.1)
}

fn norm(d: (f64, f64)) -> (f64, f64) {
    let n = (d.0 * d.0 + d.1 * d.1).sqrt() + 1e-9;
    (d.0 / n, d.1 / n)
}

impl<'a, M: Mixer> Planner<'a, M> {
    pub fn new(m: &'a M, plan: &'a ScenePlan, g: Guides, seed: u32) -> Planner<'a, M> {
        let n_regions = g.names.len();
        let cv = Canvas::new(m, g.w, g.h, g.ground);
        let lab = LabCache::new(g.w, g.h);
        let (wf, pw) = (g.w as f32, g.w as f64);
        let preset = plan.preset.as_deref().and_then(oil_scene::presets::get);
        let white = oil_scene::color::named("lead_white", "").unwrap_or([0.98, 0.97, 0.94]);
        Planner {
            m,
            g,
            plan,
            seed,
            cv,
            wf,
            pw,
            lab,
            refs: References { keep: Vec::new() },
            palettes: BTreeMap::new(),
            detail: None,
            preset,
            white,
            points: Vec::new(),
            offsets: vec![0],
            strokes: Vec::new(),
            layers: Vec::new(),
            report: Vec::new(),
            drawn: vec![([f64::INFINITY, 0.0], 0); n_regions],
        }
    }

    fn style_of(&self, r: usize, layer: &Layer) -> St {
        let name = &self.g.names[r];
        resolve(self.m, self.preset.as_ref(), self.plan.styles.get(name), layer)
    }

    fn regions_of(&self, layer: &Layer) -> Vec<usize> {
        match &layer.regions {
            RegionSel::Name(_) => (0..self.g.names.len()).collect(),
            RegionSel::List(v) => v.iter().filter_map(|n| self.g.names.iter().position(|x| x == n)).collect(),
        }
    }

    fn flow(&self, r: usize, x: f64, y: f64) -> (f64, f64) {
        let f = self.g.flow_at(Some(r), x / self.pw, y / self.pw);
        (f[0] as f64, f[1] as f64)
    }

    fn pixel(&self, x: f64, y: f64) -> (usize, usize) {
        (((x.max(0.0)) as usize).min(self.g.w - 1), ((y.max(0.0)) as usize).min(self.g.h - 1))
    }

    fn mask_at(&self, r: usize, x: f64, y: f64) -> f32 {
        let (i, j) = self.pixel(x, y);
        self.g.masks[r].at(i, j)
    }

    /// Local detail of the target in [0, 1]: blurred gradient magnitude of L*, divided by its 95th percentile.
    fn detail_at(&mut self, x: f64, y: f64) -> f64 {
        if self.detail.is_none() {
            let (w, h) = (self.g.w, self.g.h);
            let l: Vec<f32> = self.g.target.iter().map(|p| lab_fast(*p)[0]).collect();
            let (gx, gy) = (oil_image::sobel(&l, w, h, true), oil_image::sobel(&l, w, h, false));
            let mag: Vec<f32> = gx.iter().zip(&gy).map(|(a, b)| (a * a + b * b).sqrt()).collect();
            let mag = oil_image::blur(&mag, w, h, (0.01 * self.pw).max(1.0));
            let mut sorted = mag.clone();
            sorted.sort_by(f32::total_cmp);
            let p95 = sorted[(sorted.len() * 95 / 100).min(sorted.len() - 1)].max(1e-6);
            self.detail = Some(mag.iter().map(|v| (v / p95).min(1.0)).collect());
        }
        let (i, j) = self.pixel(x, y);
        self.detail.as_ref().expect("computed above")[j * self.g.w + i] as f64
    }

    fn palette(&mut self, colors: &[[f32; 3]]) -> &Palette<M::State> {
        let key: Vec<u32> = colors.iter().flatten().map(|v| v.to_bits()).collect();
        let (m, white) = (self.m, self.white);
        self.palettes.entry(key).or_insert_with(|| Palette::new(m, colors, white))
    }

    /// A stroke's colour at plan pixel (x, y): reference sample (or a palette colour), palette snap, Lab jitter and
    /// floor, flecks, warmth from the light map.
    #[allow(clippy::too_many_arguments)]
    fn stroke_color(&mut self, reference: &[[f32; 3]], x: f64, y: f64, rpx: f64, st: &St, layer: &Layer, rng: &mut Rng) -> [f32; 3] {
        let (w, h) = (self.g.w, self.g.h);
        let r = (rpx as usize).max(1);
        let (xi, yi) = self.pixel(x, y);
        let mut rgb = if layer.color_from == ColorFrom::Palette && !st.colors.is_empty() {
            st.colors[rng.below(st.colors.len() as u32) as usize]
        } else {
            let (x0, x1, y0, y1) = (xi.saturating_sub(r), (xi + r + 1).min(w), yi.saturating_sub(r), (yi + r + 1).min(h));
            let mut s = [0.0f64; 3];
            for j in y0..y1 {
                for i in x0..x1 {
                    for c in 0..3 {
                        s[c] += reference[j * w + i][c] as f64;
                    }
                }
            }
            let n = ((x1 - x0) * (y1 - y0)) as f64;
            s.map(|v| (v / n) as f32)
        };
        if !st.colors.is_empty() && st.snap > 0.0 {
            let m = self.m;
            let colors = st.colors.clone();
            rgb = self.palette(&colors).snap(m, rgb, st.snap as f32);
        }
        let mut lab = color::lab(rgb);
        lab[0] += rng.normal() * st.jitter[0];
        lab[1] += rng.normal() * st.jitter[1];
        lab[2] += rng.normal() * st.jitter[1];
        lab[0] = lab[0].max(st.l_floor);
        rgb = color::lab_to_rgb(lab);
        for (c, p) in &st.flecks {
            if rng.uniform() < *p {
                rgb = color::mix(self.m, rgb, *c, 0.75);
                break;
            }
        }
        if st.warmth > 0.0 {
            let lm = self.g.light[yi * w + xi] as f64;
            if lm > 0.02 {
                rgb = color::mix(self.m, rgb, st.warm_color, (lm * st.warmth).clamp(0.0, 0.8) as f32);
            }
        }
        rgb
    }

    /// Move a start near the canvas edge off the canvas when the flow runs inward, so strokes enter from outside.
    fn edge_start(&self, x: f64, y: f64, width: f64, r: usize, rng: &mut Rng) -> (f64, f64) {
        let (w, h) = (self.g.w as f64, self.g.h as f64);
        let f = self.flow(r, x, y);
        let m = 1.2 * width;
        let (mut x, mut y) = (x, y);
        if x < m && f.0 > 0.2 {
            x = x - rng.range(0.5, 1.0) * width - x * rng.uniform();
        } else if x > w - m && f.0 < -0.2 {
            x = x + rng.range(0.5, 1.0) * width + (w - x) * rng.uniform();
        }
        if y < m && f.1 > 0.2 {
            y = y - rng.range(0.5, 1.0) * width - y * rng.uniform();
        } else if y > h - m && f.1 < -0.2 {
            y = y + rng.range(0.5, 1.0) * width + (h - y) * rng.uniform();
        }
        (x, y)
    }

    /// Follow the region's flow from (x0, y0) (plan pixels): per-stroke angle jitter, curvature toward the flow,
    /// wobble, an optional flick at the end, stopping where the soft mask fades.
    #[allow(clippy::too_many_arguments)]
    fn path(&self, x0: f64, y0: f64, width: f64, length: f64, st: &St, r: usize, rng: &mut Rng, ignore_mask: bool) -> Vec<(f64, f64)> {
        let step = (0.5 * width).max(1.0);
        let mut n = ((length / step) as usize + 1).max(3);
        let mut d = self.flow(r, x0, y0);
        if (d.0 * d.0 + d.1 * d.1).sqrt() < 1e-3 {
            d = (1.0, 0.0);
        }
        d = norm(d);
        d = rotate(d, (1.0 - st.align) * rng.normal() * 0.7);
        if rng.uniform() < st.reverse_p {
            d = (-d.0, -d.1);
        }
        let fc = st.curvature;
        // back up so that the stroke's full-pressure part passes over the site
        let back = rng.range(0.5, 1.0) * width;
        let (mut x, mut y) = (x0 - d.0 * back, y0 - d.1 * back);
        n += (back / step) as usize + 1;
        // a flick: a curl of up to 40 degrees over the last third
        let (flick_from, flick_step) = if st.flick > 0.0 && rng.uniform() < st.flick {
            let total = rng.range(0.3, 1.0) * 40f64.to_radians() * if rng.uniform() < 0.5 { 1.0 } else { -1.0 };
            let from = n * 2 / 3;
            (from, total / (n - from).max(1) as f64)
        } else {
            (usize::MAX, 0.0)
        };
        let (w, h) = (self.g.w as f64, self.g.h as f64);
        let mut pts = vec![(x, y)];
        for i in 1..n {
            let mut f = self.flow(r, x, y);
            if f.0 * d.0 + f.1 * d.1 < 0.0 {
                f = (-f.0, -f.1);
            }
            let nd = norm(((1.0 - fc) * d.0 + fc * f.0, (1.0 - fc) * d.1 + fc * f.1));
            let mut wob = rng.normal() * (0.08 * (1.0 - st.align) + 0.02);
            if i >= flick_from {
                wob += flick_step;
            }
            d = rotate(nd, wob);
            x += d.0 * step;
            y += d.1 * step;
            if x < -width || y < -width || x > w + width || y > h + width {
                break;
            }
            if !ignore_mask && (self.mask_at(r, x, y) as f64) < rng.uniform() * st.stop_at_edge {
                break;
            }
            pts.push((x, y));
        }
        pts
    }

    /// Build a stroke starting at plan pixel (x, y), paint it on the proxy and store it. False if its path was too
    /// short.
    #[allow(clippy::too_many_arguments)]
    fn emit(&mut self, li: usize, layer: &Layer, st: &St, r: usize, reference: &[[f32; 3]], rpx: f64, x: f64, y: f64, width: f64, length: f64, rng: &mut Rng, pressure_floor: f64, ignore_mask: Option<bool>) -> bool {
        let (mut width, mut length) = (width, length);
        if let Some([y0, y1, s0, s1]) = st.size_by_y {
            let t = if y1 != y0 { ((y / self.pw - y0) / (y1 - y0)).clamp(0.0, 1.0) } else { 0.0 };
            let sc = s0 + t * (s1 - s0);
            width *= sc;
            length *= sc;
        }
        let (sx, sy) = self.edge_start(x, y, width, r, rng);
        let ignore = ignore_mask.unwrap_or_else(|| rng.uniform() < st.spill);
        let path = self.path(sx, sy, width, length, st, r, rng, ignore);
        if path.len() < 2 {
            return false;
        }
        let mut s_acc = vec![0.0f64; path.len()];
        for k in 1..path.len() {
            let (dx, dy) = (path[k].0 - path[k - 1].0, path[k].1 - path[k - 1].1);
            s_acc[k] = s_acc[k - 1] + (dx * dx + dy * dy).sqrt();
        }
        let total = s_acc[path.len() - 1].max(1e-6);
        let (mut ew, mut ep) = (st.end_width, st.end_pressure);
        if st.end_variation > 0.0 {
            ew = (ew * (1.0 + st.end_variation * rng.range(-1.0, 1.0))).clamp(0.05, 1.0);
            ep = (ep * (1.0 + st.end_variation * rng.range(-1.0, 1.0))).clamp(0.02, 1.0);
        }
        let pw = self.pw;
        let pts_cw: Vec<[f32; 4]> = path
            .iter()
            .zip(&s_acc)
            .map(|(p, s)| {
                let (wfac, pr) = profile(s / total, ew, ep);
                [(p.0 / pw) as f32, (p.1 / pw) as f32, (width * wfac / pw) as f32, pr.max(pressure_floor) as f32]
            })
            .collect();
        let color = self.stroke_color(reference, x, y, rpx, st, layer, rng);
        let color2 = if st.marble > 0.0 || st.load2.is_some() {
            match st.load2 {
                Some(c) => c,
                None => {
                    let mut l = color::lab(color);
                    if rng.uniform() < 0.6 {
                        l = [l[0] + 9.0, l[1] + 3.0, l[2] + 8.0];
                    } else {
                        l = [l[0] - 7.0, l[1] + 1.0, l[2] - 6.0];
                    }
                    color::lab_to_rgb(l)
                }
            }
        } else {
            color
        };
        let (xi, yi) = self.pixel(x, y);
        let mut opacity = rng.range(st.opacity[0], st.opacity[1]);
        if st.opacity_by_light > 0.0 {
            let lm = self.g.light[yi * self.g.w + xi] as f64;
            opacity *= (1.0 - st.opacity_by_light * (1.0 - lm)).max(0.05);
        }
        let nb = (st.nb_base + st.nb_per_cw * width / self.pw).clamp(3.0, 40.0) as i32;
        let mut hgain = st.hgain * layer.relief * rng.range(1.0 - st.hgain_jitter, 1.0 + st.hgain_jitter);
        if st.relief_by_value > 0.0 {
            let l = color::lab(color)[0];
            hgain *= (1.0 + st.relief_by_value * (l - 50.0) / 50.0).clamp(0.1, 2.0);
        }
        let seed = rng.below((1u32 << 31) - 2) + 1;
        let brush = BrushParams {
            mode: mode_of(st, layer),
            opacity: opacity as f32,
            pickup: st.pickup as f32,
            load: st.load as f32,
            deplete: st.deplete as f32,
            vdry: st.vdry as f32,
            hgain: hgain as f32,
            flatten: st.flatten as f32,
            streak: st.streak as f32,
            hardness: st.hardness as f32,
            grain: st.grain as f32,
            dry_thresh: layer.dry_thresh.unwrap_or(0.0) as f32,
            dry_width: layer.dry_width.unwrap_or(0.15) as f32,
            nb,
            seed,
            dropout: st.dropout as f32,
            ragged: st.ragged as f32,
            body: st.body as f32,
            release: st.release as f32,
            streak_mix: st.streak_mix as f32,
            ridge: st.ridge as f32,
            levee: st.levee as f32,
            furrow: st.furrow as f32,
            blob: st.blob as f32,
            stiff: st.stiff as f32,
            marble: st.marble as f32,
            splay: st.splay as f32,
        };
        // paint the proxy exactly as oil_paint::paint will replay this stroke
        let m = self.m;
        let zcol = m.encode(color);
        let zcol2 = if color2 == color { zcol } else { m.encode(color2) };
        let load = Load { zcol, zcol2, dz: oil_paint::streak_vector(m, &zcol, 1.0) };
        let wf = self.wf;
        let pts_px: Vec<[f32; 4]> = pts_cw.iter().map(|p| [p[0] * wf, p[1] * wf, p[2] * wf, p[3]]).collect();
        render_stroke_len(m, &mut self.cv.planes(), &pts_px, &load, &brush, length_in_widths(&pts_cw));
        let (mut x0, mut y0, mut x1, mut y1, mut wmax) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY, 0.0f64);
        for p in &pts_px {
            x0 = x0.min(p[0] as f64);
            y0 = y0.min(p[1] as f64);
            x1 = x1.max(p[0] as f64);
            y1 = y1.max(p[1] as f64);
            wmax = wmax.max(p[2] as f64);
        }
        let pad = 0.75 * wmax + 2.0;
        self.lab.mark(x0 - pad, y0 - pad, x1 + pad, y1 + pad);
        self.points.extend_from_slice(&pts_cw);
        self.offsets.push(self.points.len() as u32);
        self.strokes.push(Stroke { layer: li as u32, region: r as u32, color, color2, streak_amount: 1.0, brush });
        let d = &mut self.drawn[r];
        d.0 = [d.0[0].min(width / pw), d.0[1].max(width / pw)];
        if path.len() == 2 {
            d.1 += 1;
        }
        true
    }

    /// Error placement: cells of `g` px aligned to the canvas origin; a cell's error is 0.6 x its mean plus 0.4 x its
    /// maximum Lab difference between proxy and reference (weighted by the soft mask), so thin gaps also count.
    fn error_sites(&mut self, layer: &Layer, reference: &[[f32; 3]], r: usize, rpx: f64, rng: &mut Rng) -> Vec<(f64, f64, f64)> {
        let mask = &self.g.masks[r];
        if mask.w == 0 {
            return Vec::new();
        }
        let (w, h) = (self.g.w, self.g.h);
        let g = ((layer.grid_factor * rpx) as usize).max(2);
        let (cx0, cy0) = (mask.x0 / g, mask.y0 / g);
        let (cx1, cy1) = ((mask.x0 + mask.w - 1) / g, (mask.y0 + mask.h - 1) / g);
        let (bx0, by0, bx1, by1) = (cx0 * g, cy0 * g, ((cx1 + 1) * g).min(w), ((cy1 + 1) * g).min(h));
        self.lab.refresh(&self.cv.rgb, bx0, by0, bx1, by1);
        let mask = &self.g.masks[r];
        let mut sites = Vec::new();
        for cy in cy0..=cy1 {
            for cx in cx0..=cx1 {
                let (x0, y0, x1, y1) = (cx * g, cy * g, ((cx + 1) * g).min(w), ((cy + 1) * g).min(h));
                let (mut sum, mut mx, mut arg, mut cover) = (0.0f64, -1.0f32, (x0, y0), 0.0f64);
                for j in y0..y1 {
                    for i in x0..x1 {
                        let mk = mask.at(i, j);
                        cover += mk as f64;
                        let e = if mk > 0.0 { dist_fast(self.lab.lab[j * w + i], lab_fast(reference[j * w + i])) * mk } else { 0.0 };
                        sum += e as f64;
                        if e > mx {
                            mx = e;
                            arg = (i, j);
                        }
                    }
                }
                let n = ((x1 - x0) * (y1 - y0)) as f64;
                let cell = 0.6 * sum / n + 0.4 * mx as f64;
                if cell > layer.error_threshold && cover / n > 0.3 {
                    let jx = rng.range(-0.5, 0.5) * g as f64 * layer.jitter_pos;
                    let jy = rng.range(-0.5, 0.5) * g as f64 * layer.jitter_pos;
                    sites.push((arg.0 as f64 + jx, arg.1 as f64 + jy, cell));
                }
            }
        }
        sites
    }

    /// Density placement: one candidate per grid cell of `spacing` stroke widths, kept with the soft mask's value.
    fn density_sites(&self, layer: &Layer, wpx: f64, r: usize, rng: &mut Rng) -> Vec<(f64, f64, f64)> {
        let mask = &self.g.masks[r];
        if mask.w == 0 {
            return Vec::new();
        }
        let sp = match &layer.spacing {
            NumOrMap::Num(v) => *v,
            NumOrMap::Map(m) => m.get(&self.g.names[r]).copied().unwrap_or(1.6),
        };
        let g = ((sp * wpx) as usize).max(2);
        let mut sites = Vec::new();
        for cy in mask.y0 / g..=(mask.y0 + mask.h - 1) / g {
            for cx in mask.x0 / g..=(mask.x0 + mask.w - 1) / g {
                let x = (cx * g) as f64 + rng.range(0.0, g as f64);
                let y = (cy * g) as f64 + rng.range(0.0, g as f64);
                let (xi, yi) = self.pixel(x, y);
                if (mask.at(xi, yi) as f64) > rng.uniform() {
                    sites.push((x, y, 1.0));
                }
            }
        }
        sites
    }

    /// Curve placement: along the region's outline (soft mask > 0.5) or a polyline, every `curveSpacing` stroke
    /// widths, offset along the normal by `curveOffset` +- `curveJitter` widths.
    fn curve_sites(&self, layer: &Layer, wpx: f64, r: usize, rng: &mut Rng) -> Vec<(f64, f64, f64)> {
        let name = &self.g.names[r];
        let curve = layer.curve.as_ref().and_then(|m| m.get(name));
        let pts: Vec<[f64; 2]> = match curve {
            Some(CurveSpec::Points(poly)) => {
                let arr: Vec<[f64; 2]> = poly.iter().map(|p| [p[0] * self.pw, p[1] * self.pw]).collect();
                let mut out = Vec::new();
                for s in arr.windows(2) {
                    let (a, b) = (s[0], s[1]);
                    let len = ((b[0] - a[0]) * (b[0] - a[0]) + (b[1] - a[1]) * (b[1] - a[1])).sqrt();
                    let n = ((len / 2.0) as usize).max(2);
                    for k in 0..n {
                        let t = k as f64 / n as f64;
                        out.push([a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])]);
                    }
                }
                if out.is_empty() {
                    arr
                } else {
                    out
                }
            }
            _ => {
                let mk: &Mask = &self.g.masks[r];
                outer_contours(mk.x0, mk.y0, mk.x0 + mk.w, mk.y0 + mk.h, |i, j| mk.at(i, j) > 0.5).concat()
            }
        };
        if pts.is_empty() {
            return Vec::new();
        }
        let step = (layer.curve_spacing * wpx).max(1.0);
        let mut d = vec![0.0f64; pts.len()];
        for k in 1..pts.len() {
            let (dx, dy) = (pts[k][0] - pts[k - 1][0], pts[k][1] - pts[k - 1][1]);
            d[k] = d[k - 1] + (dx * dx + dy * dy).sqrt();
        }
        let last = pts.len() - 1;
        let offset = rng.range(0.0, step);
        let mut sites = Vec::new();
        let mut t = offset;
        while t - offset < d[last] {
            let i = d.partition_point(|v| *v < t).min(last);
            let (j, k) = ((i + 3).min(last), i.saturating_sub(3));
            let (tx, ty) = (pts[j][0] - pts[k][0], pts[j][1] - pts[k][1]);
            let nrm = (tx * tx + ty * ty).sqrt() + 1e-6;
            let (nx, ny) = (-ty / nrm, tx / nrm);
            let off = (layer.curve_offset + rng.range(-1.0, 1.0) * layer.curve_jitter) * wpx;
            sites.push((pts[i][0] + nx * off, pts[i][1] + ny * off, 1.0));
            t += step;
        }
        sites
    }

    /// Painterly order: bands of 3 stroke widths, top to bottom, alternating direction, with jitter; or random.
    fn order(&self, sites: Vec<(f64, f64, f64)>, layer: &Layer, rng: &mut Rng, wpx: f64) -> Vec<(f64, f64, f64)> {
        let mut sites = sites;
        if layer.order == Order::Random {
            rng.shuffle(&mut sites);
            return sites;
        }
        let band = (3.0 * wpx).max(4.0);
        let mut keyed: Vec<(i64, f64, (f64, f64, f64))> = sites
            .into_iter()
            .map(|s| {
                let b = ((s.1 + rng.range(-0.3, 0.3) * band) / band).floor() as i64;
                let kx = if b.rem_euclid(2) == 0 { s.0 } else { -s.0 };
                (b, kx + rng.range(-band, band), s)
            })
            .collect();
        keyed.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
        keyed.into_iter().map(|k| k.2).collect()
    }

    /// Plan one layer.
    pub fn plan_layer(&mut self, li: usize, layer: &Layer, clock: &dyn Fn() -> f64) {
        let t0 = clock();
        let start = self.strokes.len();
        let mut rep = LayerReport { name: layer.name.clone(), ..Default::default() };
        if !layer.enabled {
            self.layers.push(oil_strokes::Layer { start: start as u32, end: start as u32, hblur_sigma: None, dry_after: None });
            self.report.push(rep);
            return;
        }
        let mut regs: Vec<(usize, St)> = self.regions_of(layer).into_iter().map(|r| (r, self.style_of(r, layer))).collect();
        regs.sort_by_key(|(_, st)| st.priority);
        // scumble reads the blurred height: blur it once for the layer, exactly as the painter does
        let scumble = regs.iter().any(|(_, st)| mode_of(st, layer) == MODE_SCUMBLE);
        let hblur_sigma = if scumble { Some(layer.hblur_sigma as f32) } else { None };
        if let Some(s) = hblur_sigma {
            self.cv.hblur = oil_image::blur(&self.cv.hgt, self.g.w, self.g.h, (s as f64 * self.g.w as f64).max(1.0));
        }
        for (r, st) in regs {
            let mut rng = Rng::new(&[self.seed as u64, li as u64, layer.seed_offset as u64, r as u64]);
            let mut rr = RegionReport { region: self.g.names[r].clone(), ..Default::default() };
            let (wlo, whi) = (st.width[0], st.width[1]);
            let (llo, lhi) = (st.length[0], st.length[1]);
            let wmean_px = 0.5 * (wlo + whi) * self.pw;
            let rpx = 0.5 * wmean_px;
            let reference: Vec<[f32; 3]> = self.refs.get(&self.g, layer.reference_blur * rpx).to_vec();
            let mut sites = match layer.placement {
                Placement::Error => self.error_sites(layer, &reference, r, rpx, &mut rng),
                Placement::Curve => self.curve_sites(layer, wmean_px, r, &mut rng),
                Placement::Density => self.density_sites(layer, wmean_px, r, &mut rng),
            };
            let cov = match &layer.coverage {
                NumOrMap::Num(v) => *v,
                NumOrMap::Map(m) => m.get(&self.g.names[r]).copied().unwrap_or(1.0),
            };
            if cov < 1.0 {
                sites.retain(|_| rng.uniform() < cov);
            }
            let mut sites = self.order(sites, layer, &mut rng, wmean_px);
            if let Some(n) = layer.max_strokes {
                sites.truncate(n as usize);
            }
            rr.sites = sites.len();
            let (w, h) = (self.g.w, self.g.h);
            for (x, y, _) in sites {
                let (xi, yi) = self.pixel(x, y);
                let k = yi * w + xi;
                if layer.placement == Placement::Error {
                    // re-check now that earlier strokes of this layer have painted here
                    if dist_fast(lab_fast(self.cv.rgb[k]), lab_fast(reference[k])) < (0.5 * layer.error_threshold) as f32 {
                        continue;
                    }
                }
                if layer.max_cover.is_some_and(|mc| self.cv.cover[k] as f64 > mc) {
                    continue;
                }
                let mut u = rng.uniform();
                if st.size_by_detail > 0.0 {
                    u = (1.0 - st.size_by_detail) * u + st.size_by_detail * (1.0 - self.detail_at(x, y));
                }
                let width = (wlo + (whi - wlo) * u) * self.pw;
                let ul = rng.uniform();
                let ul = if st.length_skew > 0.0 { oil_math::pow(ul, 1.0 + st.length_skew) } else { ul };
                let mut length = (llo + (lhi - llo) * ul) * self.pw;
                if st.dab_share > 0.0 && rng.uniform() < st.dab_share {
                    length = width * rng.range(1.0, st.min_aspect.max(1.0));
                } else if !layer.dab {
                    length = length.max(st.min_aspect * width);
                }
                if self.emit(li, layer, &st, r, &reference, rpx, x, y, width, length, &mut rng, 0.0, None) {
                    rr.strokes += 1;
                }
            }
            // gap fill: bare canvas inside the region gets an extra stroke
            if layer.placement == Placement::Error && layer.gap_fill {
                let mk = &self.g.masks[r];
                if mk.w > 0 {
                    let gg = ((0.5 * layer.grid_factor * rpx) as usize).max(2);
                    let (mx0, my0, mx1, my1) = (mk.x0, mk.y0, mk.x0 + mk.w, mk.y0 + mk.h);
                    let mut cy = my0.div_ceil(gg) * gg;
                    while cy < my1 {
                        let mut cx = mx0.div_ceil(gg) * gg;
                        while cx < mx1 {
                            let k = cy * w + cx;
                            if cy < h && self.cv.cover[k] < layer.gap_cover as f32 && self.g.masks[r].at(cx, cy) > 0.5 {
                                let width = rng.range(wlo, whi) * self.pw * 0.8;
                                let length = (rng.range(llo, lhi) * self.pw * 0.7).max(2.0 * width);
                                let (x, y) = (cx as f64 + rng.range(-1.0, 1.0), cy as f64 + rng.range(-1.0, 1.0));
                                if self.emit(li, layer, &st, r, &reference, rpx, x, y, width, length, &mut rng, 0.6, Some(false)) {
                                    rr.strokes += 1;
                                    rr.gap_strokes += 1;
                                }
                            }
                            cx += gg;
                        }
                        cy += gg;
                    }
                }
            }
            rep.strokes += rr.strokes;
            rep.regions.push(rr);
        }
        if let Some(d) = layer.dry_after {
            self.cv.dry(d as f32);
        }
        self.layers.push(oil_strokes::Layer {
            start: start as u32,
            end: self.strokes.len() as u32,
            hblur_sigma,
            dry_after: layer.dry_after.map(|d| d as f32),
        });
        rep.ms = clock() - t0;
        self.report.push(rep);
    }
}
