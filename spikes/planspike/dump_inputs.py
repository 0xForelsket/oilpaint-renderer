import sys, os, json, time, math, numpy as np
EC = "/tmp/claude-0/-home-claude/cd4531f6-3314-512b-9953-4f74d55c5a55/scratchpad/work_lib/eval_copy"; sys.path.insert(0, EC)
from oilpaint import mix, light as Lt
from oilpaint.scene import load_scene
from oilpaint.planner import Palette, Style
from oilpaint.render import load_strokes
S = load_scene(os.path.join(EC, "scenes/storm_v3.py"))
g = S.guides(600); W, H = g.W, g.H
ref = Lt.blur(g.target, 7.5).astype(np.float32)
proxy = np.load("/tmp/claude-0/-home-claude/cd4531f6-3314-512b-9953-4f74d55c5a55/scratchpad/work_engine/oilpaint-renderer/out/v3/unlit.npy").astype(np.float32)
mask = g.masks["sky"].astype(np.float32); flow = g.region_flows["sky"].astype(np.float32)
pal = Palette(S.styles["sky"]["colors"])
strokes, _ = load_strokes("/tmp/claude-0/-home-claude/cd4531f6-3314-512b-9953-4f74d55c5a55/scratchpad/work_engine/oilpaint-renderer/out/v3/strokes.npz")
jobs = []
rng = np.random.default_rng(5)
ys, xs = np.where(mask > 0.5)
for s in strokes:
    p = s.pts; w = float(p[:, 2].max()) * W; L = float(np.hypot(np.diff(p[:, 0]), np.diff(p[:, 1])).sum()) * W
    k = rng.integers(len(xs)); jobs.append((xs[k] + rng.random(), ys[k] + rng.random(), max(w, 1.0), max(L, 2.0)))
jobs += jobs[:1300]  # 13.8k paths as in the profile (strokes + gap-fill attempts)
jobs = np.array(jobs, np.float32)
queries = np.clip(proxy.reshape(-1, 3)[rng.integers(0, W * H, 12500)], 0, 1).astype(np.float32)
os.makedirs("data", exist_ok=True)
for k, v in dict(ref=ref, proxy=proxy, mask=mask, flow=flow, pal_lat=pal.lat.astype(np.float32), pal_lab=pal.lab.astype(np.float32),
                 lut=mix._LUT.astype(np.uint8), jobs=jobs, queries=queries).items():
    np.ascontiguousarray(v).tofile(f"data/{k}.bin")
json.dump(dict(W=W, H=H, n_pal=len(pal.lat), n_jobs=len(jobs), n_q=len(queries)), open("data/meta.json", "w"))
print("dumped", W, H, "palette", len(pal.lat), "jobs", len(jobs))

# ---- Python baselines (same algorithms as planner.py; unprofiled)
def error_cells(ref, rgb, m, g, T=12.0):
    E = np.linalg.norm(mix.rgb_to_lab(rgb) - mix.rgb_to_lab(ref), axis=-1) * m
    gh, gw = H // g, W // g
    import cv2
    cell = cv2.resize(E, (gw, gh), interpolation=cv2.INTER_AREA)
    Ep = E[:gh * g, :gw * g].reshape(gh, g, gw, g).max(axis=(1, 3))
    cell = 0.6 * cell + 0.4 * Ep
    cover = cv2.resize(m, (gw, gh), interpolation=cv2.INTER_AREA)
    ys, xs = np.where((cell > T) & (cover > 0.3)); out = []
    for cy, cx in zip(ys, xs):
        patch = E[cy * g:cy * g + g, cx * g:cx * g + g]; iy, ix = np.unravel_index(np.argmax(patch), patch.shape); out.append((cx * g + ix, cy * g + iy))
    return out
t0 = time.perf_counter()
for i in range(27):
    n = len(error_cells(ref, proxy, mask, 8 + (i % 5) * 3))
t_err = time.perf_counter() - t0
t0 = time.perf_counter()
for q in queries:
    pal.snap(q, 0.85)
t_snap = time.perf_counter() - t0
st = Style(**S.styles["sky"]); rng = np.random.default_rng(7)
def flow_at(x, y):
    xi = min(max(int(x), 0), W - 1); yi = min(max(int(y), 0), H - 1); return flow[yi, xi]
def path(x0, y0, wpx, lpx):
    step = max(1.0, 0.5 * wpx); n = max(3, int(lpx / step) + 1)
    d = flow_at(x0, y0).copy(); d = d / (np.linalg.norm(d) + 1e-9)
    jit = (1 - st["align"]) * rng.normal(0, 0.7); c, s_ = math.cos(jit), math.sin(jit)
    d = np.array([c * d[0] - s_ * d[1], s_ * d[0] + c * d[1]], np.float32)
    fc = float(st["curvature"]); back = rng.uniform(0.5, 1.0) * wpx
    x0, y0 = x0 - d[0] * back, y0 - d[1] * back; n += int(back / step) + 1
    pts = [(x0, y0)]; x, y = x0, y0
    for i in range(1, n):
        f = flow_at(x, y)
        if np.dot(f, d) < 0: f = -f
        nd = (1 - fc) * d + fc * f; nd = nd / (np.linalg.norm(nd) + 1e-9)
        wob = rng.normal(0, 0.08 * (1 - st["align"]) + 0.02); c, s_ = math.cos(wob), math.sin(wob)
        d = np.array([c * nd[0] - s_ * nd[1], s_ * nd[0] + c * nd[1]], np.float32)
        x, y = x + d[0] * step, y + d[1] * step
        if x < -wpx or y < -wpx or x > W + wpx or y > H + wpx: break
        xi = min(max(int(x), 0), W - 1); yi = min(max(int(y), 0), H - 1)
        if mask[yi, xi] < rng.random() * st["stop_at_edge"]: break
        pts.append((x, y))
    return len(pts)
t0 = time.perf_counter(); tot = 0
for (x, y, w, l) in jobs:
    tot += path(x, y, w, l)
t_path = time.perf_counter() - t0
print(f"PYTHON 600px: error map x27 {t_err:.3f}s | snap x{len(queries)} {t_snap:.3f}s | paths x{len(jobs)} {t_path:.3f}s ({tot} points)")
