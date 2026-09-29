"""Replay strokes with (a) the repo kernel, (b) the row-chunk prototype at 1 and N threads; compare bits and time."""
import sys, os, time, ctypes, hashlib, resource
import numpy as np
sys.path.insert(0, os.environ.get("OP_REPO", "."))
from oilpaint.render import load_strokes, _prep_layer, _finish_layer
from oilpaint.scene import load_scene
from oilpaint.canvas import Canvas
from oilpaint._build import CCanvas, BrushParams

scene, sfile, W = sys.argv[1], sys.argv[2], int(sys.argv[3])
variants = sys.argv[4].split(",")          # e.g. orig,mt1,mt2
so = sys.argv[5] if len(sys.argv) > 5 else "../exp/brush_mt.so"
S = load_scene(scene); H = S.size(W)[1]
strokes, ranges = load_strokes(sfile)
layers = S.layer_list

def load_mt(path):
    lib = ctypes.CDLL(os.path.abspath(path))
    fp = ctypes.POINTER(ctypes.c_float)
    lib.render_stroke.argtypes = [ctypes.POINTER(CCanvas), fp, ctypes.c_int, fp, fp, fp, ctypes.POINTER(BrushParams), fp]
    lib.render_strokes.argtypes = [ctypes.POINTER(CCanvas), fp, ctypes.POINTER(ctypes.c_int), ctypes.c_int, fp, fp, fp, ctypes.POINTER(BrushParams)]
    lib.set_threads.argtypes = [ctypes.c_int]
    return lib

mt = load_mt(so)
out = {}
for v in variants:
    cv = Canvas(W, H, S.ground)
    if v.startswith("mt"):
        mt.set_threads(int(v[2:])); cv.lib = mt
    t0 = time.perf_counter(); tk = 0.0; r0 = resource.getrusage(resource.RUSAGE_SELF); la0 = open('/proc/loadavg').read().split()[0]
    for li, (a, b) in enumerate(ranges):
        _prep_layer(cv, layers[li], W)
        t1 = time.perf_counter(); cv.render(strokes[a:b]); tk += time.perf_counter() - t1
        _finish_layer(cv, layers[li])
    tt = time.perf_counter() - t0
    dig = hashlib.sha1(cv.lat.tobytes() + cv.h.tobytes() + cv.wet.tobytes() + cv.cover.tobytes() + cv.rgb.tobytes()).hexdigest()[:16]
    r1 = resource.getrusage(resource.RUSAGE_SELF); cpu = (r1.ru_utime - r0.ru_utime) + (r1.ru_stime - r0.ru_stime)
    la1 = open('/proc/loadavg').read().split()[0]
    print(f"{v:6s} {W}x{H} kernel {tk:6.2f}s total {tt:6.2f}s cpu {cpu:6.2f}s (cpu/wall {cpu/tt:4.2f}) load {la0}->{la1} digest {dig}", flush=True)
    out[v] = (cv.rgb.copy(), cv.h.copy())
    np.save(f"../exp/rgb_{v}_{W}.npy", cv.rgb); np.save(f"../exp/h_{v}_{W}.npy", cv.h)
    del cv
if "orig" in out:
    for v in out:
        if v == "orig": continue
        d = np.abs(out[v][0] - out["orig"][0]); dh = np.abs(out[v][1] - out["orig"][1])
        print(f"{v} vs orig: rgb max {d.max()*255:.3f}/255, mean {d.mean()*255:.5f}/255, frac px >0.5/255 {(d.max(-1)>0.5/255).mean():.5f}; h max {dh.max():.4f}")
