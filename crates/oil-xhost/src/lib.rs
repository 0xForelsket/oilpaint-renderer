//! Cross-host determinism cases (ci/xhost, docs/plans/LIBRARY_PLAN.md section 5, check G1).
//!
//! Each case is a deterministic function that returns bytes. The same cases run natively (`xhost` binary) and as
//! WASM in Node, Chromium and Firefox (exports below); the hosts hash the bytes with their own SHA-256 and
//! `ci/xhost/compare.mjs` requires identical digests. Cases: maths (L0) with negative controls, the test sheet's
//! StrokeList bytes and every canvas plane plus the lit image with each mixer (L1); the scene compiler's guide
//! planes for Storm Light and the validator's error report (L2); planner cases come in L3.
#![deny(unsafe_code)]

use oil_kernel::Canvas;
use oil_light::LightParams;
use oil_mix::{Mixer, OchrellMixer, RgbMixer};
use oil_mix_mixbox::MixboxMixer;
use oil_paint::testsheet::testsheet;
use std::sync::OnceLock;

/// What the comparison requires of a case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expect {
    /// Every host must produce identical bytes (the engine guarantee).
    Identical,
    /// A negative control: code the engine must not use, kept to show that the harness sees the difference.
    MayDiffer,
}

pub struct Case {
    pub name: String,
    pub expect: Expect,
    pub run: Box<dyn Fn() -> Vec<u8> + Send + Sync>,
}

pub fn cases() -> &'static [Case] {
    static CASES: OnceLock<Vec<Case>> = OnceLock::new();
    CASES.get_or_init(build_cases)
}

pub fn engine_version() -> &'static str {
    oil_kernel::ENGINE_VERSION
}

fn case(name: impl Into<String>, expect: Expect, run: impl Fn() -> Vec<u8> + Send + Sync + 'static) -> Case {
    Case { name: name.into(), expect, run: Box::new(run) }
}

/// Test-sheet width for the kernel cases: small enough for every browser, large enough for all four modes.
const SHEET_WIDTH: u32 = 400;

fn build_cases() -> Vec<Case> {
    let mut v = vec![
        case("math.exp.f64", Expect::Identical, exp_f64),
        case("math.sincos.f64", Expect::Identical, sincos_f64),
        case("math.expf.f32", Expect::Identical, expf_f32),
        case("control.libm.powf_srgb", Expect::MayDiffer, control_powf_srgb),
        case("control.libm.exp", Expect::MayDiffer, control_exp),
        case("strokes.testsheet", Expect::Identical, || testsheet().to_bytes()),
    ];
    scene_cases(&mut v);
    kernel_cases::<OchrellMixer>(&mut v, "ochrell", painted_ochrell);
    kernel_cases::<RgbMixer>(&mut v, "rgb", painted_rgb);
    kernel_cases::<MixboxMixer>(&mut v, "mixbox", painted_mixbox);
    v
}

/// The test sheet painted (and lit) once per mixer; each plane is its own case.
type Painted<M> = (Canvas<M>, Vec<[f32; 3]>);

fn paint_sheet<M: Mixer + Default>() -> Painted<M> {
    let (cv, _) = oil_paint::paint(&M::default(), &testsheet(), SHEET_WIDTH, |_, _| {});
    let lit = oil_light::relight(&cv.rgb, &cv.hgt, cv.w, cv.h, &LightParams::default());
    (cv, lit)
}

fn painted_ochrell() -> &'static Painted<OchrellMixer> {
    static P: OnceLock<Painted<OchrellMixer>> = OnceLock::new();
    P.get_or_init(paint_sheet::<OchrellMixer>)
}

fn painted_rgb() -> &'static Painted<RgbMixer> {
    static P: OnceLock<Painted<RgbMixer>> = OnceLock::new();
    P.get_or_init(paint_sheet::<RgbMixer>)
}

fn painted_mixbox() -> &'static Painted<MixboxMixer> {
    static P: OnceLock<Painted<MixboxMixer>> = OnceLock::new();
    P.get_or_init(paint_sheet::<MixboxMixer>)
}

fn kernel_cases<M: Mixer>(v: &mut Vec<Case>, id: &str, painted: fn() -> &'static Painted<M>) {
    for plane in oil_paint::PLANES {
        v.push(case(format!("kernel.testsheet@{id}@{SHEET_WIDTH}.{plane}"), Expect::Identical, move || {
            oil_paint::plane_bytes(&painted().0, plane)
        }));
    }
    v.push(case(format!("light.testsheet@{id}@{SHEET_WIDTH}.lit"), Expect::Identical, move || {
        f32_bytes(painted().1.iter().flatten().copied())
    }));
}
/// Storm Light's ScenePlan as the TS DSL writes it (scenes/storm_v3.ts).
const STORM_PLAN: &str = include_str!("../../../spec/examples/storm_v3.sceneplan.json");
/// Guide width for the compiler cases: the default plan width.
const GUIDE_WIDTH: u32 = 600;

fn storm_guides() -> &'static oil_scene::Guides {
    static G: OnceLock<oil_scene::Guides> = OnceLock::new();
    G.get_or_init(|| {
        let (plan, _) = oil_scene::load(STORM_PLAN).expect("the example ScenePlan is valid");
        oil_scene::compile(&plan, GUIDE_WIDTH, &OchrellMixer, &Default::default(), &|| 0.0).expect("it compiles").0
    })
}

fn scene_cases(v: &mut Vec<Case>) {
    let name = |plane: &str| format!("scene.storm_v3@{GUIDE_WIDTH}.{plane}");
    v.push(case(name("target"), Expect::Identical, || f32_bytes(storm_guides().target.iter().flatten().copied())));
    v.push(case(name("regionId"), Expect::Identical, || storm_guides().region_id.clone()));
    v.push(case(name("masks"), Expect::Identical, || f32_bytes(storm_guides().masks.iter().flatten().copied())));
    v.push(case(name("flow"), Expect::Identical, || f32_bytes(storm_guides().flow.iter().flatten().copied())));
    v.push(case(name("regionFlows"), Expect::Identical, || {
        f32_bytes(storm_guides().region_flows.iter().flatten().flatten().flatten().copied())
    }));
    v.push(case(name("light"), Expect::Identical, || f32_bytes(storm_guides().light.iter().copied())));
    // every error of a broken plan: codes, paths and offending values, in order, must not depend on the host
    // (messages and fixes are free text that may change without an engine-version bump, so they are left out)
    v.push(case("scene.validate.codes", Expect::Identical, || {
        let mut plan: serde_json::Value = serde_json::from_str(STORM_PLAN).expect("json");
        plan["styles"]["sky"]["width"] = serde_json::json!([25, 40]);
        plan["regions"][3]["name"] = serde_json::json!("sky");
        plan["layers"][1]["regions"][0] = serde_json::json!("stormy");
        plan["target"][1]["blob"]["color"] = serde_json::json!("cobalt_bleu");
        let plan: oil_scene::ScenePlan = serde_json::from_value(plan).expect("schema-valid");
        let r = oil_scene::validate(&plan);
        let rows: Vec<_> = r.errors.iter().chain(&r.warnings).map(|e| (e.code, e.path.clone(), e.got.clone())).collect();
        serde_json::to_vec(&rows).expect("json")
    }));
}

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

/// Paint a StrokeList (the bytes of an `.oilstrokes` file) at `width` with mixer 0 = Ochrell, 1 = rgb, 2 = Mixbox and
/// return the display (rgb) plane's bytes: `ci/xhost/scene.mjs` times this in WASM hosts and compares the digest with
/// the native CLI's `sha256.rgb` for the same file.
pub fn paint_strokelist(bytes: &[u8], width: u32, mixer: u32) -> Result<Vec<u8>, String> {
    let list = oil_strokes::StrokeList::from_bytes(bytes).map_err(|e| e[0].to_string())?;
    fn rgb<M: Mixer>(m: &M, list: &oil_strokes::StrokeList, width: u32) -> Vec<u8> {
        let (cv, _) = oil_paint::paint(m, list, width, |_, _| {});
        oil_paint::plane_bytes(&cv, "rgb")
    }
    match mixer {
        0 => Ok(rgb(&OchrellMixer, &list, width)),
        1 => Ok(rgb(&RgbMixer, &list, width)),
        2 => Ok(rgb(&MixboxMixer, &list, width)),
        _ => Err(format!("unknown mixer {mixer}")),
    }
}
/// C-ABI exports for the WASM hosts (no bindgen: the JS side is `ci/xhost/wasm-host.mjs`). Results are returned
/// through one buffer: a call returns a byte length, then `xhost_buf_ptr` gives its address in linear memory.
#[cfg(target_arch = "wasm32")]
#[allow(unsafe_code)] // #[no_mangle] exports
mod wasm_abi {
    use std::sync::Mutex;

    static BUF: Mutex<Vec<u8>> = Mutex::new(Vec::new());
    static INPUT: Mutex<Vec<u8>> = Mutex::new(Vec::new());

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
        put(super::cases()[i as usize].name.clone().into_bytes())
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

    /// Make room for `len` input bytes and return their address (the host copies a StrokeList there).
    #[no_mangle]
    pub extern "C" fn xhost_input(len: u32) -> u32 {
        let mut input = INPUT.lock().unwrap();
        input.clear();
        input.resize(len as usize, 0);
        input.as_ptr() as usize as u32
    }

    /// Paint the input StrokeList; returns the rgb plane's length in the result buffer, or 0 with an error message
    /// in the result buffer.
    #[no_mangle]
    pub extern "C" fn xhost_paint(width: u32, mixer: u32) -> u32 {
        let input = std::mem::take(&mut *INPUT.lock().unwrap());
        match super::paint_strokelist(&input, width, mixer) {
            Ok(rgb) => put(rgb),
            Err(e) => {
                put(e.into_bytes());
                0
            }
        }
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
        let names: std::collections::BTreeSet<_> = super::cases().iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names.len(), super::cases().len());
        for c in super::cases().iter().filter(|c| c.expect == super::Expect::Identical) {
            assert_eq!((c.run)(), (c.run)(), "{} is not repeatable", c.name);
        }
    }
}
