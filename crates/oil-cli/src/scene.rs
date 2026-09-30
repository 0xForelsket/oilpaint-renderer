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
pub(crate) fn write_npy(path: &Path, shape: &[usize], descr: &str, bytes: &[u8]) {
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

pub(crate) fn f32_bytes(v: impl Iterator<Item = f32>) -> Vec<u8> {
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

/// A PNG as an rgb field: sRGB in [0, 1], 3 floats per pixel (alpha dropped, 16-bit reduced to 8).
pub fn png_rgb(file: &str) -> (u32, u32, Vec<f32>) {
    let f = std::fs::File::open(file).unwrap_or_else(|e| fail(Error::new("IO", format!("cannot read {file}: {e}"))));
    let mut dec = png::Decoder::new(std::io::BufReader::new(f));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec.read_info().unwrap_or_else(|e| fail(Error::new("IO", format!("{file}: {e}"))));
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap_or_else(|e| fail(Error::new("IO", format!("{file}: {e}"))));
    let ch = info.color_type.samples();
    let n = (info.width * info.height) as usize;
    let mut data = Vec::with_capacity(n * 3);
    for k in 0..n {
        let px = &buf[k * ch..k * ch + ch];
        let (r, g, b) = if ch >= 3 { (px[0], px[1], px[2]) } else { (px[0], px[0], px[0]) };
        data.extend([r, g, b].map(|v| (v as f64 / 255.0) as f32));
    }
    (info.width, info.height, data)
}

/// `--field NAME=FILE.f32` (raw little-endian f32) and `--image NAME=FILE.png` (an rgb field).
fn load_fields(args: &[String]) -> BTreeMap<String, FieldData> {
    let mut fields = BTreeMap::new();
    for (i, a) in args.iter().enumerate() {
        if a == "--field" || a == "--image" {
            let spec = args.get(i + 1).unwrap_or_else(|| usage("--field NAME=FILE.f32 | --image NAME=FILE.png"));
            let (name, file) = spec.split_once('=').unwrap_or_else(|| usage("--field NAME=FILE.f32 | --image NAME=FILE.png"));
            let data = if a == "--image" {
                png_rgb(file).2
            } else {
                let bytes = std::fs::read(file).unwrap_or_else(|e| fail(Error::new("IO", format!("cannot read {file}: {e}"))));
                bytes.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
            };
            fields.insert(name.to_string(), FieldData { data });
        }
    }
    fields
}

/// `oil field FILE.png`: the ScenePlan declaration of a picture as an rgb field (kind, size, sha256).
pub fn field_decl(args: &[String]) {
    let file = args.first().unwrap_or_else(|| usage("field needs a PNG file"));
    let (w, h, data) = png_rgb(file);
    let sha = oil_scene::compile::sha256_f32(data.iter().copied());
    println!("{}", json!({ "kind": "rgb", "width": w, "height": h, "sha256": sha }));
}

fn run_plan<M: Mixer>(m: &M, sp: &ScenePlan, o: &oil_plan::PlanOptions, f: &BTreeMap<String, FieldData>, c: &dyn Fn() -> f64) -> (oil_strokes::StrokeList, Value) {
    let (list, rep) = oil_plan::plan(m, sp, o, f, c).unwrap_or_else(|errs| print_errors(&errs, &[]));
    (list, serde_json::to_value(&rep).unwrap())
}

/// `oil plan FILE.json --out FILE.oilstrokes [--width 600] [--seed 1907] [--mixer ID] [--strict-engine]
/// [--field|--image NAME=FILE ...] [--report FILE.json]`
pub fn plan(args: &[String]) {
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str);
    let path = args.first().filter(|a| !a.starts_with("--")).unwrap_or_else(|| usage("plan needs a ScenePlan file"));
    let num = |name: &str, d: u32| opt(name).map(|v| v.parse().unwrap_or_else(|_| usage(&format!("{name} is an integer")))).unwrap_or(d);
    let opts = oil_plan::PlanOptions { seed: num("--seed", 1907), plan_width: num("--width", 600), strict_engine: args.iter().any(|a| a == "--strict-engine") };
    if !(16..=20_000).contains(&opts.plan_width) {
        usage("--width is 16..20000");
    }
    let sp = oil_scene::parse(&read_text(path)).unwrap_or_else(|e| print_errors(&e, &[]));
    let fields = load_fields(args);
    let out = opt("--out").unwrap_or_else(|| usage("plan needs --out FILE.oilstrokes"));
    let start = Instant::now();
    let clock = || start.elapsed().as_secs_f64() * 1000.0;
    let (list, mut report) = match opt("--mixer").unwrap_or("ochrell") {
        "ochrell" | "ochrell-0.2" => run_plan(&OchrellMixer, &sp, &opts, &fields, &clock),
        "rgb" => run_plan(&RgbMixer, &sp, &opts, &fields, &clock),
        #[cfg(feature = "mixbox")]
        "mixbox" | "mixbox-2.0" => run_plan(&oil_mix_mixbox::MixboxMixer, &sp, &opts, &fields, &clock),
        other => fail(Error::new("UNKNOWN_MIXER", format!("mixer {other} is not in this build")).got(other).expected(crate::mixers().join(", "))),
    };
    let bytes = list.to_bytes();
    if let Some(dir) = Path::new(out).parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).unwrap_or_else(|e| fail(Error::new("IO", format!("cannot create {}: {e}", dir.display()))));
    }
    std::fs::write(out, &bytes).unwrap_or_else(|e| fail(Error::new("IO", format!("cannot write {out}: {e}"))));
    report["file"] = json!(out);
    report["bytes"] = json!(bytes.len());
    report["sha256"] = json!(crate::hex(&bytes));
    if let Some(rp) = opt("--report") {
        std::fs::write(rp, serde_json::to_vec_pretty(&report).unwrap()).unwrap_or_else(|e| fail(Error::new("IO", e.to_string())));
    }
    println!("{report}");
}

/// `oil guides FILE.json --width W [--mixer ID] [--field NAME=FILE.f32 ...] [--out DIR] [--npy]`
pub fn guides(args: &[String]) {
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str);
    let path = args.first().filter(|a| !a.starts_with("--")).unwrap_or_else(|| usage("guides needs a ScenePlan file"));
    let width: u32 = opt("--width").map(|v| v.parse().unwrap_or_else(|_| usage("--width is an integer"))).unwrap_or(600);
    let (plan, warnings) = oil_scene::load(&read_text(path)).unwrap_or_else(|e| print_errors(&e, &[]));
    let fields = load_fields(args);
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
