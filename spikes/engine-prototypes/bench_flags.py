import sys, os, time, ctypes, resource, numpy as np
sys.path.insert(0, "oilpaint-renderer")
from oilpaint.render import load_strokes, _prep_layer
from oilpaint.scene import load_scene
from oilpaint.canvas import Canvas
from oilpaint._build import CCanvas, BrushParams
def busy():
    v = [int(x) for x in open('/proc/stat').readline().split()[1:]]; return (sum(v) - v[3] - v[4]) / os.sysconf('SC_CLK_TCK')
S = load_scene("oilpaint-renderer/scenes/storm_v3.py"); W = 1200; H = S.size(W)[1]
strokes, ranges = load_strokes("oilpaint-renderer/out/v3/strokes.npz")
fp = ctypes.POINTER(ctypes.c_float); res = {}
for so in sys.argv[1:]:
    lib = ctypes.CDLL(os.path.abspath(so))
    lib.render_strokes.argtypes = [ctypes.POINTER(CCanvas), fp, ctypes.POINTER(ctypes.c_int), ctypes.c_int, fp, fp, fp, ctypes.POINTER(BrushParams)]
    cv = Canvas(W, H, S.ground); cv.lib = lib
    b0 = busy(); r0 = resource.getrusage(resource.RUSAGE_SELF); t0 = time.perf_counter()
    for li, (a, b) in enumerate(ranges):
        _prep_layer(cv, S.layer_list[li], W); cv.render(strokes[a:b])
        if S.layer_list[li].get("dry_after") is not None: cv.dry(S.layer_list[li]["dry_after"])
    t = time.perf_counter() - t0; r1 = resource.getrusage(resource.RUSAGE_SELF); cpu = r1.ru_utime - r0.ru_utime + r1.ru_stime - r0.ru_stime
    res[so] = cv.rgb.copy()
    print(f"{os.path.basename(so):14s} {t:6.2f}s  other-procs {max(0, busy() - b0 - cpu):.2f}s", flush=True)
ks = list(res)
for k in ks[1:]:
    d = np.abs(res[k] - res[ks[0]]).max(-1) * 255
    print(f"{os.path.basename(k)} vs {os.path.basename(ks[0])}: max {d.max():.2f}/255, p99.9 {np.percentile(d, 99.9):.3f}/255, frac>1/255 {(d > 1).mean():.5f}")
