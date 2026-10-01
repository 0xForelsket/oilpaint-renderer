//! Paint a local Old Holland Eight OPP3 package with its native 31-band model.
//! cargo run --release -p oil-palette --example old_holland_eight -- palette.opp out/old-holland
use oil_mix::Mixer;
use oil_palette::{PaletteJobN, PaletteMixerN, RecipeLoadN};
type PaletteJob = PaletteJobN<8, false, 31>;
type PaletteMixer = PaletteMixerN<8, false, 31>;
use sha2::{Digest, Sha256};
use std::{path::PathBuf, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("Expected native eight-paint palette.opp output-directory".into());
    }
    let output = PathBuf::from(&args[1]);
    std::fs::create_dir_all(&output)?;
    let mut evidence = String::new();
    let m = PaletteMixer::from_palette_bytes(&std::fs::read(&args[0])?)?;
    let black = m.match_target([0.; 3])?;
    println!(
        "closest_found_to_black={:?}, error_ok100={:.6}, recipe={:?}",
        black.achieved_srgb, black.error_ok100, black.recipe
    );
    evidence.push_str(&format!(
        "closest_found_to_black={:?}; recipe={:?}; error_ok100={}\n",
        black.achieved_srgb, black.recipe, black.error_ok100
    ));
    let start = Instant::now();
    let (matched, report) = PaletteJob::from_rgb(m, oil_paint::testsheet::testsheet())?;
    println!(
        "authoring_ms={:.3}, strokes={}, max_main_error_ok100={:.6}",
        start.elapsed().as_secs_f64() * 1000.0,
        report.strokes.len(),
        report
            .strokes
            .iter()
            .map(|r| r[0].error_ok100)
            .fold(0.0, f64::max)
    );
    // Second example uses fixed recipes; changing the loaded optical palette
    // changes its output without changing these authored material amounts.
    let m = PaletteMixer::from_palette_bytes(&std::fs::read(&args[0])?)?;
    let ground = m.paint("Mixed White")?;
    let list = oil_paint::testsheet::testsheet();
    let mut loads = Vec::new();
    for (i, _) in list.strokes.iter().enumerate() {
        let mut amounts = [0.0; 8];
        amounts[i % 7] = 1.0;
        amounts[(i + 1) % 7] = 0.2;
        amounts[7] = (i % 5) as f64 * 0.3;
        if i == 0 {
            amounts = [1.0; 8];
        }
        let main = m.recipe(amounts)?;
        let secondary = m.paint("Ultramarine Blue")?;
        loads.push(RecipeLoadN {
            main,
            secondary,
            dz: [0.0; 8],
        });
    }
    let explicit = PaletteJob::new(m, list, ground, loads)?;
    for (name, job) in [("target-matched", matched), ("explicit-recipes", explicit)] {
        let bytes = job.to_bytes()?;
        std::fs::write(output.join(format!("{name}.opj")), &bytes)?;
        let start = Instant::now();
        let (canvas, stats) = job.paint(256, |_, _| {})?;
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        let read = PaletteJob::from_bytes(&std::fs::read(output.join(format!("{name}.opj")))?)?;
        let replay = read.paint(256, |_, _| {})?.0;
        assert_eq!(bytes, read.to_bytes()?);
        let mut hashes = String::new();
        for plane in oil_paint::PLANES {
            let data = oil_paint::plane_bytes(&canvas, plane);
            assert_eq!(data, oil_paint::plane_bytes(&replay, plane));
            hashes.push_str(&format!("{plane} {:x}\n", Sha256::digest(&data)));
        }
        assert!(canvas.lat.iter().all(|z| job.mixer().is_valid(z)));
        if name == "explicit-recipes" {
            assert!(canvas.lat.iter().any(|z| z.iter().all(|v| *v > 0.0)));
        }
        let file = std::fs::File::create(output.join(format!("{name}.png")))?;
        let mut encoder = png::Encoder::new(file, canvas.w as u32, canvas.h as u32);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(
            &canvas
                .rgb
                .iter()
                .flatten()
                .map(|v| (v * 255.0 + 0.5).clamp(0.0, 255.0) as u8)
                .collect::<Vec<_>>(),
        )?;
        std::fs::write(output.join(format!("{name}-hashes.txt")), hashes)?;
        println!(
            "{name}: {}x{}, {} strokes, {:.3} ms paint, {} bytes bundle, replay bit-identical",
            canvas.w,
            canvas.h,
            stats.strokes,
            elapsed,
            bytes.len()
        );
        evidence.push_str(&format!("{name}: {}x{}, {} strokes, {:.3} ms paint, {} bytes bundle, all canvas planes replay bit-identical\n", canvas.w, canvas.h, stats.strokes, elapsed, bytes.len()));
    }
    std::fs::write(output.join("verification.txt"), evidence)?;
    Ok(())
}
