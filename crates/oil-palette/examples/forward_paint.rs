//! Compare persisted direct decoder versions on identical saved material recipes.
use oil_kernel::Canvas;
use oil_mix::palette_forward::PaletteForwardN;
use oil_palette::{ForwardDecoder, PaletteJobN, PaletteMixerN};
use sha2::{Digest, Sha256};
use std::{error::Error, fmt::Write, path::Path, time::Instant};
type Job = PaletteJobN<8, false, 31>;
type M = PaletteMixerN<8, false, 31>;
type Result<T> = std::result::Result<T, Box<dyn Error>>;
fn hashes(cv: &Canvas<M>) -> Vec<String> {
    let mut result = Vec::new();
    for plane in oil_paint::PLANES {
        let mut h = Sha256::new();
        for x in oil_paint::plane_values(cv, plane) {
            h.update(x.to_le_bytes());
        }
        result.push(format!("{:x}", h.finalize()));
    }
    let mut h = Sha256::new();
    for x in &cv.hblur {
        h.update(x.to_le_bytes());
    }
    result.push(format!("{:x}", h.finalize()));
    result
}
fn lab(rgb: [f32; 3]) -> [f64; 3] {
    let [r, g, b] = rgb.map(|v| oil_math::srgb_to_linear(v as f64));
    let l = oil_math::cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
    let m = oil_math::cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
    let s = oil_math::cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
    [
        0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    ]
}
fn png(path: &Path, cv: &Canvas<M>) -> Result<()> {
    let mut e = png::Encoder::new(std::fs::File::create(path)?, cv.w as u32, cv.h as u32);
    e.set_color(png::ColorType::Rgb);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()?.write_image_data(
        &cv.rgb
            .iter()
            .flatten()
            .map(|v| (v * 255. + 0.5).clamp(0., 255.) as u8)
            .collect::<Vec<_>>(),
    )?;
    Ok(())
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("Expected saved-job-directory output-directory width".into());
    }
    let input = Path::new(&args[0]);
    let out = Path::new(&args[1]);
    std::fs::create_dir_all(out)?;
    let width: u32 = args[2].to_str().ok_or("width")?.parse()?;
    if ![512, 1024, 2048].contains(&width) {
        return Err("Expected 512/1024/2048".into());
    }
    let methods = [
        ("reference", ForwardDecoder::Reference),
        ("algebraic", ForwardDecoder::AlgebraicV1),
        ("exp-lut", ForwardDecoder::ExpLutV1),
    ];
    let mut timings = String::from("scene,width,decoder,repetition,paint_ms\n");
    let mut errors=String::from("scene,width,decoder,pixels,changed_float_pixels,changed_8bit_pixels,channel_max,oklab100_mean,oklab100_max,sampled_spectral_max,sampled_linear_max\n");
    let mut evidence = String::from("scene,width,decoder,plane,sha256\n");
    for scene in ["fixed-recipes", "matched-targets", "renderer-fixture"] {
        let bytes = std::fs::read(input.join(format!("{scene}.opj")))?;
        let reference = Job::from_bytes(&bytes)?;
        assert_eq!(bytes, reference.to_bytes()?);
        assert_eq!(
            reference.mixer().forward_decoder(),
            ForwardDecoder::Reference
        );
        let jobs: Vec<_> = methods
            .iter()
            .map(|(_, m)| Job::from_bytes(&bytes)?.with_forward_decoder(*m))
            .collect::<std::result::Result<_, _>>()?;
        let (mut baseline, base_stats) = reference.paint_final(width)?;
        let baseline_hash = hashes(&baseline);
        let baseline_rgb = std::mem::take(&mut baseline.rgb);
        drop(baseline);
        let mut selected_hash = Vec::new();
        for (job, (_, mode)) in jobs.iter().zip(methods) {
            assert_eq!(job.loads(), reference.loads());
            assert_eq!(job.ground(), reference.ground());
            let saved = job.to_bytes()?;
            std::fs::write(out.join(format!("{scene}-{mode:?}.opj")), &saved)?;
            let restored = Job::from_bytes(&saved)?;
            assert_eq!(restored.to_bytes()?, saved);
            assert_eq!(restored.mixer().forward_decoder(), mode);
            let (cv, stats) = restored.paint_final(width)?;
            let hash = hashes(&cv);
            for i in [0, 2, 3, 4, 5] {
                assert_eq!(hash[i], baseline_hash[i]);
            }
            assert_eq!(
                (stats.strokes, stats.painted_pixels, stats.alpha.to_bits()),
                (
                    base_stats.strokes,
                    base_stats.painted_pixels,
                    base_stats.alpha.to_bits()
                )
            );
            selected_hash.push(hash);
        }
        for repetition in 0..4 {
            for offset in 0..3 {
                let index = (repetition + offset) % 3;
                let (name, mode) = methods[index];
                let job = &jobs[index];
                let start = Instant::now();
                let (cv, stats) = job.paint_final(width)?;
                let elapsed = start.elapsed().as_secs_f64() * 1e3;
                let hash = hashes(&cv);
                assert_eq!(hash, selected_hash[index]);
                assert_eq!(
                    (stats.strokes, stats.painted_pixels, stats.alpha.to_bits()),
                    (
                        base_stats.strokes,
                        base_stats.painted_pixels,
                        base_stats.alpha.to_bits()
                    )
                );
                if repetition > 0 {
                    writeln!(timings, "{scene},{width},{name},{repetition},{elapsed}")?;
                }
                if repetition == 1 {
                    let mut changed = 0;
                    let mut changed8 = 0;
                    let mut channel_max = 0.0f64;
                    let mut total = 0.;
                    let mut max = 0.0f64;
                    for (a, b) in cv.rgb.iter().zip(&baseline_rgb) {
                        assert!(a.iter().all(|v| v.is_finite() && (0. ..=1.).contains(v)));
                        if a.map(f32::to_bits) != b.map(f32::to_bits) {
                            changed += 1;
                        }
                        if a.map(|v| (v * 255. + 0.5).clamp(0., 255.) as u8)
                            != b.map(|v| (v * 255. + 0.5).clamp(0., 255.) as u8)
                        {
                            changed8 += 1;
                        }
                        channel_max = channel_max.max(
                            a.iter()
                                .zip(b)
                                .map(|(x, y)| (*x as f64 - *y as f64).abs())
                                .fold(0., f64::max),
                        );
                        let delta = 100.
                            * lab(*a)
                                .iter()
                                .zip(lab(*b))
                                .map(|(x, y)| (x - y) * (x - y))
                                .sum::<f64>()
                                .sqrt();
                        total += delta;
                        max = max.max(delta);
                    }
                    let evaluator = PaletteForwardN::new(job.mixer().palette(), mode);
                    let mut spectral = 0.0f64;
                    let mut linear = 0.0f64;
                    for c in cv.lat.iter().step_by((cv.lat.len() / 4096).max(1)) {
                        let c = c.map(|v| v as f64);
                        let expected = job.mixer().palette().recipe(c)?;
                        spectral = spectral.max(
                            evaluator
                                .reflectance(c)?
                                .iter()
                                .zip(expected.reflectance())
                                .map(|(a, b)| (a - b).abs())
                                .fold(0., f64::max),
                        );
                        linear = linear.max(
                            evaluator
                                .decode_linear(c)?
                                .iter()
                                .zip(expected.decode_linear())
                                .map(|(a, b)| (a - b).abs())
                                .fold(0., f64::max),
                        );
                    }
                    if mode == ForwardDecoder::Reference {
                        assert_eq!(changed, 0);
                    }
                    if mode == ForwardDecoder::AlgebraicV1 {
                        assert!(spectral <= 2e-12 && linear <= 2e-12 && max <= 0.001);
                    } else {
                        assert!(
                            spectral <= 1e-5
                                && linear <= 2e-5
                                && channel_max <= 1e-4
                                && max <= 0.01
                        );
                    }
                    writeln!(errors,"{scene},{width},{name},{},{changed},{changed8},{channel_max},{},{max},{spectral},{linear}",cv.rgb.len(),total/cv.rgb.len() as f64)?;
                    for (plane, h) in ["lat", "rgb", "h", "wet", "cover", "hblur"]
                        .iter()
                        .zip(hash)
                    {
                        writeln!(evidence, "{scene},{width},{name},{plane},{h}")?;
                    }
                    if width == 512 {
                        png(&out.join(format!("{scene}-{name}.png")), &cv)?;
                    }
                }
                assert_eq!(
                    job.mixer().target_cache_stats().hits + job.mixer().target_cache_stats().misses,
                    0
                );
            }
            std::fs::write(out.join(format!("timings-{width}.csv")), &timings)?;
            std::fs::write(out.join(format!("errors-{width}.csv")), &errors)?;
            std::fs::write(out.join(format!("checks-{width}.csv")), &evidence)?;
            println!("{scene} {width}: round {repetition}/3 passed");
        }
    }
    Ok(())
}
