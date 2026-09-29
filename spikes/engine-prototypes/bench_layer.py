import sys, os, time, ctypes, resource
def busy():
    f = open('/proc/stat').readline().split()[1:]
    v = [int(x) for x in f]; idle = v[3] + v[4]
    return (sum(v) - idle) / os.sysconf('SC_CLK_TCK')
import numpy as np
sys.path.insert(0, "oilpaint-renderer")
from oilpaint.render import load_strokes, _prep_layer
from oilpaint.scene import load_scene
from oilpaint.canvas import Canvas
from oilpaint._build import CCanvas, BrushParams
so, W, layer_list, variants = sys.argv[1], int(sys.argv[2]), [int(v) for v in sys.argv[3].split(",")], sys.argv[4].split(",")
S = load_scene("oilpaint-renderer/scenes/storm_v3.py"); H = S.size(W)[1]
strokes, ranges = load_strokes("oilpaint-renderer/out/v3/strokes.npz")
lib = ctypes.CDLL(os.path.abspath(so)); fp = ctypes.POINTER(ctypes.c_float)
lib.render_strokes.argtypes = [ctypes.POINTER(CCanvas), fp, ctypes.POINTER(ctypes.c_int), ctypes.c_int, fp, fp, fp, ctypes.POINTER(BrushParams)]
lib.set_threads.argtypes = [ctypes.c_int]
for li in layer_list:
    a, b = ranges[li - 1]
    for v in variants:
        cv = Canvas(W, H, S.ground); cv.lib = lib; lib.set_threads(int(v))
        _prep_layer(cv, S.layer_list[li - 1], W)
        r0 = resource.getrusage(resource.RUSAGE_SELF); b0 = busy(); t0 = time.perf_counter()
        cv.render(strokes[a:b])
        t = time.perf_counter() - t0; r1 = resource.getrusage(resource.RUSAGE_SELF); b1 = busy()
        cpu = r1.ru_utime - r0.ru_utime + r1.ru_stime - r0.ru_stime
        print(f"L{li:02d} threads {v}: {t:6.2f}s cpu {cpu:6.2f}s  other-procs cpu {max(0.0, (b1 - b0) - cpu):5.2f}s", flush=True)
