import sys, os, ctypes, numpy as np, cv2
sys.path.insert(0, "oilpaint-renderer"); sys.path.insert(0, "exp")
from oilpaint.render import load_strokes
from oilpaint.canvas import Canvas, Stroke
from oilpaint import light as L
from replay_level import ECanvas, kl   # prototype kernel with caps/edge ramp
strokes, ranges = load_strokes("oilpaint-renderer/out/v3/strokes.npz")
rng = np.random.default_rng(3)
pick = []
for li in (12, 15, 9):      # wave crests/foam, impasto touches, broken colour
    a, b = ranges[li - 1]; idx = rng.choice(np.arange(a, b), 8, replace=False); pick += [strokes[i] for i in idx]
W, H = 2400, 420
def run(cap, ramp):
    kl.set_cap(cap); kl.set_edge_ramp(ramp)
    cv = ECanvas(W, H, (0.35, 0.40, 0.62))
    for k, s in enumerate(pick):
        p = s.pts.copy(); c = p[:, :2].mean(0)
        gx, gy = k % 12, k // 12
        p[:, 0] += 0.04 + gx * 0.08 - c[0]; p[:, 1] += 0.035 + gy * 0.06 - c[1]
        st = Stroke(p, s.zcol, s.dz, s.params, s.seed, zcol2=s.zcol2)
        cv.render_stroke(st)
    return L.relight(cv.rgb, cv.h)
kl.set_cap.argtypes = [ctypes.c_float]
a = run(0.0, 0.0); b = run(1.0, 0.0); c = run(1.0, 0.3)
out = np.concatenate([L.to8(a), np.full((6, W, 3), 255, np.uint8), L.to8(b), np.full((6, W, 3), 255, np.uint8), L.to8(c)], 0)
cv2.imwrite("look/dabs_caps.png", cv2.cvtColor(out, cv2.COLOR_RGB2BGR))
# a zoomed excerpt: first 6 dabs of each row set
z = np.concatenate([L.to8(a)[:, :1150], np.full((H, 6, 3), 255, np.uint8), L.to8(b)[:, :1150]], 1)
cv2.imwrite("look/dabs_caps_zoom.png", cv2.cvtColor(z[0:200], cv2.COLOR_RGB2BGR))
print("ok")

# ---- hypothesis: facets come from piecewise-linear width/position between few points -> densify with smooth splines
from scipy.interpolate import PchipInterpolator, CubicSpline
from oilpaint.strokes import profile
def densify(p, factor=5):
    seg = np.hypot(np.diff(p[:, 0]), np.diff(p[:, 1])); s = np.concatenate([[0], np.cumsum(seg)])
    keep = np.concatenate([[True], seg > 1e-7]); p = p[keep]; s = s[keep]
    if len(p) < 3: return p
    t = s / s[-1]; tn = np.linspace(0, 1, (len(p) - 1) * factor + 1)
    x = CubicSpline(t, p[:, 0])(tn); y = CubicSpline(t, p[:, 1])(tn)
    wmax = p[:, 2].max(); wf, _ = profile(tn); w = PchipInterpolator(t, p[:, 2])(tn)
    pr = PchipInterpolator(t, p[:, 3])(tn)
    return np.stack([x, y, w, pr], 1).astype(np.float32)
def run2(cap, dens):
    kl.set_cap(cap); kl.set_edge_ramp(0.0)
    cv = ECanvas(W, H, (0.35, 0.40, 0.62))
    for k, s in enumerate(pick):
        p = s.pts.copy(); c = p[:, :2].mean(0)
        gx, gy = k % 12, k // 12
        p[:, 0] += 0.04 + gx * 0.08 - c[0]; p[:, 1] += 0.035 + gy * 0.06 - c[1]
        if dens: p = densify(p)
        cv.render_stroke(Stroke(p, s.zcol, s.dz, s.params, s.seed, zcol2=s.zcol2))
    return L.relight(cv.rgb, cv.h)
d = run2(1.0, True)
out = np.concatenate([L.to8(a), np.full((6, W, 3), 255, np.uint8), L.to8(d)], 0)
cv2.imwrite("look/dabs_dense.png", cv2.cvtColor(out, cv2.COLOR_RGB2BGR))
print("npts before/after", np.mean([len(s.pts) for s in pick]))
