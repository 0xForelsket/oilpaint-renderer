"""Plan a scene with the Python planner and write the plan as a StrokeList v2 file for the new engine.

This is the bridge between the Python planner (on main until the L3 retirement) and the Rust painter: port checks,
size-consistency tests, and level E in L3. It plans from the scene, never from a v1 `strokes.npz`. The planner's
proxy canvas is painted by the kernel selected with OILPAINT_KERNEL (default here: the new kernel, `oil`).

usage: python tools/plan_to_strokelist.py SCENE.py OUT.oilstrokes [--plan-width 600] [--seed 1907] [--mixer mixbox-material]
"""
import argparse
import os
import sys
import time

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
sys.path.insert(0, ROOT)
os.environ.setdefault("OILPAINT_KERNEL", "oil")
from oilpaint import mix, strokelist  # noqa: E402
from oilpaint.planner import Planner  # noqa: E402
from oilpaint.scene import load_scene  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("scene")
    ap.add_argument("out")
    ap.add_argument("--plan-width", type=int, default=600)
    ap.add_argument("--seed", type=int, default=1907)
    ap.add_argument("--mixer", default="mixbox-material", help="the new kernel needs a material mixer (mixbox-material; ochrell)")
    a = ap.parse_args()
    mix.set_backend(a.mixer)
    S = load_scene(a.scene)
    t0 = time.perf_counter()
    g = S.guides(a.plan_width)
    pl = Planner(g, S.styles, S.layer_list, seed=a.seed, ground_rgb=S.ground, verbose=False)
    pl.plan()
    t_plan = time.perf_counter() - t0
    layers = []
    for li, (s, e) in enumerate(pl.layer_ranges):
        L = S.layer_list[li]
        scumble = any(st.params.get("mode") == "scumble" for st in pl.strokes[s:e])
        layers.append(dict(start=s, end=e, hblur_sigma=float(L["hblur_sigma"]) if scumble else None,
                           dry_after=None if L.get("dry_after") is None else float(L["dry_after"])))
    plan = {"mixer": {"mixbox": "mixbox-2.0", "mixbox-material": "mixbox-2.0", "rgb": "rgb"}.get(a.mixer, a.mixer), "planWidth": a.plan_width,
            "seed": a.seed, "portable": False}
    data = strokelist.write(a.out, pl.strokes, layers, S.aspect, S.ground, [L["name"] for L in S.layer_list],
                            region_names=list(g.names), title=os.path.splitext(os.path.basename(a.scene))[0], plan=plan)
    print(f"planned {len(pl.strokes)} strokes in {t_plan:.1f} s -> {a.out} ({len(data) / 1e6:.2f} MB)")


if __name__ == "__main__":
    main()
