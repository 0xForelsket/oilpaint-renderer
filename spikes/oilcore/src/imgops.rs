//! Minimal image ops matching the OpenCV calls used by light.py / render.py (float32, single channel):
//! GaussianBlur (ksize from sigma like cv2: round(sigma*8+1)|1, BORDER_REFLECT_101), resize INTER_AREA
//! (integer factors exact box mean; otherwise area weights), resize INTER_LINEAR (half-pixel centres), Sobel 3x3.
//! Not bit-identical to OpenCV (its SIMD sums in another order); parity with Python is "within tolerance".

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

pub fn gaussian_kernel(sigma: f64) -> Vec<f32> {
    let n = (((sigma * 8.0 + 1.0) + 0.5).floor() as i64) | 1;
    let n = n.max(1) as usize;
    let mut k = vec![0f64; n];
    let s2 = -0.5 / (sigma * sigma);
    let mut sum = 0.0;
    for i in 0..n {
        let x = i as f64 - (n as f64 - 1.0) * 0.5;
        k[i] = (s2 * x * x).exp();
        sum += k[i];
    }
    k.iter().map(|v| (v / sum) as f32).collect()
}

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
        for x in 0..w {
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
            out[x] = acc;
        }
    }
    let mut dst = vec![0f32; w * h];
    let mut acc = vec![0f32; w];
    for y in 0..h {
        for v in acc.iter_mut() {
            *v = 0.0;
        }
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

pub fn resize_area(src: &[f32], w: usize, h: usize, dw: usize, dh: usize) -> Vec<f32> {
    let mut dst = vec![0f32; dw * dh];
    if w % dw == 0 && h % dh == 0 {
        let fx = w / dw;
        let fy = h / dh;
        let inv = 1.0f32 / (fx * fy) as f32;
        for y in 0..dh {
            for x in 0..dw {
                let mut s = 0f32;
                for yy in 0..fy {
                    let row = &src[(y * fy + yy) * w + x * fx..];
                    for xx in 0..fx {
                        s += row[xx];
                    }
                }
                dst[y * dw + x] = s * inv;
            }
        }
        return dst;
    }
    // generic area weights
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

pub fn resize_linear(src: &[f32], w: usize, h: usize, dw: usize, dh: usize) -> Vec<f32> {
    let sx = w as f64 / dw as f64;
    let sy = h as f64 / dh as f64;
    let xs: Vec<(usize, usize, f32)> = (0..dw)
        .map(|x| {
            let fx = (x as f64 + 0.5) * sx - 0.5;
            let mut ix = fx.floor() as i64;
            let mut t = (fx - ix as f64) as f32;
            if ix < 0 {
                ix = 0;
                t = 0.0;
            }
            if ix >= w as i64 - 1 {
                ix = w as i64 - 1;
                t = 0.0;
            }
            let ix1 = ((ix + 1) as usize).min(w - 1);
            (ix as usize, ix1, t)
        })
        .collect();
    let mut dst = vec![0f32; dw * dh];
    for y in 0..dh {
        let fy = (y as f64 + 0.5) * sy - 0.5;
        let mut iy = fy.floor() as i64;
        let mut ty = (fy - iy as f64) as f32;
        if iy < 0 {
            iy = 0;
            ty = 0.0;
        }
        if iy >= h as i64 - 1 {
            iy = h as i64 - 1;
            ty = 0.0;
        }
        let iy1 = ((iy + 1) as usize).min(h - 1);
        let r0 = &src[iy as usize * w..];
        let r1 = &src[iy1 * w..];
        for x in 0..dw {
            let (a, b, t) = xs[x];
            let top = r0[a] * (1.0 - t) + r0[b] * t;
            let bot = r1[a] * (1.0 - t) + r1[b] * t;
            dst[y * dw + x] = top * (1.0 - ty) + bot * ty;
        }
    }
    dst
}

/// light.blur(): Gaussian, downsampling first for sigma >= 8 (as light.py does)
pub fn blur(img: &[f32], w: usize, h: usize, sigma: f64) -> Vec<f32> {
    if sigma <= 0.0 {
        return img.to_vec();
    }
    if sigma < 8.0 {
        return gaussian_blur(img, w, h, sigma);
    }
    let f = (sigma / 4.0).ceil() as usize;
    let sw = (w / f).max(2);
    let sh = (h / f).max(2);
    let small = resize_area(img, w, h, sw, sh);
    let small = gaussian_blur(&small, sw, sh, sigma / f as f64);
    resize_linear(&small, sw, sh, w, h)
}

/// cv2.Sobel(ksize=3), dx or dy, BORDER_REFLECT_101
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
