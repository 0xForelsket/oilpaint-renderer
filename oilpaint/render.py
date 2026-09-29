"""End-to-end: scene -> guides -> plan (once, at plan size) -> replay at render size -> light -> outputs.

Iteration helpers: per-layer proxy states are saved at plan size so `--only N --from <run>` can re-plan a
single layer over the saved state of the layers before it (the layers after it are replayed unchanged).
"""
import json
import os
import time
import numpy as np
import cv2

from . import mix, light as L, metrics
from .canvas import Canvas, Stroke
from .maps import flow_preview, regions_preview
from .planner import Planner
from .scene import load_scene
from .sheet import save_sheet

MIXBOX_ATTRIBUTION = "Colour mixing: Mixbox (c) 2022 Secret Weapons, CC BY-NC 4.0, https://scrtwpns.com/mixbox (non-commercial use)"


# ------------------------------------------------------------------ guides
def write_guides(S, g, out):
    os.makedirs(out, exist_ok=True)
    L.save_png(os.path.join(out, "guide_target.png"), g.target)
    cv2.imwrite(os.path.join(out, "guide_regions.png"), cv2.cvtColor(regions_preview(g.region_id, g.names), cv2.COLOR_RGB2BGR))
    cv2.imwrite(os.path.join(out, "guide_flow.png"), cv2.cvtColor(flow_preview(g.flow, g.W, g.H, bg=g.target * 0.5 + 0.5), cv2.COLOR_RGB2BGR))
    L.save_gray(os.path.join(out, "guide_light.png"), g.light, 1.0)
    items = [(n, cv2.cvtColor(cv2.imread(os.path.join(out, f"guide_{n}.png")), cv2.COLOR_BGR2RGB))
             for n in ("target", "regions", "flow", "light")]
    save_sheet(os.path.join(out, "guides_sheet.png"), items, cols=4, cell=(360, 450))


# ------------------------------------------------------------------ strokes and states on disk
def save_strokes(path, strokes, ranges):
    if any(s.backend != mix.backend() for s in strokes):
        raise ValueError("cannot save mixed-backend strokes")
    pts = np.concatenate([s.pts for s in strokes]) if strokes else np.zeros((0, 4), np.float32)
    off = np.zeros(len(strokes) + 1, np.int64); off[1:] = np.cumsum([s.pts.shape[0] for s in strokes])
    np.savez_compressed(path, pts=pts, offsets=off, backend=mix.backend(), format_version=2,
                        material_format=_material_format(mix.backend()),
                        zcol=np.stack([s.zcol for s in strokes]) if strokes else np.zeros((0, mix.LAT), np.float32),
                        zcol2=np.stack([s.zcol2 for s in strokes]) if strokes else np.zeros((0, mix.LAT), np.float32),
                        dz=np.stack([s.dz for s in strokes]) if strokes else np.zeros((0, mix.LAT), np.float32),
                        seeds=np.array([s.seed for s in strokes], np.int64), layer=np.array([s.layer for s in strokes], np.int32),
                        region=np.array([s.region for s in strokes], np.int32),
                        params=np.array([json.dumps({k: (float(v) if isinstance(v, (np.floating, float)) else v) for k, v in s.params.items()}) for s in strokes]),
                        ranges=np.array(ranges, np.int64))


def load_strokes(path):
    d = np.load(path, allow_pickle=False)
    _check_format(d, mix.backend())
    # read each array exactly once: NpzFile decompresses a fresh copy on every key access, and slices would
    # keep every copy alive
    pts, off, zcol, dz = d["pts"], d["offsets"], d["zcol"], d["dz"]
    zcol2 = d["zcol2"] if "zcol2" in d.files else zcol
    seeds, layer, region, params, ranges = d["seeds"], d["layer"], d["region"], d["params"], d["ranges"]
    strokes = []
    for i in range(len(off) - 1):
        strokes.append(Stroke(pts[off[i]:off[i + 1]].copy(), zcol[i].copy(), dz[i].copy(), json.loads(str(params[i])),
                              int(seeds[i]), int(layer[i]), int(region[i]), zcol2=zcol2[i].copy()))
        strokes[-1].validate(mix.backend())
    return strokes, [tuple(int(v) for v in r) for r in ranges]


def _material_format(backend):
    if backend == "mixbox-material":
        return "mixbox-2.0-lat7-padded85-v1"
    return "ochrell-0.2-ks41-f32-v1" if backend.startswith("ochrell") else "legacy-7"


def _check_format(d, backend):
    if "backend" in d.files and str(d["backend"]) != backend:
        raise ValueError(f"saved mixer {str(d['backend'])} does not match {backend}")
    if "format_version" in d.files and int(d["format_version"]) != 2:
        raise ValueError("unsupported saved format version")
    if mix.is_material(backend):
        if "material_format" not in d.files or str(d["material_format"]) != _material_format(backend):
            raise ValueError("Ochrell requires matching, versioned material states; regenerate from authored RGB inputs")


def save_state(path, cv):
    if cv.amount is not None:
        np.savez_compressed(path, backend=cv.backend, format_version=2, material_format=_material_format(cv.backend),
                            lat=cv.lat, rgb=cv.rgb, h=cv.h, wet=cv.wet, cover=cv.cover,
                            amount=cv.amount, hblur=cv.hblur, region=cv.region)
        return
    # plan-size proxy state after a layer (float16 is plenty for iteration; ~5 MB per layer at 600x750)
    np.savez_compressed(path, lat=cv.lat.astype(np.float16), rgb=cv.rgb.astype(np.float16), h=cv.h.astype(np.float16),
                        wet=cv.wet.astype(np.float16), cover=cv.cover.astype(np.float16))


def load_state(path, cv):
    d = np.load(path, allow_pickle=False)
    _check_format(d, cv.backend)
    if cv.amount is not None:
        from .ochrell import validate_state
        arrays = {k: d[k] for k in ("lat", "rgb", "h", "wet", "cover", "amount", "hblur", "region")}
        for k, a in arrays.items():
            dest = getattr(cv, k)
            if a.shape != dest.shape or a.dtype != dest.dtype or not np.isfinite(a).all():
                raise ValueError(f"invalid saved {k} shape, precision or values")
        validate_state(arrays["lat"], cv.backend)
        if np.any(arrays["amount"] < 0) or np.any(arrays["wet"] < 0) or np.any(arrays["wet"] > 1):
            raise ValueError("invalid saved paint amount or wetness")
        for k, a in arrays.items():
            getattr(cv, k)[:] = a
        return
    cv.lat[:] = d["lat"]; cv.rgb[:] = d["rgb"]; cv.h[:] = d["h"]; cv.wet[:] = d["wet"]; cv.cover[:] = d["cover"]


# ------------------------------------------------------------------ replay helpers
def _prep_layer(cv, layer, W):
    # the blurred height is needed by scumble strokes (per-layer mode or per-region style override)
    cv.set_hblur(L.blur(cv.h, max(1.0, layer["hblur_sigma"] * W)))


def _finish_layer(cv, layer):
    if layer.get("dry_after") is not None:
        cv.dry(layer["dry_after"])


def layer_png_name(li, name):
    return f"L{li+1:02d}_{name.lower().replace(' ', '_').replace(',', '').replace('/', '-')}.png"


# ------------------------------------------------------------------ main entry
def render(scene_path, size=None, preview=False, plan_width=600, seed=1907, out="out/run", upto=None,
           mixer="mixbox", do_light=True, light_kw=None, save_layers=True, verbose=True, strokes_file=None,
           only=None, from_run=None, timelapse=None, video_size=(1200, 1500), fps=24, captions=True,
           frames_per_layer=40, hold_seconds=1.0, layer_hook=None):
    t_all = time.perf_counter()
    mix.set_backend(mixer)
    S = load_scene(scene_path)
    W = plan_width if (preview or size is None) else int(size[0])
    Wp = min(plan_width, W)
    os.makedirs(out, exist_ok=True)
    os.makedirs(os.path.join(out, "states"), exist_ok=True)
    timings = {}
    layers = S.layer_list
    nL = len(layers)

    # ---- guides
    t0 = time.perf_counter()
    g_plan = S.guides(Wp)
    write_guides(S, g_plan, out)
    timings["guides"] = time.perf_counter() - t0

    # ---- plan
    t0 = time.perf_counter()
    pl = Planner(g_plan, S.styles, layers, seed=seed, ground_rgb=S.ground, verbose=verbose)
    state_save_seconds = 0.

    def save_proxy(path):
        nonlocal state_save_seconds
        start = time.perf_counter()
        save_state(path, pl.proxy)
        state_save_seconds += time.perf_counter() - start

    def hook(li, layer, planner):
        save_proxy(os.path.join(out, "states", f"after_L{li+1:02d}.npz"))
        if layer_hook:
            layer_hook(li, layer, planner)

    if strokes_file:
        pl.strokes, pl.layer_ranges = load_strokes(strokes_file)
        for li, (a, b) in enumerate(pl.layer_ranges):
            _prep_layer(pl.proxy, layers[li], Wp); pl.proxy.render(pl.strokes[a:b]); _finish_layer(pl.proxy, layers[li])
    elif only is not None:
        # re-plan one layer over the saved state of the layers before it; keep the others' strokes
        assert from_run, "--only needs --from <run>"
        old_strokes, old_ranges = load_strokes(os.path.join(from_run, "strokes.npz"))
        li = only - 1
        if li > 0:
            load_state(os.path.join(from_run, "states", f"after_L{li:02d}.npz"), pl.proxy)
        a, b = old_ranges[li]
        pl.strokes = list(old_strokes[:a]); pl.layer_ranges = list(old_ranges[:li])
        pl.plan_layer(li, layers[li])
        save_proxy(os.path.join(out, "states", f"after_L{li+1:02d}.npz"))
        shift = len(pl.strokes) - b
        for lj in range(li + 1, len(old_ranges)):
            a2, b2 = old_ranges[lj]
            _prep_layer(pl.proxy, layers[lj], Wp)
            pl.proxy.render(old_strokes[a2:b2]); _finish_layer(pl.proxy, layers[lj])
            pl.strokes.extend(old_strokes[a2:b2]); pl.layer_ranges.append((a2 + shift, b2 + shift))
            save_proxy(os.path.join(out, "states", f"after_L{lj+1:02d}.npz"))
        # copy the earlier states so this run is self-contained for the next --only
        for lj in range(0, li):
            src = os.path.join(from_run, "states", f"after_L{lj+1:02d}.npz")
            dst = os.path.join(out, "states", f"after_L{lj+1:02d}.npz")
            if os.path.exists(src) and os.path.abspath(src) != os.path.abspath(dst):
                import shutil; shutil.copyfile(src, dst)
    else:
        pl.plan(upto=upto, layer_hook=hook)
    timings["plan"] = time.perf_counter() - t0
    timings["state_save"] = state_save_seconds
    timings["plan_excluding_state_save"] = timings["plan"] - state_save_seconds
    n = len(pl.strokes)
    save_strokes(os.path.join(out, "strokes.npz"), pl.strokes, pl.layer_ranges)
    ranges = pl.layer_ranges

    # ---- replay at the output size (always replayed, so layer PNGs and the time-lapse come from one pass)
    t0 = time.perf_counter()
    H = S.size(W)[1]
    cv = Canvas(W, H, S.ground)
    # the replay only needs the region-id map (for the metrics); never resize masks/flows to full size
    cv.set_region(g_plan.region_id if W == Wp else cv2.resize(g_plan.region_id, (W, H), interpolation=cv2.INTER_NEAREST))
    tl = None
    if timelapse:
        from .timelapse import TimelapseWriter
        tl = TimelapseWriter(timelapse, video_size, fps, captions, light_kw)
    layer_imgs = []
    npx = 0
    os.makedirs(os.path.join(out, "layers"), exist_ok=True)
    for li, (a, b) in enumerate(ranges):
        layer = layers[li]
        strokes = pl.strokes[a:b]
        _prep_layer(cv, layer, W)
        caption = f"Layer {li+1}/{nL}: {layer['name']}"
        if tl is not None and strokes:
            every = max(1, int(np.ceil(len(strokes) / max(1, frames_per_layer))))
            npx += cv.render(strokes, progress=lambda i, m: tl.frame(cv.rgb, cv.h, caption), every=every)
        else:
            npx += cv.render(strokes)
        _finish_layer(cv, layer)
        if save_layers or tl is not None:
            img = L.relight(cv.rgb, cv.h, **(light_kw or {})) if do_light else cv.rgb
            if save_layers:
                L.save_png(os.path.join(out, "layers", layer_png_name(li, layer["name"])), img)
                thumb = cv2.resize(L.to8(img), (360, int(360 * H / W)), interpolation=cv2.INTER_AREA) if W > 720 else L.to8(img)
                layer_imgs.append((f"L{li+1} {layer['name']}", thumb))
            del img
            if tl is not None:
                last = tl.frame(cv.rgb, cv.h, caption)
                tl.hold(last, hold_seconds)
    if tl is not None:
        last = tl.frame(cv.rgb, cv.h, f"{os.environ.get('OILPAINT_TITLE') or os.path.splitext(os.path.basename(scene_path))[0].replace('_', ' ').title()} - {nL} layers")
        tl.hold(last, 2.0)
        frames = tl.close()
        timings["timelapse_frames"] = frames
    timings["render"] = time.perf_counter() - t0

    # ---- finish
    t0 = time.perf_counter()
    lit = L.relight(cv.rgb, cv.h, **(light_kw or {})) if do_light else cv.rgb
    timings["light"] = time.perf_counter() - t0
    L.save_png(os.path.join(out, "final.png"), lit)
    L.save_png(os.path.join(out, "final_unlit.png"), cv.rgb)
    L.save_gray(os.path.join(out, "height.png"), cv.h)
    L.save_png(os.path.join(out, "normals.png"), L.normals_preview(cv.h))
    np.save(os.path.join(out, "height.npy"), cv.h)
    np.save(os.path.join(out, "unlit.npy"), cv.rgb)
    small = cv2.resize(lit, (720, int(720 * H / W)), interpolation=cv2.INTER_AREA) if W > 900 else lit
    L.save_png(os.path.join(out, "final_900.png"), cv2.resize(lit, (720, int(720 * H / W)), interpolation=cv2.INTER_AREA))
    side = np.concatenate([L.to8(small), np.full((small.shape[0], 8, 3), 40, np.uint8),
                           L.to8(cv2.resize(cv.rgb, (small.shape[1], small.shape[0]), interpolation=cv2.INTER_AREA))], 1)
    cv2.imwrite(os.path.join(out, "lit_unlit.png"), cv2.cvtColor(side, cv2.COLOR_RGB2BGR))
    if layer_imgs:
        save_sheet(os.path.join(out, "layers_sheet.png"), layer_imgs, cols=5, cell=(360, 450))

    # ---- metrics
    t0 = time.perf_counter()
    sil = [g_plan.index(nm) for nm in ("tower", "rock") if nm in g_plan.names]
    lab_cols = mix.rgb_to_lab(np.stack([mix.latent_to_rgb(s.zcol) for s in pl.strokes])) if pl.strokes else None
    m = metrics.compute(cv.rgb, lit, cv.h, cv.cover, cv.region, sil, pl.strokes, g_plan.names, lab_cols)
    timings["metrics"] = time.perf_counter() - t0
    timings["total"] = time.perf_counter() - t_all
    per_layer = [(layers[li]["name"], b - a) for li, (a, b) in enumerate(ranges)]
    run = dict(scene=os.path.abspath(scene_path), size=[W, H], plan_width=Wp, seed=seed, mixer=mixer, strokes=n,
               layers=per_layer, timings=timings, attribution=(MIXBOX_ATTRIBUTION if mixer.startswith("mixbox") else
                   "Ochrell: MIT OR Apache-2.0 code; derived CIE assets CC BY-SA 4.0; see sibling ochrell/data/README.md" if mixer.startswith("ochrell") else "sRGB interpolation"),
               light=dict(L.LIGHT_DEFAULTS, **(light_kw or {})), metrics=m, only=only, from_run=from_run,
               timelapse=timelapse)
    with open(os.path.join(out, "run.json"), "w") as f:
        json.dump(run, f, indent=2, default=float)
    with open(os.path.join(out, "metrics.json"), "w") as f:
        json.dump(m, f, indent=2, default=float)
    d = m["diagnostics"]
    summary = (f"[oilpaint] {W}x{H} {n} strokes in {timings['total']:.1f}s (plan {timings['plan']:.1f}s, render {timings['render']:.1f}s"
               f"{', tl frames ' + str(timings['timelapse_frames']) if 'timelapse_frames' in timings else ''}) | "
               f"gates {'OK' if all(v['ok'] for v in m['gates'].values()) else 'FAIL'} | L* min {d['L']['min']:.0f} p50 {d['L']['p50']:.0f} | "
               f"chroma {d['chroma']['mean']:.0f} | hue cool/warm {d['hue']['cool_lobe']:.2f}/{d['hue']['warm_lobe']:.2f} | "
               f"relief {d['paint_body']['relief_coverage']:.2f} | mud {d.get('mud', {}).get('score', float('nan')):.2f} | {out}")
    print(summary)
    run["summary"] = summary
    return run
