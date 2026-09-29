//! oilcore spike: brush.c kernel + relight in Rust, one crate for native (cdylib/rlib) and wasm32.
//! C ABI mirrors brush.c (render_stroke, render_strokes, latent_to_rgb_array) so Python can swap libraries;
//! `oc_*` functions are the self-contained entry points used by the WASM harness (corpus replay, relight).
pub mod imgops;
pub mod kernel;
pub mod light;
pub mod math;
pub mod km_tables;

use kernel::{BrushParams, Canvas, LAT};

// ------------------------------------------------------------------ brush.c-compatible C ABI
#[no_mangle]
pub unsafe extern "C" fn render_stroke(
    cv: *mut Canvas,
    pts: *const f32,
    n: i32,
    zcol: *const f32,
    zcol2: *const f32,
    dz: *const f32,
    bp: *const BrushParams,
    out_stats: *mut f32,
) -> i32 {
    let pts = std::slice::from_raw_parts(pts, (n.max(0) as usize) * 4);
    let st = if out_stats.is_null() { None } else { Some(std::slice::from_raw_parts_mut(out_stats, 3)) };
    kernel::render_stroke_impl(
        &mut *cv,
        pts,
        std::slice::from_raw_parts(zcol, LAT),
        std::slice::from_raw_parts(zcol2, LAT),
        std::slice::from_raw_parts(dz, LAT),
        &*bp,
        st,
    )
}

#[no_mangle]
pub unsafe extern "C" fn render_strokes(
    cv: *mut Canvas,
    pts_all: *const f32,
    offsets: *const i32,
    n_strokes: i32,
    zcols: *const f32,
    zcols2: *const f32,
    dzs: *const f32,
    params: *const BrushParams,
) -> i32 {
    let mut total: i64 = 0;
    for i in 0..n_strokes.max(0) as usize {
        let a = *offsets.add(i) as usize;
        let b = *offsets.add(i + 1) as usize;
        total += render_stroke(
            cv,
            pts_all.add(a * 4),
            (b - a) as i32,
            zcols.add(i * LAT),
            zcols2.add(i * LAT),
            dzs.add(i * LAT),
            params.add(i),
            std::ptr::null_mut(),
        ) as i64;
    }
    total.min(2147483647) as i32
}

#[no_mangle]
pub unsafe extern "C" fn latent_to_rgb_array(lat: *const f32, rgb: *mut f32, n: i32) {
    let lat = std::slice::from_raw_parts(lat, n.max(0) as usize * LAT);
    let rgb = std::slice::from_raw_parts_mut(rgb, n.max(0) as usize * 3);
    for i in 0..n.max(0) as usize {
        kernel::latent_to_rgb(&lat[i * LAT..i * LAT + LAT], &mut rgb[i * 3..i * 3 + 3]);
    }
}

// ------------------------------------------------------------------ corpus (stroke file v2 prototype) and replay
pub struct Layer {
    pub start: usize,
    pub end: usize,
    pub needs_hblur: bool,
    pub hblur_sigma: f32,
    pub dry_after: Option<f32>,
}

pub struct Corpus {
    pub aspect: f32,
    pub ground_lat: [f32; LAT],
    pub ground_rgb: [f32; 3],
    pub layers: Vec<Layer>,
    pub offsets: Vec<u32>,
    pub pts: Vec<f32>,
    pub zcol: Vec<f32>,
    pub zcol2: Vec<f32>,
    pub dz: Vec<f32>,
    pub params: Vec<BrushParams>,
}

struct Rd<'a> {
    b: &'a [u8],
    o: usize,
}
impl<'a> Rd<'a> {
    fn u32(&mut self) -> u32 {
        let v = u32::from_le_bytes([self.b[self.o], self.b[self.o + 1], self.b[self.o + 2], self.b[self.o + 3]]);
        self.o += 4;
        v
    }
    fn f32(&mut self) -> f32 {
        f32::from_bits(self.u32())
    }
    fn f32s(&mut self, n: usize) -> Vec<f32> {
        (0..n).map(|_| self.f32()).collect()
    }
}

pub fn parse_corpus(buf: &[u8]) -> Corpus {
    let mut r = Rd { b: buf, o: 0 };
    assert_eq!(r.u32(), 0x3143504F, "bad magic");
    let _ver = r.u32();
    let ns = r.u32() as usize;
    let np = r.u32() as usize;
    let nl = r.u32() as usize;
    assert_eq!(r.u32() as usize, LAT);
    let aspect = r.f32();
    let mut ground_lat = [0f32; LAT];
    for k in 0..LAT {
        ground_lat[k] = r.f32();
    }
    let ground_rgb = [r.f32(), r.f32(), r.f32()];
    let mut layers = Vec::with_capacity(nl);
    for _ in 0..nl {
        let start = r.u32() as usize;
        let end = r.u32() as usize;
        let needs = r.u32() != 0;
        let sig = r.f32();
        let dry = r.f32();
        layers.push(Layer { start, end, needs_hblur: needs, hblur_sigma: sig, dry_after: if dry.is_nan() { None } else { Some(dry) } });
    }
    let offsets: Vec<u32> = (0..ns + 1).map(|_| r.u32()).collect();
    let pts = r.f32s(np * 4);
    let zcol = r.f32s(ns * LAT);
    let zcol2 = r.f32s(ns * LAT);
    let dz = r.f32s(ns * LAT);
    let mut params = Vec::with_capacity(ns);
    for _ in 0..ns {
        let mut p = BrushParams::default();
        p.mode = r.u32() as i32;
        p.opacity = r.f32(); p.pickup = r.f32(); p.load = r.f32(); p.deplete = r.f32(); p.vdry = r.f32();
        p.hgain = r.f32(); p.flatten = r.f32(); p.streak = r.f32(); p.hardness = r.f32(); p.grain = r.f32();
        p.dry_thresh = r.f32(); p.dry_width = r.f32(); p.nb = r.u32() as i32; p.allow_mask = r.u32();
        p.override_p = r.f32(); p.seed = r.u32(); p.dropout = r.f32(); p.ragged = r.f32(); p.body = r.f32();
        p.release = r.f32(); p.streak_mix = r.f32(); p.ridge = r.f32(); p.levee = r.f32(); p.furrow = r.f32();
        p.blob = r.f32(); p.stiff = r.f32(); p.marble = r.f32(); p.splay = r.f32();
        params.push(p);
    }
    Corpus { aspect, ground_lat, ground_rgb, layers, offsets, pts, zcol, zcol2, dz, params }
}

pub struct Planes {
    pub w: usize,
    pub h: usize,
    pub lat: Vec<f32>,
    pub rgb: Vec<f32>,
    pub hgt: Vec<f32>,
    pub wet: Vec<f32>,
    pub cover: Vec<f32>,
    pub hblur: Vec<f32>,
}

impl Planes {
    pub fn new(w: usize, h: usize, glat: &[f32; LAT], grgb: &[f32; 3]) -> Planes {
        let n = w * h;
        let mut lat = vec![0f32; n * LAT];
        let mut rgb = vec![0f32; n * 3];
        for i in 0..n {
            lat[i * LAT..i * LAT + LAT].copy_from_slice(glat);
            rgb[i * 3..i * 3 + 3].copy_from_slice(grgb);
        }
        Planes { w, h, lat, rgb, hgt: vec![0.0; n], wet: vec![0.0; n], cover: vec![0.0; n], hblur: vec![0.0; n] }
    }
    pub fn canvas(&mut self) -> Canvas {
        Canvas {
            w: self.w as i32,
            h: self.h as i32,
            lat: self.lat.as_mut_ptr(),
            rgb: self.rgb.as_mut_ptr(),
            hgt: self.hgt.as_mut_ptr(),
            wet: self.wet.as_mut_ptr(),
            cover: self.cover.as_mut_ptr(),
            hblur: self.hblur.as_mut_ptr(),
            region: std::ptr::null_mut(),
        }
    }
}

/// Replay the whole corpus at width w (render.py semantics: hblur before layers with scumble strokes, dry_after after).
/// Returns (painted pixel count, kernel seconds measured by the caller-supplied clock if any).
pub fn replay(c: &Corpus, pl: &mut Planes, mut clock: Option<&mut dyn FnMut() -> f64>, kernel_secs: &mut f64) -> i64 {
    let (w, h) = (pl.w, pl.h);
    let wf = w as f32;
    let mut total: i64 = 0;
    for layer in &c.layers {
        if layer.needs_hblur {
            let sig = (layer.hblur_sigma as f64 * w as f64).max(1.0);
            pl.hblur = imgops::blur(&pl.hgt, w, h, sig);
        }
        let t0 = clock.as_mut().map(|f| f()).unwrap_or(0.0);
        let mut cv = pl.canvas();
        for i in layer.start..layer.end {
            let a = c.offsets[i] as usize;
            let b = c.offsets[i + 1] as usize;
            let mut pts: Vec<f32> = c.pts[a * 4..b * 4].to_vec();
            for p in pts.chunks_mut(4) {
                p[0] *= wf;
                p[1] *= wf;
                p[2] *= wf;
            }
            total += unsafe {
                kernel::render_stroke_impl(
                    &mut cv,
                    &pts,
                    &c.zcol[i * LAT..i * LAT + LAT],
                    &c.zcol2[i * LAT..i * LAT + LAT],
                    &c.dz[i * LAT..i * LAT + LAT],
                    &c.params[i],
                    None,
                )
            } as i64;
        }
        if let Some(f) = clock.as_mut() {
            *kernel_secs += f() - t0;
        }
        if let Some(d) = layer.dry_after {
            for v in pl.wet.iter_mut() {
                *v *= d;
            }
        }
    }
    total
}

// ------------------------------------------------------------------ WASM-friendly exports
#[no_mangle]
pub extern "C" fn oc_alloc(n: usize) -> *mut u8 {
    let mut v: Vec<u8> = Vec::with_capacity(n.max(1));
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

#[no_mangle]
pub unsafe extern "C" fn oc_free(p: *mut u8, n: usize) {
    drop(Vec::from_raw_parts(p, 0, n.max(1)));
}

#[cfg(target_arch = "wasm32")]
extern "C" {
    fn now_ms() -> f64; // imported from JS: performance.now()
}

/// Replay a corpus buffer at w x h; copies the final unlit rgb (w*h*3 f32) and height (w*h f32) into the outputs.
/// out_times[0] = kernel ms, out_times[1] = total ms (wasm only; native callers time themselves).
#[no_mangle]
pub unsafe extern "C" fn oc_replay(buf: *const u8, len: usize, w: i32, h: i32, out_rgb: *mut f32, out_h: *mut f32, out_times: *mut f64) -> i64 {
    let c = parse_corpus(std::slice::from_raw_parts(buf, len));
    let (w, h) = (w as usize, h as usize);
    let mut pl = Planes::new(w, h, &c.ground_lat, &c.ground_rgb);
    let mut ks = 0.0f64;
    #[cfg(target_arch = "wasm32")]
    let t_start = now_ms();
    #[cfg(target_arch = "wasm32")]
    let total = {
        let mut clk = || now_ms() / 1000.0;
        replay(&c, &mut pl, Some(&mut clk), &mut ks)
    };
    #[cfg(not(target_arch = "wasm32"))]
    let total = replay(&c, &mut pl, None, &mut ks);
    std::ptr::copy_nonoverlapping(pl.rgb.as_ptr(), out_rgb, w * h * 3);
    std::ptr::copy_nonoverlapping(pl.hgt.as_ptr(), out_h, w * h);
    if !out_times.is_null() {
        *out_times = ks * 1000.0;
        #[cfg(target_arch = "wasm32")]
        {
            *out_times.add(1) = now_ms() - t_start;
        }
    }
    total
}

#[no_mangle]
pub unsafe extern "C" fn oc_relight(rgb: *const f32, hgt: *const f32, weave: *const f32, w: i32, h: i32, params: *const light::LightParams, out: *mut f32) {
    let n = (w * h) as usize;
    let wv = if weave.is_null() { None } else { Some(std::slice::from_raw_parts(weave, n)) };
    light::relight(
        std::slice::from_raw_parts(rgb, n * 3),
        std::slice::from_raw_parts(hgt, n),
        wv,
        w as usize,
        h as usize,
        &*params,
        std::slice::from_raw_parts_mut(out, n * 3),
    );
}
