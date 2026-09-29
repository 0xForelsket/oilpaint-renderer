"""How much stroke-level parallelism do the real Storm Light stroke lists offer?

Two strokes can run concurrently (with bit-identical results) iff their footprints (all pixels read or
written) are disjoint.  Footprint = union over segments of the segment bbox grown by the kernel's hwmax
(0.5*w*1.45+1 px), exactly as brush.c computes it.  Conservative cell grid of C px.
Dependencies: each stroke waits for the last earlier stroke that touched any of its cells (every stroke
writes `cover`, so every stroke is a writer and the per-cell chain is transitively complete).
Layers are barriers (hblur prep + drying).
Reports per layer and total: critical path, list-schedule speedup on P workers (DAG, in-order priority),
and level-synchronous (BSP) speedup (GPU-style: all strokes of equal depth run together, barrier between levels)."""
import sys, os, heapq
import numpy as np
sys.path.insert(0, os.environ.get("OP_REPO", "."))
from oilpaint.render import load_strokes

sfile, W = sys.argv[1], int(sys.argv[2])
C = int(sys.argv[3]) if len(sys.argv) > 3 else 32
cost_file = sys.argv[4] if len(sys.argv) > 4 else None
strokes, ranges = load_strokes(sfile)
H = int(round(W * 1.25))
cost = np.load(cost_file)[:, 0] if cost_file else None

def cells_of(s):
    p = s.pts.astype(np.float64).copy(); p[:, :3] *= W
    out = set()
    for i in range(len(p) - 1):
        x0, y0, w0 = p[i, :3]; x1, y1, w1 = p[i + 1, :3]
        hw = 0.5 * max(w0, w1) * 1.45 + 1.0
        bx0 = max(0, int(np.floor(min(x0, x1) - hw))); bx1 = min(W - 1, int(np.ceil(max(x0, x1) + hw)))
        by0 = max(0, int(np.floor(min(y0, y1) - hw))); by1 = min(H - 1, int(np.ceil(max(y0, y1) + hw)))
        if bx1 < bx0 or by1 < by0: continue
        for cy in range(by0 // C, by1 // C + 1):
            for cx in range(bx0 // C, bx1 // C + 1):
                out.add(cy * 100000 + cx)
    return out

def list_schedule(deps, c, P):
    n = len(c)
    indeg = [len(d) for d in deps]
    succ = [[] for _ in range(n)]
    for j, d in enumerate(deps):
        for i in d: succ[i].append(j)
    ready = [j for j in range(n) if indeg[j] == 0]; heapq.heapify(ready)
    free = [0.0] * P  # worker free times (heap)
    events = []  # (finish_time, j)
    t = 0.0; done = 0; running = 0
    heapq.heapify(free)
    # event simulation
    while done < n:
        while ready and running < P:
            j = heapq.heappop(ready)
            heapq.heappush(events, (t + c[j], j)); running += 1
        ft, j = heapq.heappop(events); t = ft; running -= 1; done += 1
        for k in succ[j]:
            indeg[k] -= 1
            if indeg[k] == 0: heapq.heappush(ready, k)
    return t

def levels(deps, c):
    n = len(c); lev = np.zeros(n, int); cp = np.zeros(n)
    for j in range(n):
        if deps[j]:
            lev[j] = 1 + max(lev[i] for i in deps[j]); cp[j] = c[j] + max(cp[i] for i in deps[j])
        else:
            cp[j] = c[j]
    return lev, cp

def bsp_time(lev, c, P):
    t = 0.0
    for L in range(lev.max() + 1):
        cs = sorted(c[lev == L], reverse=True)
        loads = [0.0] * P
        for x in cs:  # LPT
            k = int(np.argmin(loads)); loads[k] += x
        t += max(loads)
    return t

Ps = [2, 4, 8, 16]
tot = {"serial": 0.0, **{f"dag{P}": 0.0 for P in Ps}, **{f"bsp{P}": 0.0 for P in Ps}, "cp": 0.0}
print(f"cell {C}px, W={W}; cost = {'measured seconds' if cost is not None else 'segment count proxy'}")
for li, (a, b) in enumerate(ranges):
    n = b - a
    if n == 0: continue
    cs = [cells_of(strokes[k]) for k in range(a, b)]
    c = cost[a:b] if cost is not None else np.array([len(x) for x in cs], float)
    last = {}
    deps = []
    for j, cells in enumerate(cs):
        d = set()
        for cc in cells:
            if cc in last: d.add(last[cc])
            last[cc] = j
        deps.append(d)
    lev, cp = levels(deps, c)
    ser = c.sum()
    row = f"L{li+1:02d} n={n:5d} serial={ser:6.2f} critpath={cp.max():6.2f} (max speedup {ser/cp.max():5.1f}x) levels={lev.max()+1:5d} avg width {n/(lev.max()+1):5.1f} |"
    tot["serial"] += ser; tot["cp"] += cp.max()
    for P in Ps:
        td = list_schedule(deps, c, P); tb = bsp_time(lev, c, P)
        tot[f"dag{P}"] += td; tot[f"bsp{P}"] += tb
        row += f" P{P}: dag {ser/td:4.2f}x bsp {ser/tb:4.2f}x"
    print(row, flush=True)
print("TOTAL serial %.2f  critical path %.2f (ideal speedup %.1fx)" % (tot["serial"], tot["cp"], tot["serial"] / tot["cp"]))
for P in Ps:
    print(f"  P={P:2d}: DAG list-schedule {tot['serial']/tot[f'dag{P}']:.2f}x  ({tot[f'dag{P}']:.2f}s)   level-sync BSP {tot['serial']/tot[f'bsp{P}']:.2f}x ({tot[f'bsp{P}']:.2f}s)")
