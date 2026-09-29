//! ctypes shim: the new kernel behind the Ochrell bridge's C ABI (`native/ochrell-brush`, `oilpaint/ochrell.py`), so the
//! Python renderer and eval harness can paint with it: `OILPAINT_KERNEL=oil` with a material backend
//! (`--mixer ochrell` or `mixbox-material`). It serves the port checks (L1) and level E (L3), and
//! is retired together with the Python harness in L5.
//!
//! The kernel has one transport, the material transport, which needs the canvas `amount` plane; v1's `brush.c` ABI
//! has no such plane and is not served (step 1 of the L1 port check used it: commit a9c573a).
//!
//! Safety contract (as for the bridge): every pointer is non-null, aligned, and addresses a live, disjoint buffer of
//! the stated length for the duration of the call. The Python side checks shapes, dtypes and values.
use oil_kernel::{render_stroke as kernel_stroke, BrushParams, Load, Planes};
use oil_mix::{Mixer, OchrellMixer};
use oil_mix_mixbox::MixboxMixer;

/// Floats per state on the wire (the bridge's format: K[41], S[41], residual[3] for Ochrell).
pub const LAT: usize = 85;
/// Bridge mixing modes this shim serves.
pub const MODE_OCHRELL: i32 = 0;
pub const MODE_MIXBOX_MATERIAL: i32 = 3;

/// Mixbox in the bridge's 85-float wire format (first 7 floats; the rest stay zero): the `mixbox-material`
/// control of the Ochrell integration. Test-only; the engine's Mixbox mixer uses a 7-float state.
#[derive(Clone, Copy, Default)]
pub struct Mixbox85;

impl Mixer for Mixbox85 {
    const ID: &'static str = "mixbox-2.0-padded85";
    type State = [f32; LAT];
    fn encode(&self, srgb: [f32; 3]) -> [f32; LAT] {
        let mut z = [0.0; LAT];
        z[..7].copy_from_slice(&MixboxMixer.encode(srgb));
        z
    }
    fn decode_srgb(&self, z: &[f32; LAT]) -> [f32; 3] {
        MixboxMixer.decode_srgb(z[..7].try_into().unwrap())
    }
}

/// The bridge's canvas struct (`oilpaint/_build.py: OchrellCanvas`).
#[repr(C)]
pub struct OchrellCanvas {
    pub w: i32,
    pub h: i32,
    pub lat: *mut f32,
    pub rgb: *mut f32,
    pub hgt: *mut f32,
    pub wet: *mut f32,
    pub cover: *mut f32,
    pub hblur: *mut f32,
    pub region: *mut u8,
    pub amount: *mut f32,
    pub mixing_mode: i32,
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
        // SAFETY: the caller guarantees `cap` writable bytes at `buf`.
        unsafe { std::ptr::copy_nonoverlapping(v.as_ptr(), buf, v.len().min(cap)) };
    }
    v.len()
}

#[no_mangle]
pub extern "C" fn ochrell_bridge_abi() -> u32 {
    1
}

#[no_mangle]
pub extern "C" fn ochrell_state_len() -> usize {
    LAT
}

/// Encodes `n` sRGB colours into 85-float states (mode 0 Ochrell, 3 Mixbox). Returns 0, -1 for bad arguments, -3 for an
/// unavailable mode.
///
/// # Safety
/// `rgb` addresses `n * 3` floats and `lat` `n * 85` writable floats.
#[no_mangle]
pub unsafe extern "C" fn ochrell_encode(rgb: *const f32, lat: *mut f32, n: usize, mode: i32) -> i32 {
    if n == 0 {
        return 0;
    }
    if rgb.is_null() || lat.is_null() {
        return -1;
    }
    // SAFETY: caller contract.
    let (rgb, lat) = unsafe { (std::slice::from_raw_parts(rgb as *const [f32; 3], n), std::slice::from_raw_parts_mut(lat as *mut [f32; LAT], n)) };
    match mode {
        MODE_OCHRELL => rgb.iter().zip(lat.iter_mut()).for_each(|(c, z)| *z = OchrellMixer.encode(*c)),
        MODE_MIXBOX_MATERIAL => rgb.iter().zip(lat.iter_mut()).for_each(|(c, z)| *z = Mixbox85.encode(*c)),
        _ => return -3,
    }
    0
}

/// Decodes `n` 85-float states to sRGB. Returns 0, -1 for bad arguments, -2 for an invalid Ochrell state, -3 for an
/// unavailable mode.
///
/// # Safety
/// `lat` addresses `n * 85` floats and `rgb` `n * 3` writable floats.
#[no_mangle]
pub unsafe extern "C" fn ochrell_decode(lat: *const f32, rgb: *mut f32, n: usize, mode: i32) -> i32 {
    if n == 0 {
        return 0;
    }
    if rgb.is_null() || lat.is_null() {
        return -1;
    }
    // SAFETY: caller contract.
    let (lat, rgb) = unsafe { (std::slice::from_raw_parts(lat as *const [f32; LAT], n), std::slice::from_raw_parts_mut(rgb as *mut [f32; 3], n)) };
    match mode {
        MODE_OCHRELL => {
            if lat.iter().any(|z| !OchrellMixer.is_valid(z)) {
                return -2;
            }
            lat.iter().zip(rgb.iter_mut()).for_each(|(z, c)| *c = OchrellMixer.decode_srgb(z))
        }
        MODE_MIXBOX_MATERIAL => lat.iter().zip(rgb.iter_mut()).for_each(|(z, c)| *c = Mixbox85.decode_srgb(z)),
        _ => return -3,
    }
    0
}

/// # Safety
/// Forwarded from `render_stroke`.
#[allow(clippy::too_many_arguments)] // mirrors the C ABI
unsafe fn paint<M: Mixer<State = [f32; LAT]>>(
    m: &M,
    cv: &mut OchrellCanvas,
    pts: *const f32,
    n: i32,
    zcol: *const f32,
    zcol2: *const f32,
    dz: *const f32,
    bp: *const CBrushParams,
    out_stats: *mut f32,
) -> i32 {
    // SAFETY: forwarded caller contract.
    unsafe {
        let npx = cv.w as usize * cv.h as usize;
        let mut planes = Planes::<M::State> {
            w: cv.w as usize,
            h: cv.h as usize,
            lat: std::slice::from_raw_parts_mut(cv.lat as *mut M::State, npx),
            rgb: std::slice::from_raw_parts_mut(cv.rgb as *mut [f32; 3], npx),
            hgt: std::slice::from_raw_parts_mut(cv.hgt, npx),
            wet: std::slice::from_raw_parts_mut(cv.wet, npx),
            cover: std::slice::from_raw_parts_mut(cv.cover, npx),
            amount: std::slice::from_raw_parts_mut(cv.amount, npx),
            hblur: if cv.hblur.is_null() { &[] } else { std::slice::from_raw_parts(cv.hblur, npx) },
        };
        let pts = std::slice::from_raw_parts(pts as *const [f32; 4], n as usize);
        let s = |p: *const f32| *(p as *const M::State);
        let load = Load { zcol: s(zcol), zcol2: s(zcol2), dz: s(dz) };
        let st = kernel_stroke(m, &mut planes, pts, &load, &BrushParams::from(&*bp));
        if !out_stats.is_null() {
            let o = std::slice::from_raw_parts_mut(out_stats, 3);
            o[0] = st.alpha as f32;
            o[1] = st.pixels as f32;
            o[2] = st.wet;
        }
        st.pixels.min(i32::MAX as i64) as i32
    }
}

/// Paints one stroke; returns the painted pixel count, -1 if the kernel panicked, -3 for an unavailable mode.
///
/// # Safety
/// See the crate documentation: `cv` and its planes, `pts` (`n * 4` floats), `zcol`/`zcol2`/`dz` (85 floats each),
/// `bp`, and `out_stats` (3 floats, or null) must be valid.
#[no_mangle]
pub unsafe extern "C" fn render_stroke(
    cv: *mut OchrellCanvas,
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
            match cv.mixing_mode {
                MODE_OCHRELL => paint(&OchrellMixer, cv, pts, n, zcol, zcol2, dz, bp, out_stats),
                MODE_MIXBOX_MATERIAL => paint(&Mixbox85, cv, pts, n, zcol, zcol2, dz, bp, out_stats),
                _ => -3,
            }
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
    cv: *mut OchrellCanvas,
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
            render_stroke(cv, pts_all.add(a * 4), (b - a) as i32, zcols.add(i * LAT), zcols2.add(i * LAT), dzs.add(i * LAT), params.add(i), std::ptr::null_mut())
        };
        if r < 0 {
            return r;
        }
        total += r as i64;
    }
    total.min(i32::MAX as i64) as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use oil_mix::State;

    #[test]
    fn padded_mixbox_matches_mixbox() {
        let c = [0.2, 0.6, 0.9];
        let z = Mixbox85.encode(c);
        assert!(z[7..].iter().all(|v| *v == 0.0));
        assert_eq!(Mixbox85.decode_srgb(&z), MixboxMixer.decode_srgb(&MixboxMixer.encode(c)));
        assert_eq!(<[f32; LAT] as State>::LEN, 85);
    }
}
