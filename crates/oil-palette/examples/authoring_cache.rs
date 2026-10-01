//! Trace exact target reuse and benchmark palette authoring. See the crate README.
//! Keep reference evaluation via legacy tagless sections for this frozen study.
use oil_mix::{palette::TargetMatchN, Mixer};
use oil_palette::{PaletteJobN, PaletteMixerN, RecipeLoadN};
use oil_strokes::StrokeList;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, error::Error, fmt::Write, path::Path, time::Instant};

type PaletteMixer = PaletteMixerN<8, false, 31>;
type Job = PaletteJobN<8, false, 31>;
type Result<T> = std::result::Result<T, Box<dyn Error>>;
type TargetTrace = Vec<([f32; 3], String)>;

fn record(
    m: &PaletteMixer,
    target: [f32; 3],
    role: &str,
    trace: &mut TargetTrace,
) -> Result<TargetMatchN<8>> {
    trace.push((target, role.into()));
    Ok(m.match_target(target)?)
}

fn trace_authoring(bytes: &[u8], geometry: StrokeList) -> Result<(Job, TargetTrace)> {
    let m = PaletteMixer::from_bytes(bytes, &[])?.with_target_cache_capacity(0);
    let mut trace = Vec::new();
    let ground = record(&m, geometry.ground, "ground", &mut trace)?;
    let mut loads = Vec::new();
    for stroke in &geometry.strokes {
        let main = record(&m, stroke.color, "main", &mut trace)?;
        let secondary = if stroke.color2 == stroke.color {
            main
        } else {
            record(&m, stroke.color2, "secondary", &mut trace)?
        };
        let mut dz = [0.; 8];
        if stroke.streak_amount != 0. {
            // Mirror the existing streak_vector input construction, then verify the
            // complete authored OPJ2 against the production from_rgb path below.
            let rgb = m.decode_srgb(&main.recipe);
            let light = [
                oil_mix::clamp01(rgb[0] * 1.10 + 0.05),
                oil_mix::clamp01(rgb[1] * 1.10 + 0.035),
                oil_mix::clamp01(rgb[2] * 1.10),
            ];
            let dark = [
                oil_mix::clamp01(rgb[0] * 0.88),
                oil_mix::clamp01(rgb[1] * 0.88 - 0.01),
                oil_mix::clamp01(rgb[2] * 0.88 + 0.02),
            ];
            let a = record(&m, light, "streak-light", &mut trace)?;
            let b = record(&m, dark, "streak-dark", &mut trace)?;
            dz = std::array::from_fn(|i| 0.5 * (a.recipe[i] - b.recipe[i]) * stroke.streak_amount);
        }
        loads.push(RecipeLoadN {
            main: main.recipe,
            secondary: secondary.recipe,
            dz,
        });
    }
    Ok((Job::new(m, geometry, ground.recipe, loads)?, trace))
}

fn match_bytes(found: TargetMatchN<8>) -> Vec<u8> {
    found
        .recipe
        .into_iter()
        .chain(found.achieved_srgb)
        .flat_map(f32::to_le_bytes)
        .chain(found.error_ok100.to_le_bytes())
        .chain(found.reference_error_ok100.to_le_bytes())
        .chain((found.evaluations as u64).to_le_bytes())
        .collect()
}

fn verify_paint(job: &Job, baseline: &Job, out: &Path, mode: &str) -> Result<()> {
    let stats = job.mixer().target_cache_stats();
    let a = baseline.paint(384, |_, _| {})?.0;
    let b = job.paint(384, |_, _| {})?.0;
    assert_eq!(stats, job.mixer().target_cache_stats());
    let bytes = job.to_bytes()?;
    std::fs::write(out.join(format!("{mode}.opj")), &bytes)?;
    let restored = Job::from_bytes(&bytes)?;
    assert_eq!(restored.mixer().target_cache_stats().entries, 0);
    assert_eq!(bytes, restored.to_bytes()?);
    let c = restored.paint(384, |_, _| {})?.0;
    let mut hashes = String::new();
    for plane in oil_paint::PLANES {
        let actual = oil_paint::plane_bytes(&b, plane);
        assert_eq!(actual, oil_paint::plane_bytes(&a, plane));
        assert_eq!(actual, oil_paint::plane_bytes(&c, plane));
        writeln!(hashes, "{plane} {:x}", Sha256::digest(&actual))?;
    }
    assert_eq!(a.hblur, b.hblur);
    assert_eq!(b.hblur, c.hblur);
    std::fs::write(out.join(format!("{mode}-planes.txt")), hashes)?;
    Ok(())
}

fn benchmark(
    bytes: &[u8],
    geometry: &StrokeList,
    trace: &[([f32; 3], String)],
    baseline: &Job,
    out: &Path,
) -> Result<()> {
    let mut state = 0x8172_991bu64;
    let unique: Vec<[f32; 3]> = (0..64)
        .map(|_| {
            std::array::from_fn(|_| {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                ((state >> 40) as f32 + 0.5) / 16777216.
            })
        })
        .collect();
    let repeated: Vec<_> = (0..64).map(|i| unique[i % 8]).collect();
    let fixture: Vec<_> = trace.iter().map(|(target, _)| *target).collect();
    let mut timings = String::from("workload,mode,repetition,author_ms,prime_ms,requests,hits,misses,evictions,entries,capacity\n");
    let mut checks = String::from("workload,mode,repetition,output_sha256\n");
    let baseline_bytes = baseline.to_bytes()?;
    for (name, targets) in [
        ("fixture", &fixture),
        ("repeated64", &repeated),
        ("unique64", &unique),
    ] {
        let reference = PaletteMixer::from_bytes(bytes, &[])?.with_target_cache_capacity(0);
        let expected: Vec<u8> = if name == "fixture" {
            baseline_bytes.clone()
        } else {
            targets
                .iter()
                .map(|&target| reference.match_target(target).map(match_bytes))
                .collect::<std::result::Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .collect()
        };
        // Round zero is discarded; rotate all three conditions to limit order bias.
        for repetition in 0..8 {
            for offset in 0..3 {
                let mode = ["disabled", "cold", "warm"][(repetition + offset) % 3];
                let m = PaletteMixer::from_bytes(bytes, &[])?
                    .with_target_cache_capacity(if mode == "disabled" { 0 } else { 1024 });
                let mut prime_ms = 0.;
                if mode == "warm" {
                    let start = Instant::now();
                    for &target in targets {
                        m.match_target(target)?;
                    }
                    prime_ms = start.elapsed().as_secs_f64() * 1e3;
                }
                let before = m.target_cache_stats();
                let list = geometry.clone();
                let start = Instant::now();
                let (output, after, elapsed) = if name == "fixture" {
                    let (job, _) = Job::from_rgb(m, list)?;
                    let elapsed = start.elapsed().as_secs_f64() * 1e3;
                    if repetition == 1 {
                        verify_paint(&job, baseline, out, mode)?;
                    }
                    (job.to_bytes()?, job.mixer().target_cache_stats(), elapsed)
                } else {
                    let mut found = Vec::with_capacity(targets.len());
                    for &target in targets {
                        found.push(m.match_target(target)?);
                    }
                    let elapsed = start.elapsed().as_secs_f64() * 1e3;
                    (
                        found.into_iter().flat_map(match_bytes).collect(),
                        m.target_cache_stats(),
                        elapsed,
                    )
                };
                assert_eq!(expected, output);
                let (hits, misses, evictions) = (
                    after.hits - before.hits,
                    after.misses - before.misses,
                    after.evictions - before.evictions,
                );
                assert_eq!((hits + misses) as usize, targets.len());
                if mode == "warm" {
                    assert_eq!(misses, 0);
                }
                if mode == "disabled" {
                    assert_eq!(hits, 0);
                    assert_eq!(after.entries, 0);
                }
                if name == "unique64" && mode == "cold" {
                    assert_eq!(hits, 0);
                }
                if repetition > 0 {
                    writeln!(timings, "{name},{mode},{repetition},{elapsed},{prime_ms},{},{hits},{misses},{evictions},{},{}", targets.len(), after.entries, after.capacity)?;
                    writeln!(
                        checks,
                        "{name},{mode},{repetition},{:x}",
                        Sha256::digest(&output)
                    )?;
                }
            }
            std::fs::write(out.join("timings.csv"), &timings)?;
            std::fs::write(out.join("checks.csv"), &checks)?;
            println!("{name}: completed round {repetition}/7");
        }
    }
    std::fs::write(out.join("verification.txt"), "All 63 measured condition/workload runs produce identical full result bytes. All 3 fixture modes have identical OPJ2 bytes, all five canvas planes and blurred heights; saved replay agrees and cache reloads empty. Painting does not touch cache counters.\n")?;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if !(2..=3).contains(&args.len()) {
        return Err("Expected palette.opp output-directory [pre-change-fixture.opj]".into());
    }
    let bytes = std::fs::read(&args[0])?;
    let out = Path::new(&args[1]);
    std::fs::create_dir_all(out)?;
    let geometry = oil_paint::testsheet::testsheet();
    let start = Instant::now();
    let (traced, trace) = trace_authoring(&bytes, geometry.clone())?;
    let traced_ms = start.elapsed().as_secs_f64() * 1e3;
    let start = Instant::now();
    let (actual, _) = Job::from_rgb(
        PaletteMixer::from_bytes(&bytes, &[])?.with_target_cache_capacity(0),
        geometry.clone(),
    )?;
    let production_ms = start.elapsed().as_secs_f64() * 1e3;
    assert_eq!(traced.to_bytes()?, actual.to_bytes()?);
    let mut groups = BTreeMap::<[u32; 3], Vec<String>>::new();
    for (target, role) in &trace {
        groups
            .entry(target.map(f32::to_bits))
            .or_default()
            .push(role.clone());
    }
    let mut csv = String::from("r_bits,g_bits,b_bits,count,roles\n");
    for (key, roles) in &groups {
        writeln!(
            csv,
            "{},{},{},{},{}",
            key[0],
            key[1],
            key[2],
            roles.len(),
            roles.join(";")
        )?;
    }
    std::fs::write(out.join("target-reuse.csv"), csv)?;
    let job = actual.to_bytes()?;
    if args.len() == 3 {
        assert_eq!(job, std::fs::read(&args[2])?);
    }
    std::fs::write(out.join("uncached-fixture.opj"), &job)?;
    let evidence = format!("requests={}\nunique_targets={}\nrepeated_requests={}\ntraced_ms={traced_ms}\nproduction_ms={production_ms}\nopj_sha256={:x}\npackage_sha256={:x}\n", trace.len(), groups.len(), trace.len()-groups.len(), Sha256::digest(&job), Sha256::digest(&bytes));
    std::fs::write(out.join("profile.txt"), &evidence)?;
    print!("{evidence}");
    benchmark(&bytes, &geometry, &trace, &actual, out)?;
    Ok(())
}
