//! Rasterisation and morphology for the scene compiler: polygon fill, exact Euclidean distance transform, and
//! square min/max filters. All exact or built from IEEE basic operations in a fixed order.

/// Fill a polygon (even-odd rule), sampling at pixel centres: pixel (x, y) is inside when (x + 0.5, y + 0.5) is.
/// `pts` are in pixels. Returns 1.0 inside, 0.0 outside.
pub fn fill_polygon(w: usize, h: usize, pts: &[[f64; 2]]) -> Vec<f32> {
    let mut out = vec![0f32; w * h];
    let n = pts.len();
    if n < 3 {
        return out;
    }
    let (ymin, ymax) = pts.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| (a.min(p[1]), b.max(p[1])));
    let y0 = ((ymin - 0.5).ceil().max(0.0) as usize).min(h);
    let y1 = (((ymax - 0.5).floor() + 1.0).max(0.0) as usize).min(h);
    let mut xs: Vec<f64> = Vec::with_capacity(n);
    for y in y0..y1 {
        let yc = y as f64 + 0.5;
        xs.clear();
        for k in 0..n {
            let (a, b) = (pts[k], pts[(k + 1) % n]);
            if (a[1] <= yc) != (b[1] <= yc) {
                xs.push(a[0] + (yc - a[1]) * (b[0] - a[0]) / (b[1] - a[1]));
            }
        }
        xs.sort_by(f64::total_cmp);
        let row = &mut out[y * w..(y + 1) * w];
        for span in xs.chunks_exact(2) {
            // centres x + 0.5 in [span0, span1)
            let a = ((span[0] - 0.5).ceil().max(0.0) as usize).min(w);
            let b = ((span[1] - 0.5).ceil().max(0.0) as usize).min(w);
            for v in &mut row[a..b.max(a)] {
                *v = 1.0;
            }
        }
    }
    out
}

/// 1-D squared distance transform of sampled function `f` (Felzenszwalb and Huttenlocher).
fn dt1(f: &[f64], d: &mut [f64], v: &mut [usize], z: &mut [f64]) {
    let n = f.len();
    let mut k = 0usize;
    v[0] = 0;
    z[0] = f64::NEG_INFINITY;
    z[1] = f64::INFINITY;
    for q in 1..n {
        let qf = q as f64;
        let inter = |k: usize| {
            let p = v[k] as f64;
            ((f[q] + qf * qf) - (f[v[k]] + p * p)) / (2.0 * qf - 2.0 * p)
        };
        // z[0] is -inf, so k never goes below 0
        let mut s = inter(k);
        while s <= z[k] {
            k -= 1;
            s = inter(k);
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f64::INFINITY;
    }
    k = 0;
    for (q, dq) in d.iter_mut().enumerate().take(n) {
        let qf = q as f64;
        while z[k + 1] < qf {
            k += 1;
        }
        let p = v[k] as f64;
        *dq = (qf - p) * (qf - p) + f[v[k]];
    }
}

/// Exact Euclidean distance (pixels) from every pixel to the nearest pixel where `inside` is false; 0 on those
/// pixels. Without any such pixel every distance is `w + h`.
pub fn edt(inside: &[bool], w: usize, h: usize) -> Vec<f32> {
    const INF: f64 = 1e20;
    let mut g: Vec<f64> = inside.iter().map(|&b| if b { INF } else { 0.0 }).collect();
    let n = w.max(h);
    let (mut f, mut d, mut v, mut z) = (vec![0f64; n], vec![0f64; n], vec![0usize; n], vec![0f64; n + 1]);
    for x in 0..w {
        for y in 0..h {
            f[y] = g[y * w + x];
        }
        dt1(&f[..h], &mut d[..h], &mut v, &mut z);
        for y in 0..h {
            g[y * w + x] = d[y];
        }
    }
    let cap = (w + h) as f64;
    let mut out = vec![0f32; w * h];
    for y in 0..h {
        f[..w].copy_from_slice(&g[y * w..(y + 1) * w]);
        dt1(&f[..w], &mut d[..w], &mut v, &mut z);
        for x in 0..w {
            out[y * w + x] = d[x].sqrt().min(cap) as f32;
        }
    }
    out
}

/// Running max (or min) over a centred window of 2k+1 samples, ignoring samples outside the array (van Herk /
/// Gil-Werman, O(1) per sample).
fn run_extreme(a: &[f32], k: usize, out: &mut [f32], take_max: bool) {
    let n = a.len();
    let m = 2 * k + 1;
    let pad = if take_max { f32::NEG_INFINITY } else { f32::INFINITY };
    let pick = |x: f32, y: f32| if take_max { x.max(y) } else { x.min(y) };
    let len = (n + 2 * k).div_ceil(m) * m;
    let mut p = vec![pad; len];
    p[k..k + n].copy_from_slice(a);
    let mut g = vec![pad; len];
    let mut hh = vec![pad; len];
    for b in (0..len).step_by(m) {
        g[b] = p[b];
        for i in b + 1..b + m {
            g[i] = pick(g[i - 1], p[i]);
        }
        hh[b + m - 1] = p[b + m - 1];
        for i in (b..b + m - 1).rev() {
            hh[i] = pick(hh[i + 1], p[i]);
        }
    }
    for (i, o) in out.iter_mut().enumerate().take(n) {
        *o = pick(hh[i], g[i + m - 1]);
    }
}

fn square_filter(src: &[f32], w: usize, h: usize, k: usize, take_max: bool) -> Vec<f32> {
    let mut tmp = vec![0f32; w * h];
    for y in 0..h {
        run_extreme(&src[y * w..(y + 1) * w], k, &mut tmp[y * w..(y + 1) * w], take_max);
    }
    let mut out = vec![0f32; w * h];
    let (mut col, mut res) = (vec![0f32; h], vec![0f32; h]);
    for x in 0..w {
        for y in 0..h {
            col[y] = tmp[y * w + x];
        }
        run_extreme(&col, k, &mut res, take_max);
        for y in 0..h {
            out[y * w + x] = res[y];
        }
    }
    out
}

/// Grey dilation with a (2k+1) x (2k+1) square; the window is clipped at the borders (OpenCV's default).
pub fn dilate(src: &[f32], w: usize, h: usize, k: usize) -> Vec<f32> {
    square_filter(src, w, h, k, true)
}

/// Grey erosion with a (2k+1) x (2k+1) square; the window is clipped at the borders.
pub fn erode(src: &[f32], w: usize, h: usize, k: usize) -> Vec<f32> {
    square_filter(src, w, h, k, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polygon_rectangle_and_triangle() {
        let m = fill_polygon(10, 8, &[[2.0, 1.0], [7.0, 1.0], [7.0, 5.0], [2.0, 5.0]]);
        let count = m.iter().filter(|v| **v > 0.0).count();
        assert_eq!(count, 5 * 4);
        assert_eq!(m[8 + 2], 0.0);
        assert_eq!(m[10 + 2], 1.0);
        assert_eq!(m[10 + 7], 0.0);
        let t = fill_polygon(100, 100, &[[0.0, 0.0], [100.0, 0.0], [0.0, 100.0]]);
        let area = t.iter().sum::<f32>();
        assert!((area - 5000.0).abs() < 60.0, "{area}");
    }

    #[test]
    fn edt_matches_brute_force() {
        let (w, h) = (23, 17);
        let inside: Vec<bool> = (0..w * h).map(|i| (i * 7919 + 13) % 11 != 0).collect();
        let d = edt(&inside, w, h);
        for y in 0..h {
            for x in 0..w {
                let mut best = f64::INFINITY;
                for yy in 0..h {
                    for xx in 0..w {
                        if !inside[yy * w + xx] {
                            let (dx, dy) = (x as f64 - xx as f64, y as f64 - yy as f64);
                            best = best.min(dx * dx + dy * dy);
                        }
                    }
                }
                assert_eq!(d[y * w + x], best.sqrt() as f32, "({x}, {y})");
            }
        }
        assert!(edt(&[true; 12], 4, 3).iter().all(|v| *v == 7.0));
    }

    #[test]
    fn min_max_match_brute_force() {
        let (w, h, k) = (13, 9, 2);
        let a: Vec<f32> = (0..w * h).map(|i| ((i * 37) % 17) as f32).collect();
        let (dl, er) = (dilate(&a, w, h, k), erode(&a, w, h, k));
        for y in 0..h {
            for x in 0..w {
                let (mut mx, mut mn) = (f32::NEG_INFINITY, f32::INFINITY);
                for yy in y.saturating_sub(k)..(y + k + 1).min(h) {
                    for xx in x.saturating_sub(k)..(x + k + 1).min(w) {
                        mx = mx.max(a[yy * w + xx]);
                        mn = mn.min(a[yy * w + xx]);
                    }
                }
                assert_eq!((dl[y * w + x], er[y * w + x]), (mx, mn));
            }
        }
    }
}
