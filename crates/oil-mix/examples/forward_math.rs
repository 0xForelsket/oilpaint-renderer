//! Screen algebraic and optional 1D-exp-table evaluation against frozen optics.
use oil_mix::{
    palette::PaletteN,
    palette_forward::{ForwardDecoder, PaletteForwardN},
    srgb,
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap, error::Error, fmt::Write, hint::black_box, path::Path, time::Instant,
};

type Palette = PaletteN<8, 31>;
type Decoder = PaletteForwardN<8, 31>;
type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn random(state: &mut u64) -> f64 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
    ((*state >> 32) as f64 + 0.5) / 4294967296.
}
fn recipes() -> Vec<(&'static str, [f64; 8])> {
    let mut r = Vec::new();
    for i in 0..8 {
        let mut x = [0.; 8];
        x[i] = 1.;
        r.push(("pure", x));
    }
    for i in 0..8 {
        for j in i + 1..8 {
            for t in (0..=1024).map(|k| k as f64 / 1024.).chain([
                1e-12,
                1e-8,
                1e-4,
                1. - 1e-4,
                1. - 1e-8,
                1. - 1e-12,
            ]) {
                let mut x = [0.; 8];
                x[i] = 1. - t;
                x[j] = t;
                r.push((if j == 7 { "white-tint" } else { "pair" }, x));
            }
        }
    }
    for i in 0..8 {
        for exponent in 1..=24 {
            let small = oil_math::exp(-(exponent as f64) * std::f64::consts::LN_10);
            let mut x = [small; 8];
            x[i] = 1.;
            r.push(("tiny", x));
        }
    }
    for x in [
        [0., 0., 1., 4., 0., 5., 0., 0.],
        [0., 0., 0., 0.25482363, 0.27008897, 0.47508743, 0., 0.],
        [0., 0., 1., 1., 0., 0., 2., 0.],
    ] {
        for k in 0..=256 {
            let mut y = x;
            y[7] = k as f64 / 16.;
            r.push(("dark-family", y));
        }
    }
    let mut state = 2026100125;
    for i in 0..16384 {
        let x = std::array::from_fn(|_| -oil_math::ln(random(&mut state)));
        r.push(("dense", x));
        let count = 1 + i % 7;
        let start = (random(&mut state) * 8.) as usize;
        let mut x = [0.; 8];
        for j in 0..count {
            x[(start + j) % 8] = -oil_math::ln(random(&mut state));
        }
        r.push(("sparse-face", x));
    }
    r.push(("dense", [1.; 8]));
    r
}
fn display(raw: [f64; 3]) -> [f32; 3] {
    ochrell::conversion::gamut_map(raw).map(|v| srgb::from_linear(v as f32))
}
fn lab(rgb: [f32; 3]) -> [f64; 3] {
    let linear = rgb.map(|v| oil_math::srgb_to_linear(v as f64));
    ochrell::conversion::mat(
        ochrell::conversion::LAB,
        ochrell::conversion::mat(ochrell::conversion::LMS, linear).map(oil_math::cbrt),
    )
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("Expected palette.opp output-directory algebraic|lookup".into());
    }
    let data = std::fs::read(&args[0])?;
    let p = Palette::from_bytes(&data)?;
    let out = Path::new(&args[1]);
    std::fs::create_dir_all(out)?;
    let include_lookup = match args[2].to_str() {
        Some("algebraic") => false,
        Some("lookup") => true,
        _ => return Err("Expected algebraic or lookup".into()),
    };
    let start = Instant::now();
    let mut decoders = vec![
        ("reference", Decoder::new(&p, ForwardDecoder::Reference)),
        ("algebraic", Decoder::new(&p, ForwardDecoder::AlgebraicV1)),
    ];
    if include_lookup {
        decoders.push(("exp-lut", Decoder::new(&p, ForwardDecoder::ExpLutV1)));
    }
    let prep_ms = start.elapsed().as_secs_f64() * 1e3;
    let recipes = recipes();
    let recipe_bytes: Vec<_> = recipes
        .iter()
        .flat_map(|(_, c)| c.iter().flat_map(|v| v.to_le_bytes()))
        .collect();
    std::fs::write(out.join("recipes.f64"), &recipe_bytes)?;
    let mut errors = BTreeMap::<(&str, &str), Vec<[f64; 4]>>::new();
    let mut exact_counts = BTreeMap::<&str, usize>::new();
    for (family, c) in &recipes {
        let spectrum = p.recipe(*c)?.reflectance();
        let raw = p.recipe(*c)?.decode_linear();
        let color = display(raw);
        let l0 = lab(color);
        for (name, decoder) in &decoders[1..] {
            let actual = decoder.reflectance(*c)?;
            let linear = decoder.decode_linear(*c)?;
            let shown = display(linear);
            let l1 = lab(shown);
            assert!(actual
                .iter()
                .all(|v| v.is_finite() && (0. ..=1.).contains(v)));
            let se = actual
                .iter()
                .zip(spectrum)
                .map(|(a, b)| (a - b).abs())
                .fold(0., f64::max);
            let re = linear
                .iter()
                .zip(raw)
                .map(|(a, b)| (a - b).abs())
                .fold(0., f64::max);
            let ce = shown
                .iter()
                .zip(color)
                .map(|(a, b)| (*a as f64 - b as f64).abs())
                .fold(0., f64::max);
            let de = 100.
                * l1.iter()
                    .zip(l0)
                    .map(|(a, b)| (a - b) * (a - b))
                    .sum::<f64>()
                    .sqrt();
            if shown.map(f32::to_bits) == color.map(f32::to_bits) {
                *exact_counts.entry(name).or_default() += 1;
            }
            if *family == "pure" {
                assert_eq!(actual.map(f64::to_bits), spectrum.map(f64::to_bits));
                assert_eq!(shown.map(f32::to_bits), color.map(f32::to_bits));
            }
            if *name == "algebraic" {
                assert!(
                    se <= 2e-12 && re <= 2e-12 && de <= 0.001,
                    "{family}: {se} {re} {de}"
                );
            } else {
                assert!(
                    se <= 1e-5 && re <= 2e-5 && ce <= 1e-4 && de <= 0.01,
                    "{family}: {se} {re} {ce} {de}"
                );
            }
            errors
                .entry((name, family))
                .or_default()
                .push([se, re, ce, de]);
        }
    }
    let mut csv = String::from("decoder,family,count,metric,mean,p95,max\n");
    for ((name, family), values) in errors {
        for (ch, metric) in ["reflectance", "raw_linear", "display_channel", "oklab100"]
            .iter()
            .enumerate()
        {
            let mut x: Vec<_> = values.iter().map(|v| v[ch]).collect();
            x.sort_by(f64::total_cmp);
            writeln!(
                csv,
                "{name},{family},{},{metric},{},{},{}",
                x.len(),
                x.iter().sum::<f64>() / x.len() as f64,
                x[((x.len() - 1) as f64 * 0.95) as usize],
                x.last().unwrap()
            )?;
        }
    }
    std::fs::write(out.join("errors.csv"), csv)?;
    let mut state = 881205;
    let corpus: Vec<_> = (0..4096)
        .map(|_| recipes[(random(&mut state) * recipes.len() as f64) as usize].1)
        .collect();
    let mut timing = String::from("decoder,repetition,ns_per_decode\n");
    for rep in 0..6 {
        for offset in 0..decoders.len() {
            let (name, decoder) = &decoders[(rep + offset) % decoders.len()];
            let start = Instant::now();
            for i in 0..100000 {
                black_box(display(
                    decoder.decode_linear(black_box(corpus[i % corpus.len()]))?,
                ));
            }
            let ns = start.elapsed().as_secs_f64() * 1e9 / 100000.;
            if rep > 0 {
                writeln!(timing, "{name},{rep},{ns}")?;
            }
        }
    }
    std::fs::write(out.join("timings.csv"), timing)?;
    let meta=format!("recipes={}\nrecipe_sha256={:x}\npackage_sha256={:x}\npreparation_all_decoders_ms={prep_ms}\nexact_display_counts={exact_counts:?}\nauxiliary_bytes={:?}\n",recipes.len(),Sha256::digest(recipe_bytes),Sha256::digest(data),decoders.iter().map(|(n,d)|(*n,d.auxiliary_bytes())).collect::<Vec<_>>());
    std::fs::write(out.join("verification.txt"), &meta)?;
    print!("{meta}");
    Ok(())
}
