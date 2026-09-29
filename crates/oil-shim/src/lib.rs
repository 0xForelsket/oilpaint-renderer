//! ctypes shim: the new kernel behind v1's `brush.c` C ABI, so the Python renderer and eval harness can paint with
//! it (`OILPAINT_KERNEL=oil`, see `oilpaint/_build.py`). It serves the port checks (P1 in L1, level E in L3) and is
//! retired together with the Python harness in L5.
//!
//! Safety contract (as for `brush.c`): every pointer is non-null, aligned, and addresses a live, disjoint buffer of
//! the stated length for the duration of the call. The Python side checks shapes and dtypes.
use oil_kernel::{render_stroke as kernel_stroke, BrushParams, Load, Planes};
use oil_mix::Mixer;
use oil_mix_mixbox::MixboxMixer;

type M = MixboxMixer;
const LAT: usize = 7;

/// v1's `Canvas` struct from `brush.c` (`oilpaint/_build.py: CCanvas`).
#[repr(C)]
pub struct CCanvas {
    pub w: i32,
    pub h: i32,
    pub lat: *mut f32,
    pub rgb: *mut f32,
    pub hgt: *mut f32,
    pub wet: *mut f32,
    pub cover: *mut f32,
    pub hblur: *mut f32,
    pub region: *mut u8,
}

/// v1's `BrushParams` layout (`oilpaint/_build.py`). `allow_mask` and `override_p` were never read by the kernel.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CBrushParams {
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

impl From<&CBrushParams> for BrushParams {
    fn from(c: &CBrushParams) -> Self {
        BrushParams {
            mode: c.mode,
            opacity: c.opacity,
            pickup: c.pickup,
            load: c.load,
            deplete: c.deplete,
            vdry: c.vdry,
            hgain: c.hgain,
            flatten: c.flatten,
            streak: c.streak,
            hardness: c.hardness,
            grain: c.grain,
            dry_thresh: c.dry_thresh,
            dry_width: c.dry_width,
            nb: c.nb,
            seed: c.seed,
            dropout: c.dropout,
            ragged: c.ragged,
            body: c.body,
            release: c.release,
            streak_mix: c.streak_mix,
            ridge: c.ridge,
            levee: c.levee,
            furrow: c.furrow,
            blob: c.blob,
            stiff: c.stiff,
            marble: c.marble,
            splay: c.splay,
        }
    }
}

/// Writes the engine version into `buf` (at most `cap` bytes) and returns its full length.
///
/// # Safety
/// `buf` must address `cap` writable bytes (or `cap` must be 0).
#[no_mangle]
pub unsafe extern "C" fn oil_engine_version(buf: *mut u8, cap: usize) -> usize {
    let v = oil_kernel::ENGINE_VERSION.as_bytes();
    if !buf.is_null() && cap > 0 {
        let n = v.len().min(cap);
        // SAFETY: the caller guarantees `cap` writable bytes at `buf`.
        unsafe { std::ptr::copy_nonoverlapping(v.as_ptr(), buf, n) };
    }
    v.len()
}

/// Paints one stroke; returns the painted pixel count, or -1 if the kernel panicked.
///
/// # Safety
/// See the crate documentation: `cv` and its planes, `pts` (`n * 4` floats), `zcol`/`zcol2`/`dz` (7 floats each),
/// `bp`, and `out_stats` (3 floats, or null) must be valid.
#[no_mangle]
pub unsafe extern "C" fn render_stroke(
    cv: *mut CCanvas,
    pts: *const f32,
    n: i32,
    zcol: *const f32,
    zcol2: *const f32,
    dz: *const f32,
    bp: *const CBrushParams,
    out_stats: *mut f32,
) -> i32 {
    if n < 2 {
        return 0;
    }
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: forwarded caller contract.
        unsafe {
            let cv = &mut *cv;
            let npx = cv.w as usize * cv.h as usize;
            let mut planes = Planes::<[f32; LAT]> {
                w: cv.w as usize,
                h: cv.h as usize,
                lat: std::slice::from_raw_parts_mut(cv.lat as *mut [f32; LAT], npx),
                rgb: std::slice::from_raw_parts_mut(cv.rgb as *mut [f32; 3], npx),
                hgt: std::slice::from_raw_parts_mut(cv.hgt, npx),
                wet: std::slice::from_raw_parts_mut(cv.wet, npx),
                cover: std::slice::from_raw_parts_mut(cv.cover, npx),
                hblur: if cv.hblur.is_null() { &[] } else { std::slice::from_raw_parts(cv.hblur, npx) },
            };
            let pts = std::slice::from_raw_parts(pts as *const [f32; 4], n as usize);
            let load = Load { zcol: *(zcol as *const [f32; LAT]), zcol2: *(zcol2 as *const [f32; LAT]), dz: *(dz as *const [f32; LAT]) };
            let st = kernel_stroke(&MixboxMixer, &mut planes, pts, &load, &BrushParams::from(&*bp));
            if !out_stats.is_null() {
                let o = std::slice::from_raw_parts_mut(out_stats, 3);
                o[0] = st.alpha as f32;
                o[1] = st.pixels as f32;
                o[2] = st.wet;
            }
            st.pixels.min(i32::MAX as i64) as i32
        }
    }))
    .unwrap_or(-1)
}

/// Paints strokes `0..n_strokes` in order (points of stroke i at `offsets[i]..offsets[i+1]`); returns the total
/// painted pixel count, or a negative value on failure.
///
/// # Safety
/// As `render_stroke`, with `offsets` holding `n_strokes + 1` entries and the colour and parameter arrays one entry
/// per stroke.
#[no_mangle]
pub unsafe extern "C" fn render_strokes(
    cv: *mut CCanvas,
    pts_all: *const f32,
    offsets: *const i32,
    n_strokes: i32,
    zcols: *const f32,
    zcols2: *const f32,
    dzs: *const f32,
    params: *const CBrushParams,
) -> i32 {
    let mut total: i64 = 0;
    for i in 0..n_strokes.max(0) as usize {
        // SAFETY: forwarded caller contract.
        let r = unsafe {
            let a = *offsets.add(i) as usize;
            let b = *offsets.add(i + 1) as usize;
            render_stroke(
                cv,
                pts_all.add(a * 4),
                (b - a) as i32,
                zcols.add(i * LAT),
                zcols2.add(i * LAT),
                dzs.add(i * LAT),
                params.add(i),
                std::ptr::null_mut(),
            )
        };
        if r < 0 {
            return r;
        }
        total += r as i64;
    }
    total.min(i32::MAX as i64) as i32
}

/// Decodes `n` Mixbox latents to sRGB.
///
/// # Safety
/// `lat` addresses `n * 7` floats and `rgb` `n * 3` writable floats.
#[no_mangle]
pub unsafe extern "C" fn latent_to_rgb_array(lat: *const f32, rgb: *mut f32, n: i32) {
    let n = n.max(0) as usize;
    // SAFETY: caller contract.
    let (lat, rgb) = unsafe {
        (std::slice::from_raw_parts(lat as *const [f32; LAT], n), std::slice::from_raw_parts_mut(rgb as *mut [f32; 3], n))
    };
    for (z, c) in lat.iter().zip(rgb.iter_mut()) {
        *c = M::default().decode_srgb(z);
    }
}
