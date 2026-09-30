//! Value noise on a hashed lattice, sampled at continuous canvas-width coordinates.
//!
//! v1 drew each octave's lattice from numpy's PCG64 and sized it from the raster; here a lattice value is a hash
//! of (seed, octave, i, j), so the field is unbounded, needs no storage, and is the same at every resolution.
//! Interpolation is OpenCV's bicubic (Keys, a = -0.75), as in v1; values are about [0, 1] (bicubic overshoots a
//! little).

#[inline(always)]
fn fmix32(mut h: u32) -> u32 {
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    h
}

/// Lattice value in [0, 1) for integer lattice point (i, j) of an octave.
#[inline(always)]
pub fn lattice(seed: u32, octave: u32, i: i64, j: i64) -> f64 {
    let mut h = fmix32(seed.wrapping_mul(0x9e37_79b1) ^ octave.wrapping_mul(0x7feb_352d));
    h = fmix32(h ^ (i as u32).wrapping_mul(0xcc9e_2d51));
    h = fmix32(h ^ (j as u32).wrapping_mul(0x1b87_3593));
    (h >> 8) as f64 * (1.0 / 16_777_216.0)
}

/// Bicubic weights for the taps at -1, 0, 1, 2 around a fractional position t in [0, 1) (OpenCV's
/// `interpolateCubic`).
#[inline(always)]
pub fn cubic_weights(t: f64) -> [f64; 4] {
    const A: f64 = -0.75;
    let w0 = ((A * (t + 1.0) - 5.0 * A) * (t + 1.0) + 8.0 * A) * (t + 1.0) - 4.0 * A;
    let w1 = ((A + 2.0) * t - (A + 3.0)) * t * t + 1.0;
    let u = 1.0 - t;
    let w2 = ((A + 2.0) * u - (A + 3.0)) * u * u + 1.0;
    [w0, w1, w2, 1.0 - w0 - w1 - w2]
}

/// Taps for coordinate `u` in lattice units: first lattice index and weights.
#[inline(always)]
fn taps(u: f64) -> (i64, [f64; 4]) {
    let i0 = u.floor();
    (i0 as i64 - 1, cubic_weights(u - i0))
}

/// Fractal value noise over pixel centres of a raster `px_per_cw` pixels per canvas width, on the pixel window
/// `x0..x1, y0..y1`. `scale` is the size of the largest feature in cw (cells = 1 / scale); octaves double the
/// frequency and halve the amplitude. Row-major output of the window.
///
/// Each value is `sum_o amp_o * sum_j wy_j * (sum_i wx_i * L_o(i, j))` over the 4x4 taps, divided by the sum of
/// amplitudes, evaluated in that order: a pixel's value does not depend on the window.
pub fn fbm_window(px_per_cw: f64, (x0, y0, x1, y1): (usize, usize, usize, usize), scale: f64, octaves: u32, seed: u32) -> Vec<f32> {
    let (ww, wh) = (x1.saturating_sub(x0), y1.saturating_sub(y0));
    let mut acc = vec![0f64; ww * wh];
    if ww == 0 || wh == 0 {
        return Vec::new();
    }
    let mut c = 1.0 / scale.max(1e-3);
    let (mut amp, mut tot) = (1.0f64, 0.0f64);
    let mut row = vec![0f64; ww];
    for o in 0..octaves {
        let xt: Vec<(i64, [f64; 4])> = (x0..x1).map(|x| taps((x as f64 + 0.5) / px_per_cw * c)).collect();
        let yt: Vec<(i64, [f64; 4])> = (y0..y1).map(|y| taps((y as f64 + 0.5) / px_per_cw * c)).collect();
        let (i_lo, i_hi) = (xt[0].0, xt[ww - 1].0 + 3);
        let (j_lo, j_hi) = (yt[0].0, yt[wh - 1].0 + 3);
        let nl = (i_hi - i_lo + 1) as usize;
        // x pass: each lattice row interpolated at every window column
        let rows: Vec<Vec<f64>> = (j_lo..=j_hi)
            .map(|j| {
                let lat: Vec<f64> = (i_lo..=i_hi).map(|i| lattice(seed, o, i, j)).collect();
                debug_assert_eq!(lat.len(), nl);
                xt.iter()
                    .map(|(i, w)| {
                        let k = (i - i_lo) as usize;
                        w[0] * lat[k] + w[1] * lat[k + 1] + w[2] * lat[k + 2] + w[3] * lat[k + 3]
                    })
                    .collect()
            })
            .collect();
        // y pass
        for (yy, (j, w)) in yt.iter().enumerate() {
            let k = (j - j_lo) as usize;
            let (r0, r1, r2, r3) = (&rows[k], &rows[k + 1], &rows[k + 2], &rows[k + 3]);
            for (x, r) in row.iter_mut().enumerate() {
                *r = w[0] * r0[x] + w[1] * r1[x] + w[2] * r2[x] + w[3] * r3[x];
            }
            let out = &mut acc[yy * ww..(yy + 1) * ww];
            for (o, r) in out.iter_mut().zip(&row) {
                *o += amp * r;
            }
        }
        tot += amp;
        amp *= 0.5;
        c *= 2.0;
    }
    acc.iter().map(|v| (v / tot) as f32).collect()
}

/// Fractal value noise at one point (x, y) in cw: the same sums in the same order as `fbm_window`, so it gives the
/// same bits at a pixel centre.
pub fn fbm_at(x: f64, y: f64, scale: f64, octaves: u32, seed: u32) -> f64 {
    let mut c = 1.0 / scale.max(1e-3);
    let (mut amp, mut tot, mut acc) = (1.0f64, 0.0f64, 0.0f64);
    for o in 0..octaves {
        let (i0, wx) = taps(x * c);
        let (j0, wy) = taps(y * c);
        let row = |j: i64| {
            wx[0] * lattice(seed, o, i0, j) + wx[1] * lattice(seed, o, i0 + 1, j) + wx[2] * lattice(seed, o, i0 + 2, j) + wx[3] * lattice(seed, o, i0 + 3, j)
        };
        let v = wy[0] * row(j0) + wy[1] * row(j0 + 1) + wy[2] * row(j0 + 2) + wy[3] * row(j0 + 3);
        acc += amp * v;
        tot += amp;
        amp *= 0.5;
        c *= 2.0;
    }
    acc / tot
}

/// `fbm_window` over a whole `w` x `h` raster whose width is one canvas width.
pub fn fbm(w: usize, h: usize, scale: f64, octaves: u32, seed: u32) -> Vec<f32> {
    fbm_window(w as f64, (0, 0, w, h), scale, octaves, seed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_sum_to_one_and_interpolate() {
        for t in [0.0, 0.25, 0.5, 0.9] {
            let w = cubic_weights(t);
            assert!((w.iter().sum::<f64>() - 1.0).abs() < 1e-15);
        }
        assert_eq!(cubic_weights(0.0), [0.0, 1.0, 0.0, 0.0]);
    }

    #[test]
    fn window_independent_and_resolution_consistent() {
        let full = fbm(64, 80, 0.2, 3, 7);
        let win = fbm_window(64.0, (10, 20, 40, 50), 0.2, 3, 7);
        for y in 0..30 {
            for x in 0..30 {
                assert_eq!(win[y * 30 + x].to_bits(), full[(y + 20) * 64 + x + 10].to_bits());
            }
        }
        let (mn, mx) = full.iter().fold((1f32, 0f32), |(a, b), v| (a.min(*v), b.max(*v)));
        assert!(mn > -0.3 && mx < 1.3 && mx - mn > 0.3, "{mn} {mx}");
        // the centre of pixel (32, 40) at 64 px is the corner shared by pixels (64..66, 80..82) at 128 px
        let hi = fbm(128, 160, 0.2, 3, 7);
        let a = full[40 * 64 + 32];
        let b = 0.25 * (hi[80 * 128 + 64] + hi[80 * 128 + 65] + hi[81 * 128 + 64] + hi[81 * 128 + 65]);
        assert!((a - b).abs() < 0.05, "{a} {b}");
        assert_ne!(fbm(16, 16, 0.2, 2, 1), fbm(16, 16, 0.2, 2, 2));
    }

    #[test]
    fn point_matches_window() {
        let (w, h) = (48usize, 60usize);
        let win = fbm(w, h, 0.13, 3, 5);
        for j in 0..h {
            for i in 0..w {
                let (x, y) = ((i as f64 + 0.5) / w as f64, (j as f64 + 0.5) / w as f64);
                assert_eq!((fbm_at(x, y, 0.13, 3, 5) as f32).to_bits(), win[j * w + i].to_bits(), "({i}, {j})");
            }
        }
    }
}
