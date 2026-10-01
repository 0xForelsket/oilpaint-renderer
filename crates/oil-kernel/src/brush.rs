//! The bristle-lane brush: all four modes (0 paint, 1 scumble, 2 smudge, 3 glaze), lanes, height and pick-up.
//!
//! Ported from `spikes/oilcore/src/kernel.rs` (itself a port of v1's `brush.c`), generic over the mixer, with a
//! counter-based bristle RNG and fixed-order pick-up sums. Maths goes through `oil-math` only, so a stroke paints the
//! same bits on every host.
//!
//! Transport (how paint moves between brush and canvas) is v1's, chosen by Sean after L1 (docs/reports/L1.md): a
//! deposit moves the pixel's mixer state toward the brush's paint by the deposit alpha, and mixing with the paint
//! underneath comes from the brush: each bristle tip picks up wet canvas paint ("dirt") and carries it along. The
//! display colour is the decoded state. A layered paint film with interface mixing is the next candidate (the film
//! spike before L7).
use crate::planes::Planes;
use oil_math::{expf, sinf};
use oil_mix::{Mixer, State};

pub const MAX_NB: usize = 72;
const ALONG_PER_WIDTH: f32 = 6.0;
const MAX_CONTRIB: usize = 10;

pub const MODE_PAINT: i32 = 0;
pub const MODE_SCUMBLE: i32 = 1;
pub const MODE_SMUDGE: i32 = 2;
pub const MODE_GLAZE: i32 = 3;

/// Per-stroke brush parameters (spec/STROKELIST_V2.md, `STRK` record).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrushParams {
    pub mode: i32,
    pub opacity: f32,
    pub pickup: f32,
    pub load: f32,
    pub deplete: f32,
    pub vdry: f32,
    pub hgain: f32,
    pub flatten: f32,
    pub streak: f32,
    pub hardness: f32,
    pub grain: f32,
    pub dry_thresh: f32,
    pub dry_width: f32,
    pub nb: i32,
    pub seed: u32,
    pub dropout: f32,
    pub ragged: f32,
    pub body: f32,
    pub release: f32,
    pub streak_mix: f32,
    pub ridge: f32,
    pub levee: f32,
    pub furrow: f32,
    pub blob: f32,
    pub stiff: f32,
    pub marble: f32,
    pub splay: f32,
}

impl Default for BrushParams {
    /// v1's `BRUSH_DEFAULTS` (oilpaint/canvas.py).
    fn default() -> Self {
        BrushParams {
            mode: MODE_PAINT,
            opacity: 0.95,
            pickup: 0.12,
            load: 1.0,
            deplete: 0.02,
            vdry: 0.25,
            hgain: 1.0,
            flatten: 0.6,
            streak: 0.25,
            hardness: 0.75,
            grain: 0.10,
            dry_thresh: 0.0,
            dry_width: 0.15,
            nb: 14,
            seed: 0,
            dropout: 0.02,
            ragged: 0.5,
            body: 0.9,
            release: 0.3,
            streak_mix: 0.8,
            ridge: 0.5,
            levee: 0.35,
            furrow: 0.15,
            blob: 0.3,
            stiff: 0.25,
            marble: 0.0,
            splay: 1.0,
        }
    }
}

/// The colours on the brush, already encoded by the mixer: main load, second load (marble) and streak direction.
#[derive(Clone, Copy)]
pub struct Load<S> {
    pub zcol: S,
    pub zcol2: S,
    pub dz: S,
}

/// What one stroke did: summed alpha, painted pixel count, alpha-weighted wetness it picked up from.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StrokeStats {
    pub alpha: f64,
    pub pixels: i64,
    pub wet: f32,
}

// ---------------- random / hashing ----------------
// Counter-based RNG: every draw is a hash of (stroke seed, lane, sample, purpose), never a position in a sequential
// stream. The number of samples along a stroke then only moves the end of the arrays, never the pattern, so a
// stroke replayed at another size keeps its bristles (G3), and any lane or sample can be evaluated on its own.
const GLOBAL: u32 = u32::MAX; // "lane" for per-stroke draws

#[derive(Clone, Copy)]
#[repr(u32)]
enum Draw {
    Splay = 1,
    ColStart,
    Phase1,
    Phase2,
    LaneU,
    LaneSig,
    LaneFlip,
    SplaySide,
    SplayColour,
    Streak,
    WobbleAmp,
    WobbleK,
    WobblePhase,
    Start,
    EndCut,
    Base,
    AlongK,
    AlongPhase,
    Drop,
    DropLength,
}

#[inline(always)]
fn fmix32(mut h: u32) -> u32 {
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    h
}

/// Uniform in [0, 1) with 24 bits, from (seed, lane, sample, purpose).
#[inline(always)]
fn crand(seed: u32, lane: u32, sample: u32, what: Draw) -> f32 {
    let h = fmix32(
        seed ^ fmix32(lane.wrapping_mul(0x9e37_79b9) ^ fmix32(sample.wrapping_mul(0x85eb_ca77) ^ (what as u32).wrapping_mul(0xc2b2_ae3d))),
    );
    (h >> 8) as f32 * (1.0f32 / 16_777_216.0f32)
}

#[inline(always)]
fn hash2(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32)
        .wrapping_mul(374_761_393)
        .wrapping_add((y as u32).wrapping_mul(668_265_263))
        .wrapping_add(seed.wrapping_mul(2_246_822_519));
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    (h >> 8) as f32 * (1.0f32 / 16_777_216.0f32)
}

#[inline(always)]
fn clamp01(x: f32) -> f32 {
    if x < 0.0 {
        0.0
    } else if x > 1.0 {
        1.0
    } else {
        x
    }
}

/// A rate `r` defined per reference distance, applied over `q` reference distances: 1 - (1 - r)^q.
#[inline(always)]
fn per_distance(r: f32, q: f32) -> f32 {
    let keep = clamp01(1.0 - r) as f64;
    (1.0 - oil_math::pow(keep, q as f64)) as f32
}

#[inline(always)]
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = clamp01((x - e0) / (e1 - e0));
    t * t * (3.0 - 2.0 * t)
}

#[inline(always)]
fn vnoise(x: f32, y: f32, seed: u32) -> f32 {
    let fx = x.floor();
    let fy = y.floor();
    let ix = fx as i32;
    let iy = fy as i32;
    let mut tx = x - fx;
    let mut ty = y - fy;
    tx = tx * tx * (3.0 - 2.0 * tx);
    ty = ty * ty * (3.0 - 2.0 * ty);
    let a = hash2(ix, iy, seed);
    let b = hash2(ix + 1, iy, seed);
    let c = hash2(ix, iy + 1, seed);
    let d = hash2(ix + 1, iy + 1, seed);
    (a * (1.0 - tx) + b * tx) * (1.0 - ty) + (c * (1.0 - tx) + d * tx) * ty
}

// ---------------- bristle lanes ----------------
struct Bristles {
    nb: usize,
    ns: usize,
    ntot: usize,
    u0: [f32; MAX_NB],
    sig_r: [f32; MAX_NB],
    sig_f: [f32; MAX_NB],
    tstreak: [f32; MAX_NB],
    col_b: [f32; MAX_NB],
    wob_amp: [f32; MAX_NB],
    wob_k: [f32; MAX_NB],
    wob_ph: [f32; MAX_NB],
    along: Vec<f32>,
    along_f: Vec<f32>,
    nph1: f32,
    nph2: f32,
}

fn make_bristles(bp: &BrushParams, total_s: f32) -> Bristles {
    let seed = bp.seed;
    let r = |lane: usize, what: Draw| crand(seed, lane as u32, 0, what);
    let nb_i = bp.nb.clamp(2, MAX_NB as i32 - 4);
    let nsplay = ((bp.splay + crand(seed, GLOBAL, 0, Draw::Splay)) as i32).min(3);
    let nb = nb_i as usize;
    let ntot = (nb_i + nsplay) as usize;
    let ns_i = (total_s * ALONG_PER_WIDTH).ceil() as i32 + 3;
    let ns = ns_i as usize;
    let mut b = Bristles {
        nb,
        ns,
        ntot,
        u0: [0.0; MAX_NB],
        sig_r: [0.0; MAX_NB],
        sig_f: [0.0; MAX_NB],
        tstreak: [0.0; MAX_NB],
        col_b: [0.0; MAX_NB],
        wob_amp: [0.0; MAX_NB],
        wob_k: [0.0; MAX_NB],
        wob_ph: [0.0; MAX_NB],
        along: vec![0.0; ntot * ns],
        along_f: vec![0.0; ntot * ns],
        nph1: 0.0,
        nph2: 0.0,
    };
    let hard = clamp01(bp.hardness);
    let pitch = 2.0f32 / nb_i as f32;
    let mut colstate = crand(seed, GLOBAL, 0, Draw::ColStart) < 0.5;
    b.nph1 = crand(seed, GLOBAL, 0, Draw::Phase1) * 100.0;
    b.nph2 = crand(seed, GLOBAL, 0, Draw::Phase2) * 100.0;
    for l in 0..ntot {
        let is_splay = l >= nb;
        let (u, sig);
        if !is_splay {
            u = -1.0f32 + (l as f32 + 0.5) * pitch + (r(l, Draw::LaneU) - 0.5) * 0.5 * pitch;
            sig = (0.28f32 + 0.30 * r(l, Draw::LaneSig)) * pitch;
            if r(l, Draw::LaneFlip) < 0.35 {
                colstate = !colstate;
            }
            b.col_b[l] = if colstate { 1.0 } else { 0.0 };
        } else {
            let side = if r(l, Draw::SplaySide) < 0.5 { -1.0f32 } else { 1.0 };
            u = side * (0.98f32 + 0.18 * r(l, Draw::LaneU));
            sig = (0.18f32 + 0.15 * r(l, Draw::LaneSig)) * pitch;
            b.col_b[l] = if r(l, Draw::SplayColour) < 0.5 { 1.0 } else { 0.0 };
        }
        b.u0[l] = u;
        b.sig_r[l] = sig;
        b.sig_f[l] = if is_splay { sig * 1.3 } else { sig * (2.8 - 1.0 * hard) };
        b.tstreak[l] = 2.0 * r(l, Draw::Streak) - 1.0;
        b.wob_amp[l] = (0.10f32 + 0.30 * r(l, Draw::WobbleAmp)) * pitch * (if is_splay { 2.0 } else { 1.0 });
        b.wob_k[l] = 6.283_185_3_f32 / (1.5 + 4.0 * r(l, Draw::WobbleK));
        b.wob_ph[l] = 6.283_185_3_f32 * r(l, Draw::WobblePhase);
        let a = &mut b.along[l * ns..(l + 1) * ns];
        let start = r(l, Draw::Start) * bp.ragged * ALONG_PER_WIDTH;
        let endcut = r(l, Draw::EndCut) * bp.ragged * ALONG_PER_WIDTH * (if is_splay { 3.0 } else { 1.0 });
        let mut drop_left: i32 = 0;
        let base = 0.85f32 + 0.15 * r(l, Draw::Base);
        let lk = 6.283_185_3_f32 / ((2.0 + 4.0 * r(l, Draw::AlongK)) * ALONG_PER_WIDTH);
        let lph = 6.283_185_3_f32 * r(l, Draw::AlongPhase);
        for (i, ai) in a.iter_mut().enumerate() {
            let s_w = i as f32 / ALONG_PER_WIDTH;
            let v_ = bp.load - bp.deplete * s_w;
            let dryness = if v_ < bp.vdry { clamp01(1.0 - v_ / bp.vdry) } else { 0.0 };
            let pdrop = bp.dropout * (1.0 + 6.0 * dryness) * (if is_splay { 4.0 } else { 1.0 });
            if (i as f32) > (ns_i - 2) as f32 - endcut {
                *ai = 0.0;
                continue;
            }
            if drop_left > 0 {
                drop_left -= 1;
                *ai = 0.0;
                continue;
            }
            if crand(seed, l as u32, i as u32, Draw::Drop) < pdrop {
                drop_left = 2 + (crand(seed, l as u32, i as u32, Draw::DropLength) * (3.0 + 8.0 * dryness)) as i32;
                *ai = 0.0;
                continue;
            }
            let mut v = base * (0.92 + 0.08 * sinf(lk * i as f32 + lph));
            if (i as f32) < start {
                v *= clamp01((i as f32 - start + 2.0) / 2.0);
            }
            *ai = v;
        }
        for _pass in 0..2 {
            for i in 1..ns.saturating_sub(1) {
                let m = 0.25f32 * a[i - 1] + 0.5 * a[i] + 0.25 * a[i + 1];
                a[i] = 0.5 * a[i] + 0.5 * m;
            }
        }
        let a = &b.along[l * ns..(l + 1) * ns];
        let af = &mut b.along_f[l * ns..(l + 1) * ns];
        for i in 0..ns {
            let mut amax = 0.0f32;
            for j in -4i32..=4 {
                let ii = i as i32 + j;
                if ii >= 0 && (ii as usize) < ns && a[ii as usize] > amax {
                    amax = a[ii as usize];
                }
            }
            af[i] = if is_splay { a[i] } else { amax };
        }
    }
    b
}

/// Pick-up sums of one row chunk, per lane.
struct Partials<S> {
    z: [S; MAX_NB],
    a: [f32; MAX_NB],
    wet: [f32; MAX_NB],
    alpha: f64,
    touched: u128,
}

impl<S: State> Partials<S> {
    /// Add this chunk's sums to the segment totals (lanes in increasing order) and clear it.
    #[inline]
    fn flush(&mut self, seg_z: &mut [S; MAX_NB], seg_a: &mut [f32; MAX_NB], seg_wet: &mut [f32; MAX_NB], alpha: &mut f64) {
        let mut t = self.touched;
        while t != 0 {
            let l = t.trailing_zeros() as usize;
            t &= t - 1;
            let (dst, src) = (seg_z[l].as_mut_slice(), self.z[l].as_mut_slice());
            for k in 0..dst.len() {
                dst[k] += src[k];
                src[k] = 0.0;
            }
            seg_a[l] += self.a[l];
            seg_wet[l] += self.wet[l];
            self.a[l] = 0.0;
            self.wet[l] = 0.0;
        }
        self.touched = 0;
        *alpha += self.alpha;
        self.alpha = 0.0;
    }
}

struct Sample {
    cov: f32,
    ridge: f32,
    n: usize,
    idx: [usize; MAX_CONTRIB],
    w_r: [f32; MAX_CONTRIB],
    w_f: [f32; MAX_CONTRIB],
}

struct SegLanes {
    uc: [f32; MAX_NB],
    isf: [f32; MAX_NB],
    gf: [f32; MAX_NB],
    isr: [f32; MAX_NB],
    gr: [f32; MAX_NB],
    reach: i32,
}

#[inline(always)]
fn prep_lanes(b: &Bristles, s_mid: f32, min_sig: f32, g: &mut SegLanes) {
    for l in 0..b.ntot {
        g.uc[l] = b.u0[l] + b.wob_amp[l] * sinf(b.wob_k[l] * s_mid + b.wob_ph[l]);
        let mut sf = b.sig_f[l];
        let mut gf = 1.0f32;
        if sf < min_sig {
            gf = sf / min_sig;
            sf = min_sig;
        }
        let mut sr = b.sig_r[l];
        let mut gr = 1.0f32;
        if sr < min_sig {
            gr = sr / min_sig;
            sr = min_sig;
        }
        g.isf[l] = 1.0 / sf;
        g.gf[l] = gf;
        g.isr[l] = 1.0 / sr;
        g.gr[l] = gr;
    }
    g.reach = 3 + (min_sig * b.nb as f32) as i32 + 1;
}

#[inline(always)]
fn eval_lanes(b: &Bristles, g: &SegLanes, u: f32, s: f32, bodyf: f32, o: &mut Sample) {
    let fs = s * ALONG_PER_WIDTH;
    let is = (fs as i32).max(0).min(b.ns as i32 - 2);
    let ts = (fs - is as f32).clamp(0.0, 1.0);
    let is = is as usize;
    o.cov = 0.0;
    o.ridge = 0.0;
    o.n = 0;
    let bc = ((u + 1.0) * 0.5 * b.nb as f32) as i32;
    let b0 = (bc - g.reach).max(0);
    let b1 = (bc + g.reach).min(b.nb as i32 - 1);
    let ns = b.ns;
    for pass in 0..2 {
        let (lo, hi) = if pass == 1 { (b.nb as i32, b.ntot as i32 - 1) } else { (b0, b1) };
        let mut l = lo;
        while l <= hi {
            let li = l as usize;
            l += 1;
            let d = u - g.uc[li];
            let df = d * g.isf[li];
            if !(-2.0..=2.0).contains(&df) {
                continue;
            }
            let a_arr = &b.along[li * ns..];
            let a = a_arr[is] * (1.0 - ts) + a_arr[is + 1] * ts;
            let mut afill = a;
            if pass == 0 && bodyf > 0.0 {
                let af = &b.along_f[li * ns..];
                let body_a = bodyf * (af[is] * (1.0 - ts) + af[is + 1] * ts);
                if body_a > afill {
                    afill = body_a;
                }
            }
            if a <= 0.003 && afill <= 0.003 {
                continue;
            }
            let mut bf = 1.0f32 - 0.25 * df * df;
            if bf > 0.0 {
                bf *= bf * g.gf[li];
                o.cov += bf * afill;
            }
            let dr = d * g.isr[li];
            let mut br = 1.0f32 - 0.25 * dr * dr;
            if br > 0.0 {
                br *= br * g.gr[li];
                if br * a > o.ridge {
                    o.ridge = br * a;
                }
            }
            if (bf > 0.0 || br > 0.0) && o.n < MAX_CONTRIB {
                o.idx[o.n] = li;
                o.w_r[o.n] = if br > 0.0 { br * a } else { 0.0 };
                o.w_f[o.n] = if bf > 0.0 { bf * afill } else { 0.0 };
                o.n += 1;
            }
        }
    }
}

/// Rows per reduction chunk. Chunks are fixed by absolute row index (row / CHUNK_ROWS), and each chunk's pick-up sums
/// are added to the segment totals in chunk order, so splitting a segment's rows across threads gives the same bits.
pub const CHUNK_ROWS: i32 = 16;

/// Stroke length in brush widths, measured in canvas-width units: independent of the canvas size.
/// `pts_cw` are (x, y, width, pressure) in cw.
pub fn length_in_widths(pts_cw: &[[f32; 4]]) -> f32 {
    let wref = pts_cw.iter().fold(1e-9f64, |w, p| w.max(p[2] as f64));
    let total: f64 = pts_cw
        .windows(2)
        .map(|p| {
            let (dx, dy) = ((p[1][0] - p[0][0]) as f64, (p[1][1] - p[0][1]) as f64);
            (dx * dx + dy * dy).sqrt()
        })
        .sum();
    (total / wref) as f32
}

/// Paint one stroke. `pts` are (x, y, width, pressure) in pixels. The stroke length in widths, which sets the
/// number of bristle samples, is measured from the pixel coordinates; use `render_stroke_len` with
/// `length_in_widths` of the cw points to make it exactly size-independent.
pub fn render_stroke<M: Mixer>(
    m: &M,
    cv: &mut Planes<'_, M::State>,
    pts: &[[f32; 4]],
    load: &Load<M::State>,
    bp: &BrushParams,
) -> StrokeStats {
    if pts.len() < 2 {
        return StrokeStats::default();
    }
    let wref = pts.iter().fold(1e-3f32, |w, p| if p[2] > w { p[2] } else { w });
    let total: f32 = pts
        .windows(2)
        .map(|p| {
            let (dx, dy) = (p[1][0] - p[0][0], p[1][1] - p[0][1]);
            (dx * dx + dy * dy).sqrt()
        })
        .sum();
    render_stroke_len(m, cv, pts, load, bp, total / wref)
}

/// Paint one stroke whose length in brush widths is given (see `length_in_widths`).
pub fn render_stroke_len<M: Mixer>(
    m: &M,
    cv: &mut Planes<'_, M::State>,
    pts: &[[f32; 4]],
    load: &Load<M::State>,
    bp: &BrushParams,
    total_s: f32,
) -> StrokeStats {
    render_stroke_len_impl::<M, true>(m, cv, pts, load, bp, total_s)
}

/// Advance material/height/wetness/coverage while leaving RGB untouched.
/// Brush transport does not read RGB. A host using this path must refresh display
/// from the final material states before exposing an image. This is for deferred
/// final rendering; `render_stroke_len` keeps immediate display behavior.
pub fn render_stroke_len_materials<M: Mixer>(
    m: &M,
    cv: &mut Planes<'_, M::State>,
    pts: &[[f32; 4]],
    load: &Load<M::State>,
    bp: &BrushParams,
    total_s: f32,
) -> StrokeStats {
    render_stroke_len_impl::<M, false>(m, cv, pts, load, bp, total_s)
}

fn render_stroke_len_impl<M: Mixer, const DISPLAY: bool>(
    m: &M,
    cv: &mut Planes<'_, M::State>,
    pts: &[[f32; 4]],
    load: &Load<M::State>,
    bp: &BrushParams,
    total_s: f32,
) -> StrokeStats {
    let n = pts.len();
    if n < 2 {
        return StrokeStats::default();
    }
    let (w_i, h_i) = (cv.w as i32, cv.h as i32);
    let wu = cv.w;
    let wref = pts.iter().fold(1e-3f32, |w, p| if p[2] > w { p[2] } else { w });
    let bb = make_bristles(bp, total_s);
    let mode = bp.mode;

    let zero = M::State::zero();
    let mut zload = [zero; MAX_NB];
    let mut ztip = [zero; MAX_NB];
    let mut dirt = [0.0f32; MAX_NB];
    let mut seg_z = [zero; MAX_NB];
    let mut seg_a = [0.0f32; MAX_NB];
    let mut seg_wet = [0.0f32; MAX_NB];
    // per-chunk partial sums, flushed into seg_* in chunk order
    let mut part = Partials { z: [zero; MAX_NB], a: [0.0; MAX_NB], wet: [0.0; MAX_NB], alpha: 0.0, touched: 0 };
    {
        let (zc, zc2) = (load.zcol.as_slice(), load.zcol2.as_slice());
        for l in 0..bb.ntot {
            let m_b = bp.marble * bb.col_b[l];
            let st = bp.streak * bb.tstreak[l];
            let zl = zload[l].as_mut_slice();
            for k in 0..zl.len() {
                zl[k] = zc[k] + m_b * (zc2[k] - zc[k]);
            }
            // the mixer adds the streak (plain `+ st * dz` for rgb and Mixbox, bit-identical to v1; limited so that
            // K >= 0 and S > 0 for Ochrell)
            m.streak(&mut zload[l], &load.dz, st);
            ztip[l] = zload[l];
            dirt[l] = if mode == MODE_SMUDGE { 1.0 } else { 0.0 };
        }
    }
    let mut v_load = bp.load;
    let mut hbase = 0.0f32;
    let mut have_mean = false;
    let mut stat_alpha = 0.0f64;
    let mut stat_pix: i64 = 0;
    let mut sum_wet_all = 0.0f64;
    let mut sum_a_all = 0.0f64;
    let mut s_acc = 0.0f32;
    let (mut ptx, mut pty, mut pnx, mut pny, mut plen, mut px0, mut py0, mut pw0, mut pw1) =
        (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let mut have_prev = false;
    let lw = 0.13f32;
    let mut g = SegLanes { uc: [0.0; MAX_NB], isf: [0.0; MAX_NB], gf: [0.0; MAX_NB], isr: [0.0; MAX_NB], gr: [0.0; MAX_NB], reach: 0 };
    let mut smp = Sample { cov: 0.0, ridge: 0.0, n: 0, idx: [0; MAX_CONTRIB], w_r: [0.0; MAX_CONTRIB], w_f: [0.0; MAX_CONTRIB] };

    for i in 0..n - 1 {
        let [x0, y0, w0, p0] = pts[i];
        let [x1, y1, w1, p1] = pts[i + 1];
        let dx = x1 - x0;
        let dy = y1 - y0;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-4 {
            continue;
        }
        let tx = dx / len;
        let ty = dy / len;
        let nx = -ty;
        let ny = tx;
        let hwmax = 0.5f32 * (if w0 > w1 { w0 } else { w1 }) * 1.45 + 1.0;
        let bx0 = ((x0.min(x1) - hwmax).floor() as i32).max(0);
        let bx1 = ((x0.max(x1) + hwmax).ceil() as i32).min(w_i - 1);
        let by0 = ((y0.min(y1) - hwmax).floor() as i32).max(0);
        let by1 = ((y0.max(y1) + hwmax).ceil() as i32).min(h_i - 1);
        let last = i + 2 == n;
        let wedge_t = if have_prev { -(0.6f32 * w0 / len) } else { 0.0 };

        let mut hcur = 0.0f32;
        let mut hn: i32 = 0;
        for j in 0..8 {
            let tj = (j as f32 + 0.5) / 8.0;
            let cxj = x0 + dx * tj;
            let cyj = y0 + dy * tj;
            let hwj = 0.5f32 * (w0 + (w1 - w0) * tj);
            for k in -1i32..=1 {
                let sx = (cxj + nx * hwj * 0.5 * k as f32) as i32;
                let sy = (cyj + ny * hwj * 0.5 * k as f32) as i32;
                if sx >= 0 && sy >= 0 && sx < w_i && sy < h_i {
                    hcur += cv.hgt[sy as usize * wu + sx as usize];
                    hn += 1;
                }
            }
        }
        hcur = if hn != 0 { hcur / hn as f32 } else { 0.0 };
        let hprev = if have_mean { hbase } else { hcur };
        prep_lanes(&bb, (s_acc + 0.5 * len) / wref, 0.7f32 / (0.25 * (w0 + w1) + 0.25), &mut g);
        let loadf = if mode == MODE_GLAZE { 1.0 } else { clamp01(v_load / bp.vdry) };
        let thick = clamp01(v_load);
        let bodyf = if mode == MODE_SCUMBLE { 0.0 } else { bp.body * loadf };
        let can_deposit = mode != MODE_SMUDGE || have_mean;
        for l in 0..bb.ntot {
            seg_a[l] = 0.0;
            seg_wet[l] = 0.0;
            seg_z[l] = zero;
        }
        let e0 = 0.35f32;
        let e1 = e0 + 0.12 + 0.7 * (1.0 - bp.hardness);

        if bx1 >= bx0 && by1 >= by0 {
            for py in by0..=by1 {
                if py > by0 && py % CHUNK_ROWS == 0 {
                    part.flush(&mut seg_z, &mut seg_a, &mut seg_wet, &mut stat_alpha);
                }
                let row = py as usize * wu;
                for px in bx0..=bx1 {
                    let vx = px as f32 + 0.5 - x0;
                    let vy = py as f32 + 0.5 - y0;
                    let mut t = (vx * tx + vy * ty) / len;
                    if t < wedge_t || (if last { t > 1.0 } else { t >= 1.0 }) {
                        continue;
                    }
                    if have_prev {
                        let qx = px as f32 + 0.5 - px0;
                        let qy = py as f32 + 0.5 - py0;
                        let tp = (qx * ptx + qy * pty) / plen;
                        let hwp = 0.5f32 * (pw0 + (pw1 - pw0) * tp);
                        let up = if hwp > 0.0 { (qx * pnx + qy * pny) / hwp } else { 9.0 };
                        let prev_owns = (0.0..1.0).contains(&tp) && (-1.45..=1.45).contains(&up);
                        if prev_owns {
                            continue;
                        }
                        if t < 0.0 && !(tp >= 1.0 && (-1.45..=1.45).contains(&up)) {
                            continue;
                        }
                        if t < 0.0 {
                            t = 0.0;
                        }
                    }
                    let hw = 0.5f32 * (w0 + (w1 - w0) * t);
                    if hw < 0.25 {
                        continue;
                    }
                    let u = (vx * nx + vy * ny) / hw;
                    if !(-1.45..=1.45).contains(&u) {
                        continue;
                    }
                    let s = (s_acc + t * len) / wref;
                    eval_lanes(&bb, &g, u, s, bodyf, &mut smp);
                    if smp.cov <= 0.01 {
                        continue;
                    }
                    let acov = smoothstep(e0, e1, smp.cov);
                    let pres = p0 + (p1 - p0) * t;
                    let lanes = bodyf + (1.0 - bodyf) * smp.ridge;
                    let mut alpha = acov * lanes * pres * bp.opacity * loadf;
                    if bp.grain > 0.0 {
                        alpha *= 1.0 + bp.grain * (hash2(px, py, bp.seed) - 0.5);
                    }
                    let idx = row + px as usize;
                    if mode == MODE_SCUMBLE {
                        let hb = if cv.hblur.is_empty() { 0.0 } else { cv.hblur[idx] };
                        alpha *= smoothstep(bp.dry_thresh - bp.dry_width, bp.dry_thresh + bp.dry_width, cv.hgt[idx] - hb);
                    }
                    if alpha <= 0.002 {
                        continue;
                    }
                    if alpha > 1.0 {
                        alpha = 1.0;
                    }
                    let mut wsum = 0.0f32;
                    let mut use_r = true;
                    for c in 0..smp.n {
                        wsum += smp.w_r[c];
                    }
                    if wsum < 0.05 {
                        use_r = false;
                        wsum = 0.0;
                        for c in 0..smp.n {
                            wsum += smp.w_f[c];
                        }
                    }
                    if wsum < 1e-6 {
                        continue;
                    }
                    let mut bd = smp.idx[0];
                    let mut wd = -1.0f32;
                    for c in 0..smp.n {
                        let w = if use_r { smp.w_r[c] } else { smp.w_f[c] };
                        if w > wd {
                            wd = w;
                            bd = smp.idx[c];
                        }
                    }
                    {
                        let lpix = cv.lat[idx].as_slice();
                        let sz = part.z[bd].as_mut_slice();
                        for k in 0..sz.len() {
                            sz[k] += alpha * lpix[k];
                        }
                    }
                    part.a[bd] += alpha;
                    part.wet[bd] += alpha * cv.wet[idx];
                    part.touched |= 1u128 << bd;
                    if !can_deposit {
                        continue;
                    }
                    let mut zpix = zero;
                    {
                        let zp = zpix.as_mut_slice();
                        if mode == MODE_SMUDGE {
                            for c in 0..smp.n {
                                let w = (if use_r { smp.w_r[c] } else { smp.w_f[c] }) / wsum;
                                let zt = ztip[smp.idx[c]].as_slice();
                                for k in 0..zp.len() {
                                    zp[k] += w * zt[k];
                                }
                            }
                        } else {
                            for c in 0..smp.n {
                                let w = (if use_r { smp.w_r[c] } else { smp.w_f[c] }) / wsum;
                                let l = smp.idx[c];
                                let dpix = clamp01(dirt[l] * (1.0 + bp.streak_mix * bb.tstreak[l]));
                                let (zl, zt) = (zload[l].as_slice(), ztip[l].as_slice());
                                for k in 0..zp.len() {
                                    zp[k] += w * (zl[k] + dpix * (zt[k] - zl[k]));
                                }
                            }
                        }
                    }
                    let a = alpha;
                    {
                        let lpix = cv.lat[idx].as_mut_slice();
                        let zp = zpix.as_slice();
                        for k in 0..lpix.len() {
                            lpix[k] += a * (zp[k] - lpix[k]);
                        }
                    }
                    if DISPLAY {
                        cv.rgb[idx] = m.decode_srgb(&cv.lat[idx]);
                    }
                    let base = hprev + (hcur - hprev) * t;
                    if mode == MODE_PAINT {
                        let au = u.abs();
                        let lv = expf(-((au - (1.0 - lw)) * (au - (1.0 - lw))) / (0.10f32 * 0.10f32));
                        let fur = expf(-(u * u) / (0.35f32 * 0.35f32));
                        let sb = expf(-(s * s) / (0.45f32 * 0.45f32));
                        let nz = vnoise(u * bb.nb as f32 * 0.5 + bb.nph1, s * 4.0 + bb.nph2, bp.seed)
                            + 0.5 * vnoise(u * bb.nb as f32 * 1.5 + bb.nph2, s * 12.0 + bb.nph1, bp.seed.wrapping_add(7))
                            - 0.75;
                        let shape = acov * (1.0 + bp.levee * lv * thick - bp.furrow * fur * thick + bp.blob * sb * thick)
                            + bp.ridge * (smp.ridge - 0.45) * acov * (0.5 + 0.5 * loadf)
                            + bp.stiff * nz * acov;
                        let hadd = bp.hgain * thick * shape;
                        let fl = bp.flatten;
                        cv.hgt[idx] += a * fl * (base + hadd - cv.hgt[idx]) + a * (1.0 - fl) * 0.5 * hadd;
                        if cv.wet[idx] < a {
                            cv.wet[idx] = a;
                        }
                    } else if mode == MODE_SCUMBLE {
                        cv.hgt[idx] += a * bp.hgain * 0.5 * (0.4 + 0.6 * smp.ridge);
                        if cv.wet[idx] < 0.5 * a {
                            cv.wet[idx] = 0.5 * a;
                        }
                    } else if mode == MODE_SMUDGE {
                        cv.hgt[idx] -= a * bp.flatten * (cv.hgt[idx] - base);
                    }
                    cv.cover[idx] += a;
                    part.alpha += a as f64;
                    stat_pix += 1;
                }
            }
            part.flush(&mut seg_z, &mut seg_a, &mut seg_wet, &mut stat_alpha);
        }
        // Pick-up and release rates are per reference distance (half a brush width, the planner's step), not per
        // segment: a stroke sampled with more points picks up the same paint.
        let q = len / (0.5 * wref);
        let r_release = per_distance(bp.release, q);
        let r_approach = per_distance(0.6, q);
        let r_pickup = per_distance(bp.pickup, q);
        for l in 0..bb.ntot {
            if seg_a[l] <= 1e-6 {
                dirt[l] -= r_release * dirt[l] * (if mode == MODE_SMUDGE { 0.0 } else { 1.0 });
                continue;
            }
            let wetavg = seg_wet[l] / seg_a[l];
            sum_wet_all += seg_wet[l] as f64;
            sum_a_all += seg_a[l] as f64;
            let (zt, sz, sa) = (ztip[l].as_mut_slice(), seg_z[l].as_slice(), seg_a[l]);
            if mode == MODE_SMUDGE {
                for k in 0..zt.len() {
                    zt[k] += r_pickup * (sz[k] / sa - zt[k]);
                }
                dirt[l] = 1.0;
            } else if mode != MODE_GLAZE {
                let rate = per_distance(0.6f32 * wetavg, q);
                for k in 0..zt.len() {
                    zt[k] += rate * (sz[k] / sa - zt[k]);
                }
                let target = bp.pickup * wetavg;
                if target > dirt[l] {
                    dirt[l] += r_approach * (target - dirt[l]);
                } else {
                    dirt[l] -= r_release * (dirt[l] - target);
                }
            }
        }
        hbase = hcur;
        have_mean = true;
        v_load -= bp.deplete * (len / wref);
        if v_load < 0.0 {
            v_load = 0.0; // not max(0.0): keeps the sign of zero exactly as v1 did
        }
        s_acc += len;
        (ptx, pty, pnx, pny, plen, px0, py0, pw0, pw1) = (tx, ty, nx, ny, len, x0, y0, w0, w1);
        have_prev = true;
    }
    StrokeStats {
        alpha: stat_alpha,
        pixels: stat_pix,
        wet: if sum_a_all > 0.0 { (sum_wet_all / sum_a_all) as f32 } else { 0.0 },
    }
}
