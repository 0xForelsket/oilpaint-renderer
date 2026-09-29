"""CLI.  See README.md.

  python -m oilpaint render scenes/storm_light.py --preview [--upto N] [--only N --from out/preview] [--seed S]
  python -m oilpaint render scenes/storm_light.py --size 2400x3000 --timelapse out/final/timelapse.mp4
  python -m oilpaint guides scenes/storm_light.py [--size 600]
  python -m oilpaint relight out/run [--light x,y,z] [--spec S] [--bump B] [--contrast C] [--matte]
  python -m oilpaint crop out/run --box 0.55,0.30,0.80,0.60 [--out file.png] [--unlit]
  python -m oilpaint info out/run
  python -m oilpaint test [t1 t2 ... | all]
  python -m oilpaint eval run|compare|reference ...   (measuring stick; see README "Evaluating changes")
"""
import argparse
import os
import sys


def _size(s):
    w, h = s.lower().split("x")
    return int(w), int(h)


def _light_kw(a):
    lk = {}
    if getattr(a, "light", None): lk["light_dir"] = tuple(float(v) for v in a.light.split(","))
    for k in ("spec", "bump", "contrast", "cavity", "weave_amp", "bump_fine", "shade_blur", "hsmooth"):
        v = getattr(a, k, None)
        if v is not None: lk[k] = v
    if getattr(a, "matte", False): lk["matte"] = True
    return lk


def _add_light_args(p):
    p.add_argument("--light", default=None, help="light direction x,y,z (default -0.5,-0.6,0.62 = upper left)")
    p.add_argument("--spec", type=float, default=None); p.add_argument("--bump", type=float, default=None)
    p.add_argument("--contrast", type=float, default=None); p.add_argument("--cavity", type=float, default=None)
    p.add_argument("--weave-amp", dest="weave_amp", type=float, default=None); p.add_argument("--matte", action="store_true")
    p.add_argument("--bump-fine", dest="bump_fine", type=float, default=None); p.add_argument("--shade-blur", dest="shade_blur", type=float, default=None)
    p.add_argument("--hsmooth", type=float, default=None)


def main(argv=None):
    ap = argparse.ArgumentParser(prog="oilpaint")
    sub = ap.add_subparsers(dest="cmd", required=True)

    r = sub.add_parser("render")
    r.add_argument("scene")
    r.add_argument("--preview", action="store_true", help="plan and render at the plan width (default 600)")
    r.add_argument("--size", type=_size, default=None, help="output size WxH, e.g. 2400x3000")
    r.add_argument("--plan-width", type=int, default=600)
    r.add_argument("--seed", type=int, default=1907)
    r.add_argument("--out", default=None)
    r.add_argument("--upto", type=int, default=None, help="stop after layer N")
    r.add_argument("--only", type=int, default=None, help="re-plan only layer N over the saved state of --from")
    r.add_argument("--from", dest="from_run", default=None, help="run directory with strokes.npz and states/")
    r.add_argument("--strokes", default=None, help="replay a saved strokes.npz instead of planning")
    r.add_argument("--mixer", choices=["mixbox", "rgb", "ochrell", "ochrell-roundtrip", "ochrell-srgb"], default="mixbox")
    r.add_argument("--no-light", action="store_true")
    r.add_argument("--no-layers", action="store_true", help="skip per-layer PNGs")
    r.add_argument("--timelapse", default=None, help="write an mp4 time-lapse to this path")
    r.add_argument("--video-size", type=_size, default=(1200, 1500))
    r.add_argument("--fps", type=int, default=24)
    r.add_argument("--no-captions", action="store_true")
    r.add_argument("--frames-per-layer", type=int, default=40)
    r.add_argument("--hold", type=float, default=1.0, help="seconds to hold at the end of each layer")
    r.add_argument("--quiet", action="store_true")
    _add_light_args(r)

    gq = sub.add_parser("guides")
    gq.add_argument("scene"); gq.add_argument("--size", type=int, default=600); gq.add_argument("--out", default=None)

    rl = sub.add_parser("relight")
    rl.add_argument("run"); rl.add_argument("--out", default=None); _add_light_args(rl)

    cr = sub.add_parser("crop")
    cr.add_argument("run"); cr.add_argument("--box", required=True, help="x0,y0,x1,y1 as fractions of width/height")
    cr.add_argument("--out", default=None); cr.add_argument("--unlit", action="store_true")
    cr.add_argument("--layer", type=int, default=None, help="crop a per-layer PNG instead of final.png")

    inf = sub.add_parser("info"); inf.add_argument("run")

    t = sub.add_parser("test")
    t.add_argument("which", nargs="*", default=["all"])

    ev = sub.add_parser("eval", add_help=False, help="evaluation harness: run | compare | reference")
    ev.add_argument("rest", nargs=argparse.REMAINDER)

    a = ap.parse_args(argv)
    if a.cmd == "eval":
        from .eval import main as eval_main
        sys.exit(eval_main(a.rest))
    if a.cmd == "render":
        from .render import render
        out = a.out or ("out/preview" if (a.preview or a.size is None) else f"out/render_{a.size[0]}")
        render(a.scene, size=a.size, preview=a.preview, plan_width=a.plan_width, seed=a.seed, out=out, upto=a.upto,
               mixer=a.mixer, do_light=not a.no_light, light_kw=_light_kw(a), save_layers=not a.no_layers,
               verbose=not a.quiet, strokes_file=a.strokes, only=a.only, from_run=a.from_run, timelapse=a.timelapse,
               video_size=a.video_size, fps=a.fps, captions=not a.no_captions, frames_per_layer=a.frames_per_layer,
               hold_seconds=a.hold)
    elif a.cmd == "guides":
        from .scene import load_scene
        from .render import write_guides
        S = load_scene(a.scene)
        out = a.out or "out/guides"
        write_guides(S, S.guides(a.size), out)
        print("guides written to", out, "(guides_sheet.png)")
    elif a.cmd == "relight":
        import numpy as np
        from . import light as Lt
        h = np.load(os.path.join(a.run, "height.npy")); rgb = np.load(os.path.join(a.run, "unlit.npy"))
        out = a.out or os.path.join(a.run, "relit.png")
        Lt.save_png(out, Lt.relight(rgb, h, **_light_kw(a)))
        print("written", out)
    elif a.cmd == "crop":
        import cv2
        x0, y0, x1, y1 = (float(v) for v in a.box.split(","))
        if a.layer is not None:
            import glob
            files = sorted(glob.glob(os.path.join(a.run, "layers", f"L{a.layer:02d}_*.png")))
            src = files[0]
        else:
            src = os.path.join(a.run, "final_unlit.png" if a.unlit else "final.png")
        img = cv2.imread(src)
        H, W = img.shape[:2]
        crop = img[int(y0 * H):int(y1 * H), int(x0 * W):int(x1 * W)]
        out = a.out or os.path.join(a.run, f"crop_{x0:.2f}_{y0:.2f}_{x1:.2f}_{y1:.2f}{'_unlit' if a.unlit else ''}.png")
        cv2.imwrite(out, crop)
        print(f"written {out} ({crop.shape[1]}x{crop.shape[0]} native px from {os.path.basename(src)})")
    elif a.cmd == "info":
        import json
        r = json.load(open(os.path.join(a.run, "run.json")))
        print(r.get("summary", ""))
        print("layers:", ", ".join(f"{i+1}:{nm} ({n})" for i, (nm, n) in enumerate(r["layers"])))
        print("timings:", {k: (round(v, 2) if isinstance(v, float) else v) for k, v in r["timings"].items()})
        print("gates:", {k: v["ok"] for k, v in r["metrics"]["gates"].items()})
        print(json.dumps({k: v for k, v in r["metrics"]["diagnostics"].items() if k != "strokes"}, indent=1, default=float)[:2500])
    elif a.cmd == "test":
        sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "tests"))
        import checkpoints
        checkpoints.main(a.which)


if __name__ == "__main__":
    main()
