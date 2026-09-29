import sys, os, numpy as np, cv2
sys.path.insert(0, "oilpaint-renderer")
from oilpaint import light as L
run = sys.argv[1]; out = sys.argv[2]; os.makedirs(out, exist_ok=True)
h = np.load(os.path.join(run, "height.npy")); rgb = np.load(os.path.join(run, "unlit.npy"))
H, W = h.shape
REAL = dict(bump=0.95, bump_fine=1.0, shade_blur=0.002, contrast=0.30, spec=0.10, cavity=0.02)
boxes = {"sky_strata": (0.04, 0.37, 0.30, 0.47), "halo": (0.58, 0.20, 0.78, 0.36), "tower_base": (0.58, 0.52, 0.80, 0.64),
         "rock": (0.50, 0.60, 0.85, 0.72), "sky_top": (0.30, 0.05, 0.55, 0.15)}
lit_def = L.relight(rgb, h)            # engine defaults (strata visible)
lit_real = L.relight(rgb, h, **REAL)   # the lighting used for the real painting
for k, (x0, y0, x1, y1) in boxes.items():
    sl = (slice(int(y0 * H), int(y1 * H)), slice(int(x0 * W), int(x1 * W)))
    L.save_png(os.path.join(out, f"{k}_default.png"), lit_def[sl]); L.save_png(os.path.join(out, f"{k}_real.png"), lit_real[sl])
    L.save_png(os.path.join(out, f"{k}_unlit.png"), rgb[sl])
    hh = h[sl]; L.save_gray(os.path.join(out, f"{k}_height.png"), (hh - hh.min()) / (np.ptp(hh) + 1e-6), 1.0)
print("ok", W, H)
