"""Port check P1 (docs/plans/LIBRARY_PLAN.md, section 5): paint the same strokes with two kernel libraries behind
the brush.c ABI and compare every canvas plane.

Step 1 compares the new kernel (crates/oil-shim) with the Rust spike kernel (spikes/oilcore) built with `detmath`,
which uses the same pure-Rust exp/sin as oil-math: the port must be bit-identical. The spike itself was shown
bit-identical to strict-built C with platform libm (docs/plans/SPIKE_LOG.txt); `--ref libm` compares against that
build instead (expect small, chaotic differences from exp/sin only).

usage: python tools/port_check.py [--ref detmath|libm] [--sheet-width 1600] [--storm STROKES.npz] [--storm-width 600]
                                  [--json FILE]
"""
import argparse
import ctypes
import json
import os
import subprocess
import sys
import time

import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
sys.path.insert(0, ROOT)
os.environ.setdefault("OILPAINT_KERNEL", "oil")  # Canvas() loads a kernel on construction; paint() swaps it per run
from oilpaint import eval as E, light as Lt, mix  # noqa: E402
from oilpaint._build import BrushParams, CCanvas  # noqa: E402
from oilpaint.canvas import Canvas  # noqa: E402
from oilpaint.render import load_strokes  # noqa: E402
from oilpaint.scene import load_scene  # noqa: E402

DLL = (lambda stem: stem + ".dll") if sys.platform == "win32" else (lambda stem: "lib" + stem + (".dylib" if sys.platform == "darwin" else ".so"))
PLANES = ("lat", "rgb", "h", "wet", "cover")


def cargo(*args):
    subprocess.run([os.environ.get("CARGO", "cargo"), *args], check=True, cwd=ROOT)


def load(path):
    lib = ctypes.CDLL(path)
    fp = ctypes.POINTER(ctypes.c_float)
    lib.render_stroke.argtypes = [ctypes.POINTER(CCanvas), fp, ctypes.c_int, fp, fp, fp, ctypes.POINTER(BrushParams), fp]
    lib.render_stroke.restype = ctypes.c_int
    lib.render_strokes.argtypes = [ctypes.POINTER(CCanvas), fp, ctypes.POINTER(ctypes.c_int), ctypes.c_int, fp, fp, fp,
                                   ctypes.POINTER(BrushParams)]
    lib.render_strokes.restype = ctypes.c_int
    return lib


def libraries(ref):
    spike = os.path.join(ROOT, "spikes", "oilcore")
    tdir = os.path.join(spike, "target_detmath" if ref == "detmath" else "target")
    feats = ["--features", "detmath"] if ref == "detmath" else []
    cargo("build", "--release", "--offline", "--lib", "--manifest-path", os.path.join(spike, "Cargo.toml"), "--target-dir", tdir, *feats)
    cargo("build", "--release", "-p", "oil-shim")
    return {f"spike-{ref}": load(os.path.join(tdir, "release", DLL("oilcore"))),
            "oil-kernel": load(os.path.join(ROOT, "target", "release", DLL("oil_shim")))}


def paint(lib, layers, W, H, ground, hblur_always):
    cv = Canvas(W, H, ground)
    cv.lib = lib
    dt = 0.0
    for l in layers:
        if hblur_always or E._needs_hblur(l):
            cv.set_hblur(Lt.blur(cv.h, max(1.0, l["hblur_sigma"] * W)))
        t0 = time.perf_counter()
        if cv.render(l["strokes"]) < 0:
            raise RuntimeError("kernel failed")
        dt += time.perf_counter() - t0
        if l["dry_after"] is not None:
            cv.dry(l["dry_after"])
    return cv, dt


def compare(a, b):
    out = {}
    for name in PLANES:
        x, y = getattr(a, name), getattr(b, name)
        bits = x.view(np.uint32) != y.view(np.uint32)
        px = bits.reshape(a.H, a.W, -1).any(-1) if bits.ndim == 3 else bits
        d = np.abs(x.astype(np.float64) - y.astype(np.float64))
        out[name] = dict(pixels_differing=int(px.sum()), share=float(px.mean()), max_abs=float(d.max()))
    return out


def run(case, layers, W, H, ground, libs, hblur_always):
    print(f"{case}: {W}x{H}, {sum(len(l['strokes']) for l in layers)} strokes")
    canvases, secs = {}, {}
    for name, lib in libs.items():
        canvases[name], secs[name] = paint(lib, layers, W, H, ground, hblur_always)
        print(f"  {name}: kernel {secs[name]:.3f} s")
    a, b = canvases.values()
    res = compare(a, b)
    identical = all(r["pixels_differing"] == 0 for r in res.values())
    for name, r in res.items():
        print(f"  {name:6s} differing pixels {r['pixels_differing']:8d} ({100 * r['share']:.4f}%)  max |diff| {r['max_abs']:.3g}")
    print(f"  -> {'BIT-IDENTICAL' if identical else 'DIFFERENT'}")
    rgb8 = [np.round(np.clip(c.rgb, 0, 1) * 255).astype(np.int16) for c in (a, b)]
    return dict(size=[W, H], seconds=secs, planes=res, identical=identical,
                rgb8_max_diff=int(np.abs(rgb8[0] - rgb8[1]).max()),
                rgb8_share_over_1=float((np.abs(rgb8[0] - rgb8[1]).max(-1) > 1).mean()))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ref", choices=("detmath", "libm"), default="detmath")
    ap.add_argument("--sheet-width", type=int, default=1600)
    ap.add_argument("--storm", default=os.path.join(ROOT, "..", "storm-light-painting", "scene", "strokes.npz"))
    ap.add_argument("--storm-width", type=int, default=600)
    ap.add_argument("--json")
    a = ap.parse_args()
    mix.set_backend("mixbox")
    libs = libraries(a.ref)
    report = dict(ref=f"spike-{a.ref}", cases={})

    sh = E.sheet_layers(1907)
    swm = E.swatches_module()
    W = a.sheet_width
    H = int(round(W * swm.ASPECT[1] / swm.ASPECT[0]))
    report["cases"][f"sheet@{W}"] = run(f"sheet@{W}", sh.layers, W, H, mix.color_spec_to_rgb(swm.GROUND), libs, False)

    if a.storm and os.path.exists(a.storm):
        S = load_scene(os.path.join(ROOT, "scenes", "storm_v3.py"))
        strokes, ranges = load_strokes(a.storm)
        layers = [dict(strokes=strokes[s:e], hblur_sigma=S.layer_list[i]["hblur_sigma"], dry_after=S.layer_list[i].get("dry_after"))
                  for i, (s, e) in enumerate(ranges)]
        W = a.storm_width
        H = int(round(W * S.aspect[1] / S.aspect[0]))
        report["cases"][f"storm@{W}"] = run(f"storm@{W}", layers, W, H, S.ground, libs, True)

    if a.json:
        os.makedirs(os.path.dirname(os.path.abspath(a.json)), exist_ok=True)
        with open(a.json, "w") as f:
            json.dump(report, f, indent=1)
    return 0 if all(c["identical"] for c in report["cases"].values()) or a.ref == "libm" else 1


if __name__ == "__main__":
    sys.exit(main())
