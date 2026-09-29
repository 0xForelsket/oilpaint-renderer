import sys, os, time, ctypes, numpy as np, cv2
EC = os.path.join(os.path.dirname(os.path.abspath(__file__)), "eval_copy"); sys.path.insert(0, EC)
from oilpaint import light as Lt
class LP(ctypes.Structure):
    _fields_ = [("light_dir", ctypes.c_float * 3)] + [(k, ctypes.c_float) for k in ("bump", "bump_fine", "contrast", "spec", "shininess", "gloss_h0", "gloss_h1", "cavity", "hsmooth", "broad", "shade_blur", "tint", "weave_amp", "weave_h0")] + [("matte", ctypes.c_int)]
lib = ctypes.CDLL(os.path.abspath("libs/rust_std.so")); fp = ctypes.POINTER(ctypes.c_float)
lib.oc_relight.argtypes = [fp, fp, fp, ctypes.c_int, ctypes.c_int, ctypes.POINTER(LP), fp]
def rust_relight(rgb, h, weave, **kw):
    p = dict(Lt.LIGHT_DEFAULTS); p.update(kw)
    lp = LP(); lp.light_dir[:] = list(p["light_dir"])
    for k in ("bump", "bump_fine", "contrast", "spec", "shininess", "cavity", "hsmooth", "broad", "shade_blur", "tint", "weave_amp", "weave_h0"):
        setattr(lp, k, float(p[k]))
    lp.gloss_h0, lp.gloss_h1 = p["gloss_h"]; lp.matte = int(p["matte"])
    H, W = h.shape; out = np.zeros((H, W, 3), np.float32)
    rgb = np.ascontiguousarray(rgb, np.float32); h = np.ascontiguousarray(h, np.float32)
    wv = None if weave is None else np.ascontiguousarray(weave, np.float32)
    t0 = time.perf_counter()
    lib.oc_relight(rgb.ctypes.data_as(fp), h.ctypes.data_as(fp), None if wv is None else wv.ctypes.data_as(fp), W, H, ctypes.byref(lp), out.ctypes.data_as(fp))
    return out, time.perf_counter() - t0
REAL = dict(bump=0.95, bump_fine=1.0, shade_blur=0.002, contrast=0.30, spec=0.10, cavity=0.02)
for W in (600, 1200):
    rgb = np.load(f"out_corpus_storm_c_strict_{W}_rgb.npy"); h = np.load(f"out_corpus_storm_c_strict_{W}_h.npy")
    H = h.shape[0]
    for name, kw in (("default", {}), ("painting", REAL)):
        wv = Lt.weave(H, W, 0.18)
        t0 = time.perf_counter(); a = Lt.relight(rgb, h, **kw); tp = time.perf_counter() - t0
        b, tr = rust_relight(rgb, h, wv, **kw)
        d = np.abs(a - b).max(-1) * 255
        print(f"relight {W} {name:8s}: python {tp:.3f}s rust {tr:.3f}s | max {d.max():.3f}/255 p99.9 {np.percentile(d, 99.9):.3f}/255 frac>1/255 {(d > 1).mean():.6f}")
        if W == 1200 and name == "default":
            cv2.imwrite("look_relight_py_vs_rust.png", np.concatenate([Lt.to8(a[300:600, 0:500])[..., ::-1], np.full((300, 6, 3), 255, np.uint8), Lt.to8(b[300:600, 0:500])[..., ::-1]], 1))
