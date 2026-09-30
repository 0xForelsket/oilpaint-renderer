//! `oil scene validate|schema` and `oil guides`.
use crate::{fail, usage, write_png};
use oil_mix::{Mixer, OchrellMixer, RgbMixer};
use oil_scene::{preview, Error, FieldData, ScenePlan};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

fn read_text(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| fail(Error::new("IO", format!("cannot read {path}: {e}"))))
}

fn print_errors(errs: &[Error], warnings: &[Error]) -> ! {
    println!("{}", json!({ "valid": false, "error": errs[0], "errors": errs, "warnings": warnings }));
    std::process::exit(1)
}

/// `oil scene validate FILE` and `oil scene schema`.
pub fn scene(args: &[String]) {
    match args.first().map(String::as_str) {
        Some("validate") => {
            let path = args.get(1).unwrap_or_else(|| usage("scene validate needs a file"));
            let plan = oil_scene::parse(&read_text(path)).unwrap_or_else(|e| print_errors(&e, &[]));
            let r = oil_scene::validate(&plan);
            if !r.errors.is_empty() {
                print_errors(&r.errors, &r.warnings);
            }
            println!(
                "{}",
                json!({ "valid": true, "errors": [], "warnings": r.warnings, "title": plan.title, "regions": plan.regions.len(),
                        "layers": plan.layers.len(), "targetOps": plan.target.len(), "engine": oil_kernel::ENGINE_VERSION })
            );
        }
        Some("schema") => println!("{}", serde_json::to_string_pretty(&oil_scene::schema()).unwrap()),
        _ => usage("scene validate FILE | scene schema"),
    }
}

/// A NumPy .npy (v1.0) file of little-endian f32 or u8 values.
fn write_npy(path: &Path, shape: &[usize], descr: &str, bytes: &[u8]) {
    let dims: Vec<String> = shape.iter().map(|d| d.to_string()).collect();
    let tuple = if dims.len() == 1 { format!("({},)", dims[0]) } else { format!("({})", dims.join(", ")) };
    let mut header = format!("{{'descr': '{descr}', 'fortran_order': False, 'shape': {tuple}, }}");
    while (10 + header.len() + 1) % 64 != 0 {
        header.push(' ');
    }
    header.push('\n');
    let mut out = b"\x93NUMPY\x01\x00".to_vec();
    out.extend_from_slice(&(header.len() as u16).to_le_bytes());
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(bytes);
    std::fs::write(path, out).unwrap_or_else(|e| fail(Error::new("IO", format!("cannot write {}: {e}", path.display()))));
}

fn f32_bytes(v: impl Iterator<Item = f32>) -> Vec<u8> {
    v.flat_map(|x| x.to_le_bytes()).collect()
}

fn guides_with<M: Mixer>(m: &M, plan: &ScenePlan, width: u32, fields: &BTreeMap<String, FieldData>, out: Option<&Path>, npy: bool) -> Value {
    let start = Instant::now();
    let clock = || start.elapsed().as_secs_f64() * 1000.0;
    let (g, timings) = oil_scene::compile(plan, width, m, fields, &clock).unwrap_or_else(|errs| print_errors(&errs, &[]));
    let n = g.w * g.h;
    let mut counts = vec![0usize; g.names.len()];
    for id in &g.region_id {
        counts[*id as usize] += 1;
    }
    let regions: Vec<Value> = g
        .names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            json!({ "id": i, "name": name, "pixels": counts[i], "share": counts[i] as f64 / n as f64,
                    "softArea": g.masks[i].data.iter().map(|v| *v as f64 / 65535.0).sum::<f64>() / n as f64, "flow": g.flows[i].is_some() })
        })
        .collect();
    let light_mean = g.light.iter().map(|v| *v as f64).sum::<f64>() / n as f64;
    let mut files = Vec::new();
    if let Some(dir) = out {
        std::fs::create_dir_all(dir).unwrap_or_else(|e| fail(Error::new("IO", format!("cannot create {}: {e}", dir.display()))));
        let t0 = Instant::now();
        for (name, img) in [
            ("guide_target.png", preview::target(&g)),
            ("guide_regions.png", preview::regions(&g)),
            ("guide_flow.png", preview::flow(&g)),
            ("guide_light.png", preview::light(&g)),
            ("guides_sheet.png", preview::sheet(&g)),
        ] {
            write_png(&dir.join(name), img.w, img.h, &img.data, png::ColorType::Rgb);
            files.push(name.to_string());
        }
        if npy {
            let (w, h) = (g.w, g.h);
            write_npy(&dir.join("target.npy"), &[h, w, 3], "<f4", &f32_bytes(g.target.iter().flatten().copied()));
            write_npy(&dir.join("region_id.npy"), &[h, w], "|u1", &g.region_id);
            write_npy(&dir.join("masks.npy"), &[g.masks.len(), h, w], "<f4", &f32_bytes(g.masks.iter().flat_map(|m| m.to_full(w, h))));
            write_npy(&dir.join("flow.npy"), &[h, w, 2], "<f4", &f32_bytes(g.flow_raster().into_iter().flatten()));
            write_npy(&dir.join("light.npy"), &[h, w], "<f4", &f32_bytes(g.light.iter().copied()));
            for i in 0..g.names.len() {
                if let Some(rf) = g.region_flow_raster(i) {
                    let name = format!("flow_{}.npy", g.names[i]);
                    write_npy(&dir.join(&name), &[h, w, 2], "<f4", &f32_bytes(rf.iter().flatten().copied()));
                }
            }
            files.extend(["target.npy", "region_id.npy", "masks.npy", "flow.npy", "light.npy"].map(String::from));
        }
        eprintln!("previews written in {:.0} ms", t0.elapsed().as_secs_f64() * 1000.0);
    }
    json!({
        "engine": oil_kernel::ENGINE_VERSION,
        "mixer": M::ID,
        "title": plan.title,
        "size": [g.w, g.h],
        "regions": regions,
        "light": { "mean": light_mean, "max": g.light.iter().fold(0f32, |a, b| a.max(*b)) },
        "timings": timings,
        "sha256": g.hashes(),
        "files": files,
    })
}

/// `oil guides FILE.json --width W [--mixer ID] [--field NAME=FILE.f32 ...] [--out DIR] [--npy]`
pub fn guides(args: &[String]) {
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str);
    let path = args.first().filter(|a| !a.starts_with("--")).unwrap_or_else(|| usage("guides needs a ScenePlan file"));
    let width: u32 = opt("--width").map(|v| v.parse().unwrap_or_else(|_| usage("--width is an integer"))).unwrap_or(600);
    let (plan, warnings) = oil_scene::load(&read_text(path)).unwrap_or_else(|e| print_errors(&e, &[]));
    let mut fields = BTreeMap::new();
    for (i, a) in args.iter().enumerate() {
        if a == "--field" {
            let spec = args.get(i + 1).unwrap_or_else(|| usage("--field NAME=FILE.f32"));
            let (name, file) = spec.split_once('=').unwrap_or_else(|| usage("--field NAME=FILE.f32"));
            let bytes = std::fs::read(file).unwrap_or_else(|e| fail(Error::new("IO", format!("cannot read {file}: {e}"))));
            let data = bytes.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
            fields.insert(name.to_string(), FieldData { data });
        }
    }
    let out = opt("--out").map(Path::new);
    let npy = args.iter().any(|a| a == "--npy");
    let mut report = match opt("--mixer").unwrap_or("ochrell") {
        "ochrell" | "ochrell-0.2" => guides_with(&OchrellMixer, &plan, width, &fields, out, npy),
        "rgb" => guides_with(&RgbMixer, &plan, width, &fields, out, npy),
        #[cfg(feature = "mixbox")]
        "mixbox" | "mixbox-2.0" => guides_with(&oil_mix_mixbox::MixboxMixer, &plan, width, &fields, out, npy),
        other => fail(Error::new("UNKNOWN_MIXER", format!("mixer {other} is not in this build")).got(other).expected(crate::mixers().join(", "))),
    };
    report["warnings"] = json!(warnings);
    if let Some(dir) = out {
        std::fs::write(dir.join("guides.json"), serde_json::to_vec_pretty(&report).unwrap()).unwrap_or_else(|e| fail(Error::new("IO", e.to_string())));
    }
    println!("{report}");
}
