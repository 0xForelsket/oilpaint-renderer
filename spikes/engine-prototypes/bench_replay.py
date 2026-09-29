"""Replay a strokes.npz at a given size with per-layer timing, per-mode timing and pixel counts."""
import sys, time, json, os
import numpy as np
sys.path.insert(0, os.environ.get("OP_REPO", "."))
from oilpaint.render import load_strokes, _prep_layer, _finish_layer
from oilpaint.scene import load_scene
from oilpaint.canvas import Canvas, MODES

scene, sfile, W = sys.argv[1], sys.argv[2], int(sys.argv[3])
per_stroke = len(sys.argv) > 4 and sys.argv[4] == "per_stroke"
S = load_scene(scene)
H = S.size(W)[1]
strokes, ranges = load_strokes(sfile)
cv = Canvas(W, H, S.ground)
layers = S.layer_list
res = []
t_all = time.perf_counter()
mode_t = {}
st_times = []
for li, (a, b) in enumerate(ranges):
    t0 = time.perf_counter()
    _prep_layer(cv, layers[li], W)
    t1 = time.perf_counter()
    if per_stroke:
        npx = 0
        for s in strokes[a:b]:
            ts = time.perf_counter()
            n = cv.render_stroke(s)
            dt = time.perf_counter() - ts
            npx += n
            m = s.params.get("mode", "paint")
            mode_t.setdefault(m, [0.0, 0, 0]); mode_t[m][0] += dt; mode_t[m][1] += n; mode_t[m][2] += 1
            st_times.append((dt, n, li, s.params.get("nb", 0), float(s.pts[:, 2].max()) * W, s.pts.shape[0]))
    else:
        npx = cv.render(strokes[a:b])
    t2 = time.perf_counter()
    _finish_layer(cv, layers[li])
    res.append(dict(layer=li + 1, name=layers[li]["name"], n=b - a, prep=t1 - t0, render=t2 - t1, mpix=npx / 1e6))
    print(f"L{li+1:02d} {layers[li]['name'][:28]:28s} n={b-a:5d} prep={t1-t0:5.2f}s render={t2-t1:6.2f}s px={npx/1e6:7.2f}M  {npx/1e6/max(t2-t1,1e-9):5.2f} Mpix/s", flush=True)
tot = time.perf_counter() - t_all
print(f"TOTAL {W}x{H} {len(strokes)} strokes {tot:.2f}s; kernel {sum(r['render'] for r in res):.2f}s; prep {sum(r['prep'] for r in res):.2f}s; px {sum(r['mpix'] for r in res):.1f}M")
if per_stroke:
    for m, (t, n, c) in mode_t.items():
        print(f"mode {m}: {c} strokes {t:.2f}s {n/1e6:.1f}Mpx {n/1e6/max(t,1e-9):.2f} Mpix/s")
    np.save(os.environ.get("OUT_ST", "st_times.npy"), np.array(st_times))
np.save(os.environ.get("OUT_RGB", "/dev/null") if os.environ.get("OUT_RGB") else "/tmp/claude-0/-home-claude/cd4531f6-3314-512b-9953-4f74d55c5a55/scratchpad/work_engine/exp/_last_rgb.npy", cv.rgb[::4, ::4])
if os.environ.get("SAVE_STATE"):
    np.save(os.environ["SAVE_STATE"] + "_h.npy", cv.h); np.save(os.environ["SAVE_STATE"] + "_rgb.npy", cv.rgb)
