"""Replay Storm Light strokes with the prototype kernel variants and per-layer levelling; save h/rgb + crops."""
import sys, os, time, ctypes, json
import numpy as np, cv2
sys.path.insert(0, "oilpaint-renderer")
from oilpaint.render import load_strokes, _prep_layer, _finish_layer
from oilpaint.scene import load_scene
from oilpaint.canvas import Canvas
from oilpaint._build import BrushParams
from oilpaint import light as L
sys.path.insert(0, "exp")
from level_test import metrics

fp = ctypes.POINTER(ctypes.c_float)
class CCanvasE(ctypes.Structure):
    _fields_ = [("W", ctypes.c_int), ("H", ctypes.c_int), ("lat", fp), ("rgb", fp), ("h", fp), ("wet", fp), ("cover", fp),
                ("hblur", fp), ("region", ctypes.POINTER(ctypes.c_uint8)), ("edge", fp)]
kl = ctypes.CDLL(os.path.abspath("exp/brush_proto.so"))
kl.render_strokes.argtypes = [ctypes.POINTER(CCanvasE), fp, ctypes.POINTER(ctypes.c_int), ctypes.c_int, fp, fp, fp, ctypes.POINTER(BrushParams)]
kl.render_stroke.argtypes = [ctypes.POINTER(CCanvasE), fp, ctypes.c_int, fp, fp, fp, ctypes.POINTER(BrushParams), fp]
kl.set_threads.argtypes = [ctypes.c_int]; kl.set_edge_ramp.argtypes = [ctypes.c_float]; kl.set_displace.argtypes = [ctypes.c_float]; kl.set_cap.argtypes = [ctypes.c_float]
lv = ctypes.CDLL(os.path.abspath("exp/level2.so"))
lv.level_mob.argtypes = [fp, fp, ctypes.c_int, ctypes.c_int, ctypes.c_float, ctypes.c_float, ctypes.c_int]

class ECanvas(Canvas):
    def __init__(self, *a, **k):
        super().__init__(*a, **k)
        self.edge = np.zeros((self.H, self.W), np.float32)
        self._cc = CCanvasE(); self._bind(); self._cc.edge = self.edge.ctypes.data_as(fp); self.lib = kl
    def set_hblur(self, hb):
        super().set_hblur(hb); self._cc.edge = self.edge.ctypes.data_as(fp)

if __name__ != '__main__':
    pass
else:
  scene, sfile, W = sys.argv[1], sys.argv[2], int(sys.argv[3])
  name, ramp, lvl_s, lvl_floor = sys.argv[4], float(sys.argv[5]), float(sys.argv[6]), float(sys.argv[7])
  fresh_only = os.environ.get("FRESH", "0") == "1"
  iters_base = int(os.environ.get("ITERS", "20"))
  threads = int(os.environ.get("THREADS", "1"))
  out = f"look/lv_{name}_{W}"; os.makedirs(out, exist_ok=True)
  S = load_scene(scene); H = S.size(W)[1]; layers = S.layer_list
  strokes, ranges = load_strokes(sfile)
  kl.set_threads(threads); kl.set_edge_ramp(ramp); kl.set_displace(float(os.environ.get('DISPLACE', '0')))
  cv = ECanvas(W, H, S.ground)
  t0 = time.perf_counter(); tlev = 0.0
  iters = max(1, int(round(iters_base * W / 2400)))
  for li, (a, b) in enumerate(ranges):
      _prep_layer(cv, layers[li], W)
      cov0 = cv.cover.copy() if fresh_only else None
      cv.render(strokes[a:b])
      if lvl_s > 0:
          t1 = time.perf_counter()
          mob = cv.wet * (lvl_floor + (1 - lvl_floor) * cv.edge)
          if fresh_only:   # open time: only paint laid in this layer still flows
              mob = mob * np.clip(cv.cover - cov0, 0, 1)
          mob = np.ascontiguousarray(mob, np.float32)
          lv.level_mob(cv.h.ctypes.data_as(fp), mob.ctypes.data_as(fp), W, H, lvl_s / W, 0.1, iters)
          tlev += time.perf_counter() - t1
      _finish_layer(cv, layers[li])
  tt = time.perf_counter() - t0
  np.save(f"{out}/height.npy", cv.h); np.save(f"{out}/unlit.npy", cv.rgb)
  boxes = {"sky_strata": (0.04, 0.37, 0.30, 0.47), "sky_top": (0.30, 0.05, 0.55, 0.15), "rock": (0.50, 0.60, 0.85, 0.72),
           "sea": (0.05, 0.75, 0.35, 0.85), "tower_base": (0.58, 0.52, 0.80, 0.64)}
  res = {}
  for bn, (x0, y0, x1, y1) in boxes.items():
      sl = (slice(int(y0 * H), int(y1 * H)), slice(int(x0 * W), int(x1 * W)))
      m, lit = metrics(cv.rgb[sl], np.ascontiguousarray(cv.h[sl]))
      L.save_png(f"{out}/{bn}_default.png", lit); res[bn] = m
      print(f"{name:10s} {bn:10s} hair {m['hair']:.4f} bristle {m['bristle']:.4f} |grad| p99 {m['g99']:.0f} p99.9 {m['g999']:.0f}", flush=True)
  lit = L.relight(cv.rgb, cv.h); L.save_png(f"{out}/final_default.png", lit)
  L.save_png(f"{out}/final_default_900.png", cv2.resize(lit, (720, 900), interpolation=cv2.INTER_AREA))
  json.dump(dict(time=tt, level_time=tlev, iters=iters, metrics=res), open(f"{out}/stats.json", "w"), indent=1)
  print(f"{name}: total {tt:.1f}s, levelling {tlev:.1f}s ({iters} iters/layer)")
