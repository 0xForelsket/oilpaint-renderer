//! `oil`: the native CLI of the new engine.
//!
//!   oil version                                   engine version and mixers in this build (JSON)
//!   oil info FILE.oilstrokes                      header, counts and validation (JSON)
//!   oil testsheet --out FILE.oilstrokes           write the procedural test sheet
//!   oil paint FILE.oilstrokes --width W [--mixer ID] [--light default|painting|none] [--out DIR]
//!                                                 paint; writes unlit.png, lit.png, height.png, report.json
//!
//! Every command prints one JSON document on stdout. Errors are JSON too ({"error": {...}}, spec/ERRORS.md) with
//! exit code 1; usage errors exit with 2.
use oil_light::LightParams;
use oil_mix::{Mixer, RgbMixer};
use oil_strokes::{Error, StrokeList};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn mixers() -> Vec<&'static str> {
    #[cfg_attr(not(feature = "mixbox"), allow(unused_mut))]
    let mut m = vec![RgbMixer::ID];
    #[cfg(feature = "mixbox")]
    m.push(oil_mix_mixbox::MixboxMixer::ID);
    m
}

fn fail(e: Error) -> ! {
    println!("{}", json!({ "error": e }));
    std::process::exit(1);
}

fn usage(msg: &str) -> ! {
    eprintln!("{msg}\nusage: oil version | info FILE | testsheet --out FILE | paint FILE --width W [--mixer ID] [--light default|painting|none] [--out DIR]");
    std::process::exit(2);
}

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn read_list(path: &str) -> StrokeList {
    let bytes = std::fs::read(path).unwrap_or_else(|e| fail(Error::new("IO", format!("cannot read {path}: {e}"))));
    StrokeList::from_bytes(&bytes).unwrap_or_else(|errs| {
        let first = errs[0].clone();
        println!("{}", json!({ "error": first, "errors": errs }));
        std::process::exit(1)
    })
}

fn to8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

fn write_png(path: &Path, w: usize, h: usize, rgb: &[u8], channels: png::ColorType) {
    let f = std::fs::File::create(path).unwrap_or_else(|e| fail(Error::new("IO", format!("cannot write {}: {e}", path.display()))));
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w as u32, h as u32);
    enc.set_color(channels);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().and_then(|mut wr| wr.write_image_data(rgb)).unwrap_or_else(|e| fail(Error::new("IO", format!("PNG: {e}"))));
}

fn paint_with<M: Mixer>(m: &M, list: &StrokeList, w: u32, light: Option<LightParams>, out: Option<&Path>) -> Value {
    let t0 = Instant::now();
    let (cv, stats) = oil_paint::paint(m, list, w, |_, _| {});
    let paint_s = t0.elapsed().as_secs_f64();
    let (wu, hu) = (cv.w, cv.h);
    let mut hashes = serde_json::Map::new();
    for plane in oil_paint::PLANES {
        hashes.insert(plane.into(), json!(hex(&oil_paint::plane_bytes(&cv, plane))));
    }
    let t1 = Instant::now();
    let lit = light.map(|p| oil_light::relight(&cv.rgb, &cv.hgt, wu, hu, &p));
    let light_s = t1.elapsed().as_secs_f64();
    if let Some(lit) = &lit {
        let bytes: Vec<u8> = lit.iter().flatten().flat_map(|v| v.to_bits().to_le_bytes()).collect();
        hashes.insert("lit".into(), json!(hex(&bytes)));
    }
    let mut files = Vec::new();
    if let Some(dir) = out {
        std::fs::create_dir_all(dir).unwrap_or_else(|e| fail(Error::new("IO", format!("cannot create {}: {e}", dir.display()))));
        let unlit: Vec<u8> = cv.rgb.iter().flatten().map(|v| to8(*v)).collect();
        write_png(&dir.join("unlit.png"), wu, hu, &unlit, png::ColorType::Rgb);
        files.push("unlit.png");
        if let Some(lit) = &lit {
            let bytes: Vec<u8> = lit.iter().flatten().map(|v| to8(*v)).collect();
            write_png(&dir.join("lit.png"), wu, hu, &bytes, png::ColorType::Rgb);
            files.push("lit.png");
        }
        let hmax = cv.hgt.iter().fold(1e-6f32, |a, b| a.max(*b));
        let height: Vec<u8> = cv.hgt.iter().map(|v| to8(v / hmax)).collect();
        write_png(&dir.join("height.png"), wu, hu, &height, png::ColorType::Grayscale);
        files.push("height.png");
    }
    json!({
        "engine": oil_kernel::ENGINE_VERSION,
        "mixer": M::ID,
        "size": [wu, hu],
        "strokes": stats.strokes,
        "layers": list.layers.len(),
        "paintedPixels": stats.painted_pixels,
        "bytesPerPixel": oil_kernel::Canvas::<M>::bytes_per_pixel(),
        "timings": { "paintSeconds": paint_s, "lightSeconds": light_s },
        "sha256": hashes,
        "files": files,
    })
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str);
    match args.first().map(String::as_str) {
        Some("version") => println!("{}", json!({ "engine": oil_kernel::ENGINE_VERSION, "mixers": mixers() })),
        Some("info") => {
            let path = args.get(1).unwrap_or_else(|| usage("info needs a file"));
            let bytes = std::fs::read(path).unwrap_or_else(|e| fail(Error::new("IO", format!("cannot read {path}: {e}"))));
            let version = oil_strokes::read_version(&bytes).unwrap_or_else(|e| fail(e));
            match StrokeList::from_bytes_with_version(&bytes, &version) {
                Ok(l) => println!(
                    "{}",
                    json!({ "engineVersion": version, "paintable": version == oil_kernel::ENGINE_VERSION, "strokes": l.strokes.len(),
                            "points": l.points.len(), "layers": l.meta.layers, "regions": l.meta.regions, "aspect": l.aspect,
                            "meta": serde_json::to_value(&l.meta).unwrap(), "sha256": hex(&bytes) })
                ),
                Err(errs) => println!("{}", json!({ "engineVersion": version, "errors": errs })),
            }
        }
        Some("testsheet") => {
            let out = opt("--out").unwrap_or_else(|| usage("testsheet needs --out FILE"));
            let bytes = oil_paint::testsheet::testsheet().to_bytes();
            if let Some(dir) = Path::new(out).parent().filter(|d| !d.as_os_str().is_empty()) {
                std::fs::create_dir_all(dir).unwrap_or_else(|e| fail(Error::new("IO", format!("cannot create {}: {e}", dir.display()))));
            }
            std::fs::write(out, &bytes).unwrap_or_else(|e| fail(Error::new("IO", format!("cannot write {out}: {e}"))));
            println!("{}", json!({ "file": out, "bytes": bytes.len(), "sha256": hex(&bytes), "engine": oil_kernel::ENGINE_VERSION }));
        }
        Some("paint") => {
            let path = args.get(1).unwrap_or_else(|| usage("paint needs a file"));
            let w: u32 = opt("--width").and_then(|v| v.parse().ok()).filter(|w| (16..=20_000).contains(w)).unwrap_or_else(|| usage("paint needs --width 16..20000"));
            let light = match opt("--light").unwrap_or("default") {
                "default" => Some(LightParams::default()),
                "painting" => Some(LightParams::painting()),
                "none" => None,
                other => usage(&format!("unknown light preset {other}")),
            };
            let out = opt("--out").map(PathBuf::from);
            let list = read_list(path);
            let mixer = opt("--mixer").unwrap_or("rgb");
            let report = match mixer {
                "rgb" => paint_with(&RgbMixer, &list, w, light, out.as_deref()),
                #[cfg(feature = "mixbox")]
                "mixbox" | "mixbox-2.0" => paint_with(&oil_mix_mixbox::MixboxMixer, &list, w, light, out.as_deref()),
                other => fail(Error::new("UNKNOWN_MIXER", format!("mixer {other} is not in this build")).got(other).expected(mixers().join(", "))),
            };
            if let Some(dir) = &out {
                std::fs::write(dir.join("report.json"), serde_json::to_vec_pretty(&report).unwrap()).unwrap_or_else(|e| fail(Error::new("IO", e.to_string())));
            }
            println!("{report}");
        }
        _ => usage("unknown command"),
    }
}
