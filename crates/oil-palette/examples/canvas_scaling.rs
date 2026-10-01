//! Profile three saved eight-paint jobs at realistic sizes; no RGB authoring.
use oil_kernel::{
    brush::{length_in_widths, render_stroke_len},
    Canvas, Load,
};
use oil_mix::Mixer;
use oil_palette::{PaletteJobN, PaletteMixerN};
use sha2::{Digest, Sha256};
use std::{error::Error, fmt::Write, hint::black_box, path::Path, time::Instant};

type Job = PaletteJobN<8, false, 31>;
type PaletteMixer = PaletteMixerN<8, false, 31>;
type Result<T> = std::result::Result<T, Box<dyn Error>>;

struct DisplayBypass(PaletteMixer);
impl Mixer for DisplayBypass {
    const ID: &'static str = "diagnostic-display-bypass";
    type State = [f32; 8];
    fn encode(&self, _: [f32; 3]) -> Self::State {
        panic!("pre-authored jobs only")
    }
    fn decode_srgb(&self, z: &Self::State) -> [f32; 3] {
        black_box(z);
        black_box([0.; 3])
    }
    fn streak(&self, base: &mut Self::State, delta: &Self::State, amplitude: f32) {
        self.0.streak(base, delta, amplitude);
    }
}

struct Run<M: Mixer> {
    canvas: Canvas<M>,
    stats: oil_paint::PaintStats,
    blur_ms: Option<f64>,
}

// Same public brush and blur operations as PaletteJob::paint. Every direct
// harness result is checked against that public API at each scene and size.
fn render<M: Mixer<State = [f32; 8]>>(job: &Job, m: &M, width: u32) -> Run<M> {
    let (w, h) = (width as usize, job.geometry().height_for(width) as usize);
    let n = w * h;
    let mut cv = Canvas {
        w,
        h,
        lat: vec![job.ground(); n],
        rgb: vec![m.decode_srgb(&job.ground()); n],
        hgt: vec![0.; n],
        wet: vec![0.; n],
        cover: vec![0.; n],
        hblur: vec![],
    };
    let mut stats = oil_paint::PaintStats::default();
    let mut points = Vec::new();
    let mut blur_ms = 0.;
    for layer in &job.geometry().layers {
        if let Some(sigma) = layer.hblur_sigma {
            let start = Instant::now();
            cv.hblur = oil_image::blur(&cv.hgt, w, h, (sigma as f64 * width as f64).max(1.));
            blur_ms += start.elapsed().as_secs_f64() * 1e3;
        }
        for i in layer.start as usize..layer.end as usize {
            let stroke = &job.geometry().strokes[i];
            let path = job.geometry().stroke_points(i);
            points.clear();
            points.extend(path.iter().map(|p| {
                [
                    p[0] * width as f32,
                    p[1] * width as f32,
                    p[2] * width as f32,
                    p[3],
                ]
            }));
            let r = job.loads()[i];
            let load = Load {
                zcol: r.main,
                zcol2: r.secondary,
                dz: r.dz,
            };
            let s = render_stroke_len(
                m,
                &mut cv.planes(),
                &points,
                &load,
                &stroke.brush,
                length_in_widths(path),
            );
            stats.strokes += 1;
            stats.painted_pixels += s.pixels;
            stats.alpha += s.alpha;
        }
        if let Some(d) = layer.dry_after {
            cv.dry(d);
        }
    }
    Run {
        canvas: cv,
        stats,
        blur_ms: Some(blur_ms),
    }
}

fn hashes<M: Mixer>(cv: &Canvas<M>) -> Vec<String> {
    let mut hashes = Vec::new();
    for plane in oil_paint::PLANES {
        let mut digest = Sha256::new();
        for value in oil_paint::plane_values(cv, plane) {
            digest.update(value.to_le_bytes());
        }
        hashes.push(format!("{:x}", digest.finalize()));
    }
    let mut digest = Sha256::new();
    for value in &cv.hblur {
        digest.update(value.to_le_bytes());
    }
    hashes.push(format!("{:x}", digest.finalize()));
    hashes
}

fn buffers<M: Mixer<State = [f32; 8]>>(cv: &Canvas<M>) -> usize {
    cv.lat.capacity() * 32
        + cv.rgb.capacity() * 12
        + (cv.hgt.capacity() + cv.wet.capacity() + cv.cover.capacity() + cv.hblur.capacity()) * 4
}

struct Case<'a> {
    scene: &'a str,
    width: u32,
    repetition: usize,
}

fn record<M: Mixer<State = [f32; 8]>>(
    run: Run<M>,
    elapsed_ms: f64,
    mode: &str,
    case: &Case<'_>,
    expected: &[String],
    csv: &mut String,
    evidence: &mut String,
) -> Result<()> {
    let (scene, width, repetition) = (case.scene, case.width, case.repetition);
    let actual = hashes(&run.canvas);
    for (i, value) in actual.iter().enumerate() {
        if mode != "bypass" || i != 1 {
            assert_eq!(value, &expected[i]);
        }
    }
    let touched = run.canvas.cover.iter().filter(|v| **v > 0.).count();
    writeln!(
        csv,
        "{scene},{width},{},{mode},{repetition},{elapsed_ms},{},{},{},{},{},{}",
        run.canvas.h,
        run.blur_ms.map(|x| x.to_string()).unwrap_or_default(),
        run.stats.strokes,
        run.stats.painted_pixels,
        touched,
        buffers(&run.canvas),
        run.stats.alpha.to_bits()
    )?;
    if repetition == 1 {
        for (plane, hash) in ["lat", "rgb", "h", "wet", "cover", "hblur"]
            .iter()
            .zip(actual)
        {
            writeln!(evidence, "{scene},{width},{mode},{plane},{hash}")?;
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("Expected saved-job-directory output-directory width".into());
    }
    let input = Path::new(&args[0]);
    let out = Path::new(&args[1]);
    let width: u32 = args[2].to_str().ok_or("width encoding")?.parse()?;
    if ![512, 1024, 2048].contains(&width) {
        return Err("Expected width 512, 1024 or 2048".into());
    }
    std::fs::create_dir_all(out)?;
    let mut csv = String::from("scene,width,height,mode,repetition,paint_ms,blur_ms,strokes,deposits,touched_pixels,canvas_buffer_bytes,alpha_bits\n");
    let mut evidence = String::from("scene,width,mode,plane,sha256\n");
    for scene in ["fixed-recipes", "matched-targets", "renderer-fixture"] {
        let bytes = std::fs::read(input.join(format!("{scene}.opj")))?;
        let job = Job::from_bytes(&bytes)?;
        assert_eq!(bytes, job.to_bytes()?);
        let m = job.mixer();
        let bypass = DisplayBypass(PaletteMixer::from_palette_bytes(&m.palette().to_bytes())?);
        let expected = hashes(&job.paint(width, |_, _| {})?.0);
        for repetition in 0..4 {
            let case = Case {
                scene,
                width,
                repetition,
            };
            for offset in 0..3 {
                if (repetition + offset) % 3 == 0 {
                    let start = Instant::now();
                    let run = render(&job, m, width);
                    let elapsed = start.elapsed().as_secs_f64() * 1e3;
                    record(
                        run,
                        elapsed,
                        "direct",
                        &case,
                        &expected,
                        &mut csv,
                        &mut evidence,
                    )?;
                } else if (repetition + offset) % 3 == 1 {
                    let start = Instant::now();
                    let run = render(&job, &bypass, width);
                    let elapsed = start.elapsed().as_secs_f64() * 1e3;
                    record(
                        run,
                        elapsed,
                        "bypass",
                        &case,
                        &expected,
                        &mut csv,
                        &mut evidence,
                    )?;
                } else {
                    let start = Instant::now();
                    let (canvas, stats) = job.paint_final(width)?;
                    let elapsed = start.elapsed().as_secs_f64() * 1e3;
                    record(
                        Run {
                            canvas,
                            stats,
                            blur_ms: None,
                        },
                        elapsed,
                        "deferred",
                        &case,
                        &expected,
                        &mut csv,
                        &mut evidence,
                    )?;
                }
            }
            std::fs::write(out.join(format!("timings-{width}.csv")), &csv)?;
            std::fs::write(out.join(format!("checks-{width}.csv")), &evidence)?;
            println!("{scene} {width}: round {repetition}/3 checked");
        }
        assert_eq!(
            m.target_cache_stats().hits + m.target_cache_stats().misses,
            0
        );
    }
    Ok(())
}
