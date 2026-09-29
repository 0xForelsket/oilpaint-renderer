//! native bench: bench <corpus.bin> <W> [reps] [--light weave.f32]  -> kernel s, total s, FNV-1a digests of rgb/h
use std::time::Instant;
fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}
fn as_bytes(v: &[f32]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 4) }
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let buf = std::fs::read(&a[1]).unwrap();
    let w: usize = a[2].parse().unwrap();
    let reps: usize = a.get(3).map(|s| s.parse().unwrap()).unwrap_or(1);
    let c = oilcore::parse_corpus(&buf);
    let h = (w as f32 * c.aspect).round() as usize;
    for r in 0..reps {
        let mut pl = oilcore::Planes::new(w, h, &c.ground_lat, &c.ground_rgb);
        let t0 = Instant::now();
        let mut ks = 0.0;
        let tref = Instant::now();
        let mut clk = || tref.elapsed().as_secs_f64();
        let npx = oilcore::replay(&c, &mut pl, Some(&mut clk), &mut ks);
        let tt = t0.elapsed().as_secs_f64();
        let t1 = Instant::now();
        let mut lit = vec![0f32; w * h * 3];
        oilcore::light::relight(&pl.rgb, &pl.hgt, None, w, h, &oilcore::light::LightParams { weave_amp: 0.0, ..Default::default() }, &mut lit);
        let tl = t1.elapsed().as_secs_f64();
        println!("rep {} {}x{} npx {} kernel {:.3}s replay {:.3}s relight {:.3}s digest rgb {:016x} h {:016x}",
                 r, w, h, npx, ks, tt, tl, fnv(as_bytes(&pl.rgb)), fnv(as_bytes(&pl.hgt)));
        if r + 1 == reps {
            if let Some(out) = a.iter().position(|s| s == "--dump").map(|i| a[i + 1].clone()) {
                std::fs::write(format!("{}_rgb.f32", out), as_bytes(&pl.rgb)).unwrap();
                std::fs::write(format!("{}_h.f32", out), as_bytes(&pl.hgt)).unwrap();
            }
        }
    }
}
