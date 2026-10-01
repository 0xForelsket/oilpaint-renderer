//! Compare two local eight-paint OPP3 packages without changing the renderer default.
//! Arguments: reference.opp candidate.opp output-directory
use oil_kernel::{BrushParams, Canvas};
use oil_mix::Mixer;
use oil_palette::{PaletteJobN, PaletteMixerN, RecipeLoadN};
use oil_strokes::{Layer, Meta, Stroke, StrokeList, NO_REGION};
use sha2::{Digest, Sha256};
use std::{error::Error, fmt::Write, hint::black_box, path::Path, time::Instant};

type Job = PaletteJobN<8, false, 31>;
type PaletteMixer = PaletteMixerN<8, false, 31>;
type Result<T> = std::result::Result<T, Box<dyn Error>>;
const LABELS: [&str; 2] = ["previous", "balanced"];

fn recipes() -> Vec<(String, [f64; 8])> {
    let mut rows = Vec::new();
    for i in 0..8 {
        let mut x = [0.; 8];
        x[i] = 1.;
        rows.push((format!("pure-{}", i + 1), x));
    }
    let mixtures = [
        ("lemon-cobalt", [1., 0., 0., 0., 1., 0., 0., 0.]),
        ("cadmium-viridian", [0., 1., 0., 0., 0., 0., 1., 0.]),
        ("scarlet-ultramarine", [0., 0., 1., 0., 0., 1., 0., 0.]),
        ("alizarine-viridian", [0., 0., 0., 1., 0., 0., 1., 0.]),
        ("dark-red-blue", [0., 0., 1., 4., 0., 5., 0., 0.]),
        ("dark-red-green", [0., 0., 1., 1., 0., 0., 2., 0.]),
        ("chromatic-neutral", [1., 1., 1., 1., 1., 1., 1., 0.]),
        ("blue-green", [0., 0., 0., 0., 1., 1., 1., 0.]),
    ];
    rows.extend(mixtures.map(|(name, x)| (name.into(), x)));
    for i in 0..7 {
        let mut x = [0.; 8];
        x[i] = 1.;
        x[7] = 4.;
        rows.push((format!("white-tint-{}", i + 1), x));
    }
    rows.push(("neutral-tint".into(), [0., 0., 1., 4., 0., 5., 0., 10.]));
    rows.push(("equal-eight".into(), [1.; 8]));
    for i in 0..7 {
        let mut x = [0.05; 8];
        x[i] = 0.65;
        rows.push((format!("all-eight-led-by-{}", i + 1), x));
    }
    assert_eq!(rows.len(), 32);
    rows
}

fn targets() -> Vec<[f32; 3]> {
    [
        0x000000u32,
        0x101010,
        0x202020,
        0x404040,
        0x606060,
        0x808080,
        0xc0c0c0,
        0xffffff,
        0xff0000,
        0x00ff00,
        0x0000ff,
        0xffff00,
        0x00ffff,
        0xff00ff,
        0xff8000,
        0x8000ff,
        0xf2d7ad,
        0xeeb7b7,
        0xbccbe8,
        0xcadbac,
        0xd4c2de,
        0xa8d7cc,
        0xe8dfbd,
        0xf1e9e0,
        0x28233b,
        0x294035,
        0x623b35,
        0x3e5e8a,
        0x948354,
        0x78915b,
        0xb4554a,
        0x456c77,
    ]
    .map(|x| [(x >> 16) & 255, (x >> 8) & 255, x & 255].map(|v| v as f32 / 255.))
    .to_vec()
}

fn sheet(colors: &[[f32; 3]]) -> StrokeList {
    let mut list = StrokeList {
        meta: Meta {
            generator: "Old Holland package comparison".into(),
            layers: vec!["swatches".into()],
            ..Meta::default()
        },
        aspect: [4, 5],
        ground: [1.; 3],
        layers: vec![Layer {
            start: 0,
            end: 0,
            hblur_sigma: None,
            dry_after: None,
        }],
        offsets: vec![0],
        points: vec![],
        strokes: vec![],
    };
    for (i, &color) in colors.iter().enumerate() {
        for line in 0..3 {
            let x = 0.035 + (i % 4) as f32 * 0.25;
            let y = 0.045 + (i / 4) as f32 * 0.153 + line as f32 * 0.025;
            for k in 0..16 {
                list.points.push([x + k as f32 / 15. * 0.18, y, 0.031, 1.]);
            }
            list.offsets.push(list.points.len() as u32);
            list.strokes.push(Stroke {
                layer: 0,
                region: NO_REGION,
                color,
                color2: color,
                streak_amount: 0.,
                brush: BrushParams {
                    seed: 1701 + i as u32 * 11 + line,
                    nb: 16,
                    pickup: 0.,
                    opacity: 1.,
                    ..BrushParams::default()
                },
            });
        }
    }
    list.layers[0].end = list.strokes.len() as u32;
    assert!(list.validate().is_empty(), "{:?}", list.validate());
    list
}

fn png(path: &Path, canvas: &Canvas<PaletteMixer>) -> Result<()> {
    let mut encoder = png::Encoder::new(
        std::fs::File::create(path)?,
        canvas.w as u32,
        canvas.h as u32,
    );
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(
        &canvas
            .rgb
            .iter()
            .flatten()
            .map(|v| (v * 255. + 0.5).clamp(0., 255.) as u8)
            .collect::<Vec<_>>(),
    )?;
    Ok(())
}

fn verify_job(
    job: &Job,
    dir: &Path,
    name: &str,
    evidence: &mut String,
) -> Result<Canvas<PaletteMixer>> {
    let bytes = job.to_bytes()?;
    let file = dir.join(format!("{name}.opj"));
    std::fs::write(&file, &bytes)?;
    let restored = Job::from_bytes(&std::fs::read(file)?)?;
    assert_eq!(bytes, restored.to_bytes()?);
    let (canvas, stats) = job.paint(384, |_, _| {})?;
    let replay = restored.paint(384, |_, _| {})?.0;
    for plane in oil_paint::PLANES {
        let data = oil_paint::plane_bytes(&canvas, plane);
        assert_eq!(data, oil_paint::plane_bytes(&replay, plane));
        writeln!(evidence, "{name},{plane},{:x}", Sha256::digest(&data))?;
    }
    assert!(canvas.lat.iter().all(|z| job.mixer().is_valid(z)));
    assert!(canvas
        .rgb
        .iter()
        .flatten()
        .all(|v| v.is_finite() && (0. ..=1.).contains(v)));
    // Restoring a job retains the model used by subsequent material mixing.
    for load in job.loads() {
        for paint in 0..8 {
            let x =
                std::array::from_fn(|i| load.main[i] * 0.87 + if i == paint { 0.13 } else { 0. });
            assert_eq!(
                job.mixer().decode_srgb(&x),
                restored.mixer().decode_srgb(&x)
            );
        }
    }
    png(&dir.join(format!("{name}.png")), &canvas)?;
    println!(
        "{name}: {}x{}, {} strokes; package, all planes and future mixing replay exactly",
        canvas.w, canvas.h, stats.strokes
    );
    Ok(canvas)
}

fn prepare(bytes: &[u8], label: &str, timings: &mut String) -> Result<PaletteMixer> {
    let start = Instant::now();
    let mixer = PaletteMixer::from_palette_bytes(bytes)?;
    writeln!(
        timings,
        "{label},prepare,0,{},1",
        start.elapsed().as_secs_f64() * 1e3
    )?;
    Ok(mixer)
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("Expected previous.opp balanced.opp output-directory".into());
    }
    let out = Path::new(&args[2]);
    std::fs::create_dir_all(out)?;
    let bytes = [std::fs::read(&args[0])?, std::fs::read(&args[1])?];
    let mut timings = String::from("model,operation,repetition,milliseconds,operations\n");
    let mut matches = String::from("model,target_id,r,g,b,achieved_r,achieved_g,achieved_b,error_ok100,evaluations,milliseconds,recipe\n");
    let mut swatches = String::from("model,recipe_id,name,r,g,b,recipe\n");
    let mut plane_hashes = String::from("job,plane,sha256\n");
    let fixed = recipes();
    let targets = targets();
    let mut jobs = Vec::new();
    let mut material_reference: Option<Vec<Vec<u8>>> = None;
    for (index, &label) in LABELS.iter().enumerate() {
        let dir = out.join(label);
        std::fs::create_dir_all(&dir)?;
        let m = prepare(&bytes[index], label, &mut timings)?;
        let amounts: Vec<_> = fixed
            .iter()
            .map(|(_, x)| m.recipe(*x))
            .collect::<std::result::Result<_, _>>()?;
        for (i, ((name, _), recipe)) in fixed.iter().zip(&amounts).enumerate() {
            let color = m.decode_srgb(recipe);
            let proportion = recipe
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(";");
            writeln!(
                swatches,
                "{label},{i},{name},{},{},{},{proportion}",
                color[0], color[1], color[2]
            )?;
        }
        // Authored RGB is deliberately fixed; only supplied material loads drive this job.
        let loads = amounts
            .iter()
            .flat_map(|&x| [RecipeLoadN::solid(x); 3])
            .collect();
        let ground = m.paint("Mixed White")?;
        let explicit = Job::new(m, sheet(&[[0.5; 3]; 32]), ground, loads)?;
        let canvas = verify_job(&explicit, &dir, "fixed-recipes", &mut plane_hashes)?;
        assert!(canvas.lat.iter().any(|z| z.iter().all(|v| *v > 0.)));
        let material: Vec<_> = ["lat", "h", "wet", "cover"]
            .map(|plane| oil_paint::plane_bytes(&canvas, plane))
            .into();
        if let Some(reference) = &material_reference {
            assert_eq!(reference, &material);
        } else {
            material_reference = Some(material);
        }
        let m = prepare(&bytes[index], label, &mut timings)?;
        let start = Instant::now();
        let mut loads = Vec::new();
        for (i, &target) in targets.iter().enumerate() {
            let begin = Instant::now();
            let matched = m.match_target(target)?;
            let elapsed = begin.elapsed().as_secs_f64() * 1e3;
            assert!(m.is_valid(&matched.recipe));
            assert_eq!(m.decode_srgb(&matched.recipe), matched.achieved_srgb);
            let color = matched.achieved_srgb;
            let proportion = matched
                .recipe
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(";");
            writeln!(
                matches,
                "{label},{i},{},{},{},{},{},{},{},{},{elapsed},{proportion}",
                target[0],
                target[1],
                target[2],
                color[0],
                color[1],
                color[2],
                matched.error_ok100,
                matched.evaluations
            )?;
            loads.extend([RecipeLoadN::solid(matched.recipe); 3]);
        }
        writeln!(
            timings,
            "{label},match-32-targets,0,{},32",
            start.elapsed().as_secs_f64() * 1e3
        )?;
        let ground = m.paint("Mixed White")?;
        let matched = Job::new(m, sheet(&targets), ground, loads)?;
        verify_job(&matched, &dir, "matched-targets", &mut plane_hashes)?;
        let m = prepare(&bytes[index], label, &mut timings)?;
        let start = Instant::now();
        let (fixture, _) = Job::from_rgb(m, oil_paint::testsheet::testsheet())?;
        writeln!(
            timings,
            "{label},author-fixture,0,{},94",
            start.elapsed().as_secs_f64() * 1e3
        )?;
        verify_job(&fixture, &dir, "renderer-fixture", &mut plane_hashes)?;
        jobs.push([explicit, matched, fixture]);
    }
    let mut state = 8102026u64;
    let corpus: Vec<_> = (0..4096)
        .map(|i| {
            let x = std::array::from_fn(|j| {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                if i % 4 == 0 && j > i % 8 {
                    0.
                } else {
                    ((state >> 32) as u32 as f64 + 1.) / (u32::MAX as f64 + 1.)
                }
            });
            jobs[0][0].mixer().recipe(x).unwrap()
        })
        .collect();
    // One discarded warmup and seven alternating paired observations.
    for repetition in 0..8 {
        for offset in 0..2 {
            let index = (repetition + offset) % 2;
            let label = LABELS[index];
            let m = jobs[index][0].mixer();
            let start = Instant::now();
            for i in 0..100_000 {
                black_box(m.decode_srgb(black_box(&corpus[i % corpus.len()])));
            }
            let elapsed = start.elapsed().as_secs_f64() * 1e3;
            if repetition > 0 {
                writeln!(timings, "{label},decode-srgb,{repetition},{elapsed},100000")?;
            }
            for (name, job) in ["fixed-recipes", "matched-targets", "renderer-fixture"]
                .iter()
                .zip(&jobs[index])
            {
                let start = Instant::now();
                black_box(job.paint(384, |_, _| {})?);
                if repetition > 0 {
                    writeln!(
                        timings,
                        "{label},paint-{name},{repetition},{},1",
                        start.elapsed().as_secs_f64() * 1e3
                    )?;
                }
            }
        }
    }
    std::fs::write(out.join("timings.csv"), timings)?;
    std::fs::write(out.join("matches.csv"), matches)?;
    std::fs::write(out.join("swatches.csv"), swatches)?;
    std::fs::write(out.join("plane-hashes.csv"), plane_hashes)?;
    std::fs::write(out.join("verification.txt"), "6 jobs replay all five canvas planes exactly; package and future mixing replay exact; fixed-recipe material/height/wetness/coverage identical between packages; valid eight-material states; 32 matched targets per package; 7 alternating warmed timings per operation\n")?;
    Ok(())
}
