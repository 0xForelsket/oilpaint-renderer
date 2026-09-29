//! Native arithmetic comparison. Mixbox states come from the optional installed
//! encoder in a temporary input file, never from a vendored LUT or fitted data.
use ochrell::{Color, FastPigmentMixer};
use std::{hint::black_box as bb, time::Instant};
fn main() {
    let bytes = std::fs::read(std::env::args().nth(1).expect("temporary inputs path")).unwrap();
    let floats: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|a| f32::from_le_bytes(a.try_into().unwrap()))
        .collect();
    assert_eq!(floats.len() % 10, 0);
    let colors: Vec<_> = floats
        .chunks_exact(10)
        .map(|c| Color::srgb(c[0], c[1], c[2]).unwrap())
        .collect();
    let mb: Vec<[f32; 7]> = floats
        .chunks_exact(10)
        .map(|c| c[3..].try_into().unwrap())
        .collect();
    let m = FastPigmentMixer;
    let oc: Vec<_> = colors.iter().map(|c| m.encode(*c)).collect();
    println!("method,repeat,iterations,ns_per_operation");
    for method in [
        "mixbox_state_mix",
        "ochrell_state_mix",
        "mixbox_decode",
        "ochrell_decode",
        "mixbox_cached_mix_decode",
        "ochrell_cached_mix_decode",
        "ochrell_encode",
        "ochrell_full_rgb",
    ] {
        for rep in 0..8 {
            let n = 500000;
            let t0 = Instant::now();
            for i in 0..n {
                let j = i % colors.len();
                let k = (j + 137) % colors.len();
                let t = bb(0.13_f32);
                match method {
                    "mixbox_state_mix" => {
                        let a = bb(mb[j]);
                        let b = bb(mb[k]);
                        bb(std::array::from_fn::<_, 7, _>(|ch| {
                            a[ch] * (1. - t) + b[ch] * t
                        }));
                    }
                    "ochrell_state_mix" => {
                        bb(bb(oc[j]).interpolate(bb(oc[k]), t));
                    }
                    "mixbox_decode" => {
                        let mut rgb = [0.; 3];
                        mixer_comparison::kernel::mixbox_latent_to_rgb(&bb(mb[j]), &mut rgb);
                        bb(rgb);
                    }
                    "ochrell_decode" => {
                        bb(m.decode(bb(oc[j])));
                    }
                    "mixbox_cached_mix_decode" => {
                        let a = bb(mb[j]);
                        let b = bb(mb[k]);
                        let z = std::array::from_fn::<_, 7, _>(|ch| a[ch] * (1. - t) + b[ch] * t);
                        let mut rgb = [0.; 3];
                        mixer_comparison::kernel::mixbox_latent_to_rgb(&z, &mut rgb);
                        bb(rgb);
                    }
                    "ochrell_cached_mix_decode" => {
                        bb(m.decode(bb(oc[j]).interpolate(bb(oc[k]), t)));
                    }
                    "ochrell_encode" => {
                        bb(m.encode(bb(colors[j])));
                    }
                    _ => {
                        bb(m.mix(bb(colors[j]), bb(colors[k]), t));
                    }
                }
            }
            if rep > 0 {
                println!(
                    "{method},{},{n},{:.4}",
                    rep - 1,
                    t0.elapsed().as_nanos() as f64 / n as f64
                );
            }
        }
    }
}
