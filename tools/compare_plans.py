"""v1's Python planner against the engine's Rust planner (L3), for information: the planner port is a cutover, so
nothing here is a gate (docs/plans/LIBRARY_PLAN.md, section 5).

Both plans are painted by the same engine (`oil paint`, same kernel and mixer), so only the planner differs:
- v1 plans through tools/plan_to_strokelist.py, whose proxy canvas is the engine kernel via crates/oil-shim;
- the engine plans with `oil plan` from the scene's ScenePlan.
Metrics are the eval harness's (oilpaint/eval_metrics.py), per region on the engine's region map, plus per-layer
stroke statistics. Writes compare.json, compare.md and side_by_side.png.

  .venv\\Scripts\\python.exe tools\\compare_plans.py --v1-scene scenes/storm_v3.py --spec spec/examples/storm_v3.sceneplan.json --out out/l3/compare_storm
"""
import argparse
import json
import os
import subprocess
import sys
import time

import cv2
import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
sys.path.insert(0, ROOT)
from oilpaint import eval_metrics as EM  # noqa: E402
from oilpaint import strokelist  # noqa: E402

OIL = os.path.join(ROOT, "target", "release", "oil.exe" if sys.platform == "win32" else "oil")
HEADLINE = ("hairline", "bristle_L", "edge_step_p99", "h_p95", "C_mean", "L_p50")


def run(cmd, env=None):
    t0 = time.perf_counter()
    r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, env=env)
    if r.returncode != 0:
        raise SystemExit(f"{' '.join(cmd)} failed:\n{r.stdout[-2000:]}\n{r.stderr[-2000:]}")
    return r.stdout, time.perf_counter() - t0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--v1-scene", required=True)
    ap.add_argument("--spec", required=True)
    ap.add_argument("--width", type=int, default=600)
    ap.add_argument("--seed", type=int, default=1907)
    ap.add_argument("--mixer", default="ochrell")
    ap.add_argument("--out", required=True)
    ap.add_argument("--min-region", type=float, default=0.025)
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    W = a.width
    v1_file, new_file = os.path.join(a.out, "v1.oilstrokes"), os.path.join(a.out, "new.oilstrokes")

    env = dict(os.environ, OILPAINT_KERNEL="oil", PYTHONIOENCODING="utf-8")
    _, t_v1 = run([sys.executable, "tools/plan_to_strokelist.py", a.v1_scene, v1_file, "--plan-width", str(W), "--seed", str(a.seed), "--mixer", a.mixer], env)
    out, t_new = run([OIL, "plan", a.spec, "--out", new_file, "--width", str(W), "--seed", str(a.seed), "--mixer", a.mixer])
    new_report = json.loads(out)
    run([OIL, "guides", a.spec, "--width", str(W), "--out", os.path.join(a.out, "guides"), "--npy"])
    names = [r["name"] for r in json.load(open(os.path.join(a.out, "guides", "guides.json")))["regions"]]
    rid = np.load(os.path.join(a.out, "guides", "region_id.npy"))

    doc = dict(scene=a.v1_scene, spec=a.spec, width=W, seed=a.seed, mixer=a.mixer,
               plan_seconds=dict(v1_python_planner=t_v1, engine=t_new, note="wall clock of each command, unpinned"), plans={})
    imgs = {}
    for tag, f in (("v1", v1_file), ("engine", new_file)):
        d = os.path.join(a.out, tag)
        run([OIL, "paint", f, "--width", str(W), "--mixer", a.mixer, "--light", "painting", "--out", d, "--npy"])
        lit, unlit, h = np.load(os.path.join(d, "lit.npy")), np.load(os.path.join(d, "unlit.npy")), np.load(os.path.join(d, "height.npy"))
        imgs[tag] = lit
        lab = EM.lab_of(lit)
        maps = EM.structure_maps(lab)
        sl = strokelist.read(f)
        per_layer = [dict(name=n, strokes=l["end"] - l["start"]) for n, l in zip(sl["meta"]["layers"], sl["layers"])]
        regions = {}
        for i, n in enumerate(names):
            m = rid == i
            if m.mean() >= a.min_region:
                regions[n] = EM.image_metrics(lit, unlit, h, mask=m, lab=lab, maps=maps)
        doc["plans"][tag] = dict(strokes=len(sl["strokes"]), layers=per_layer,
                                 overall=EM.image_metrics(lit, unlit, h, mask=None, lab=lab, maps=maps), regions=regions,
                                 stroke_stats=EM.stroke_stats(sl["strokes"], names))
    doc["engine_report"] = dict(bare=new_report["bare"], regions=new_report["regions"])

    # markdown summary
    v1, en = doc["plans"]["v1"], doc["plans"]["engine"]
    md = [f"# v1 planner vs engine planner: {os.path.basename(a.v1_scene)} at {W} px ({a.mixer}, seed {a.seed})", "",
          "For information: the planner port is a cutover (no parity gate). Both plans painted by the same `oil paint`.", "",
          f"Strokes: v1 {v1['strokes']}, engine {en['strokes']}. Planning (unpinned wall clock): v1 {t_v1:.1f} s, engine {t_new:.1f} s.", "",
          "| layer | v1 strokes | engine strokes |", "|---|---|---|"]
    for lv, le in zip(v1["layers"], en["layers"]):
        md.append(f"| {lv['name']} | {lv['strokes']} | {le['strokes']} |")
    md += ["", "| region | metric | v1 | engine |", "|---|---|---|---|"]
    for n in names:
        if n in v1["regions"] and n in en["regions"]:
            for k in HEADLINE:
                x, y = v1["regions"][n].get(k), en["regions"][n].get(k)
                if isinstance(x, (int, float)) and isinstance(y, (int, float)):
                    md.append(f"| {n} | {k} | {x:.3f} | {y:.3f} |")
    md += ["", "| region | v1 width med | engine width med | v1 length med | engine length med | v1 n | engine n |", "|---|---|---|---|---|---|---|"]
    for n in names:
        sv, se = v1["stroke_stats"].get("regions", {}).get(n), en["stroke_stats"].get("regions", {}).get(n)
        if sv and se:
            md.append(f"| {n} | {sv['width_med']:.4f} | {se['width_med']:.4f} | {sv['length_med']:.3f} | {se['length_med']:.3f} | {sv['n']} | {se['n']} |")
    open(os.path.join(a.out, "compare.md"), "w", encoding="utf-8", newline="\n").write("\n".join(md) + "\n")
    json.dump(doc, open(os.path.join(a.out, "compare.json"), "w"), indent=1, default=float)

    to8 = lambda x: cv2.cvtColor((np.clip(x, 0, 1) * 255 + 0.5).astype(np.uint8), cv2.COLOR_RGB2BGR)
    pad = np.full((imgs["v1"].shape[0], 12, 3), 40, np.uint8)
    side = np.hstack([to8(imgs["v1"]), pad, to8(imgs["engine"])])
    for x, label in ((8, "v1 planner"), (imgs["v1"].shape[1] + 20, "engine planner (L3)")):
        cv2.rectangle(side, (x - 4, 4), (x + 12 * len(label), 30), (20, 20, 20), -1)
        cv2.putText(side, label, (x, 24), cv2.FONT_HERSHEY_SIMPLEX, 0.6, (240, 240, 240), 1, cv2.LINE_AA)
    cv2.imwrite(os.path.join(a.out, "side_by_side.png"), side)
    print("\n".join(md[:8 + len(v1["layers"])]))


if __name__ == "__main__":
    main()
