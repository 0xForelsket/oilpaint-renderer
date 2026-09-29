import sys, os, ctypes, time, numpy as np, cv2
sys.path.insert(0, "oilpaint-renderer")
from oilpaint import light as L
lib = ctypes.CDLL(os.path.abspath("exp/level.so"))
fp = ctypes.POINTER(ctypes.c_float)
lib.level_height.argtypes = [fp, fp, ctypes.c_int, ctypes.c_int, ctypes.c_float, ctypes.c_float, ctypes.c_float, ctypes.c_int]

def level(h, s_cw, iters, rate=0.1, wet=None, wet_pow=1.0):
    h = np.ascontiguousarray(h, np.float32).copy(); H, W = h.shape
    w = None if wet is None else np.ascontiguousarray(wet, np.float32)
    lib.level_height(h.ctypes.data_as(fp), None if w is None else w.ctypes.data_as(fp), W, H, s_cw / W, rate, wet_pow, iters)
    return h

def metrics(rgb, h, **lk):
    """hairline: energy of thin dark dips in the shading (black top-hat of lit/unlit, r=2 px at 2400, scaled);
    bristle: band-pass energy of the shading at lane scale (|shade - blur(shade, 2px)| outside hairline pixels)."""
    lit = L.relight(rgb, h, **lk)
    lum_l = lit.mean(-1); lum_u = np.maximum(rgb.mean(-1), 1e-3)
    sh = (lum_l / lum_u).astype(np.float32)
    W = h.shape[1]; r = max(1, int(round(2 * W / 2400)))
    k = cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (2 * r + 1, 2 * r + 1))
    bth = cv2.morphologyEx(sh, cv2.MORPH_BLACKHAT, k)
    hair = float((bth > 0.06).mean())
    bp = np.abs(sh - cv2.GaussianBlur(sh, (0, 0), 2.0 * W / 2400))
    bristle = float(bp[bth <= 0.06].mean())
    gx = cv2.Sobel(h, cv2.CV_32F, 1, 0, ksize=3) / 8; gy = cv2.Sobel(h, cv2.CV_32F, 0, 1, ksize=3) / 8
    g = np.hypot(gx, gy) * W
    return dict(hair=hair, bristle=bristle, g99=float(np.percentile(g, 99)), g999=float(np.percentile(g, 99.9))), lit

if __name__ == "__main__":
    run = sys.argv[1]; out = sys.argv[2]; os.makedirs(out, exist_ok=True)
    h0 = np.load(os.path.join(run, "height.npy")); rgb = np.load(os.path.join(run, "unlit.npy"))
    H, W = h0.shape
    boxes = {"sky_strata": (0.04, 0.37, 0.30, 0.47), "sky_top": (0.30, 0.05, 0.55, 0.15), "rock": (0.50, 0.60, 0.85, 0.72)}
    configs = [("base", None)] + [(f"s{s}_i{it}", (s, it)) for s, it in [(300, 20), (500, 20), (500, 60), (800, 30)]]
    for name, cfg in configs:
        t0 = time.perf_counter()
        h = h0 if cfg is None else level(h0, cfg[0], cfg[1])
        tl = time.perf_counter() - t0
        for bn, (x0, y0, x1, y1) in boxes.items():
            sl = (slice(int(y0 * H), int(y1 * H)), slice(int(x0 * W), int(x1 * W)))
            m, lit = metrics(rgb[sl], np.ascontiguousarray(h[sl]))
            L.save_png(os.path.join(out, f"{bn}_{name}_default.png"), lit)
            print(f"{name:12s} {bn:10s} level {tl:5.2f}s  hair {m['hair']:.4f} bristle {m['bristle']:.4f} |grad| p99 {m['g99']:.0f} p99.9 {m['g999']:.0f} /cw", flush=True)
