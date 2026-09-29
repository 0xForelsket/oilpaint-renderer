//! Planner hot loops in Rust (native + wasm32): error map, palette snap, path tracing.  Same algorithms as spike_js.mjs.
use std::f64::consts::PI;
static mut LINLUT: [f32; 65537] = [0.0; 65537];
fn srgb2lin_exact(c: f64) -> f64 { if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) } }
#[no_mangle] pub extern "C" fn ps_init() { unsafe { for i in 0..=65536 { LINLUT[i] = srgb2lin_exact(i as f64 / 65536.0) as f32; } } }
#[inline(always)] fn srgb2lin(c: f32) -> f32 {
    let x = c.clamp(0.0, 1.0) * 65536.0; let i = x as usize; let t = x - i as f32;
    unsafe { if i >= 65536 { LINLUT[65536] } else { LINLUT[i] + t * (LINLUT[i + 1] - LINLUT[i]) } }
}
#[inline(always)] fn flab(t: f32) -> f32 { if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 } }
#[inline(always)] fn rgb2lab(r: f32, g: f32, b: f32) -> [f32; 3] {
    let (lr, lg, lb) = (srgb2lin(r), srgb2lin(g), srgb2lin(b));
    let x = flab((0.4124564 * lr + 0.3575761 * lg + 0.1804375 * lb) / 0.95047);
    let y = flab(0.2126729 * lr + 0.7151522 * lg + 0.0721750 * lb);
    let z = flab((0.0193339 * lr + 0.1191920 * lg + 0.9503041 * lb) / 1.08883);
    [116.0 * y - 16.0, 500.0 * (x - y), 200.0 * (y - z)]
}
const COEF: [[f32; 3]; 20] = [[0.07717053, 0.02826978, 0.24832992], [0.95912302, 0.80256528, 0.03561839], [0.74683774, 0.04868586, 0.0], [0.99518138, 0.99978149, 0.99704802], [0.04819146, 0.83363781, 0.32515377], [-0.68146950, 1.46107803, 1.06980936], [0.27058419, -0.15324870, 1.98735057], [0.80478189, 0.67093710, 0.18424500], [-0.35031003, 1.37855826, 3.68865000], [1.05128046, 1.97815239, 2.82989073], [3.21607125, 0.81270228, 1.03384539], [2.78893374, 0.41565549, -0.04487295], [3.02162577, 2.55374103, 0.32766114], [2.95124691, 2.81201112, 1.17578442], [2.82677043, 0.79933038, 1.81715262], [2.99691099, 1.22593053, 1.80653661], [1.87394106, 2.05027182, -0.29835996], [2.56609566, 7.03428198, 0.62575374], [4.08329484, -1.40408358, 2.14995522], [6.00078678, 2.55552042, 1.90739502]];
fn poly(c0: f32, c1: f32, c2: f32, c3: f32) -> [f32; 3] {
    let (c00, c11, c22, c33, c01, c02, c12) = (c0 * c0, c1 * c1, c2 * c2, c3 * c3, c0 * c1, c0 * c2, c1 * c2);
    let t = [c0 * c00, c1 * c11, c2 * c22, c3 * c33, c00 * c1, c01 * c1, c00 * c2, c02 * c2, c00 * c3, c0 * c33, c11 * c2, c1 * c22, c11 * c3, c1 * c33, c22 * c3, c2 * c33, c01 * c2, c01 * c3, c02 * c3, c12 * c3];
    let mut o = [0f32; 3]; for i in 0..20 { for k in 0..3 { o[k] += t[i] * COEF[i][k]; } } o
}
fn rgb2latent(r: f32, g: f32, b: f32, lut: &[u8]) -> [f32; 7] {
    let (x, y, z) = (r * 63.0, g * 63.0, b * 63.0);
    let (ix, iy, iz) = ((x as usize).min(62), (y as usize).min(62), (z as usize).min(62));
    let (tx, ty, tz) = (x - ix as f32, y - iy as f32, z - iz as f32);
    let base = ix + iy * 64 + iz * 4096; let (mut c0, mut c1, mut c2) = (0f32, 0f32, 0f32);
    for k in 0..8 { let (dx, dy, dz) = (k & 1, (k >> 1) & 1, k >> 2);
        let w = (if dx == 1 { tx } else { 1.0 - tx }) * (if dy == 1 { ty } else { 1.0 - ty }) * (if dz == 1 { tz } else { 1.0 - tz });
        let i = base + dx + dy * 64 + dz * 4096; c0 += w * lut[i + 192] as f32; c1 += w * lut[i + 262336] as f32; c2 += w * lut[i + 524480] as f32; }
    c0 /= 255.0; c1 /= 255.0; c2 /= 255.0; let c3 = 1.0 - c0 - c1 - c2; let p = poly(c0, c1, c2, c3);
    [c0, c1, c2, c3, r - p[0], g - p[1], b - p[2]]
}
#[no_mangle] pub unsafe extern "C" fn ps_error_cells(rf: *const f32, px: *const f32, mk: *const f32, w: usize, h: usize, g: usize, t: f32, e: *mut f32, out: *mut u32) -> u32 {
    let n = w * h; let rf = std::slice::from_raw_parts(rf, n * 3); let px = std::slice::from_raw_parts(px, n * 3);
    let mk = std::slice::from_raw_parts(mk, n); let e = std::slice::from_raw_parts_mut(e, n);
    for i in 0..n { let a = rgb2lab(px[3 * i], px[3 * i + 1], px[3 * i + 2]); let b = rgb2lab(rf[3 * i], rf[3 * i + 1], rf[3 * i + 2]);
        let (d0, d1, d2) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]); e[i] = (d0 * d0 + d1 * d1 + d2 * d2).sqrt() * mk[i]; }
    let (gw, gh) = (w / g, h / g); let mut k = 0u32;
    for cy in 0..gh { for cx in 0..gw { let (mut s, mut mx, mut mi, mut cov) = (0f32, -1f32, 0usize, 0f32);
        for y in cy * g..cy * g + g { for x in cx * g..cx * g + g { let v = e[y * w + x]; s += v; cov += mk[y * w + x]; if v > mx { mx = v; mi = y * w + x; } } }
        let cell = 0.6 * s / (g * g) as f32 + 0.4 * mx;
        if cell > t && cov / ((g * g) as f32) > 0.3 { *out.add(k as usize) = mi as u32; k += 1; } } }
    k
}
#[no_mangle] pub unsafe extern "C" fn ps_snap(q: *const f32, n: usize, pal_lab: *const f32, pal_lat: *const f32, np: usize, lut: *const u8, amount: f32, out: *mut f32) {
    let q = std::slice::from_raw_parts(q, n * 3); let pl = std::slice::from_raw_parts(pal_lab, np * 3); let pz = std::slice::from_raw_parts(pal_lat, np * 7);
    let lut = std::slice::from_raw_parts(lut, 786432 + 192); let out = std::slice::from_raw_parts_mut(out, n * 3);
    for j in 0..n { let (r, g, b) = (q[3 * j], q[3 * j + 1], q[3 * j + 2]); let l = rgb2lab(r, g, b);
        let (mut best, mut bd) = (0usize, f32::INFINITY);
        for k in 0..np { let (a, c, d) = (pl[3 * k] - l[0], pl[3 * k + 1] - l[1], pl[3 * k + 2] - l[2]); let e = a * a + c * c + d * d; if e < bd { bd = e; best = k; } }
        let zs = rgb2latent(r, g, b, lut); let mut z = [0f32; 7]; for k in 0..7 { z[k] = zs[k] + amount * (pz[7 * best + k] - zs[k]); }
        let p = poly(z[0], z[1], z[2], z[3]); out[3 * j] = p[0] + z[4]; out[3 * j + 1] = p[1] + z[5]; out[3 * j + 2] = p[2] + z[6]; }
}
struct Pcg(u64);
impl Pcg { fn u32(&mut self) -> u32 { let o = self.0; self.0 = o.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let x = (((o >> 18) ^ o) >> 27) as u32; let r = (o >> 59) as u32; x.rotate_right(r) }
    fn f(&mut self) -> f64 { self.u32() as f64 / 4294967296.0 } }
#[no_mangle] pub unsafe extern "C" fn ps_paths(jobs: *const f32, nj: usize, flow: *const f32, mk: *const f32, w: usize, h: usize, align: f64, curv: f64, stop: f64, seed: u64, pts: *mut f32) -> u64 {
    let jobs = std::slice::from_raw_parts(jobs, nj * 4); let fl = std::slice::from_raw_parts(flow, w * h * 2); let mk = std::slice::from_raw_parts(mk, w * h);
    let pts = std::slice::from_raw_parts_mut(pts, 4096); let mut rng = Pcg(seed | 1); let mut spare: Option<f64> = None; let mut total = 0u64;
    let mut normal = |rng: &mut Pcg, s: f64| -> f64 { if let Some(v) = spare.take() { return v * s; } let mut u = 0.0; while u == 0.0 { u = rng.f(); } let v = rng.f();
        let r = (-2.0 * u.ln()).sqrt(); spare = Some(r * (2.0 * PI * v).sin()); r * (2.0 * PI * v).cos() * s };
    let flow_at = |x: f64, y: f64| { let xi = (x as i64).clamp(0, w as i64 - 1) as usize; let yi = (y as i64).clamp(0, h as i64 - 1) as usize; let i = 2 * (yi * w + xi); (fl[i] as f64, fl[i + 1] as f64) };
    for j in 0..nj { let (mut x0, mut y0, wpx, lpx) = (jobs[4 * j] as f64, jobs[4 * j + 1] as f64, jobs[4 * j + 2] as f64, jobs[4 * j + 3] as f64);
        let step = (0.5 * wpx).max(1.0); let mut n = ((lpx / step) as i64 + 1).max(3);
        let (fx, fy) = flow_at(x0, y0); let dn = fx.hypot(fy) + 1e-9; let (mut dx, mut dy) = (fx / dn, fy / dn);
        let jit = (1.0 - align) * normal(&mut rng, 0.7); let (c, s) = (jit.cos(), jit.sin()); let (ndx, ndy) = (c * dx - s * dy, s * dx + c * dy); dx = ndx; dy = ndy;
        let back = (0.5 + 0.5 * rng.f()) * wpx; x0 -= dx * back; y0 -= dy * back; n += (back / step) as i64 + 1;
        let (mut x, mut y) = (x0, y0); let mut np_ = 1usize; pts[0] = x as f32; pts[1] = y as f32;
        for _ in 1..n { let (mut fx, mut fy) = flow_at(x, y); if fx * dx + fy * dy < 0.0 { fx = -fx; fy = -fy; }
            let (mut nx, mut ny) = ((1.0 - curv) * dx + curv * fx, (1.0 - curv) * dy + curv * fy); let nn = nx.hypot(ny) + 1e-9; nx /= nn; ny /= nn;
            let wob = normal(&mut rng, 0.08 * (1.0 - align) + 0.02); let (cw, sw) = (wob.cos(), wob.sin()); dx = cw * nx - sw * ny; dy = sw * nx + cw * ny;
            x += dx * step; y += dy * step;
            if x < -wpx || y < -wpx || x > w as f64 + wpx || y > h as f64 + wpx { break; }
            let xi = (x as i64).clamp(0, w as i64 - 1) as usize; let yi = (y as i64).clamp(0, h as i64 - 1) as usize;
            if (mk[yi * w + xi] as f64) < rng.f() * stop { break; }
            if np_ < 2047 { pts[2 * np_] = x as f32; pts[2 * np_ + 1] = y as f32; np_ += 1; } }
        total += np_ as u64; }
    total
}
#[no_mangle] pub extern "C" fn ps_alloc(n: usize) -> *mut u8 { let mut v: Vec<u8> = Vec::with_capacity(n.max(1)); let p = v.as_mut_ptr(); std::mem::forget(v); p }
