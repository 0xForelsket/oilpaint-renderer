// Planner hot loops in plain JS (typed arrays): error map, palette snap, flow-following path tracing.
// Same algorithms as oilpaint/planner.py; RNG is our own (sfc32 + Box-Muller), so results match Python statistically only.
import fs from 'fs';
const D = new URL('./data/', import.meta.url).pathname;
const meta = JSON.parse(fs.readFileSync(D + 'meta.json'));
const f32 = (n) => { const b = fs.readFileSync(D + n + '.bin'); return new Float32Array(b.buffer, b.byteOffset, b.length / 4); };
const { W, H } = meta;
const ref = f32('ref'), proxy = f32('proxy'), mask = f32('mask'), flow = f32('flow');
const palLat = f32('pal_lat'), palLab = f32('pal_lab'), jobs = f32('jobs'), queries = f32('queries');
const lutB = fs.readFileSync(D + 'lut.bin'); const LUT = new Float32Array(lutB.length); for (let i = 0; i < lutB.length; i++) LUT[i] = lutB[i];

// ---------------- colour
const srgb2linExact = (c) => (c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4));
const LINLUT = new Float32Array(65537); for (let i = 0; i <= 65536; i++) LINLUT[i] = srgb2linExact(i / 65536);
const USE_LUT = process.env.LUT === '1';
const srgb2lin = USE_LUT ? (c) => { const x = (c < 0 ? 0 : c > 1 ? 1 : c) * 65536, i = x | 0, t = x - i; return i >= 65536 ? LINLUT[65536] : LINLUT[i] + t * (LINLUT[i + 1] - LINLUT[i]); } : srgb2linExact;
const M = [0.4124564, 0.3575761, 0.1804375, 0.2126729, 0.7151522, 0.0721750, 0.0193339, 0.1191920, 0.9503041];
const WH = [0.95047, 1.0, 1.08883];
const fLab = (t) => (t > 0.008856 ? Math.cbrt(t) : 7.787 * t + 16 / 116);
function rgb2lab(r, g, b, out, o) {
  const lr = srgb2lin(r), lg = srgb2lin(g), lb = srgb2lin(b);
  const x = fLab((M[0] * lr + M[1] * lg + M[2] * lb) / WH[0]), y = fLab((M[3] * lr + M[4] * lg + M[5] * lb) / WH[1]), z = fLab((M[6] * lr + M[7] * lg + M[8] * lb) / WH[2]);
  out[o] = 116 * y - 16; out[o + 1] = 500 * (x - y); out[o + 2] = 200 * (y - z);
}
const COEF = [[0.07717053, 0.02826978, 0.24832992], [0.95912302, 0.80256528, 0.03561839], [0.74683774, 0.04868586, 0.0], [0.99518138, 0.99978149, 0.99704802], [0.04819146, 0.83363781, 0.32515377], [-0.68146950, 1.46107803, 1.06980936], [0.27058419, -0.15324870, 1.98735057], [0.80478189, 0.67093710, 0.18424500], [-0.35031003, 1.37855826, 3.68865000], [1.05128046, 1.97815239, 2.82989073], [3.21607125, 0.81270228, 1.03384539], [2.78893374, 0.41565549, -0.04487295], [3.02162577, 2.55374103, 0.32766114], [2.95124691, 2.81201112, 1.17578442], [2.82677043, 0.79933038, 1.81715262], [2.99691099, 1.22593053, 1.80653661], [1.87394106, 2.05027182, -0.29835996], [2.56609566, 7.03428198, 0.62575374], [4.08329484, -1.40408358, 2.14995522], [6.00078678, 2.55552042, 1.90739502]].flat();
function poly(c0, c1, c2, c3, o) {
  const c00 = c0 * c0, c11 = c1 * c1, c22 = c2 * c2, c33 = c3 * c3, c01 = c0 * c1, c02 = c0 * c2, c12 = c1 * c2;
  const t = [c0 * c00, c1 * c11, c2 * c22, c3 * c33, c00 * c1, c01 * c1, c00 * c2, c02 * c2, c00 * c3, c0 * c33, c11 * c2, c1 * c22, c11 * c3, c1 * c33, c22 * c3, c2 * c33, c01 * c2, c01 * c3, c02 * c3, c12 * c3];
  let r = 0, g = 0, b = 0; for (let i = 0; i < 20; i++) { r += t[i] * COEF[3 * i]; g += t[i] * COEF[3 * i + 1]; b += t[i] * COEF[3 * i + 2]; }
  o[0] = r; o[1] = g; o[2] = b;
}
const P = [0, 0, 0];
function rgb2latent(r, g, b, out) {
  const x = r * 63, y = g * 63, z = b * 63;
  const ix = Math.min(x | 0, 62), iy = Math.min(y | 0, 62), iz = Math.min(z | 0, 62), tx = x - ix, ty = y - iy, tz = z - iz;
  const base = ix + iy * 64 + iz * 4096; let c0 = 0, c1 = 0, c2 = 0;
  for (let k = 0; k < 8; k++) {
    const dx = k & 1, dy = (k >> 1) & 1, dz = k >> 2;
    const w = (dx ? tx : 1 - tx) * (dy ? ty : 1 - ty) * (dz ? tz : 1 - tz), i = base + dx + dy * 64 + dz * 4096;
    c0 += w * LUT[i + 192]; c1 += w * LUT[i + 262336]; c2 += w * LUT[i + 524480];
  }
  c0 /= 255; c1 /= 255; c2 /= 255; const c3 = 1 - c0 - c1 - c2; poly(c0, c1, c2, c3, P);
  out[0] = c0; out[1] = c1; out[2] = c2; out[3] = c3; out[4] = r - P[0]; out[5] = g - P[1]; out[6] = b - P[2];
}

// ---------------- error map (same cells as planner._error_cells; block mean instead of cv2 INTER_AREA)
const labRef = new Float32Array(W * H * 3), labCan = new Float32Array(W * H * 3), E = new Float32Array(W * H);
function errorCells(g, T) {
  for (let i = 0; i < W * H; i++) { rgb2lab(proxy[3 * i], proxy[3 * i + 1], proxy[3 * i + 2], labCan, 3 * i); rgb2lab(ref[3 * i], ref[3 * i + 1], ref[3 * i + 2], labRef, 3 * i); }
  for (let i = 0; i < W * H; i++) { const a = labCan[3 * i] - labRef[3 * i], b = labCan[3 * i + 1] - labRef[3 * i + 1], c = labCan[3 * i + 2] - labRef[3 * i + 2]; E[i] = Math.sqrt(a * a + b * b + c * c) * mask[i]; }
  const gw = (W / g) | 0, gh = (H / g) | 0, out = [];
  for (let cy = 0; cy < gh; cy++) for (let cx = 0; cx < gw; cx++) {
    let s = 0, mx = -1, mi = 0, cov = 0;
    for (let y = cy * g; y < cy * g + g; y++) for (let x = cx * g; x < cx * g + g; x++) { const e = E[y * W + x]; s += e; cov += mask[y * W + x]; if (e > mx) { mx = e; mi = y * W + x; } }
    const cell = 0.6 * s / (g * g) + 0.4 * mx;
    if (cell > T && cov / (g * g) > 0.3) out.push(mi);
  }
  return out;
}
// ---------------- palette snap (brute-force nearest in Lab; 729 candidates)
const nPal = meta.n_pal, lab = new Float32Array(3), zs = new Float64Array(7), zo = [0, 0, 0];
function snap(r, g, b, amount, out) {
  rgb2lab(r, g, b, lab, 0); let best = 0, bd = Infinity;
  for (let k = 0; k < nPal; k++) { const a = palLab[3 * k] - lab[0], c = palLab[3 * k + 1] - lab[1], d = palLab[3 * k + 2] - lab[2], e = a * a + c * c + d * d; if (e < bd) { bd = e; best = k; } }
  rgb2latent(r, g, b, zs);
  const z = [0, 0, 0, 0, 0, 0, 0]; for (let k = 0; k < 7; k++) z[k] = zs[k] + amount * (palLat[7 * best + k] - zs[k]);
  poly(z[0], z[1], z[2], z[3], zo); out[0] = zo[0] + z[4]; out[1] = zo[1] + z[5]; out[2] = zo[2] + z[6];
}
// ---------------- RNG + path tracing
function sfc32(a, b, c, d) { return () => { a |= 0; b |= 0; c |= 0; d |= 0; const t = (((a + b) | 0) + d) | 0; d = (d + 1) | 0; a = b ^ (b >>> 9); b = (c + (c << 3)) | 0; c = (c << 21) | (c >>> 11); c = (c + t) | 0; return (t >>> 0) / 4294967296; }; }
const rnd = sfc32(1, 2, 3, 7); let spare = null;
const normal = (s) => { if (spare !== null) { const v = spare; spare = null; return v * s; } let u = 0, v = 0; while (u === 0) u = rnd(); v = rnd(); const r = Math.sqrt(-2 * Math.log(u)); spare = r * Math.sin(2 * Math.PI * v); return r * Math.cos(2 * Math.PI * v) * s; };
const align = 0.80, curv = 0.35, stopEdge = 0.9;
const flowAt = (x, y, o) => { const xi = Math.min(Math.max(x | 0, 0), W - 1), yi = Math.min(Math.max(y | 0, 0), H - 1), i = 2 * (yi * W + xi); o[0] = flow[i]; o[1] = flow[i + 1]; };
const fv = [0, 0], pts = new Float32Array(4096);
function path(x0, y0, wpx, lpx) {
  const step = Math.max(1, 0.5 * wpx); let n = Math.max(3, ((lpx / step) | 0) + 1);
  flowAt(x0, y0, fv); let dn = Math.hypot(fv[0], fv[1]) + 1e-9, dx = fv[0] / dn, dy = fv[1] / dn;
  const jit = (1 - align) * normal(0.7), c = Math.cos(jit), s = Math.sin(jit); [dx, dy] = [c * dx - s * dy, s * dx + c * dy];
  const back = (0.5 + 0.5 * rnd()) * wpx; x0 -= dx * back; y0 -= dy * back; n += ((back / step) | 0) + 1;
  let x = x0, y = y0, np_ = 1; pts[0] = x; pts[1] = y;
  for (let i = 1; i < n; i++) {
    flowAt(x, y, fv); let fx = fv[0], fy = fv[1]; if (fx * dx + fy * dy < 0) { fx = -fx; fy = -fy; }
    let nx = (1 - curv) * dx + curv * fx, ny = (1 - curv) * dy + curv * fy; const nn = Math.hypot(nx, ny) + 1e-9; nx /= nn; ny /= nn;
    const wob = normal(0.08 * (1 - align) + 0.02), cw = Math.cos(wob), sw = Math.sin(wob); dx = cw * nx - sw * ny; dy = sw * nx + cw * ny;
    x += dx * step; y += dy * step;
    if (x < -wpx || y < -wpx || x > W + wpx || y > H + wpx) break;
    const xi = Math.min(Math.max(x | 0, 0), W - 1), yi = Math.min(Math.max(y | 0, 0), H - 1);
    if (mask[yi * W + xi] < rnd() * stopEdge) break;
    if (np_ < 2047) { pts[2 * np_] = x; pts[2 * np_ + 1] = y; np_++; }
  }
  return np_;
}
// ---------------- run
for (let rep = 0; rep < 2; rep++) {
  let t0 = performance.now(), cells = 0;
  for (let i = 0; i < 27; i++) cells += errorCells(8 + (i % 5) * 3, 12).length;
  const tErr = (performance.now() - t0) / 1000;
  t0 = performance.now(); const o = [0, 0, 0]; let acc = 0;
  for (let q = 0; q < meta.n_q; q++) { snap(queries[3 * q], queries[3 * q + 1], queries[3 * q + 2], 0.85, o); acc += o[0]; }
  const tSnap = (performance.now() - t0) / 1000;
  t0 = performance.now(); let tot = 0;
  for (let j = 0; j < meta.n_jobs; j++) tot += path(jobs[4 * j], jobs[4 * j + 1], jobs[4 * j + 2], jobs[4 * j + 3]);
  const tPath = (performance.now() - t0) / 1000;
  console.log(`JS(node ${process.version}${USE_LUT ? ', sRGB LUT' : ''}) rep ${rep}: error map x27 ${tErr.toFixed(3)}s (${cells} cells) | snap x${meta.n_q} ${tSnap.toFixed(3)}s | paths x${meta.n_jobs} ${tPath.toFixed(3)}s (${tot} points)`);
}
