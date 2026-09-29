"""T5: coarse-to-fine error-driven painting of a synthetic photo-like target, plan-once / replay-at-2x."""
import os
import sys
import time
import numpy as np
import cv2

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
sys.path.insert(0, ROOT)
from oilpaint import mix, light
from oilpaint.canvas import Canvas
from oilpaint.maps import GuideMaps, flow_preview
from oilpaint.planner import Planner, Style, Layer
from oilpaint.sheet import save_sheet


def synthetic_target(W, H):
    y, x = np.mgrid[0:H, 0:W].astype(np.float32)
    X, Y = x / W, y / W
    # gradient background: warm top-left to cool bottom-right
    top = np.array([0.93, 0.80, 0.62], np.float32); bot = np.array([0.35, 0.45, 0.70], np.float32)
    t = (0.6 * X + 0.6 * Y)[..., None]
    img = top * (1 - t) + bot * t
    # a lit sphere with a soft edge
    cx, cy, r = 0.5, 0.55, 0.26
    d = np.sqrt((X - cx) ** 2 + (Y - cy) ** 2)
    inside = np.clip((r - d) / 0.02, 0, 1)
    nx = (X - cx) / r; ny = (Y - cy) / r
    nz = np.sqrt(np.clip(1 - nx ** 2 - ny ** 2, 0, 1))
    L = np.array([-0.5, -0.6, 0.62]); L /= np.linalg.norm(L)
    ndl = np.clip(nx * L[0] + ny * L[1] + nz * L[2], 0, 1)
    sphere = np.array([0.55, 0.25, 0.30], np.float32)[None, None] * (0.25 + 0.75 * ndl)[..., None] + 0.4 * (ndl ** 30)[..., None]
    img = img * (1 - inside[..., None]) + sphere * inside[..., None]
    # a soft shadow
    sh = np.exp(-(((X - 0.58) ** 2) / 0.06 + ((Y - 0.86) ** 2) / 0.004)) * 0.35
    img = img * (1 - sh[..., None])
    return np.clip(img, 0, 1).astype(np.float32)


def t5(OUT, gate):
    print("T5 coarse-to-fine on a synthetic target")
    W, H = 400, 400
    target = synthetic_target(W, H)
    flow, aniso = __import__("oilpaint.maps", fromlist=["structure_flow"]).structure_flow(target, sigma_px=0.03 * W)
    g = GuideMaps.from_arrays(target, flow=flow)
    style = Style(colors=None, snap=0.0, width=(0.02, 0.03), length=(0.06, 0.16), curvature=0.5, align=0.9,
                  pickup=0.2, streak=0.5, jitter=(2.0, 1.5))
    layers = [Layer("L1 big", width=(0.10, 0.14), length=(0.2, 0.4), T=8, fs=0.6, fg=1.0),
              Layer("L2", width=(0.06, 0.08), length=(0.12, 0.25), T=10, fs=0.5),
              Layer("L3", width=(0.035, 0.045), length=(0.08, 0.16), T=12, fs=0.5),
              Layer("L4", width=(0.02, 0.026), length=(0.05, 0.10), T=14, fs=0.5),
              Layer("L5 fine", width=(0.010, 0.014), length=(0.03, 0.06), T=16, fs=0.5)]
    pl = Planner(g, {"all": style}, layers, seed=42, verbose=True)
    errs = []
    imgs = []
    t0 = time.perf_counter()
    for li, layer in enumerate(layers):
        pl.plan_layer(li, layer)
        e = np.linalg.norm(mix.rgb_to_lab(pl.proxy.rgb) - mix.rgb_to_lab(target), axis=-1).mean()
        errs.append(float(e))
        imgs.append((f"T5 after {layer['name']} (err {e:.1f})", light.to8(pl.proxy.rgb)))
        print(f"  {layer['name']}: {pl.layer_ranges[-1][1]-pl.layer_ranges[-1][0]} strokes, mean Lab err {e:.2f}")
    dt = time.perf_counter() - t0
    n = len(pl.strokes)
    print(f"  planned {n} strokes in {dt:.2f}s")
    gate("T5 mean Lab error decreases every layer", all(errs[i + 1] < errs[i] for i in range(len(errs) - 1)),
         " -> ".join(f"{e:.1f}" for e in errs))
    # strokes near the sphere edge should run along the tangent
    cx, cy, r = 0.5, 0.55, 0.26
    tang_err = []
    for s in pl.strokes:
        if s.layer < 2:
            continue
        p = s.pts[:, :2]
        mid = p[len(p) // 2]
        d = np.hypot(mid[0] - cx, mid[1] - cy)
        if abs(d - r) < 0.03 and len(p) > 2:
            v = p[-1] - p[0]; v /= (np.linalg.norm(v) + 1e-9)
            radial = np.array([mid[0] - cx, mid[1] - cy]); radial /= (np.linalg.norm(radial) + 1e-9)
            tang_err.append(abs(float(np.dot(v, radial))))   # 0 = tangent, 1 = radial
    frac = float(np.mean(np.array(tang_err) < 0.5)) if tang_err else 0.0
    gate("T5 strokes near the sphere edge follow the contour (>= 65% within 60 deg of tangent)", frac >= 0.65,
         f"{frac*100:.0f}% of {len(tang_err)} edge strokes")
    # plan-once / replay-at-2x: same strokes on a 800 px canvas, downsample, compare
    cv2x = Canvas(800, 800)
    cv2x.set_region(cv2.resize(g.region_id, (800, 800), interpolation=cv2.INTER_NEAREST))
    t0 = time.perf_counter(); npx = cv2x.render(pl.strokes); dt2 = time.perf_counter() - t0
    print(f"  replay at 800px: {npx} px in {dt2:.2f}s -> {npx/dt2/1e6:.1f} Mpix/s")
    down = cv2.resize(cv2x.rgb, (W, H), interpolation=cv2.INTER_AREA)
    diff = float(np.abs(down - pl.proxy.rgb).mean())
    gate("T5 replay at 2x matches the plan render (mean |diff| < 0.05)", diff < 0.05, f"mean abs diff {diff:.4f}")
    lit_hi = light.relight(cv2x.rgb, cv2x.h)
    imgs = [("T5 target", light.to8(target)), ("T5 flow", flow_preview(flow, W, H))] + imgs + \
           [("T5 replay 800px lit", light.to8(lit_hi)), ("T5 replay 800px crop", light.to8(lit_hi[300:500, 250:450]))]
    save_sheet(os.path.join(OUT, "T5_sheet.png"), imgs, cols=3, cell=(330, 330))
    light.save_png(os.path.join(OUT, "t5_replay_800_lit.png"), lit_hi)
    light.save_png(os.path.join(OUT, "t5_plan_400.png"), pl.proxy.rgb)
    return imgs[:1] + imgs[-3:-1]


if __name__ == "__main__":
    def gate(n, ok, d):
        print(f"  [{'PASS' if ok else 'FAIL'}] {n}: {d}")
    t5(os.path.join(ROOT, "out", "checkpoints"), gate)
