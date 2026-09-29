"""Check G3 (docs/plans/LIBRARY_PLAN.md, section 5): replay the same strokes at two sizes, area-downsample both to
600 px, and report the share of pixels whose largest channel differs by more than 8/255. Target: <= 1%.

Engine: `oil paint` on a StrokeList v2 file (Mixbox, unlit). v1 behaviour (xorshift bristles): Storm Light planned
by the Python planner and replayed with the spike kernel through the Python renderer (plan and replay on the spike).

usage: python tools/size_consistency.py SCENE.py STROKES.oilstrokes [--widths 1800,2400] [--v1] [--json FILE]
"""
import argparse
import json
import os
import subprocess
import sys

import cv2
import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
sys.path.insert(0, ROOT)
OIL = os.path.join(ROOT, "target", "release", "oil.exe" if sys.platform == "win32" else "oil")


def downsample(img, w=600):
    h = int(round(img.shape[0] * w / img.shape[1]))
    return cv2.resize(img, (w, h), interpolation=cv2.INTER_AREA)


def share_over(a, b, t=8):
    d = np.abs(a.astype(np.int16) - b.astype(np.int16)).max(-1)
    return float((d > t).mean()), float((d > 1).mean())


def engine(strokes, widths, out):
    subprocess.run([os.environ.get("CARGO", "cargo"), "build", "--release", "-q", "-p", "oil-cli", "--features", "mixbox"],
                   cwd=ROOT, check=True)
    imgs = {}
    for w in widths:
        d = os.path.join(out, f"engine_{w}")
        r = subprocess.run([OIL, "paint", strokes, "--width", str(w), "--mixer", "mixbox", "--light", "none", "--out", d],
                           capture_output=True, text=True, check=True)
        rep = json.loads(r.stdout)
        print(f"  engine {w}: paint {rep['timings']['paintSeconds']:.2f} s")
        imgs[w] = downsample(cv2.cvtColor(cv2.imread(os.path.join(d, "unlit.png")), cv2.COLOR_BGR2RGB))
    return imgs


def v1(scene, widths):
    os.environ["OILPAINT_KERNEL"] = "rust"  # v1 end to end: plan and replay with the spike kernel
    from oilpaint import light as Lt, mix
    from oilpaint.canvas import Canvas
    from oilpaint.planner import Planner
    from oilpaint.scene import load_scene
    mix.set_backend("mixbox")
    S = load_scene(scene)
    pl = Planner(S.guides(600), S.styles, S.layer_list, seed=1907, ground_rgb=S.ground, verbose=False)
    pl.plan()
    imgs = {}
    for w in widths:
        h = int(round(w * S.aspect[1] / S.aspect[0]))
        cv = Canvas(w, h, S.ground)
        for li, (a, b) in enumerate(pl.layer_ranges):
            L = S.layer_list[li]
            if any(s.params.get("mode") == "scumble" for s in pl.strokes[a:b]):
                cv.set_hblur(Lt.blur(cv.h, max(1.0, L["hblur_sigma"] * w)))
            cv.render(pl.strokes[a:b])
            if L.get("dry_after") is not None:
                cv.dry(L["dry_after"])
        imgs[w] = downsample((np.clip(cv.rgb, 0, 1) * 255 + 0.5).astype(np.uint8))
        print(f"  v1 (spike kernel) {w}: done")
        del cv
    return imgs


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("scene")
    ap.add_argument("strokes")
    ap.add_argument("--widths", default="1800,2400")
    ap.add_argument("--v1", action="store_true", help="also measure v1's xorshift bristles with the spike kernel")
    ap.add_argument("--json")
    a = ap.parse_args()
    widths = [int(v) for v in a.widths.split(",")]
    out = os.path.join(ROOT, "out", "l1", "g3")
    report = {}
    runs = [("engine", lambda: engine(a.strokes, widths, out))]
    if a.v1:
        runs.append(("v1", lambda: v1(a.scene, widths)))
    for name, fn in runs:
        imgs = fn()
        s8, s1 = share_over(imgs[widths[0]], imgs[widths[1]])
        report[name] = dict(widths=widths, share_over_8=s8, share_over_1=s1)
        print(f"{name}: {widths[0]} vs {widths[1]} at 600 px: {100 * s8:.2f}% of pixels > 8/255 ({100 * s1:.1f}% > 1/255)")
    if a.json:
        with open(a.json, "w") as f:
            json.dump(report, f, indent=1)


if __name__ == "__main__":
    main()
