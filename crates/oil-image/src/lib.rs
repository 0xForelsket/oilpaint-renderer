//! Image operations on single-channel f32 planes, ported from the kernel spike (`spikes/oilcore/src/imgops.rs`).
//! They follow the OpenCV calls v1 used (GaussianBlur with ksize round(8 sigma + 1) | 1 and BORDER_REFLECT_101,
//! resize INTER_AREA / INTER_LINEAR, Sobel 3x3) closely, but the engine defines them: every sum has a fixed order
//! and the only transcendental is `oil_math::exp`, so results are identical on every host.
//! L2 added the scene-compiler operations: hashed value noise (`noise`), polygon fill, EDT and min/max filters
//! (`geom`).
#![forbid(unsafe_code)]

pub mod geom;
pub mod noise;

#[inline(always)]
fn reflect101(i: i64, n: i64) -> usize {
    if n == 1 {
        return 0;
    }
    let mut i = i;
    loop {
        if i < 0 {
            i = -i;
        } else if i >= n {
            i = 2 * n - 2 - i;
        } else {
            return i as usize;
        }
    }
}

/// Normalised Gaussian kernel with OpenCV's size rule for float images: round(8 sigma + 1), made odd.
pub fn gaussian_kernel(sigma: f64) -> Vec<f32> {
    let n = ((((sigma * 8.0 + 1.0) + 0.5).floor() as i64) | 1).max(1) as usize;
    let s2 = -0.5 / (sigma * sigma);
    let k: Vec<f64> = (0..n)
        .map(|i| {
            let x = i as f64 - (n as f64 - 1.0) * 0.5;
            oil_math::exp(s2 * x * x)
        })
        .collect();
    let sum: f64 = k.iter().sum();
    k.iter().map(|v| (v / sum) as f32).collect()
}

/// Separable Gaussian blur, reflect-101 borders.
pub fn gaussian_blur(src: &[f32], w: usize, h: usize, sigma: f64) -> Vec<f32> {
    if sigma <= 0.0 {
        return src.to_vec();
    }
    let k = gaussian_kernel(sigma);
    let r = (k.len() / 2) as i64;
    let mut tmp = vec![0f32; w * h];
    for y in 0..h {
        let row = &src[y * w..(y + 1) * w];
        let out = &mut tmp[y * w..(y + 1) * w];
        for (x, o) in out.iter_mut().enumerate() {
            let mut acc = 0f32;
            if (x as i64) >= r && (x as i64) + r < w as i64 {
                let base = x - r as usize;
                for (j, kv) in k.iter().enumerate() {
                    acc += kv * row[base + j];
                }
            } else {
                for (j, kv) in k.iter().enumerate() {
                    acc += kv * row[reflect101(x as i64 + j as i64 - r, w as i64)];
                }
            }
            *o = acc;
        }
    }
    let mut dst = vec![0f32; w * h];
    let mut acc = vec![0f32; w];
    for y in 0..h {
        acc.iter_mut().for_each(|v| *v = 0.0);
        for (j, kv) in k.iter().enumerate() {
            let sy = reflect101(y as i64 + j as i64 - r, h as i64);
            let row = &tmp[sy * w..(sy + 1) * w];
            for x in 0..w {
                acc[x] += kv * row[x];
            }
        }
        dst[y * w..(y + 1) * w].copy_from_slice(&acc);
    }
    dst
}

/// Area resize (box mean for integer factors, exact area weights otherwise).
pub fn resize_area(src: &[f32], w: usize, h: usize, dw: usize, dh: usize) -> Vec<f32> {
    let mut dst = vec![0f32; dw * dh];
    if w % dw == 0 && h % dh == 0 {
        let (fx, fy) = (w / dw, h / dh);
        let inv = 1.0f32 / (fx * fy) as f32;
        for y in 0..dh {
            for x in 0..dw {
                let mut s = 0f32;
                for yy in 0..fy {
                    let row = &src[(y * fy + yy) * w + x * fx..];
                    for v in &row[..fx] {
                        s += v;
                    }
                }
                dst[y * dw + x] = s * inv;
            }
        }
        return dst;
    }
    let sx = w as f64 / dw as f64;
    let sy = h as f64 / dh as f64;
    for y in 0..dh {
        let y0 = y as f64 * sy;
        let y1 = y0 + sy;
        for x in 0..dw {
            let x0 = x as f64 * sx;
            let x1 = x0 + sx;
            let mut s = 0f64;
            let mut yy = y0.floor() as usize;
            while (yy as f64) < y1 && yy < h {
                let wy = (y1.min(yy as f64 + 1.0) - y0.max(yy as f64)).max(0.0);
                let mut xx = x0.floor() as usize;
                while (xx as f64) < x1 && xx < w {
                    let wx = (x1.min(xx as f64 + 1.0) - x0.max(xx as f64)).max(0.0);
                    s += wx * wy * src[yy * w + xx] as f64;
                    xx += 1;
                }
                yy += 1;
            }
            dst[y * dw + x] = (s / (sx * sy)) as f32;
        }
    }
    dst
}

/// Bilinear resize with half-pixel centres.
pub fn resize_linear(src: &[f32], w: usize, h: usize, dw: usize, dh: usize) -> Vec<f32> {
    let axis = |i: usize, s: f64, n: usize| {
        let f = (i as f64 + 0.5) * s - 0.5;
        let mut i0 = f.floor() as i64;
        let mut t = (f - i0 as f64) as f32;
        if i0 < 0 {
            i0 = 0;
            t = 0.0;
        }
        if i0 >= n as i64 - 1 {
            i0 = n as i64 - 1;
            t = 0.0;
        }
        (i0 as usize, ((i0 + 1) as usize).min(n - 1), t)
    };
    let (sx, sy) = (w as f64 / dw as f64, h as f64 / dh as f64);
    let xs: Vec<(usize, usize, f32)> = (0..dw).map(|x| axis(x, sx, w)).collect();
    let mut dst = vec![0f32; dw * dh];
    for y in 0..dh {
        let (iy, iy1, ty) = axis(y, sy, h);
        let (r0, r1) = (&src[iy * w..], &src[iy1 * w..]);
        for (x, &(a, b, t)) in xs.iter().enumerate() {
            let top = r0[a] * (1.0 - t) + r0[b] * t;
            let bot = r1[a] * (1.0 - t) + r1[b] * t;
            dst[y * dw + x] = top * (1.0 - ty) + bot * ty;
        }
    }
    dst
}

/// v1's `light.blur`: Gaussian, downsampling first when sigma >= 8.
pub fn blur(img: &[f32], w: usize, h: usize, sigma: f64) -> Vec<f32> {
    if sigma <= 0.0 {
        return img.to_vec();
    }
    if sigma < 8.0 {
        return gaussian_blur(img, w, h, sigma);
    }
    let f = (sigma / 4.0).ceil() as usize;
    let (sw, sh) = ((w / f).max(2), (h / f).max(2));
    let small = resize_area(img, w, h, sw, sh);
    let small = gaussian_blur(&small, sw, sh, sigma / f as f64);
    resize_linear(&small, sw, sh, w, h)
}

/// 3x3 Sobel derivative along x (`dx = true`) or y, reflect-101 borders.
pub fn sobel(src: &[f32], w: usize, h: usize, dx: bool) -> Vec<f32> {
    let mut out = vec![0f32; w * h];
    for y in 0..h {
        let ym = reflect101(y as i64 - 1, h as i64);
        let yp = reflect101(y as i64 + 1, h as i64);
        for x in 0..w {
            let xm = reflect101(x as i64 - 1, w as i64);
            let xp = reflect101(x as i64 + 1, w as i64);
            let v = |yy: usize, xx: usize| src[yy * w + xx];
            out[y * w + x] = if dx {
                (v(ym, xp) - v(ym, xm)) + 2.0 * (v(y, xp) - v(y, xm)) + (v(yp, xp) - v(yp, xm))
            } else {
                (v(yp, xm) - v(ym, xm)) + 2.0 * (v(yp, x) - v(ym, x)) + (v(yp, xp) - v(ym, xp))
            };
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_keeps_the_mean_and_constants() {
        let (w, h) = (37, 23);
        let img: Vec<f32> = (0..w * h).map(|i| ((i * 7919) % 101) as f32 / 100.0).collect();
        let k = gaussian_kernel(2.0);
        assert_eq!(k.len(), 17);
        assert!((k.iter().sum::<f32>() - 1.0).abs() < 1e-6);
        let flat = vec![0.25f32; w * h];
        assert!(blur(&flat, w, h, 3.0).iter().all(|v| (v - 0.25).abs() < 1e-6));
        assert!(blur(&flat, w, h, 12.0).iter().all(|v| (v - 0.25).abs() < 1e-5));
        let b = gaussian_blur(&img, w, h, 1.5);
        let (m0, m1) = (img.iter().sum::<f32>() / img.len() as f32, b.iter().sum::<f32>() / b.len() as f32);
        assert!((m0 - m1).abs() < 0.02);
        assert_eq!(resize_area(&img, w, h, w, h), img);
    }

    #[test]
    fn sobel_of_a_ramp() {
        let (w, h) = (8, 6);
        let img: Vec<f32> = (0..w * h).map(|i| (i % w) as f32).collect();
        let g = sobel(&img, w, h, true);
        assert_eq!(g[2 * w + 3], 8.0);
        assert!(sobel(&img, w, h, false).iter().all(|v| *v == 0.0));
    }
}
