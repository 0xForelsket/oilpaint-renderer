"""Evaluation harness ("measuring stick") for the oil-paint renderer.  See README.md, "Evaluating changes".

    python -m oilpaint eval run [--out DIR] [--width 1600] [--light default|painting|run] [--scene scenes/storm_v3.py] [--run RUNDIR]
    python -m oilpaint eval compare A.json B.json [--thresholds eval/thresholds.json] [--all]
    python -m oilpaint eval reference DIR [--out calibration.json]

`run` paints the swatch sheet (scenes/swatches.py: explicit strokes, so only the kernel and the lighting can move it) at --width,
lights it, and measures every swatch and the whole sheet; optionally it also evaluates real scenes (--scene renders the preview,
--run takes an existing render directory).  It writes eval.json, contact_sheet.png and summary.md.  `compare` diffs two eval.json
files against per-metric thresholds (a JSON file) and exits 1 on a regression.  `reference` measures a directory of real
paintings and writes a calibration JSON that could replace the guessed targets in oilpaint/calib.py.

Metric definitions live in eval_metrics.py (image-level) and eval_swatch.py (per swatch); the swatches in scenes/swatches.py.
"""
import argparse
import datetime
import fnmatch
import glob
import importlib.util
import json
import os
import platform
import subprocess
import sys
import time

import numpy as np
import cv2

from . import light as Lt
from . import mix
from . import eval_metrics as EM
from . import eval_swatch as ES
from .canvas import Canvas

HARNESS_VERSION = 1
DEFAULT_WIDTH = 1600
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SWATCH_PY = os.path.join(ROOT, "scenes", "swatches.py")
DEFAULT_THRESHOLDS = os.path.join(ROOT, "eval", "thresholds.json")

LIGHT_PRESETS = {
    "default": {},                                                       # the engine's LIGHT_DEFAULTS
    "painting": dict(bump=0.95, bump_fine=1.0, shade_blur=0.002, contrast=0.30, spec=0.10, cavity=0.02),   # README flags of the real painting
}
LIGHT_KEYS = ("spec", "bump", "contrast", "cavity", "weave_amp", "bump_fine", "shade_blur", "hsmooth")


# ------------------------------------------------------------------ small utilities
_SWM = {}


def swatches_module():
    if "m" not in _SWM:
        spec = importlib.util.spec_from_file_location("oilpaint_swatches", SWATCH_PY)
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        _SWM["m"] = mod
    return _SWM["m"]


def _clean(o):
    """JSON-safe copy: numpy scalars -> python, nan/inf -> None."""
    if isinstance(o, dict):
        return {str(k): _clean(v) for k, v in o.items()}
    if isinstance(o, (list, tuple)):
        return [_clean(v) for v in o]
    if isinstance(o, (np.bool_, bool)):
        return bool(o)
    if isinstance(o, (np.integer,)):
        return int(o)
    if isinstance(o, (np.floating, float)):
        f = float(o)
        return f if np.isfinite(f) else None
    return o


def _git_rev():
    try:
        r = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=ROOT, capture_output=True, text=True, timeout=10)
        return r.stdout.strip() or None
    except Exception:
        return None


def _light_kw(args):
    """Effective light parameters from --light preset + explicit flags.  `run` (use each run's own) returns None."""
    name = getattr(args, "light", "default") or "default"
    if name == "run":
        return None
    kw = dict(LIGHT_PRESETS[name])
    for k in LIGHT_KEYS:
        v = getattr(args, k, None)
        if v is not None:
            kw[k] = v
    return kw


def _light_name(args):
    n = getattr(args, "light", "default") or "default"
    ex = [k for k in LIGHT_KEYS if getattr(args, k, None) is not None]
    return n + ("+" + ",".join(f"{k}={getattr(args, k)}" for k in ex) if ex else "")


def _rss_mb(peak=True):
    """Resident memory in MB of this process: the high-water mark (VmHWM) or the current size (VmRSS).  /proc is used rather than
    ru_maxrss because ru_maxrss survives exec, so a child spawned from a big parent would report the parent's peak."""
    if sys.platform == "win32":
        import ctypes
        from ctypes import wintypes
        class Counters(ctypes.Structure):
            _fields_ = [("cb", wintypes.DWORD), ("faults", wintypes.DWORD)] + [
                (name, ctypes.c_size_t) for name in ("peak_working", "working", "peak_paged", "paged",
                    "peak_nonpaged", "nonpaged", "pagefile", "peak_pagefile")]
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.GetCurrentProcess.argtypes = []
        kernel.GetCurrentProcess.restype = wintypes.HANDLE
        kernel.K32GetProcessMemoryInfo.argtypes = [wintypes.HANDLE, ctypes.POINTER(Counters), wintypes.DWORD]
        kernel.K32GetProcessMemoryInfo.restype = wintypes.BOOL
        info = Counters(); info.cb = ctypes.sizeof(info)
        if not kernel.K32GetProcessMemoryInfo(kernel.GetCurrentProcess(), ctypes.byref(info), info.cb):
            raise ctypes.WinError(ctypes.get_last_error())
        return (info.peak_working if peak else info.working) / 1e6
    try:
        key = "VmHWM:" if peak else "VmRSS:"
        for line in open("/proc/self/status"):
            if line.startswith(key):
                return float(line.split()[1]) / 1e3
    except OSError:
        pass
    import resource
    v = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    return v / 1e6 if sys.platform == "darwin" else v / 1e3


# ------------------------------------------------------------------ the sheet: build, replay, save, load
def sheet_layers(seed, mixer="mixbox"):
    mix.set_backend(mixer)
    sh = swatches_module().build_sheet(seed)
    return sh


def _needs_hblur(layer):
    return any(s.params.get("mode") == "scumble" for s in layer["strokes"])


def replay(layers, W, H, ground_rgb):
    """Paint the layer list on a fresh canvas the way render.py does (hblur before a scumble layer, dry_after after).
    Returns (canvas, dict(seconds, strokes, npx)); `seconds` is kernel time only."""
    cv = Canvas(W, H, ground_rgb)
    dt = 0.0; npx = 0; n = 0
    for l in layers:
        if _needs_hblur(l):
            cv.set_hblur(Lt.blur(cv.h, max(1.0, l["hblur_sigma"] * W)))
        t0 = time.perf_counter()
        npx += cv.render(l["strokes"])
        dt += time.perf_counter() - t0
        n += len(l["strokes"])
        if l["dry_after"] is not None:
            cv.dry(l["dry_after"])
    return cv, dict(seconds=dt, strokes=n, npx=int(npx))


def steps_of(layers):
    steps = {}
    for l in layers:
        sw, step = l["name"].split(":", 1)
        steps.setdefault(sw, {}).setdefault(step, []).extend(l["strokes"])
    return steps


def save_sheet_run(out_dir, sh, cv, lit, W, H, seed, mixer):
    from .render import save_strokes
    os.makedirs(out_dir, exist_ok=True)
    swm = swatches_module()
    allst, ranges = [], []
    for li, l in enumerate(sh.layers):
        a = len(allst)
        for s in l["strokes"]:
            s.layer = li
        allst.extend(l["strokes"]); ranges.append((a, len(allst)))
    save_strokes(os.path.join(out_dir, "strokes.npz"), allst, ranges)
    man = swm.manifest_dict(sh)
    man["mixer"] = mixer
    with open(os.path.join(out_dir, "manifest.json"), "w") as f:
        json.dump(man, f, indent=1)
    np.save(os.path.join(out_dir, "unlit.npy"), cv.rgb)
    np.save(os.path.join(out_dir, "height.npy"), cv.h)
    Lt.save_png(os.path.join(out_dir, "final.png"), lit)
    Lt.save_png(os.path.join(out_dir, "final_unlit.png"), cv.rgb)
    with open(os.path.join(out_dir, "run.json"), "w") as f:
        json.dump(dict(scene=SWATCH_PY, size=[W, H], plan_width=W, seed=seed, mixer=mixer, strokes=len(allst), kind="swatch_sheet"), f, indent=1)


def load_sheet_run(run_dir):
    """Rebuild (layers, manifest) of a saved sheet run: strokes.npz + manifest.json."""
    from .render import load_strokes
    man = json.load(open(os.path.join(run_dir, "manifest.json")))
    strokes, ranges = load_strokes(os.path.join(run_dir, "strokes.npz"))
    layers = []
    for lm, (a, b) in zip(man["layers"], ranges):
        layers.append(dict(name=lm["name"], swatch=lm["swatch"], strokes=strokes[a:b], dry_after=lm["dry_after"], hblur_sigma=lm["hblur_sigma"]))
    return layers, man


# ------------------------------------------------------------------ measuring the sheet
def evaluate_sheet(lit, unlit, h, layers, man, W):
    ground = mix.color_spec_to_rgb(man["sheet"]["ground"])
    steps = steps_of(layers)
    out = {}
    for sw in man["swatches"]:
        ctx = ES.Ctx(sw["name"], sw["box"], W, lit, unlit, h, steps.get(sw["name"], {}), ground)
        out[sw["name"]] = ES.swatch_metrics(ctx, sw["kind"], lit, unlit, h)
    # the whole sheet, painted pixels only
    out["_overall"] = EM.image_metrics(lit, unlit, h, mask=h > EM.PAINT_H, detail=False)
    d = {}
    try:
        wet, dry = out["blend_wet_in_wet"]["pk030"], out["blend_wet_on_dry"]["pk030"]
        d["wet_dry_mix_contrast"] = wet["mix_t"] - dry["mix_t"]
        d["wet_dry_mix_contrast_pk060"] = out["blend_wet_in_wet"]["pk060"]["mix_t"] - out["blend_wet_on_dry"]["pk060"]["mix_t"]
        d["pickup_monotonic"] = bool(out["blend_wet_in_wet"]["pk012"]["mix_t"] <= wet["mix_t"] + 0.02 <= out["blend_wet_in_wet"]["pk060"]["mix_t"] + 0.04)
    except (KeyError, TypeError):
        pass
    try:
        d["pile_over_single_h"] = out["pile_up"]["pile_h_p50"] / max(out["mode_paint"]["w048"]["h_body"], 1e-6)
    except (KeyError, TypeError):
        pass
    out["_derived"] = d
    return out


def check_determinism(seed, mixer, width=400):
    """Same seed -> bit-identical strokes and image; different seed -> different strokes; planner reproducible."""
    swm = swatches_module()

    def sig(sh):
        return [(s.pts.tobytes(), s.zcol.tobytes(), s.zcol2.tobytes(), s.dz.tobytes(), s.seed, json.dumps(s.params, sort_keys=True, default=float))
                for l in sh.layers for s in l["strokes"]]

    mix.set_backend(mixer)
    a, b, c = swm.build_sheet(seed), swm.build_sheet(seed), swm.build_sheet(seed + 1)
    same = sig(a) == sig(b)
    differs = sig(a) != sig(c)
    H = int(round(width * swm.ASPECT[1] / swm.ASPECT[0]))
    ground = mix.color_spec_to_rgb(swm.GROUND)
    cv1, _ = replay(a.layers, width, H, ground)
    cv2_, _ = replay(b.layers, width, H, ground)
    out = dict(strokes_identical=bool(same), seed_changes_strokes=bool(differs), replay_max_abs_diff_rgb=float(np.abs(cv1.rgb - cv2_.rgb).max()),
               replay_max_abs_diff_h=float(np.abs(cv1.h - cv2_.h).max()))
    out["image_near_identical"] = bool(out["replay_max_abs_diff_rgb"] < 1e-4 and out["replay_max_abs_diff_h"] < 1e-4)
    out["planner_identical"] = _planner_repro()
    return out


def _planner_repro():
    """A tiny planned scene (2 layers, ~1 s) planned twice with one seed: strokes must be bit-identical."""
    from .maps import GuideMaps, structure_flow
    from .planner import Planner, Style, Layer
    Wp, Hp = 240, 300
    y, x = np.mgrid[0:Hp, 0:Wp].astype(np.float32)
    X, Y = x / Wp, y / Wp
    t = np.clip(Y / 1.25, 0, 1)[..., None]
    target = np.array([0.25, 0.30, 0.55], np.float32) * (1 - t) + np.array([0.95, 0.80, 0.65], np.float32) * t
    d = np.hypot(X - 0.5, Y - 0.7)
    target = np.where((d < 0.2)[..., None], np.array([0.6, 0.25, 0.25], np.float32), target).astype(np.float32)
    style = Style(colors=None, snap=0.0, width=(0.03, 0.05), length=(0.08, 0.16), pickup=0.15, jitter=(2.0, 1.5))
    layers = [Layer("a", T=10, fs=0.6), Layer("b", T=8, fs=0.4, width=(0.015, 0.025), length=(0.04, 0.08))]
    res = []
    for _ in range(2):
        # (GuideMaps.from_arrays(target) with no flow crashes: structure_flow returns (flow, aniso).  Engine bug, reported not fixed.)
        g = GuideMaps.from_arrays(target, flow=structure_flow(target, sigma_px=0.03 * Wp)[0])
        pl = Planner(g, {"all": style}, layers, seed=7)
        pl.plan()
        res.append([(s.pts.tobytes(), s.zcol.tobytes(), s.seed) for s in pl.strokes])
    return bool(res[0] == res[1] and len(res[0]) > 20)


def probe_memory(width, mixer):
    """Peak RSS of paint + light of the swatch sheet in a fresh process (the engine's own footprint, not the harness's)."""
    cmd = [sys.executable, "-m", "oilpaint", "eval", "_probe", "--width", str(width), "--mixer", mixer]
    try:
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, timeout=300)
        line = [l for l in r.stdout.strip().splitlines() if l.startswith("{")][-1]
        d = json.loads(line)
        d["engine_mb"] = d["peak_mb"] - d["baseline_mb"]
        d["engine_mb_per_mpx"] = d["engine_mb"] / max(d["mpx"], 1e-6)
        return d
    except Exception as e:
        return dict(error=f"{type(e).__name__}: {e}")


def _probe(args):
    swm = swatches_module()
    mix.set_backend(args.mixer)
    sh = swm.build_sheet(swm.SEED)
    base = _rss_mb(peak=False)          # imports, scene built, nothing painted yet
    W = args.width
    H = int(round(W * swm.ASPECT[1] / swm.ASPECT[0]))
    cv, _ = replay(sh.layers, W, H, mix.color_spec_to_rgb(swm.GROUND))
    Lt.relight(cv.rgb, cv.h)
    print(json.dumps(dict(baseline_mb=base, peak_mb=_rss_mb(), mpx=W * H / 1e6, width=W)))


# ------------------------------------------------------------------ scenes (real pictures)
def scene_run_metrics(run_dir, light_kw, speed=True, min_region=0.025):
    """Metrics of a render directory (unlit.npy, height.npy, strokes.npz, run.json): whole image + per region + strokes."""
    from .render import load_strokes
    from .scene import load_scene
    run = json.load(open(os.path.join(run_dir, "run.json")))
    unlit = np.load(os.path.join(run_dir, "unlit.npy"))
    h = np.load(os.path.join(run_dir, "height.npy"))
    H, W = h.shape
    lk = dict(run.get("light", {})) if light_kw is None else dict(light_kw)
    if light_kw is None:
        lk = {k: (tuple(v) if isinstance(v, list) else v) for k, v in lk.items()}
    t0 = time.perf_counter()
    lit = Lt.relight(unlit, h, **lk)
    t_light = time.perf_counter() - t0
    lab = EM.lab_of(lit)
    maps = EM.structure_maps(lab)
    out = dict(size=[W, H], strokes=run.get("strokes"), plan_seconds=run.get("timings", {}).get("plan"), light_seconds=t_light,
               overall=EM.image_metrics(lit, unlit, h, mask=None, lab=lab, maps=maps), regions={})
    S = load_scene(run["scene"])
    Wp = run.get("plan_width", min(W, 600))
    g = S.guides(Wp)
    rid = g.region_id if Wp == W else cv2.resize(g.region_id, (W, H), interpolation=cv2.INTER_NEAREST)
    for i, name in enumerate(g.names):
        mask = rid == i
        if mask.mean() >= min_region:
            out["regions"][name] = EM.image_metrics(lit, unlit, h, mask=mask, lab=lab, maps=maps)
    strokes, ranges = load_strokes(os.path.join(run_dir, "strokes.npz"))
    out["stroke_stats"] = EM.stroke_stats(strokes, g.names)
    rt = run.get("timings", {}).get("render")
    if speed and strokes and rt:         # kernel replay time the engine recorded for this run (no second replay: a 1600 px scene takes ~18 s)
        out["speed"] = dict(replay_seconds=rt, strokes_per_s=len(strokes) / rt, note="single run, from run.json")
    out["_lit"] = lit
    return out


def render_scene(scene, out_dir, width, reuse):
    """Render `scene` (preview at 600, or replay of the preview's strokes at `width`) into out_dir; returns the run directory."""
    from .render import render
    from .scene import load_scene
    stem = os.path.splitext(os.path.basename(scene))[0]
    prev = os.path.join(out_dir, f"{stem}_600")
    if not (reuse and os.path.exists(os.path.join(prev, "run.json"))):
        render(scene, preview=True, out=prev, do_light=False, save_layers=False, verbose=False)
    if width in (None, 600):
        return prev, f"{stem}@600"
    full = os.path.join(out_dir, f"{stem}_{width}")
    if not (reuse and os.path.exists(os.path.join(full, "run.json"))):
        S = load_scene(scene)
        render(scene, size=(width, S.size(width)[1]), strokes_file=os.path.join(prev, "strokes.npz"), out=full, do_light=False,
               save_layers=False, verbose=False)
    return full, f"{stem}@{width}"


# ------------------------------------------------------------------ outputs
HEADLINE = ("hairline", "bristle_L", "edge_step_p99", "h_p95", "C_mean")


def _tile(items, cols=3, cell=(560, 360), pad=6, strip=22):
    """Contact sheet: each image gets a dark title strip above it (never over the paint), so labels stay legible on any ground."""
    cw, ch = cell
    rows = (len(items) + cols - 1) // cols
    out = np.full((rows * (ch + strip + pad) + pad, cols * (cw + pad) + pad, 3), 40, np.uint8)
    for i, (title, img) in enumerate(items):
        h, w = img.shape[:2]
        sc = min(cw / w, ch / h)
        nw, nh = max(1, int(w * sc)), max(1, int(h * sc))
        im = cv2.resize(img, (nw, nh), interpolation=cv2.INTER_AREA if sc < 1 else cv2.INTER_NEAREST)
        r, c = divmod(i, cols)
        y0 = pad + r * (ch + strip + pad)
        x0 = pad + c * (cw + pad)
        cv2.putText(out, title, (x0 + 2, y0 + 16), cv2.FONT_HERSHEY_SIMPLEX, 0.5, (235, 235, 235), 1, cv2.LINE_AA)
        out[y0 + strip:y0 + strip + nh, x0:x0 + nw] = im
    return out


def contact_sheet(path, lit, man, W, res, scene_imgs):
    items = []
    for sw in man["swatches"]:
        x0, y0, x1, y1 = sw["box"]
        crop = Lt.to8(lit[int(y0 * W):int(y1 * W), int(x0 * W):int(x1 * W)])
        m = res.get(sw["name"], {})
        bits = [f"{sw['name']}"]
        if m.get("hairline") is not None:
            bits.append(f"hair {m['hairline']:.3f}")
        if m.get("bristle_L") is not None:
            bits.append(f"brs {m['bristle_L']:.2f}")
        items.append((" | ".join(bits), crop))
    for name, img in scene_imgs:
        items.append((name, Lt.to8(img)))
    cv2.imwrite(path, cv2.cvtColor(_tile(items, cols=3, cell=(560, 360)), cv2.COLOR_RGB2BGR))


def _f(v, fmt="{:.3f}"):
    return "-" if v is None else fmt.format(v)


def write_summary(path, doc):
    L = []
    c = doc["config"]
    L.append(f"# oilpaint eval `{doc['config'].get('name', '')}`\n")
    L.append(f"harness v{doc['harness']['version']}, engine `{doc['harness'].get('git')}`, width {c['width']}px, seed {c['seed']}, mixer {c['mixer']}, light `{c['light_name']}`\n")
    if doc.get("sheet"):
        L.append("## Swatch sheet\n")
        L.append("| swatch | hairline | hairline_lit | bristle_L | bristle_h | edge_step_p99 | h_p95 | C_p90 |\n|---|---|---|---|---|---|---|---|")
        for name, m in doc["sheet"].items():
            if name.startswith("_"):
                continue
            L.append(f"| {name} | {_f(m.get('hairline'))} | {_f(m.get('hairline_lit'))} | {_f(m.get('bristle_L'), '{:.2f}')} | {_f(m.get('bristle_h'), '{:.3f}')} | "
                     f"{_f(m.get('edge_step_p99'))} | {_f(m.get('h_p95'), '{:.2f}')} | {_f(m.get('C_p90'), '{:.1f}')} |")
        o = doc["sheet"].get("_overall", {})
        L.append(f"| **sheet (painted px)** | {_f(o.get('hairline'))} | {_f(o.get('hairline_lit'))} | {_f(o.get('bristle_L'), '{:.2f}')} | {_f(o.get('bristle_h'), '{:.3f}')} | "
                 f"{_f(o.get('edge_step_p99'))} | {_f(o.get('h_p95'), '{:.2f}')} | {_f(o.get('C_p90'), '{:.1f}')} |")
        L.append("\nDerived: " + ", ".join(f"{k} = {_f(v) if not isinstance(v, bool) else v}" for k, v in doc["sheet"].get("_derived", {}).items()))
        for name in ("blend_wet_in_wet", "blend_wet_on_dry", "blend_complementary"):
            m = doc["sheet"].get(name)
            if m:
                L.append(f"\n**{name}** (mix_t / off_curve_dE / chroma_vs_curve): " + "; ".join(
                    f"{k}: {_f(v.get('mix_t'), '{:.2f}')} / {_f(v.get('off_curve_dE'), '{:.1f}')} / {_f(v.get('chroma_vs_curve'), '{:.2f}')}"
                    for k, v in m.items() if isinstance(v, dict)))
    for sname, s in (doc.get("scenes") or {}).items():
        o = s["overall"]
        L.append(f"\n## Scene `{sname}` ({s['size'][0]}x{s['size'][1]}, {s.get('strokes')} strokes)\n")
        L.append("| region | hairline | hairline_lit | bristle_L | edge_step_p99 | C_mean | L_p50 | coh_local | dir_R | orient | acf_minor | acf_major |\n|---|---|---|---|---|---|---|---|---|---|---|---|")
        rows = [("(whole image)", o)] + list(s["regions"].items())
        for name, m in rows:
            L.append(f"| {name} | {_f(m.get('hairline'))} | {_f(m.get('hairline_lit'))} | {_f(m.get('bristle_L'), '{:.2f}')} | {_f(m.get('edge_step_p99'))} | "
                     f"{_f(m.get('C_mean'), '{:.1f}')} | {_f(m.get('L_p50'), '{:.0f}')} | {_f(m.get('coh_local'), '{:.2f}')} | {_f(m.get('dir_R'), '{:.2f}')} | "
                     f"{_f(m.get('orient_deg'), '{:.0f}')} | {_f(m.get('acf_minor'), '{:.3f}')} | {_f(m.get('acf_major'), '{:.3f}')} |")
        if not o.get("bristle_resolved", True):
            L.append(f"\n_Width {s['size'][0]} px is below {EM.RESOLVED_W}: bristle and hairline scales are not resolved (compare only against runs at the same width)._")
        if s.get("speed"):
            L.append(f"\nReplay: {s['speed']['strokes_per_s']:.0f} strokes/s (single run, includes the engine's per-layer work).")
    sysd = doc.get("system", {})
    if sysd:
        L.append("\n## System\n")
        for k, v in sysd.items():
            L.append(f"- **{k}**: " + ", ".join(f"{a} = {(_f(b) if isinstance(b, float) else b)}" for a, b in v.items()))
    with open(path, "w") as f:
        f.write("\n".join(L) + "\n")


# ------------------------------------------------------------------ eval run
def cmd_run(args):
    t_start = time.perf_counter()
    W = args.width
    seed = args.seed
    mixer = args.mixer
    lk = _light_kw(args)
    name = args.name or time.strftime("%Y%m%d_%H%M%S")
    out = args.out or os.path.join(ROOT, "out", "eval", name)
    os.makedirs(out, exist_ok=True)
    swm = swatches_module()
    doc = dict(harness=dict(version=HARNESS_VERSION, git=_git_rev(), created=datetime.datetime.now().isoformat(timespec="seconds"),
                            python=platform.python_version(), cpu_count=os.cpu_count()),
               config=dict(name=name, width=W, seed=seed, mixer=mixer, light_name=_light_name(args), light=None, argv=sys.argv[1:]))
    scene_imgs = []
    lit = None
    man = None
    res = None
    # ---- the swatch sheet
    if not args.no_sheet:
        sheet_dir = os.path.join(out, "sheet")
        if args.sheet_run:                                   # re-evaluate a saved sheet (e.g. under another light)
            layers, man = load_sheet_run(args.sheet_run)
            unlit = np.load(os.path.join(args.sheet_run, "unlit.npy")); h = np.load(os.path.join(args.sheet_run, "height.npy"))
            H, W = h.shape
            mixer = man.get("mixer", mixer)
            doc["config"].update(width=W, seed=man["sheet"]["seed"], mixer=mixer, source_run=os.path.abspath(args.sheet_run))
            speed_doc = None
        else:
            sh = sheet_layers(seed, mixer)
            man = swm.manifest_dict(sh)
            man["mixer"] = mixer
            layers = sh.layers
            H = int(round(W * swm.ASPECT[1] / swm.ASPECT[0]))
            ground = mix.color_spec_to_rgb(swm.GROUND)
            cv, t1 = replay(layers, W, H, ground)
            unlit, h = cv.rgb, cv.h
            times = [t1["seconds"]]
            if not args.no_speed:
                for _ in range(2):
                    _, tt = replay(layers, W, H, ground)
                    times.append(tt["seconds"])
            speed_doc = dict(strokes=t1["strokes"], replay_seconds=min(times), strokes_per_s=t1["strokes"] / min(times),
                             mpix_per_s=t1["npx"] / min(times) / 1e6, timings_n=len(times))
        kw = lk if lk is not None else {}
        t0 = time.perf_counter()
        lit = Lt.relight(unlit, h, **kw)
        t_light = time.perf_counter() - t0
        doc["config"]["light"] = dict(Lt.LIGHT_DEFAULTS, **kw)
        res = evaluate_sheet(lit, unlit, h, layers, man, W)
        doc["manifest"] = man["swatches"]
        doc["sheet"] = res
        doc["sheet_size"] = [W, H]
        if not args.sheet_run:
            save_sheet_run(sheet_dir, sh, cv, lit, W, H, seed, mixer)
        doc.setdefault("system", {})
        if speed_doc:
            speed_doc["light_seconds"] = t_light
            doc["system"]["speed"] = speed_doc
        del unlit, h
    # ---- real scenes
    scene_docs = {}
    targets = []
    for scene in (args.scene or []):
        for i, wd in enumerate(int(v) for v in str(args.scene_width).split(",")):
            rd, sname = render_scene(scene, os.path.join(out, "scenes"), wd, args.reuse or i > 0)      # the 600 px plan is made once
            targets.append((rd, sname))
    for rd in (args.run or []):
        if not os.path.exists(os.path.join(rd, "run.json")):
            raise SystemExit(f"--run {rd}: not a scene run directory (no run.json); use --sheet-run for a swatch-sheet run")
        rj = json.load(open(os.path.join(rd, "run.json")))
        targets.append((rd, os.path.splitext(os.path.basename(rj.get("scene", os.path.basename(os.path.normpath(rd)))))[0] + f"@{rj['size'][0]}"))
    for rd, sname in targets:
        s = scene_run_metrics(rd, lk, speed=not args.no_speed)
        limg = s.pop("_lit")
        scene_docs[sname] = s
        if lk is not None and doc["config"]["light"] is None:
            doc["config"]["light"] = dict(Lt.LIGHT_DEFAULTS, **lk)
        thumb = limg if limg.shape[1] <= 1200 else cv2.resize(limg, (1200, int(limg.shape[0] * 1200 / limg.shape[1])), interpolation=cv2.INTER_AREA)
        scene_imgs.append((sname, thumb))
        Lt.save_png(os.path.join(out, f"scene_{sname.replace('@', '_')}_lit.png"), limg)
        del limg
    if scene_docs:
        doc["scenes"] = scene_docs
        if args.no_sheet:
            doc["config"]["width"] = int(next(iter(scene_docs.values()))["size"][0])
    # ---- system checks
    if not args.no_sheet:
        if not args.no_checks and not args.sheet_run:
            doc["system"]["determinism"] = check_determinism(seed, mixer)
            if not args.no_probe:
                doc["system"]["memory"] = probe_memory(W, mixer)
    doc["config"]["seconds"] = time.perf_counter() - t_start
    doc = _clean(doc)
    with open(os.path.join(out, "eval.json"), "w") as f:
        json.dump(doc, f, indent=1)
    if lit is not None:
        contact_sheet(os.path.join(out, "contact_sheet.png"), lit, man, W, res, scene_imgs)
        Lt.save_png(os.path.join(out, "sheet_lit.png"), lit)
    write_summary(os.path.join(out, "summary.md"), doc)
    print(f"[oilpaint eval] {name}: {out}/eval.json  ({doc['config']['seconds']:.1f}s, width {doc['config']['width']}, light {doc['config']['light_name']})")
    if not args.quiet:
        print(open(os.path.join(out, "summary.md")).read())
    return 0


# ------------------------------------------------------------------ eval compare
def flatten(d, prefix="", out=None):
    out = {} if out is None else out
    for k, v in d.items():
        if k in ("manifest", "argv", "hue_hist24") or (not prefix and k in ("config", "harness")):
            continue
        key = f"{prefix}.{k}" if prefix else str(k)
        if isinstance(v, dict):
            flatten(v, key, out)
        elif isinstance(v, bool):
            out[key] = v
        elif isinstance(v, (int, float)) and v is not None:
            out[key] = float(v)
    return out


def _tol(spec, a):
    if spec is None:
        return None
    if isinstance(spec, (int, float)):
        return float(spec)
    return max(float(spec.get("abs", 0.0)), float(spec.get("rel", 0.0)) * abs(a))


def load_eval(path):
    if os.path.isdir(path):
        path = os.path.join(path, "eval.json")
    return json.load(open(path))


def compare_docs(A, B, rules, show_all=False):
    fa, fb = flatten(A), flatten(B)
    rows = []
    for key in sorted(set(fa) | set(fb)):
        rule = next((r for r in rules if fnmatch.fnmatchcase(key, r["match"])), None)
        if rule is None or rule.get("direction") == "info":
            continue
        a, b = fa.get(key), fb.get(key)
        d = rule["direction"]
        if d == "true":
            if b is None:
                continue
            st = "ok" if b is True else "FAIL"
            rows.append((st, key, a, b, None, rule)); continue
        if a is None or b is None or isinstance(a, bool) or isinstance(b, bool):
            if a is not None and b is None:
                rows.append(("WARN", key, a, None, None, rule))
            continue
        delta = b - a
        worse = {"lower": delta, "higher": -delta, "match": abs(delta)}[d]
        tw, tf = _tol(rule.get("warn"), a), _tol(rule.get("fail"), a)
        if tf is not None and worse > tf:
            st = "FAIL"
        elif tw is not None and worse > tw:
            st = "WARN"
        elif d != "match" and -worse > (tw if tw is not None else (tf or 0.0)) and (tw or tf):
            st = "better"
        else:
            st = "ok"
        rows.append((st, key, a, b, delta, rule))
    order = {"FAIL": 0, "WARN": 1, "better": 2, "ok": 3}
    rows.sort(key=lambda r: (order[r[0]], r[1]))
    return rows


def cmd_compare(args):
    A, B = load_eval(args.a), load_eval(args.b)
    th = json.load(open(args.thresholds or DEFAULT_THRESHOLDS))
    wa, wb = A["config"]["width"], B["config"]["width"]
    if wa != wb and not args.force:
        print(f"error: the two runs used different widths ({wa} vs {wb}); resolution-dependent metrics are not comparable. Re-run at the same --width (or --force).",
              file=sys.stderr)
        return 2
    rows = compare_docs(A, B, th["rules"], args.all)
    cnt = {k: sum(1 for r in rows if r[0] == k) for k in ("FAIL", "WARN", "better", "ok")}
    print(f"compare  A={args.a}  ({A['config'].get('light_name')}, git {A['harness'].get('git')})\n         B={args.b}  ({B['config'].get('light_name')}, git {B['harness'].get('git')})")
    fmt = lambda v: "-" if v is None else (str(v) if isinstance(v, bool) else f"{v:.4g}")
    shown = [r for r in rows if r[0] in ("FAIL", "WARN") or args.all or (args.better and r[0] == "better")]
    # digest by metric family (the last component of the key), so a change that moves one metric in 17 swatches reads as one line
    fam = {}
    for r in rows:
        f = fam.setdefault(r[1].rsplit(".", 1)[-1], dict(FAIL=0, WARN=0, better=0, ok=0))
        f[r[0]] += 1
    dig = [(k, v) for k, v in fam.items() if v["FAIL"] or v["WARN"] or v["better"]]
    if dig:
        print("by metric:  " + "   ".join(f"{k}: " + "/".join(f"{v[s]}{s[0].lower() if s != 'better' else '+'}" for s in ("FAIL", "WARN", "better") if v[s])
                                          for k, v in sorted(dig, key=lambda kv: (-kv[1]["FAIL"], -kv[1]["WARN"], kv[0]))))
        print("            (f = FAIL, w = WARN, + = better; counts are swatches / regions)\n")
    print(f"{'status':7s} {'metric':62s} {'A':>10s} {'B':>10s} {'delta':>10s}  rule")
    top = args.top if args.top and args.top > 0 else len(shown)
    for st, key, a, b, dl, rule in shown[:top]:
        print(f"{st:7s} {key:62s} {fmt(a):>10s} {fmt(b):>10s} {('' if dl is None else f'{dl:+.4g}'):>10s}  {rule['direction']}")
    if len(shown) > top:
        print(f"... {len(shown) - top} more rows (--top 0 for all, --json FILE for everything)")
    print(f"\n{cnt['FAIL']} FAIL, {cnt['WARN']} WARN, {cnt['better']} better, {cnt['ok']} ok  ({len(rows)} rule-covered metrics)")
    bad = cnt["FAIL"] + (cnt["WARN"] if args.strict else 0)
    if args.json:
        json.dump(_clean(dict(counts=cnt, rows=[dict(status=r[0], key=r[1], a=r[2], b=r[3], delta=r[4]) for r in rows])), open(args.json, "w"), indent=1)
    print("RESULT: " + ("REGRESSION" if bad else "no regression"))
    return 1 if bad else 0


# ------------------------------------------------------------------ eval reference
IMG_EXT = (".jpg", ".jpeg", ".png", ".tif", ".tiff", ".webp", ".bmp")
CALIB_MAP = dict(L_p01="L_p01", L_p99="L_p99", L_p50="L_p50", chroma_mean="C_mean", chroma_p90="C_p90")


def load_image(path, width):
    im = cv2.imread(path, cv2.IMREAD_COLOR)
    if im is None:
        raise ValueError(f"cannot read {path}")
    im = cv2.cvtColor(im, cv2.COLOR_BGR2RGB).astype(np.float32) / 255.0
    H0, W0 = im.shape[:2]
    if W0 > width:
        im = cv2.resize(im, (width, max(1, int(round(H0 * width / W0)))), interpolation=cv2.INTER_AREA)
    return im, (W0, H0)


def cmd_reference(args):
    files = sorted(f for f in glob.glob(os.path.join(args.dir, "*")) if f.lower().endswith(IMG_EXT))
    if not files:
        print(f"error: no images in {args.dir}", file=sys.stderr)
        return 2
    prov = {}
    pj = os.path.join(args.dir, "PROVENANCE.json")
    if os.path.exists(pj):
        prov = {p["file"]: p for p in json.load(open(pj))}
    imgs = {}
    for f in files:
        im, orig = load_image(f, args.width)
        m = EM.image_metrics(im, mask=None, detail=True)
        m["original_px"] = list(orig)
        m["analysed_px"] = [im.shape[1], im.shape[0]]
        if os.path.basename(f) in prov:
            p = prov[os.path.basename(f)]
            m["provenance"] = {k: p.get(k) for k in ("title", "artist", "date", "source_page", "object_page", "licence", "download_url", "image_url")}
        imgs[os.path.basename(f)] = m
        print(f"  {os.path.basename(f)[:58]:58s} {im.shape[1]}x{im.shape[0]}  L50 {m['L_p50']:.0f}  C_mean {m['C_mean']:.1f}  bristle_L {m.get('bristle_L', float('nan')):.2f}  coh {m.get('coh_local', float('nan')):.2f}")
    keys = sorted({k for m in imgs.values() for k, v in m.items() if isinstance(v, (int, float)) and not isinstance(v, bool)
                   and k not in ("width_px", "area_cw2")})
    summary = {}
    for k in keys:
        v = np.array([m[k] for m in imgs.values() if isinstance(m.get(k), (int, float))], np.float64)
        if len(v):
            summary[k] = dict(n=int(len(v)), mean=float(v.mean()), std=float(v.std()), min=float(v.min()), max=float(v.max()),
                              p10=float(np.percentile(v, 10)), p50=float(np.percentile(v, 50)), p90=float(np.percentile(v, 90)))
    sugg = {}
    for ck, mk in CALIB_MAP.items():
        s = summary.get(mk)
        if s:
            sugg[ck] = [round(s["p10"], 2), round(s["p90"], 2)] if ck in ("L_p50", "chroma_mean", "chroma_p90") else round(s["mean"], 2)
    doc = dict(harness_version=HARNESS_VERSION, kind="reference_calibration", synthetic=bool(args.synthetic), analysed_width=args.width,
               n_images=len(imgs), directory=os.path.basename(os.path.abspath(args.dir)), images=imgs, summary=summary, suggested_calib_targets=sugg,
               notes=("Metrics of the LIT image only (a scan or photograph has no albedo or height), at the analysed width; JPEG artefacts, glare and "
                      "the varnish/photography of each source are included.  calib.TARGETS keys: L_p50, chroma_mean and chroma_p90 take the p10..p90 range "
                      "over the images, L_p01 and L_p99 the mean.  Nothing here has been written into calib.py."))
    if args.synthetic:
        doc["notes"] = "SYNTHETIC STAND-INS, NOT REAL PAINTINGS: only for testing the tool.  " + doc["notes"]
    out = args.out or os.path.join(ROOT, "eval", "reference_calibration.json")
    os.makedirs(os.path.dirname(os.path.abspath(out)), exist_ok=True)
    with open(out, "w") as f:
        json.dump(_clean(doc), f, indent=1)
    print(f"[oilpaint eval] reference calibration of {len(imgs)} images -> {out}")
    for k in ("L_p50", "C_mean", "C_p90", "bristle_L", "bristle_frac", "hairline_lit", "coh_local", "acf_minor", "acf_major"):
        if k in summary:
            s = summary[k]
            print(f"  {k:14s} mean {s['mean']:8.3f}  p10..p90 {s['p10']:8.3f} .. {s['p90']:8.3f}")
    return 0


# ------------------------------------------------------------------ CLI
def build_parser():
    ap = argparse.ArgumentParser(prog="python -m oilpaint eval", description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("run", help="paint + measure the swatch sheet (and optionally real scenes)")
    r.add_argument("--out", default=None, help="output directory (default out/eval/<name>)")
    r.add_argument("--name", default=None)
    r.add_argument("--width", type=int, default=DEFAULT_WIDTH, help=f"sheet width in px (default {DEFAULT_WIDTH}; below {EM.RESOLVED_W} bristle/hairline scales are unresolved)")
    r.add_argument("--seed", type=int, default=1907)
    r.add_argument("--mixer", choices=["mixbox", "rgb", "mixbox-material", "ochrell"], default="mixbox")
    r.add_argument("--light", choices=["default", "painting", "run"], default="default",
                   help="light preset: default = engine defaults, painting = the README flags of the real painting, run = each run's own recorded light")
    for k in LIGHT_KEYS:
        r.add_argument("--" + k.replace("_", "-"), dest=k, type=float, default=None)
    r.add_argument("--no-sheet", action="store_true", help="skip the swatch sheet (scenes only)")
    r.add_argument("--sheet-run", default=None, help="re-evaluate a saved sheet run directory (relight only, no painting)")
    r.add_argument("--scene", action="append", help="render this scene's preview and evaluate it (repeatable)")
    r.add_argument("--scene-width", default="1600", help="width(s) to evaluate the scene at, comma separated: 600 = the preview itself, others = replay of its strokes (default 1600; the bristle and hairline scales are unresolved below 1250)")
    r.add_argument("--run", action="append", help="evaluate an existing render directory (repeatable)")
    r.add_argument("--reuse", action="store_true", help="reuse scene renders already in the output directory")
    r.add_argument("--no-speed", action="store_true", help="skip the repeated replays that measure speed")
    r.add_argument("--no-checks", action="store_true", help="skip determinism and memory checks")
    r.add_argument("--no-probe", action="store_true", help="skip the fresh-process memory probe")
    r.add_argument("--quiet", action="store_true", help="do not print the summary")
    c = sub.add_parser("compare", help="diff two eval.json files against thresholds; exit 1 on regression")
    c.add_argument("a"); c.add_argument("b")
    c.add_argument("--thresholds", default=None, help=f"thresholds JSON (default {os.path.relpath(DEFAULT_THRESHOLDS, ROOT)})")
    c.add_argument("--all", action="store_true", help="print every rule-covered metric, not just changes")
    c.add_argument("--top", type=int, default=30, help="print at most this many rows (worst first; 0 = all)")
    c.add_argument("--better", action="store_true", help="also print improvements")
    c.add_argument("--strict", action="store_true", help="treat WARN as failure")
    c.add_argument("--force", action="store_true", help="compare runs of different widths")
    c.add_argument("--json", default=None, help="write the rows to this file")
    f = sub.add_parser("reference", help="measure a directory of real paintings; write a calibration JSON")
    f.add_argument("dir"); f.add_argument("--out", default=None); f.add_argument("--width", type=int, default=DEFAULT_WIDTH)
    f.add_argument("--synthetic", action="store_true", help="mark the images as synthetic stand-ins (testing only)")
    p = sub.add_parser("_probe"); p.add_argument("--width", type=int, default=DEFAULT_WIDTH); p.add_argument("--mixer", default="mixbox")
    return ap


def main(argv=None):
    a = build_parser().parse_args(argv)
    if a.cmd == "run":
        return cmd_run(a)
    if a.cmd == "compare":
        return cmd_compare(a)
    if a.cmd == "reference":
        return cmd_reference(a)
    if a.cmd == "_probe":
        _probe(a)
        return 0


if __name__ == "__main__":
    sys.exit(main())
