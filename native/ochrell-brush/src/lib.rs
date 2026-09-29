//! Renderer-specific trusted-buffer C ABI. Python checks shapes/dtypes/values.
//! All pointer arguments must be aligned, non-null, and reference disjoint live
//! buffers of the stated length. The caller retains ownership for the call.
//! Do not expose these unsafe entry points directly to untrusted pointer input.
#[cfg(any(not(feature = "mixer_ochrell"), feature = "mixer_km12"))]
compile_error!("the bridge requires mixer_ochrell and excludes mixer_km12");
#[path = "../../../spikes/oilcore/src/kernel.rs"]
pub mod kernel;
pub mod material;
#[path = "../../../spikes/oilcore/src/math.rs"]
pub mod math;
use kernel::{BrushParams, Canvas, LAT};

#[no_mangle]
pub extern "C" fn ochrell_bridge_abi() -> u32 {
    1
}
#[no_mangle]
pub extern "C" fn ochrell_state_len() -> usize {
    LAT
}

// Diagnostic-only reference adapter for reproduction (81 bands / f64).
#[no_mangle]
pub unsafe extern "C" fn ochrell_reference_encode(rgb: *const f32, out: *mut f64, n: usize) -> i32 {
    if n == 0 {
        return 0;
    }
    if rgb.is_null() || out.is_null() {
        return -1;
    }
    let rgb = std::slice::from_raw_parts(rgb, n * 3);
    let out = std::slice::from_raw_parts_mut(out, n * 165);
    for (c, z) in rgb.chunks_exact(3).zip(out.chunks_exact_mut(165)) {
        let Ok(c) = ochrell::Color::srgb(c[0], c[1], c[2]) else {
            return -2;
        };
        let a = ochrell::ReferenceSpectralMixer.encode(c);
        z[..81].copy_from_slice(a.absorption());
        z[81..162].copy_from_slice(a.scattering());
        z[162..].copy_from_slice(&a.residual());
    }
    0
}
#[no_mangle]
pub unsafe extern "C" fn ochrell_reference_decode(lat: *const f64, out: *mut f32, n: usize) -> i32 {
    if n == 0 {
        return 0;
    }
    if lat.is_null() || out.is_null() {
        return -1;
    }
    let lat = std::slice::from_raw_parts(lat, n * 165);
    let out = std::slice::from_raw_parts_mut(out, n * 3);
    for (z, c) in lat.chunks_exact(165).zip(out.chunks_exact_mut(3)) {
        let Ok(z) = ochrell::ReferenceLatent::try_from_parts(
            z[..81].try_into().unwrap(),
            z[81..162].try_into().unwrap(),
            z[162..].try_into().unwrap(),
        ) else {
            return -2;
        };
        c.copy_from_slice(&ochrell::ReferenceSpectralMixer.decode(z).channels());
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn ochrell_encode(
    rgb: *const f32,
    lat: *mut f32,
    n: usize,
    mode: i32,
) -> i32 {
    if n == 0 {
        return 0;
    }
    if rgb.is_null() || lat.is_null() || !(0..=2).contains(&mode) {
        return -1;
    }
    let rgb = std::slice::from_raw_parts(rgb, n * 3);
    let lat = std::slice::from_raw_parts_mut(lat, n * LAT);
    for (c, z) in rgb.chunks_exact(3).zip(lat.chunks_exact_mut(LAT)) {
        if material::encode(c.try_into().unwrap(), z, mode).is_err() {
            return -2;
        }
    }
    0
}
#[no_mangle]
pub unsafe extern "C" fn ochrell_decode(
    lat: *const f32,
    rgb: *mut f32,
    n: usize,
    mode: i32,
) -> i32 {
    if n == 0 {
        return 0;
    }
    let max_mode = if cfg!(feature = "mixbox_comparison") {
        3
    } else {
        2
    };
    if rgb.is_null() || lat.is_null() || !(0..=max_mode).contains(&mode) {
        return -1;
    }
    let lat = std::slice::from_raw_parts(lat, n * LAT);
    let rgb = std::slice::from_raw_parts_mut(rgb, n * 3);
    for (z, c) in lat.chunks_exact(LAT).zip(rgb.chunks_exact_mut(3)) {
        if mode <= 1 && material::unpack(z).is_err() {
            return -2;
        }
        c.copy_from_slice(&material::decode(z, mode));
    }
    0
}
#[no_mangle]
pub unsafe extern "C" fn render_stroke(
    cv: *mut Canvas,
    pts: *const f32,
    n: i32,
    zcol: *const f32,
    zcol2: *const f32,
    dz: *const f32,
    bp: *const BrushParams,
    stats: *mut f32,
) -> i32 {
    if n < 2 {
        return 0;
    }
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        kernel::render_stroke_impl(
            &mut *cv,
            std::slice::from_raw_parts(pts, n as usize * 4),
            std::slice::from_raw_parts(zcol, LAT),
            std::slice::from_raw_parts(zcol2, LAT),
            std::slice::from_raw_parts(dz, LAT),
            &*bp,
            if stats.is_null() {
                None
            } else {
                Some(std::slice::from_raw_parts_mut(stats, 3))
            },
        )
    }))
    .unwrap_or(-1)
}
#[no_mangle]
pub unsafe extern "C" fn render_strokes(
    cv: *mut Canvas,
    pts: *const f32,
    offsets: *const i32,
    n: i32,
    zcol: *const f32,
    zcol2: *const f32,
    dz: *const f32,
    params: *const BrushParams,
) -> i32 {
    let mut count = 0_i64;
    for i in 0..n.max(0) as usize {
        let a = *offsets.add(i);
        let b = *offsets.add(i + 1);
        let result = render_stroke(
            cv,
            pts.add(a as usize * 4),
            b - a,
            zcol.add(i * LAT),
            zcol2.add(i * LAT),
            dz.add(i * LAT),
            params.add(i),
            std::ptr::null_mut(),
        );
        if result < 0 {
            return result;
        }
        count += result as i64;
    }
    count.min(i32::MAX as i64) as i32
}
