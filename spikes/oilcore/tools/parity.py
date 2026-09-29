"""Kernel parity + speed: replay a corpus through Python/ctypes with each library (hblur from cv2 for all, so only the
kernel differs).  usage: python parity.py corpus.bin W lib1,lib2,..."""
import sys, os, time, ctypes, struct, hashlib, resource
import numpy as np
EC = os.path.join(os.path.dirname(os.path.abspath(__file__)), "eval_copy"); sys.path.insert(0, EC)
from oilpaint.canvas import Canvas
from oilpaint._build import CCanvas, BrushParams, LAT
from oilpaint import light as Lt

def busy():
    v = [int(x) for x in open('/proc/stat').readline().split()[1:]]; return (sum(v) - v[3] - v[4]) / os.sysconf('SC_CLK_TCK')

def read_corpus(path):
    b = open(path, "rb").read(); o = 0
    def u(n): nonlocal o; v = struct.unpack_from(f"<{n}I", b, o); o += 4 * n; return v
    def f(n): nonlocal o; v = np.frombuffer(b, np.float32, n, o).copy(); o += 4 * n; return v
    magic, ver, ns, np_, nl, lat = u(6); aspect = f(1)[0]; glat = f(7); grgb = f(3)
    layers = []
    for _ in range(nl):
        s, e, nh = u(3); sig, dry = f(2); layers.append((s, e, bool(nh), float(sig), None if np.isnan(dry) else float(dry)))
    off = np.frombuffer(b, np.uint32, ns + 1, o).astype(np.int64); o += 4 * (ns + 1)
    pts = f(np_ * 4).reshape(-1, 4); zc = f(ns * 7).reshape(-1, 7); zc2 = f(ns * 7).reshape(-1, 7); dz = f(ns * 7).reshape(-1, 7)
    params = (BrushParams * ns).from_buffer_copy(b[o:o + ns * ctypes.sizeof(BrushParams)])
    return dict(aspect=float(aspect), glat=glat, grgb=grgb, layers=layers, off=off, pts=pts, zc=zc, zc2=zc2, dz=dz, params=params)

def load(path):
    lib = ctypes.CDLL(os.path.abspath(path)); fp = ctypes.POINTER(ctypes.c_float)
    lib.render_strokes.argtypes = [ctypes.POINTER(CCanvas), fp, ctypes.POINTER(ctypes.c_int), ctypes.c_int, fp, fp, fp, ctypes.POINTER(BrushParams)]
    lib.render_strokes.restype = ctypes.c_int
    return lib

def replay(C, W, lib):
    H = int(round(W * C["aspect"]))
    cv = Canvas(W, H, (0.5, 0.5, 0.5)); cv.lat[:] = C["glat"]; cv.rgb[:] = C["grgb"]
    fp = ctypes.POINTER(ctypes.c_float); tk = 0.0; npx = 0
    for (s, e, nh, sig, dry) in C["layers"]:
        if nh:
            cv.set_hblur(Lt.blur(cv.h, max(1.0, sig * W)))
        if e > s:
            a, b = C["off"][s], C["off"][e]
            pts = np.ascontiguousarray(C["pts"][a:b]).copy(); pts[:, :3] *= W
            off = (C["off"][s:e + 1] - a).astype(np.int32)
            zc, zc2, dz = (np.ascontiguousarray(C[k][s:e]) for k in ("zc", "zc2", "dz"))
            prm = (BrushParams * (e - s))(*C["params"][s:e])
            t0 = time.perf_counter()
            npx += lib.render_strokes(ctypes.byref(cv._cc), pts.ctypes.data_as(fp), off.ctypes.data_as(ctypes.POINTER(ctypes.c_int)), e - s,
                                      zc.ctypes.data_as(fp), zc2.ctypes.data_as(fp), dz.ctypes.data_as(fp), prm)
            tk += time.perf_counter() - t0
        if dry is not None:
            cv.dry(dry)
    return cv, tk, npx

if __name__ == "__main__":
    C = read_corpus(sys.argv[1]); W = int(sys.argv[2]); names = sys.argv[3].split(",")
    reps = int(sys.argv[4]) if len(sys.argv) > 4 else 1
    res = {}
    for nm in names:
        lib = load(f"libs/{nm}.so")
        best = None
        for r in range(reps):
            b0 = busy(); r0 = resource.getrusage(resource.RUSAGE_SELF)
            cv, tk, npx = replay(C, W, lib)
            r1 = resource.getrusage(resource.RUSAGE_SELF); cpu = r1.ru_utime - r0.ru_utime + r1.ru_stime - r0.ru_stime
            other = max(0.0, busy() - b0 - cpu)
            best = tk if best is None else min(best, tk)
            dig = hashlib.sha1(cv.rgb.tobytes() + cv.h.tobytes() + cv.lat.tobytes() + cv.wet.tobytes() + cv.cover.tobytes()).hexdigest()[:12]
            print(f"{nm:9s} W={W} kernel {tk:7.3f}s  npx {npx/1e6:6.1f}M  {npx/1e6/max(tk,1e-9):5.2f} Mpix/s  other-procs {other:4.1f}s  digest {dig}", flush=True)
        res[nm] = (cv.rgb.copy(), cv.h.copy(), best)
        np.save(f"out_{os.path.basename(sys.argv[1])[:-4]}_{nm}_{W}_rgb.npy", cv.rgb); np.save(f"out_{os.path.basename(sys.argv[1])[:-4]}_{nm}_{W}_h.npy", cv.h)
    ref = names[0]
    for nm in names[1:]:
        d = np.abs(res[nm][0] - res[ref][0]).max(-1) * 255; dh = np.abs(res[nm][1] - res[ref][1])
        same = np.array_equal(res[nm][0], res[ref][0]) and np.array_equal(res[nm][1], res[ref][1])
        print(f"{nm} vs {ref}: bit-identical={same}  rgb max {d.max():.3f}/255  frac>1/255 {(d>1).mean():.5f}  frac>8/255 {(d>8).mean():.5f}  h max {dh.max():.4g}")
