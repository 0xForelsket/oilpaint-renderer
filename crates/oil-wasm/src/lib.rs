//! C-ABI exports for the TypeScript package (`packages/oilpaint/src/engine.ts`). No bindgen and no imports: the
//! host copies input into a buffer (`oil_input`), calls a function that returns a byte length, and reads the
//! result at `oil_buf_ptr()`. Results are JSON unless stated otherwise.
//!
//! L2 exports ScenePlan validation, the schema and the guide compiler (planes and preview images); L3 adds planning
//! (`oil_plan`, `oil_plan_strokes`). Painting is added in L4.
#![deny(unsafe_code)]

use oil_mix::{OchrellMixer, RgbMixer};
use oil_scene::{preview, FieldData, Guides};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Mixer ids across the ABI.
pub const MIXERS: [&str; 2] = ["ochrell", "rgb"];

/// `{valid, errors, warnings}` for a ScenePlan in JSON.
pub fn validate(text: &str) -> Value {
    match oil_scene::parse(text) {
        Err(errors) => json!({ "valid": false, "errors": errors, "warnings": [] }),
        Ok(plan) => {
            let r = oil_scene::validate(&plan);
            json!({ "valid": r.errors.is_empty(), "errors": r.errors, "warnings": r.warnings })
        }
    }
}

/// Compile guides. Returns the summary JSON (or `{errors}`) and the guides.
pub fn guides(text: &str, width: u32, mixer: u32, fields: &BTreeMap<String, FieldData>) -> (Value, Option<Guides>) {
    let (plan, warnings) = match oil_scene::load(text) {
        Ok(p) => p,
        Err(errors) => return (json!({ "errors": errors }), None),
    };
    let clock = || 0.0;
    let res = match mixer {
        0 => oil_scene::compile(&plan, width, &OchrellMixer, fields, &clock),
        1 => oil_scene::compile(&plan, width, &RgbMixer, fields, &clock),
        _ => {
            let e = oil_scene::Error::new("UNKNOWN_MIXER", format!("mixer id {mixer} is not in this build")).got(mixer).expected("0 (ochrell) or 1 (rgb)");
            return (json!({ "errors": [e] }), None);
        }
    };
    match res {
        Err(errors) => (json!({ "errors": errors }), None),
        Ok((g, _)) => {
            let n = (g.w * g.h) as f64;
            let mut counts = vec![0usize; g.names.len()];
            for id in &g.region_id {
                counts[*id as usize] += 1;
            }
            let regions: Vec<Value> = g
                .names
                .iter()
                .enumerate()
                .map(|(i, name)| json!({ "id": i, "name": name, "pixels": counts[i], "share": counts[i] as f64 / n, "flow": g.flows[i].is_some() }))
                .collect();
            let summary = json!({
                "engine": oil_kernel::ENGINE_VERSION,
                "mixer": MIXERS[mixer as usize],
                "size": [g.w, g.h],
                "ground": g.ground,
                "regions": regions,
                "sha256": g.hashes(),
                "warnings": warnings,
            });
            (summary, Some(g))
        }
    }
}

/// Plan a ScenePlan: the StrokeList bytes and the plan report (or `{errors}`).
pub fn plan(text: &str, width: u32, seed: u32, mixer: u32, strict: bool, fields: &BTreeMap<String, FieldData>) -> (Value, Option<Vec<u8>>) {
    let sp = match oil_scene::parse(text) {
        Ok(p) => p,
        Err(errors) => return (json!({ "errors": errors }), None),
    };
    let opts = oil_plan::PlanOptions { seed, plan_width: width, strict_engine: strict };
    let clock = || 0.0;
    let res = match mixer {
        0 => oil_plan::plan(&OchrellMixer, &sp, &opts, fields, &clock),
        1 => oil_plan::plan(&RgbMixer, &sp, &opts, fields, &clock),
        _ => {
            let e = oil_scene::Error::new("UNKNOWN_MIXER", format!("mixer id {mixer} is not in this build")).got(mixer).expected("0 (ochrell) or 1 (rgb)");
            return (json!({ "errors": [e] }), None);
        }
    };
    match res {
        Err(errors) => (json!({ "errors": errors }), None),
        Ok((list, report)) => {
            let bytes = list.to_bytes();
            let mut v = serde_json::to_value(&report).unwrap_or_default();
            v["bytes"] = json!(bytes.len());
            (v, Some(bytes))
        }
    }
}

/// Plane or preview bytes. Planes: 0 target (f32 RGB), 1 region ids (u8), 2 soft masks (f32, region-major),
/// 3 flow (f32 xy), 4 light (f32). Previews (u32 LE width, u32 LE height, then RGB8): 10 target, 11 regions,
/// 12 flow, 13 light, 14 the guide sheet. Region flows: 100 + region index (f32 xy; empty if none).
pub fn plane(g: &Guides, which: u32) -> Option<Vec<u8>> {
    let f32s = |v: &mut dyn Iterator<Item = f32>| -> Vec<u8> { v.flat_map(|x| x.to_le_bytes()).collect() };
    let img = |p: preview::Rgb8| -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + p.data.len());
        out.extend_from_slice(&(p.w as u32).to_le_bytes());
        out.extend_from_slice(&(p.h as u32).to_le_bytes());
        out.extend_from_slice(&p.data);
        out
    };
    Some(match which {
        0 => f32s(&mut g.target.iter().flatten().copied()),
        1 => g.region_id.clone(),
        2 => f32s(&mut g.masks.iter().flat_map(|m| m.to_full(g.w, g.h))),
        3 => f32s(&mut g.flow_raster().into_iter().flatten()),
        4 => f32s(&mut g.light.iter().copied()),
        10 => img(preview::target(g)),
        11 => img(preview::regions(g)),
        12 => img(preview::flow(g)),
        13 => img(preview::light(g)),
        14 => img(preview::sheet(g)),
        n if n >= 100 => {
            let r = (n - 100) as usize;
            if r >= g.flows.len() {
                return None;
            }
            match g.region_flow_raster(r) {
                Some(rf) => f32s(&mut rf.into_iter().flatten()),
                None => Vec::new(),
            }
        }
        _ => return None,
    })
}

#[cfg(target_arch = "wasm32")]
#[allow(unsafe_code)] // #[no_mangle] exports
mod wasm_abi {
    use super::*;
    use std::sync::Mutex;

    static BUF: Mutex<Vec<u8>> = Mutex::new(Vec::new());
    static INPUT: Mutex<Vec<u8>> = Mutex::new(Vec::new());
    static GUIDES: Mutex<Option<Guides>> = Mutex::new(None);
    static STROKES: Mutex<Vec<u8>> = Mutex::new(Vec::new());
    static FIELDS: Mutex<BTreeMap<String, FieldData>> = Mutex::new(BTreeMap::new());

    fn put(bytes: Vec<u8>) -> u32 {
        let len = bytes.len() as u32;
        *BUF.lock().unwrap() = bytes;
        len
    }

    fn put_json(v: &Value) -> u32 {
        put(v.to_string().into_bytes())
    }

    fn input_text() -> String {
        String::from_utf8_lossy(&std::mem::take(&mut *INPUT.lock().unwrap())).into_owned()
    }

    #[no_mangle]
    pub extern "C" fn oil_buf_ptr() -> u32 {
        BUF.lock().unwrap().as_ptr() as usize as u32
    }

    /// Make room for `len` input bytes and return their address.
    #[no_mangle]
    pub extern "C" fn oil_input(len: u32) -> u32 {
        let mut input = INPUT.lock().unwrap();
        input.clear();
        input.resize(len as usize, 0);
        input.as_ptr() as usize as u32
    }

    #[no_mangle]
    pub extern "C" fn oil_engine_version() -> u32 {
        put(oil_kernel::ENGINE_VERSION.as_bytes().to_vec())
    }

    #[no_mangle]
    pub extern "C" fn oil_scene_schema() -> u32 {
        put_json(&oil_scene::schema())
    }

    /// Validate the ScenePlan JSON in the input buffer.
    #[no_mangle]
    pub extern "C" fn oil_scene_validate() -> u32 {
        put_json(&validate(&input_text()))
    }

    /// Add a sampled field: the input holds the name's byte length (u32 LE), the name, then f32 LE values.
    #[no_mangle]
    pub extern "C" fn oil_field_add() -> u32 {
        let input = std::mem::take(&mut *INPUT.lock().unwrap());
        if input.len() < 4 {
            return 0;
        }
        let n = u32::from_le_bytes([input[0], input[1], input[2], input[3]]) as usize;
        let Some(name) = input.get(4..4 + n) else { return 0 };
        let data = input[4 + n..].chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        FIELDS.lock().unwrap().insert(String::from_utf8_lossy(name).into_owned(), FieldData { data });
        1
    }

    #[no_mangle]
    pub extern "C" fn oil_fields_clear() {
        FIELDS.lock().unwrap().clear();
    }

    /// Compile the ScenePlan in the input buffer at `width` with mixer id `mixer`; keeps the guides for
    /// `oil_guides_plane` and returns the summary JSON.
    /// Plan the ScenePlan in the input buffer; keeps the StrokeList for `oil_plan_strokes` and returns the report
    /// JSON (`strict` 1 refuses a plan authored for another engine version).
    #[no_mangle]
    pub extern "C" fn oil_plan(width: u32, seed: u32, mixer: u32, strict: u32) -> u32 {
        let (report, bytes) = plan(&input_text(), width, seed, mixer, strict != 0, &FIELDS.lock().unwrap());
        *STROKES.lock().unwrap() = bytes.unwrap_or_default();
        put_json(&report)
    }

    /// The StrokeList bytes of the last plan (0 if it failed).
    #[no_mangle]
    pub extern "C" fn oil_plan_strokes() -> u32 {
        put(std::mem::take(&mut *STROKES.lock().unwrap()))
    }

    #[no_mangle]
    pub extern "C" fn oil_guides(width: u32, mixer: u32) -> u32 {
        let (summary, g) = guides(&input_text(), width, mixer, &FIELDS.lock().unwrap());
        *GUIDES.lock().unwrap() = g;
        put_json(&summary)
    }

    /// Bytes of a plane or preview of the last guides (see `plane`); 0 if there is none.
    #[no_mangle]
    pub extern "C" fn oil_guides_plane(which: u32) -> u32 {
        let g = GUIDES.lock().unwrap();
        match g.as_ref().and_then(|g| plane(g, which)) {
            Some(b) => put(b),
            None => 0,
        }
    }

    #[no_mangle]
    pub extern "C" fn oil_guides_free() {
        *GUIDES.lock().unwrap() = None;
        *BUF.lock().unwrap() = Vec::new();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn validate_and_compile_a_small_plan() {
        let text = r##"{"sceneplan": 1, "canvas": {"aspect": [4, 5], "ground": "#e9e1d6"},
            "target": [{"fill": {"gradientV": [[0, "#232a58"], [1.25, "#e8cbb8"]]}}],
            "regions": [{"name": "all", "shape": {"all": true}, "flow": {"constant": {"angle": 10}}}],
            "styles": {}, "layers": []}"##;
        assert_eq!(super::validate(text)["valid"], true);
        let (summary, g) = super::guides(text, 64, 1, &Default::default());
        assert_eq!(summary["size"], serde_json::json!([64, 80]));
        let g = g.unwrap();
        assert_eq!(super::plane(&g, 0).unwrap().len(), 64 * 80 * 12);
        assert_eq!(super::plane(&g, 100).unwrap().len(), 64 * 80 * 8);
        let bad = super::validate(r#"{"sceneplan": 1}"#);
        assert_eq!(bad["errors"][0]["code"], "SCHEMA");
    }
}
