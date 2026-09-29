//! Port of oilpaint/csrc/brush.c (v2 bristle-lane surface model), operation for operation:
//! same float evaluation order, same xorshift stream and draw order, wrapping u32 hashes, truncating casts.
//! All four modes (0 paint, 1 scumble, 2 smudge, 3 glaze).  C ABI identical to brush.c so the Python
//! wrapper can load either library.
use crate::math::{expf, sinf};

#[cfg(not(any(feature = "mixer_km12", feature = "mixer_ochrell")))]
pub const LAT: usize = 7;
#[cfg(feature = "mixer_km12")]
pub const LAT: usize = 16;
#[cfg(feature = "mixer_ochrell")]
pub const LAT: usize = 85;

/// latent -> display sRGB for the compiled-in mixer
#[inline(always)]
pub fn latent_to_rgb(l: &[f32], rgb: &mut [f32]) {
    #[cfg(not(any(feature = "mixer_km12", feature = "mixer_ochrell")))]
    mixbox_latent_to_rgb(l, rgb);
    #[cfg(feature = "mixer_km12")]
    crate::km_tables::km_latent_to_rgb(l, rgb);
    #[cfg(feature = "mixer_ochrell")]
    rgb.copy_from_slice(&crate::material::decode(l, 0));
}
const MAX_NB: usize = 72;
const ALONG_PER_WIDTH: f32 = 6.0;
const MAX_CONTRIB: usize = 10;

#[repr(C)]
pub struct Canvas {
    pub w: i32,
    pub h: i32,
    pub lat: *mut f32,
    pub rgb: *mut f32,
    pub hgt: *mut f32,
    pub wet: *mut f32,
    pub cover: *mut f32,
    pub hblur: *mut f32,
    pub region: *mut u8,
    #[cfg(feature = "mixer_ochrell")]
    pub amount: *mut f32,
    #[cfg(feature = "mixer_ochrell")]
    pub mixing_mode: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
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
    pub allow_mask: u32,
    pub override_p: f32,
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

// ---------------- random / hashing ----------------
#[inline(always)]
fn xs32(s: &mut u32) -> u32 {
    let mut x = *s;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *s = x;
    x
}
#[inline(always)]
fn frand(s: &mut u32) -> f32 {
    (xs32(s) >> 8) as f32 * (1.0f32 / 16777216.0f32)
}
#[inline(always)]
fn hash2(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32)
        .wrapping_mul(374761393)
        .wrapping_add((y as u32).wrapping_mul(668265263))
        .wrapping_add(seed.wrapping_mul(2246822519));
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^= h >> 16;
    (h >> 8) as f32 * (1.0f32 / 16777216.0f32)
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

// ---------------- Mixbox polynomial (latent -> rgb), (c) 2022 Secret Weapons, CC BY-NC 4.0 ----------------
#[inline(always)]
#[cfg(any(not(feature = "mixer_ochrell"), feature = "mixbox_comparison"))]
pub fn mixbox_latent_to_rgb(l: &[f32], rgb: &mut [f32]) {
    let (c0, c1, c2, c3) = (l[0], l[1], l[2], l[3]);
    let (c00, c11, c22, c33, c01, c02, c12) = (c0 * c0, c1 * c1, c2 * c2, c3 * c3, c0 * c1, c0 * c2, c1 * c2);
    let (mut r, mut g, mut b) = (0.0f32, 0.0f32, 0.0f32);
    let mut w;
    w = c0 * c00; r += 0.07717053 * w; g += 0.02826978 * w; b += 0.24832992 * w;
    w = c1 * c11; r += 0.95912302 * w; g += 0.80256528 * w; b += 0.03561839 * w;
    w = c2 * c22; r += 0.74683774 * w; g += 0.04868586 * w;
    w = c3 * c33; r += 0.99518138 * w; g += 0.99978149 * w; b += 0.99704802 * w;
    w = c00 * c1; r += 0.04819146 * w; g += 0.83363781 * w; b += 0.32515377 * w;
    w = c01 * c1; r += -0.68146950 * w; g += 1.46107803 * w; b += 1.06980936 * w;
    w = c00 * c2; r += 0.27058419 * w; g += -0.15324870 * w; b += 1.98735057 * w;
    w = c02 * c2; r += 0.80478189 * w; g += 0.67093710 * w; b += 0.18424500 * w;
    w = c00 * c3; r += -0.35031003 * w; g += 1.37855826 * w; b += 3.68865000 * w;
    w = c0 * c33; r += 1.05128046 * w; g += 1.97815239 * w; b += 2.82989073 * w;
    w = c11 * c2; r += 3.21607125 * w; g += 0.81270228 * w; b += 1.03384539 * w;
    w = c1 * c22; r += 2.78893374 * w; g += 0.41565549 * w; b += -0.04487295 * w;
    w = c11 * c3; r += 3.02162577 * w; g += 2.55374103 * w; b += 0.32766114 * w;
    w = c1 * c33; r += 2.95124691 * w; g += 2.81201112 * w; b += 1.17578442 * w;
    w = c22 * c3; r += 2.82677043 * w; g += 0.79933038 * w; b += 1.81715262 * w;
    w = c2 * c33; r += 2.99691099 * w; g += 1.22593053 * w; b += 1.80653661 * w;
    w = c01 * c2; r += 1.87394106 * w; g += 2.05027182 * w; b += -0.29835996 * w;
    w = c01 * c3; r += 2.56609566 * w; g += 7.03428198 * w; b += 0.62575374 * w;
    w = c02 * c3; r += 4.08329484 * w; g += -1.40408358 * w; b += 2.14995522 * w;
    w = c12 * c3; r += 6.00078678 * w; g += 2.55552042 * w; b += 1.90739502 * w;
    rgb[0] = clamp01(r + l[4]);
    rgb[1] = clamp01(g + l[5]);
    rgb[2] = clamp01(b + l[6]);
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

fn make_bristles(bp: &BrushParams, total_s: f32, rs: &mut u32) -> Bristles {
    let nb_i = if bp.nb < 2 { 2 } else if bp.nb > (MAX_NB as i32) - 4 { MAX_NB as i32 - 4 } else { bp.nb };
    let mut nsplay = (bp.splay + frand(rs)) as i32;
    if nsplay > 3 {
        nsplay = 3;
    }
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
    let mut colstate = frand(rs) < 0.5;
    b.nph1 = frand(rs) * 100.0;
    b.nph2 = frand(rs) * 100.0;
    for l in 0..ntot {
        let is_splay = l >= nb;
        let (u, sig, _w);
        if !is_splay {
            u = -1.0f32 + (l as f32 + 0.5) * pitch + (frand(rs) - 0.5) * 0.5 * pitch;
            sig = (0.28f32 + 0.30 * frand(rs)) * pitch;
            _w = 0.55f32 + 0.45 * frand(rs);
            if frand(rs) < 0.35 {
                colstate = !colstate;
            }
            b.col_b[l] = if colstate { 1.0 } else { 0.0 };
        } else {
            let side = if frand(rs) < 0.5 { -1.0f32 } else { 1.0 };
            u = side * (0.98f32 + 0.18 * frand(rs));
            sig = (0.18f32 + 0.15 * frand(rs)) * pitch;
            _w = 0.25f32 + 0.25 * frand(rs);
            b.col_b[l] = if frand(rs) < 0.5 { 1.0 } else { 0.0 };
        }
        b.u0[l] = u;
        b.sig_r[l] = sig;
        b.sig_f[l] = if is_splay { sig * 1.3 } else { sig * (2.8 - 1.0 * hard) };
        b.tstreak[l] = 2.0 * frand(rs) - 1.0;
        b.wob_amp[l] = (0.10f32 + 0.30 * frand(rs)) * pitch * (if is_splay { 2.0 } else { 1.0 });
        b.wob_k[l] = 6.2831853f32 / (1.5 + 4.0 * frand(rs));
        b.wob_ph[l] = 6.2831853f32 * frand(rs);
        let a = &mut b.along[l * ns..(l + 1) * ns];
        let start = frand(rs) * bp.ragged * ALONG_PER_WIDTH;
        let endcut = frand(rs) * bp.ragged * ALONG_PER_WIDTH * (if is_splay { 3.0 } else { 1.0 });
        let mut drop_left: i32 = 0;
        let base = 0.85f32 + 0.15 * frand(rs);
        let lk = 6.2831853f32 / ((2.0 + 4.0 * frand(rs)) * ALONG_PER_WIDTH);
        let lph = 6.2831853f32 * frand(rs);
        for i in 0..ns {
            let s_w = i as f32 / ALONG_PER_WIDTH;
            let v_ = bp.load - bp.deplete * s_w;
            let dryness = if v_ < bp.vdry { clamp01(1.0 - v_ / bp.vdry) } else { 0.0 };
            let pdrop = bp.dropout * (1.0 + 6.0 * dryness) * (if is_splay { 4.0 } else { 1.0 });
            if (i as f32) > (ns_i - 2) as f32 - endcut {
                a[i] = 0.0;
                continue;
            }
            if drop_left > 0 {
                drop_left -= 1;
                a[i] = 0.0;
                continue;
            }
            if frand(rs) < pdrop {
                drop_left = 2 + (frand(rs) * (3.0 + 8.0 * dryness)) as i32;
                a[i] = 0.0;
                continue;
            }
            let mut v = base * (0.92 + 0.08 * sinf(lk * i as f32 + lph));
            if (i as f32) < start {
                v *= clamp01((i as f32 - start + 2.0) / 2.0);
            }
            a[i] = v;
        }
        for _pass in 0..2 {
            for i in 1..ns.saturating_sub(1) {
                let m = 0.25f32 * a[i - 1] + 0.5 * a[i] + 0.25 * a[i + 1];
                a[i] = 0.5 * a[i] + 0.5 * m;
            }
        }
        let af = &mut b.along_f[l * ns..(l + 1) * ns];
        let a = &b.along[l * ns..(l + 1) * ns];
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
    let mut is = fs as i32;
    if is < 0 {
        is = 0;
    }
    if is >= b.ns as i32 - 1 {
        is = b.ns as i32 - 2;
    }
    let mut ts = fs - is as f32;
    if ts < 0.0 {
        ts = 0.0;
    }
    if ts > 1.0 {
        ts = 1.0;
    }
    let is = is as usize;
    o.cov = 0.0;
    o.ridge = 0.0;
    o.n = 0;
    let bc = ((u + 1.0) * 0.5 * b.nb as f32) as i32;
    let reach = g.reach;
    let mut b0 = bc - reach;
    let mut b1 = bc + reach;
    if b0 < 0 {
        b0 = 0;
    }
    if b1 > b.nb as i32 - 1 {
        b1 = b.nb as i32 - 1;
    }
    let ns = b.ns;
    for pass in 0..2 {
        let (lo, hi) = if pass == 1 { (b.nb as i32, b.ntot as i32 - 1) } else { (b0, b1) };
        let mut l = lo;
        while l <= hi {
            let li = l as usize;
            l += 1;
            let d = u - g.uc[li];
            let df = d * g.isf[li];
            if df > 2.0 || df < -2.0 {
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
            let gr = g.gr[li];
            if br > 0.0 {
                br *= br * gr;
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

/// Render one stroke.  `pts` = n * (x, y, width, pressure) in pixels.
pub unsafe fn render_stroke_impl(
    cv: &mut Canvas,
    pts: &[f32],
    zcol: &[f32],
    zcol2: &[f32],
    dz: &[f32],
    bp: &BrushParams,
    out_stats: Option<&mut [f32]>,
) -> i32 {
    let n = pts.len() / 4;
    if n < 2 {
        return 0;
    }
    let (w_i, h_i) = (cv.w, cv.h);
    let wu = w_i as usize;
    let npx = (cv.w as usize) * (cv.h as usize);
    let lat = std::slice::from_raw_parts_mut(cv.lat, npx * LAT);
    let rgb = std::slice::from_raw_parts_mut(cv.rgb, npx * 3);
    let hh = std::slice::from_raw_parts_mut(cv.hgt, npx);
    let wet = std::slice::from_raw_parts_mut(cv.wet, npx);
    let cover = std::slice::from_raw_parts_mut(cv.cover, npx);
    #[cfg(feature = "mixer_ochrell")]
    let amount = std::slice::from_raw_parts_mut(cv.amount, npx);
    let hblur: Option<&[f32]> = if cv.hblur.is_null() { None } else { Some(std::slice::from_raw_parts(cv.hblur, npx)) };

    let mut rs: u32 = bp.seed.wrapping_mul(747796405).wrapping_add(2891336453);
    if rs == 0 {
        rs = 1;
    }
    let mut wref = 1e-3f32;
    for i in 0..n {
        if pts[i * 4 + 2] > wref {
            wref = pts[i * 4 + 2];
        }
    }
    let mut total = 0.0f32;
    for i in 0..n - 1 {
        let dx = pts[(i + 1) * 4] - pts[i * 4];
        let dy = pts[(i + 1) * 4 + 1] - pts[i * 4 + 1];
        total += (dx * dx + dy * dy).sqrt();
    }
    let total_s = total / wref;
    let bb = make_bristles(bp, total_s, &mut rs);
    let mode = bp.mode;

    let mut zload = [[0.0f32; LAT]; MAX_NB];
    let mut ztip = [[0.0f32; LAT]; MAX_NB];
    let mut dirt = [0.0f32; MAX_NB];
    let mut seg_z = [[0.0f32; LAT]; MAX_NB];
    let mut seg_a = [0.0f32; MAX_NB];
    let mut seg_wet = [0.0f32; MAX_NB];
    #[cfg(feature = "mixer_ochrell")]
    let mut seg_cov = [0.0f32; MAX_NB];
    #[cfg(feature = "mixer_ochrell")]
    let mut tip_amount = [if mode == 2 { 0. } else { bp.load }; MAX_NB];
    for l in 0..bb.ntot {
        let m_b = bp.marble * bb.col_b[l];
        let st = bp.streak * bb.tstreak[l];
        #[cfg(not(feature = "mixer_ochrell"))]
        for k in 0..LAT {
            zload[l][k] = zcol[k] + m_b * (zcol2[k] - zcol[k]) + st * dz[k];
            ztip[l][k] = zload[l][k];
        }
        #[cfg(feature = "mixer_ochrell")]
        {
            for k in 0..LAT { zload[l][k] = (1. - m_b) * zcol[k] + m_b * zcol2[k]; }
            crate::material::streak(&mut zload[l], dz, st, cv.mixing_mode);
            ztip[l] = zload[l];
        }
        dirt[l] = if mode == 2 { 1.0 } else { 0.0 };
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
        let (x0, y0, w0, p0) = (pts[i * 4], pts[i * 4 + 1], pts[i * 4 + 2], pts[i * 4 + 3]);
        let (x1, y1, w1, p1) = (pts[(i + 1) * 4], pts[(i + 1) * 4 + 1], pts[(i + 1) * 4 + 2], pts[(i + 1) * 4 + 3]);
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
        let mut bx0 = (x0.min(x1) - hwmax).floor() as i32;
        let mut bx1 = (x0.max(x1) + hwmax).ceil() as i32;
        let mut by0 = (y0.min(y1) - hwmax).floor() as i32;
        let mut by1 = (y0.max(y1) + hwmax).ceil() as i32;
        if bx0 < 0 { bx0 = 0; }
        if by0 < 0 { by0 = 0; }
        if bx1 > w_i - 1 { bx1 = w_i - 1; }
        if by1 > h_i - 1 { by1 = h_i - 1; }
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
                    hcur += hh[sy as usize * wu + sx as usize];
                    hn += 1;
                }
            }
        }
        hcur = if hn != 0 { hcur / hn as f32 } else { 0.0 };
        let hprev = if have_mean { hbase } else { hcur };
        prep_lanes(&bb, (s_acc + 0.5 * len) / wref, 0.7f32 / (0.25 * (w0 + w1) + 0.25), &mut g);
        let loadf = if mode == 3 { 1.0 } else { clamp01(v_load / bp.vdry) };
        let thick = clamp01(v_load);
        let bodyf = if mode == 1 { 0.0 } else { bp.body * loadf };
        let can_deposit = mode != 2 || have_mean;
        for l in 0..bb.ntot {
            seg_a[l] = 0.0;
            seg_wet[l] = 0.0;
            #[cfg(feature = "mixer_ochrell")]
            { seg_cov[l] = 0.; }
            for k in 0..LAT {
                seg_z[l][k] = 0.0;
            }
        }
        let e0 = 0.35f32;
        let e1 = e0 + 0.12 + 0.7 * (1.0 - bp.hardness);

        if bx1 >= bx0 && by1 >= by0 {
            for py in by0..=by1 {
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
                        let prev_owns = tp >= 0.0 && tp < 1.0 && up >= -1.45 && up <= 1.45;
                        if prev_owns {
                            continue;
                        }
                        if t < 0.0 && !(tp >= 1.0 && up >= -1.45 && up <= 1.45) {
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
                    if u < -1.45 || u > 1.45 {
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
                    if mode == 1 {
                        let hb = match hblur { Some(a) => a[idx], None => 0.0 };
                        alpha *= smoothstep(bp.dry_thresh - bp.dry_width, bp.dry_thresh + bp.dry_width, hh[idx] - hb);
                    }
                    if alpha <= 0.002 {
                        continue;
                    }
                    if alpha > 1.0 {
                        alpha = 1.0;
                    }
                    let lpix = &mut lat[idx * LAT..idx * LAT + LAT];
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
                    #[cfg(not(feature = "mixer_ochrell"))]
                    let sample_amount = alpha;
                    #[cfg(feature = "mixer_ochrell")]
                    let sample_amount = alpha * amount[idx];
                    for k in 0..LAT { seg_z[bd][k] += sample_amount * lpix[k]; }
                    seg_a[bd] += sample_amount;
                    seg_wet[bd] += sample_amount * wet[idx];
                    #[cfg(feature = "mixer_ochrell")]
                    { seg_cov[bd] += alpha; }
                    if !can_deposit {
                        continue;
                    }
                    let mut zpix = [0.0f32; LAT];
                    #[cfg(feature = "mixer_ochrell")]
                    let source_amount = if mode == 2 {
                        (0..smp.n).map(|c| {
                            (if use_r { smp.w_r[c] } else { smp.w_f[c] }) / wsum * tip_amount[smp.idx[c]]
                        }).sum::<f32>()
                    } else { v_load.max(0.) };
                    #[cfg(feature = "mixer_ochrell")]
                    if source_amount <= 1e-8 { continue; }
                    if mode == 2 {
                        for c in 0..smp.n {
                            let w = (if use_r { smp.w_r[c] } else { smp.w_f[c] }) / wsum;
                            let l = smp.idx[c];
                            #[cfg(feature = "mixer_ochrell")]
                            let w = w * tip_amount[l] / source_amount;
                            for k in 0..LAT {
                                zpix[k] += w * ztip[l][k];
                            }
                        }
                    } else {
                        for c in 0..smp.n {
                            let w = (if use_r { smp.w_r[c] } else { smp.w_f[c] }) / wsum;
                            let l = smp.idx[c];
                            #[cfg(not(feature = "mixer_ochrell"))]
                            let dpix = clamp01(dirt[l] * (1.0 + bp.streak_mix * bb.tstreak[l]));
                            // Ochrell tips already contain load + collected paint,
                            // combined by their amounts. No second dirt dilution.
                            #[cfg(feature = "mixer_ochrell")]
                            let dpix = 1.0;
                            for k in 0..LAT {
                                #[cfg(not(feature = "mixer_ochrell"))]
                                { zpix[k] += w * (zload[l][k] + dpix * (ztip[l][k] - zload[l][k])); }
                                #[cfg(feature = "mixer_ochrell")]
                                { zpix[k] += w * ((1. - dpix) * zload[l][k] + dpix * ztip[l][k]); }
                            }
                        }
                    }
                    let a = alpha;
                    #[cfg(not(feature = "mixer_ochrell"))]
                    {
                    for k in 0..LAT {
                        lpix[k] += a * (zpix[k] - lpix[k]);
                    }
                    latent_to_rgb(lpix, &mut rgb[idx * 3..idx * 3 + 3]);
                    }
                    #[cfg(feature = "mixer_ochrell")]
                    crate::material::deposit(lpix, &mut rgb[idx * 3..idx * 3 + 3], &mut amount[idx],
                        &zpix, a * source_amount, wet[idx], a, mode == 3, cv.mixing_mode);
                    let base = hprev + (hcur - hprev) * t;
                    if mode == 0 {
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
                        hh[idx] += a * fl * (base + hadd - hh[idx]) + a * (1.0 - fl) * 0.5 * hadd;
                        if wet[idx] < a {
                            wet[idx] = a;
                        }
                    } else if mode == 1 {
                        hh[idx] += a * bp.hgain * 0.5 * (0.4 + 0.6 * smp.ridge);
                        if wet[idx] < 0.5 * a {
                            wet[idx] = 0.5 * a;
                        }
                    } else if mode == 2 {
                        hh[idx] -= a * bp.flatten * (hh[idx] - base);
                        #[cfg(feature = "mixer_ochrell")]
                        { wet[idx] = wet[idx].max(a); }
                    }
                    cover[idx] += a;
                    stat_alpha += a as f64;
                    stat_pix += 1;
                }
            }
        }
        for l in 0..bb.ntot {
            if seg_a[l] <= 1e-6 {
                dirt[l] -= bp.release * dirt[l] * (if mode == 2 { 0.0 } else { 1.0 });
                #[cfg(feature = "mixer_ochrell")]
                if mode != 2 && mode != 3 {
                    let release = bp.release.clamp(0., 1.);
                    for k in 0..LAT { ztip[l][k] = (1. - release) * ztip[l][k] + release * zload[l][k]; }
                    tip_amount[l] = (1. - release) * tip_amount[l] + release * v_load.max(0.);
                    crate::material::roundtrip(&mut ztip[l], cv.mixing_mode);
                }
                continue;
            }
            let wetavg = seg_wet[l] / seg_a[l];
            sum_wet_all += seg_wet[l] as f64;
            sum_a_all += seg_a[l] as f64;
            #[cfg(feature = "mixer_ochrell")]
            {
                if mode != 3 {
                    let picked = bp.pickup * wetavg * seg_a[l] / seg_cov[l].max(1e-6);
                    if picked > 0. {
                        let t = picked / (tip_amount[l] + picked);
                        for k in 0..LAT { ztip[l][k] = (1. - t) * ztip[l][k] + t * seg_z[l][k] / seg_a[l]; }
                        tip_amount[l] += picked;
                        crate::material::roundtrip(&mut ztip[l], cv.mixing_mode);
                    }
                    if mode != 2 {
                        // Existing release control replenishes the tip with fresh
                        // reservoir paint. It is not a conserved brush volume.
                        let release = bp.release.clamp(0., 1.);
                        for k in 0..LAT { ztip[l][k] = (1. - release) * ztip[l][k] + release * zload[l][k]; }
                        tip_amount[l] = (1. - release) * tip_amount[l] + release * v_load.max(0.);
                        crate::material::roundtrip(&mut ztip[l], cv.mixing_mode);
                    }
                }
            }
            #[cfg(not(feature = "mixer_ochrell"))]
            {
            if mode == 2 {
                for k in 0..LAT {
                    ztip[l][k] += bp.pickup * (seg_z[l][k] / seg_a[l] - ztip[l][k]);
                }
                dirt[l] = 1.0;
            } else if mode != 3 {
                let rate = 0.6f32 * wetavg;
                for k in 0..LAT {
                    ztip[l][k] += rate * (seg_z[l][k] / seg_a[l] - ztip[l][k]);
                }
                let target = bp.pickup * wetavg;
                if target > dirt[l] {
                    dirt[l] += 0.6 * (target - dirt[l]);
                } else {
                    dirt[l] -= bp.release * (dirt[l] - target);
                }
            }
            }
        }
        hbase = hcur;
        have_mean = true;
        v_load -= bp.deplete * (len / wref);
        if v_load < 0.0 {
            v_load = 0.0;
        }
        s_acc += len;
        ptx = tx; pty = ty; pnx = nx; pny = ny; plen = len; px0 = x0; py0 = y0; pw0 = w0; pw1 = w1;
        have_prev = true;
    }
    if let Some(o) = out_stats {
        o[0] = stat_alpha as f32;
        o[1] = stat_pix as f32;
        o[2] = if sum_a_all > 0.0 { (sum_wet_all / sum_a_all) as f32 } else { 0.0 };
    }
    let _ = &region_unused;
    stat_pix as i32
}

#[allow(non_upper_case_globals)]
const region_unused: u8 = 0;
