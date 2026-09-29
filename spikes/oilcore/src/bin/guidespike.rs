//! spec-compiler cost probe at 600x750: the 20 Storm Light mask blurs and a 4-octave hashed-lattice value-noise field
use std::time::Instant;
fn hash(ix: i32, iy: i32, seed: u32) -> f32 {
    let mut h = (ix as u32).wrapping_mul(374761393).wrapping_add((iy as u32).wrapping_mul(668265263)).wrapping_add(seed.wrapping_mul(2246822519));
    h = (h ^ (h >> 13)).wrapping_mul(1274126177); h ^= h >> 16; (h >> 8) as f32 * (1.0 / 16777216.0)
}
fn cubic(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 { // Catmull-Rom
    p1 + 0.5 * t * (p2 - p0 + t * (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3 + t * (3.0 * (p1 - p2) + p3 - p0)))
}
fn noise(x: f32, y: f32, seed: u32) -> f32 {
    let (fx, fy) = (x.floor(), y.floor()); let (ix, iy) = (fx as i32, fy as i32); let (tx, ty) = (x - fx, y - fy);
    let mut col = [0f32; 4];
    for j in 0..4 { let yy = iy - 1 + j as i32;
        col[j] = cubic(hash(ix - 1, yy, seed), hash(ix, yy, seed), hash(ix + 1, yy, seed), hash(ix + 2, yy, seed), tx); }
    cubic(col[0], col[1], col[2], col[3], ty)
}
fn main() {
    let (w, h) = (600usize, 750usize);
    let sig: Vec<f64> = std::fs::read("../planspike/data/sigmas.npy").map(|b| {
        let body = &b[b.len() - 8 * ((b.len() - 128) / 8)..]; body.chunks(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect() }).unwrap();
    let mask: Vec<f32> = (0..w * h).map(|i| if (i % w) > 200 && (i / w) > 300 { 1.0 } else { 0.0 }).collect();
    for rep in 0..2 {
        let t0 = Instant::now(); for s in &sig { std::hint::black_box(oilcore::imgops::gaussian_blur(&mask, w, h, *s)); } let td = t0.elapsed().as_secs_f64();
        let t0 = Instant::now(); for s in &sig { std::hint::black_box(oilcore::imgops::blur(&mask, w, h, *s)); } let tb = t0.elapsed().as_secs_f64();
        let t0 = Instant::now(); let mut acc = 0f32;
        for y in 0..h { for x in 0..w { let (u, v) = ((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / w as f32);
            let (mut a, mut amp, mut c, mut tot) = (0f32, 1f32, 10f32, 0f32);
            for o in 0..4 { a += amp * noise(u * c, v * c, 40 + o); tot += amp; amp *= 0.5; c *= 2.0; } acc += a / tot; } }
        let tn = t0.elapsed().as_secs_f64();
        println!("rep {}: {} mask blurs direct {:.3}s, with downsample for sigma>=8 {:.3}s | 4-octave value noise, full 600x750 canvas {:.3}s (sum {:.1})", rep, sig.len(), td, tb, tn, acc);
    }
}
