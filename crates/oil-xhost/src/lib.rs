//! Cross-host determinism cases (ci/xhost, docs/plans/LIBRARY_PLAN.md section 5, check G1).
//!
//! Each case is a deterministic function that returns bytes. The same cases run natively (`xhost` binary) and as
//! WASM in Node, Chromium and Firefox (exports below); the hosts hash the bytes with their own SHA-256 and
//! `ci/xhost/compare.mjs` requires identical digests. L0 carries maths cases plus a negative control; L1 adds
//! kernel cases (canvas planes of the swatch sheet), L3 planner cases (StrokeList bytes).
#![deny(unsafe_code)]

/// What the comparison requires of a case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expect {
    /// Every host must produce identical bytes (the engine guarantee).
    Identical,
    /// A negative control: code the engine must not use, kept to show that the harness sees the difference.
    MayDiffer,
}

pub struct Case {
    pub name: &'static str,
    pub expect: Expect,
    pub run: fn() -> Vec<u8>,
}

pub fn cases() -> &'static [Case] {
    &CASES
}

pub fn engine_version() -> &'static str {
    oil_kernel::ENGINE_VERSION
}

static CASES: [Case; 5] = [
    Case { name: "math.exp.f64", expect: Expect::Identical, run: exp_f64 },
    Case { name: "math.sincos.f64", expect: Expect::Identical, run: sincos_f64 },
    Case { name: "math.expf.f32", expect: Expect::Identical, run: expf_f32 },
    Case { name: "control.libm.powf_srgb", expect: Expect::MayDiffer, run: control_powf_srgb },
    Case { name: "control.libm.exp", expect: Expect::MayDiffer, run: control_exp },
];

const N: usize = 200_000;

/// x_i = lo + (hi - lo) * i / n, evaluated identically everywhere (basic ops only).
fn grid(n: usize, lo: f64, hi: f64) -> impl Iterator<Item = f64> {
    (0..n).map(move |i| lo + (hi - lo) * (i as f64) / (n as f64))
}

fn f64_bytes(values: impl Iterator<Item = f64>) -> Vec<u8> {
    values.flat_map(|v| v.to_bits().to_le_bytes()).collect()
}

fn f32_bytes(values: impl Iterator<Item = f32>) -> Vec<u8> {
    values.flat_map(|v| v.to_bits().to_le_bytes()).collect()
}

fn exp_f64() -> Vec<u8> {
    f64_bytes(grid(N, -80.0, 20.0).map(oil_math::exp))
}

fn sincos_f64() -> Vec<u8> {
    f64_bytes(grid(N, -400.0, 400.0).flat_map(|x| [oil_math::sin(x), oil_math::cos(x)]))
}

fn expf_f32() -> Vec<u8> {
    f32_bytes(grid(N, -30.0, 10.0).map(|x| oil_math::expf(x as f32)))
}

/// The sRGB transfer as Ochrell 0.2 computes it (`conversion::srgb_to_linear` / `linear_to_srgb`), with the
/// platform's `powf`. Native targets call the C runtime's pow, wasm32 calls Rust's own libm port.
#[allow(clippy::disallowed_methods)]
fn control_powf_srgb() -> Vec<u8> {
    f64_bytes(grid(N, 0.0, 1.0).flat_map(|x| {
        let lin = if x <= 0.040_45 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) };
        let back = if x <= 0.003_130_8 { 12.92 * x } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 };
        [lin, back]
    }))
}

#[allow(clippy::disallowed_methods)]
fn control_exp() -> Vec<u8> {
    f64_bytes(grid(N, -80.0, 20.0).map(f64::exp))
}

/// C-ABI exports for the WASM hosts (no bindgen: the JS side is `ci/xhost/wasm-host.mjs`). Results are returned
/// through one buffer: a call returns a byte length, then `xhost_buf_ptr` gives its address in linear memory.
#[cfg(target_arch = "wasm32")]
#[allow(unsafe_code)] // #[no_mangle] exports
mod wasm_abi {
    use std::sync::Mutex;

    static BUF: Mutex<Vec<u8>> = Mutex::new(Vec::new());

    fn put(bytes: Vec<u8>) -> u32 {
        let len = bytes.len() as u32;
        *BUF.lock().unwrap() = bytes;
        len
    }

    #[no_mangle]
    pub extern "C" fn xhost_case_count() -> u32 {
        super::cases().len() as u32
    }

    #[no_mangle]
    pub extern "C" fn xhost_case_name(i: u32) -> u32 {
        put(super::cases()[i as usize].name.as_bytes().to_vec())
    }

    #[no_mangle]
    pub extern "C" fn xhost_case_expect(i: u32) -> u32 {
        match super::cases()[i as usize].expect {
            super::Expect::Identical => 0,
            super::Expect::MayDiffer => 1,
        }
    }

    #[no_mangle]
    pub extern "C" fn xhost_engine_version() -> u32 {
        put(super::engine_version().as_bytes().to_vec())
    }

    #[no_mangle]
    pub extern "C" fn xhost_run(i: u32) -> u32 {
        put((super::cases()[i as usize].run)())
    }

    #[no_mangle]
    pub extern "C" fn xhost_buf_ptr() -> u32 {
        BUF.lock().unwrap().as_ptr() as usize as u32
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn case_names_are_unique_and_outputs_repeat() {
        let names: std::collections::BTreeSet<_> = super::cases().iter().map(|c| c.name).collect();
        assert_eq!(names.len(), super::cases().len());
        for c in super::cases().iter().filter(|c| c.expect == super::Expect::Identical) {
            assert_eq!((c.run)(), (c.run)(), "{} is not repeatable", c.name);
        }
    }
}
