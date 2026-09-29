"""Visual checkpoints T1-T6 with mechanical gates.  Usage: python tests/checkpoints.py [t1 t2 ... | all]"""
import os
import sys
import json
import time
import numpy as np
import cv2

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
sys.path.insert(0, ROOT)
from oilpaint import mix, light, strokes as S
from oilpaint.canvas import Canvas
from oilpaint.sheet import save_sheet, sheet

OUT = os.path.join(ROOT, "out", "checkpoints")
os.makedirs(OUT, exist_ok=True)
RESULTS = {}


def gate(name, ok, detail):
    RESULTS[name] = {"pass": bool(ok), "detail": detail}
    print(f"  [{'PASS' if ok else 'FAIL'}] {name}: {detail}")


def rgb8(x):
    return light.to8(x)


def lit(cv, **kw):
    return light.relight(cv.rgb, cv.h, **kw)


# ------------------------------------------------------------------ T1
def t1():
    print("T1 single strokes")
    items = []
    W, H = 600, 600
    cv = Canvas(W, H)
    widths = [0.012, 0.025, 0.05]
    cols = ["cobalt_blue", "vermilion", ("cadmium_yellow", "lead_white", 0.4)]
    sts = []
    for i, w in enumerate(widths):
        y = 0.12 + i * 0.16
        sts.append(S.make_stroke(S.straight(0.06, y, 0.55, y, w), cols[i], 100 + i, nb=int(w * 500) + 4,
                                 streak=0.7, pickup=0.0))
        sts.append(S.make_stroke(S.arc(0.78, y + 0.25, 0.2, np.pi * 1.1, np.pi * 1.9, w), cols[i], 200 + i,
                                 nb=int(w * 500) + 4, streak=0.7, pickup=0.0))
    # a long stroke that runs dry, and a soft round-ish brush
    sts.append(S.make_stroke(S.straight(0.05, 0.66, 0.95, 0.70, 0.03), "viridian", 300, nb=16, deplete=0.06,
                             vdry=0.5, streak=0.5))
    sts.append(S.make_stroke(S.straight(0.05, 0.80, 0.95, 0.84, 0.03), "cobalt_violet_light", 301, nb=16,
                             hardness=0.0, streak=0.3))
    sts.append(S.make_stroke(S.straight(0.05, 0.92, 0.95, 0.92, 0.03), ("ultramarine", "lead_white", 0.3), 302,
                             nb=16, hardness=1.0, streak=0.9))
    t0 = time.perf_counter()
    npx = cv.render(sts)
    dt = time.perf_counter() - t0
    print(f"  rendered {npx} px in {dt*1000:.1f} ms")
    unlit = cv.rgb.copy()
    litimg = lit(cv)
    items += [("T1 unlit 600px", rgb8(unlit)), ("T1 lit 600px", rgb8(litimg)),
              ("T1 height", rgb8(np.repeat((cv.h / max(cv.h.max(), 1e-6))[..., None], 3, -1)))]
    light.save_png(os.path.join(OUT, "t1_unlit.png"), unlit)
    light.save_png(os.path.join(OUT, "t1_lit.png"), litimg)
    light.save_gray(os.path.join(OUT, "t1_height.png"), cv.h)

    # gates: across-profile of the middle straight stroke (w=0.025 at y=0.28): count local maxima
    w = 0.025; yc = int(0.28 * W); x = int(0.3 * W)
    half = int(w * W * 0.6)
    prof = cv.cover[yc - half:yc + half + 1, x - 2:x + 3].mean(1)
    prof_s = np.convolve(prof, [0.25, 0.5, 0.25], mode="same")
    maxima = sum(1 for i in range(1, len(prof_s) - 1) if prof_s[i] > prof_s[i - 1] and prof_s[i] >= prof_s[i + 1] and prof_s[i] > 0.15)
    gate("T1 across-profile ridges (w=2.5%cw @600px) >= 3", maxima >= 3, f"{maxima} local maxima in {len(prof)} px")
    # along profile of the drying stroke: tail vs middle
    yc = int(0.68 * W)
    band = cv.cover[yc - 12:yc + 12, :]
    mid = band[:, int(0.35 * W):int(0.5 * W)].mean(); tail = band[:, int(0.86 * W):int(0.93 * W)].mean()
    gate("T1 dry tail < 60% of middle", tail < 0.6 * mid, f"middle {mid:.3f} tail {tail:.3f}")
    # aliasing / double coverage: (a) cover never exceeds 1 (each pixel painted once per stroke),
    # (b) max step of cover between neighbours strictly inside strokes (mask eroded by 1 px)
    inside = cv2.erode((cv.cover > 0.3).astype(np.uint8), np.ones((3, 3), np.uint8)).astype(bool)
    d = np.abs(np.diff(cv.cover, axis=1)); dv = np.abs(np.diff(cv.cover, axis=0))
    m = inside[:, 1:] & inside[:, :-1]; mv = inside[1:, :] & inside[:-1, :]
    step = max(d[m].max() if m.any() else 0, dv[mv].max() if mv.any() else 0)
    # (the v2 lane model has crisp lane/dropout edges by design; the step bound guards against aliasing only)
    gate("T1 single coverage per stroke (max cover <= 1.001) and no aliasing steps inside (< 0.8)",
         cv.cover.max() <= 1.001 and step < 0.8, f"max cover {cv.cover.max():.3f}, max interior step {step:.2f}")

    # resolution independence: same strokes at 600 and 2400, compare a crop
    cv2400 = Canvas(2400, 2400)
    cv2400.render(sts)
    crop_lo = cv.rgb[int(0.24 * 600):int(0.34 * 600), int(0.08 * 600):int(0.30 * 600)]
    crop_hi = cv2400.rgb[int(0.24 * 2400):int(0.34 * 2400), int(0.08 * 2400):int(0.30 * 2400)]
    hi_down = cv2.resize(crop_hi, (crop_lo.shape[1], crop_lo.shape[0]), interpolation=cv2.INTER_AREA)
    diff = np.abs(hi_down - crop_lo).mean()
    gate("T1 replay 2400 downsampled ~= 600 render (mean |diff| < 0.06)", diff < 0.06, f"mean abs diff {diff:.4f}")
    lo_up = cv2.resize(crop_lo, (crop_hi.shape[1], crop_hi.shape[0]), interpolation=cv2.INTER_CUBIC)
    items += [("600px crop (upscaled x4)", rgb8(lo_up)), ("2400px crop (native)", rgb8(crop_hi)),
              ("2400px lit", rgb8(light.relight(crop_hi, cv2400.h[int(0.24 * 2400):int(0.34 * 2400), int(0.08 * 2400):int(0.30 * 2400)])))]
    light.save_png(os.path.join(OUT, "t1_crop_600_up.png"), lo_up)
    light.save_png(os.path.join(OUT, "t1_crop_2400.png"), crop_hi)
    save_sheet(os.path.join(OUT, "T1_sheet.png"), items, cols=3, cell=(380, 380))
    return items


# ------------------------------------------------------------------ T2
def t2():
    print("T2 mixing")
    pairs = [("cobalt_blue", "cadmium_yellow"), ("cobalt_violet_light", "#f0b98f"), ("#2f5490", "lead_white"),
             ("vermilion", "viridian"), ("ultramarine", "#f6d09a"), ("madder", "lead_white")]
    K = 7
    sw = 60
    rows = []
    hue_ok = None
    for a, b in pairs:
        ra, rb = mix.tube(a) if not a.startswith("#") else mix.hex_to_rgb(a), mix.tube(b) if not b.startswith("#") else mix.hex_to_rgb(b)
        row = []
        for backend in ("mixbox", "rgb"):
            mix.set_backend(backend)
            for k in range(K):
                t = k / (K - 1)
                c = mix.mix_rgb(ra, rb, t)
                row.append(np.tile(c[None, None], (sw, sw, 1)))
            row.append(np.ones((sw, 8, 3), np.float32) * 0.15)
            if backend == "mixbox" and a == "cobalt_blue":
                lab = mix.rgb_to_lab(mix.mix_rgb(ra, rb, 0.5))
                hue = np.degrees(np.arctan2(lab[2], lab[1])) % 360
                hue_ok = 90 <= hue <= 160
                hue_val = hue
            if backend == "rgb" and a == "cobalt_blue":
                lab = mix.rgb_to_lab(mix.mix_rgb(ra, rb, 0.5))
                hue_rgb = np.degrees(np.arctan2(lab[2], lab[1])) % 360
        mix.set_backend("mixbox")
        rows.append(np.concatenate(row, 1))
    img = np.concatenate(rows, 0)
    img8 = rgb8(img)
    img8 = cv2.copyMakeBorder(img8, 24, 0, 0, 0, cv2.BORDER_CONSTANT, value=(30, 30, 30))
    cv2.putText(img8, "left: Mixbox   right: RGB lerp   rows: blue+yellow, violet+peach, sea+white, red+green, ultramarine+gold, madder+white",
                (6, 16), cv2.FONT_HERSHEY_SIMPLEX, 0.42, (230, 230, 230), 1, cv2.LINE_AA)
    light.save_png(os.path.join(OUT, "T2_mixing.png"), img8 / 255.0)
    gate("T2 mixbox blue+yellow midpoint is green (Lab hue 90-160)", hue_ok, f"mixbox hue {hue_val:.0f} deg, rgb-lerp hue {hue_rgb:.0f} deg")
    return [("T2 mixing (Mixbox | RGB)", img8)]


# ------------------------------------------------------------------ T3
def t3():
    print("T3 pick-up and smear")
    W, H = 600, 400
    cv = Canvas(W, H)
    # two colour fields: left half wet blue, right half dry (wet=0) orange
    fills = []
    for i in range(12):
        y = 0.04 + i * 0.058
        fills.append(S.make_stroke(S.straight(0.0, y, 0.5, y, 0.07, n=12), "cobalt_blue", 10 + i, nb=20, streak=0.3, pickup=0.0, hgain=0.6))
        fills.append(S.make_stroke(S.straight(0.5, y, 1.0, y, 0.07, n=12), "cadmium_orange", 40 + i, nb=20, streak=0.3, pickup=0.0, hgain=0.6))
    cv.render(fills)
    cv.wet[:, W // 2:] = 0.0   # right half is dry
    base = cv.rgb.copy()
    # loaded white brush dragged left->right across both halves, twice at different heights
    drag = [S.make_stroke(S.straight(0.1, 0.2, 0.9, 0.2, 0.05), "lead_white", 500, nb=14, pickup=0.35, streak=0.3),
            S.make_stroke(S.straight(0.9, 0.35, 0.1, 0.35, 0.05), "lead_white", 501, nb=14, pickup=0.35, streak=0.3)]
    # smudge (no paint) dragged from the blue into the orange across the wet/dry boundary
    smudge = [S.make_stroke(S.straight(0.35, 0.5, 0.75, 0.5, 0.05), "lead_white", 600, nb=14, mode="smudge", pickup=0.3, opacity=0.6, flatten=0.25, body=0.5),
              S.make_stroke(S.straight(0.35, 0.58, 0.75, 0.58, 0.05), "lead_white", 601, nb=14, mode="smudge", pickup=0.6, opacity=0.8, flatten=0.25, body=0.5)]
    cv.render(drag + smudge)
    unlit = cv.rgb.copy(); li = lit(cv)
    light.save_png(os.path.join(OUT, "t3_unlit.png"), unlit); light.save_png(os.path.join(OUT, "t3_lit.png"), li)
    # gate 1: along the first drag (left->right over wet blue then dry orange): colour on wet side moves toward blue
    yc = int(0.2 * W)
    line = cv.rgb[yc - 3:yc + 3].mean(0)
    lab = mix.rgb_to_lab(line)
    wet_part = lab[int(0.15 * W):int(0.48 * W)]
    dry_near = lab[int(0.53 * W):int(0.62 * W)]
    dry_far = lab[int(0.66 * W):int(0.74 * W)]
    # the brush starts white: on the wet side it picks up blue (b* < -8); on the dry side the carried blue
    # fades along the trail (near b* < far b*) and the far end is back to near-white (L* > 85)
    b_wet = wet_part[:, 2].mean(); b_near = dry_near[:, 2].mean(); b_far = dry_far[:, 2].mean(); L_far = dry_far[:, 0].mean()
    gate("T3 pick-up on wet side (b* < -8), trail fades on dry side (near < far), far end clean (L* > 84)",
         b_wet < -8 and b_near < b_far and L_far > 84,
         f"wet b* {b_wet:.1f}, dry near b* {b_near:.1f}, dry far b* {b_far:.1f}, far L* {L_far:.1f}")
    # gate 2: smudge stroke: blue carried into the orange field, decaying
    yc = int(0.5 * W)
    lab2 = mix.rgb_to_lab(cv.rgb[yc - 3:yc + 3].mean(0))
    near = lab2[int(0.52 * W):int(0.58 * W), 2].mean(); far = lab2[int(0.68 * W):int(0.74 * W), 2].mean()
    base_or = mix.rgb_to_lab(base[yc, int(0.7 * W)])[2]
    gate("T3 smudge carries blue into orange and decays (b* near < far <= orange)", near < far - 2 and far <= base_or + 2,
         f"b* near {near:.1f}, far {far:.1f}, untouched orange {base_or:.1f}")
    return [("T3 unlit", rgb8(unlit)), ("T3 lit", rgb8(li))]


# ------------------------------------------------------------------ T4 + T6
def t4():
    print("T4 lighting / weave / cavity; T6 scumble")
    W, H = 600, 600
    cv = Canvas(W, H)
    sts = []
    rng = np.random.default_rng(5)
    cols = [("ultramarine", "lead_white", 0.45), ("cadmium_orange", "lead_white", 0.5), "viridian", ("madder", "lead_white", 0.5)]
    for i in range(26):
        x0, y0 = rng.uniform(0.1, 0.6), rng.uniform(0.15, 0.65)
        ang = rng.uniform(-0.5, 0.5) + (np.pi / 2 if i % 3 == 0 else 0)
        L = rng.uniform(0.15, 0.3)
        sts.append(S.make_stroke(S.straight(x0, y0, x0 + L * np.cos(ang), y0 + L * np.sin(ang), rng.uniform(0.025, 0.05)),
                                 cols[i % 4], 700 + i, nb=int(rng.integers(10, 20)), hgain=rng.uniform(0.8, 1.6), streak=0.6))
    cv.render(sts)
    items = []
    base = cv.rgb.copy(); hbase = cv.h.copy()
    for name, kw in [("light upper-left", {}), ("light upper-right", dict(light_dir=(0.5, -0.6, 0.62))),
                     ("light left, low", dict(light_dir=(-0.8, -0.1, 0.35))), ("matte", dict(matte=True)),
                     ("no weave", dict(weave_amp=0.0)), ("unlit", None)]:
        img = base if kw is None else light.relight(base, hbase, **kw)
        items.append((f"T4 {name}", rgb8(img)))
        light.save_png(os.path.join(OUT, f"t4_{name.replace(' ', '_').replace(',', '')}.png"), img)
    # gate: the lighting responds to the height field: slopes facing the light are brighter (shade) and carry
    # more specular than slopes facing away; flat paint keeps its albedo (shade ~ 1)
    _, comp = light.relight(base, hbase, components=True)
    L = np.array(light.LIGHT_DEFAULTS["light_dir"], np.float32)
    facing = -(comp["gx"] * L[0] + comp["gy"] * L[1])      # >0: slope faces the light
    painted = cv.cover > 0.5
    slope = np.hypot(comp["gx"], comp["gy"])
    steep = painted & (slope > np.percentile(slope[painted], 70))
    toward = steep & (facing > 0); away = steep & (facing < 0)
    sh_t, sh_a = comp["shade"][toward].mean(), comp["shade"][away].mean()
    sp_t, sp_a = comp["spec"][toward].mean(), comp["spec"][away].mean()
    flat = comp["shade"][cv.cover < 0.05].mean()
    gate("T4 lighting follows height (shade toward > away, spec toward > 2x away, flat ~ 1)",
         sh_t > sh_a + 0.05 and sp_t > 2 * sp_a and abs(flat - 1) < 0.08,
         f"shade toward {sh_t:.2f} away {sh_a:.2f}; spec toward {sp_t:.4f} away {sp_a:.4f}; flat shade {flat:.3f}")
    # gate: weave visibility in thick vs bare
    wv = light.weave(H, W, 0.12)
    vis = np.exp(-hbase / light.LIGHT_DEFAULTS["weave_h0"])
    thick = vis[cv.cover > 1.0].mean() if (cv.cover > 1.0).any() else 1.0
    bare = vis[cv.cover < 0.05].mean()
    gate("T4 weave attenuated under thick paint (< 50% of bare)", thick < 0.5 * bare, f"thick {thick:.2f} bare {bare:.2f}")

    # T6 scumble: dry-brush pale strokes over the ridges; alpha should correlate with height
    hb = light.blur(cv.h, 6.0)
    cv.set_hblur(hb)
    cover_before = cv.cover.copy()
    sc = []
    for i in range(10):
        y = 0.2 + i * 0.05
        sc.append(S.make_stroke(S.straight(0.05, y, 0.95, y + 0.02, 0.05), ("cadmium_yellow", "lead_white", 0.6), 900 + i,
                                nb=18, mode="scumble", opacity=0.6, dry_thresh=0.02, dry_width=0.08, streak=0.3, load=0.5, vdry=0.6, deplete=0.0))
    cv.render(sc)
    applied = cv.cover - cover_before
    m = (applied > 0.01) | ((cover_before > 0.5) & (applied >= 0))
    sel = (cover_before > 0.5)
    rel = hbase - hb
    corr = np.corrcoef(applied[sel].ravel(), rel[sel].ravel())[0, 1]
    gate("T6 scumble alpha correlates with height above local mean (corr > 0.3)", corr > 0.3, f"corr {corr:.3f}")
    sc_img = light.relight(cv.rgb, cv.h)
    items.append(("T6 scumble over impasto (lit)", rgb8(sc_img)))
    items.append(("T6 scumble unlit", rgb8(cv.rgb)))
    light.save_png(os.path.join(OUT, "t6_scumble_lit.png"), sc_img)
    save_sheet(os.path.join(OUT, "T4_T6_sheet.png"), items, cols=4, cell=(300, 300))
    return items


def main(which):
    items = []
    if "t1" in which or "all" in which: items += t1()
    if "t2" in which or "all" in which: items += t2()
    if "t3" in which or "all" in which: items += t3()
    if "t4" in which or "all" in which or "t6" in which: items += t4()
    if "t5" in which or "all" in which:
        from t5_coarse2fine import t5
        items += t5(OUT, gate)
    if "t7" in which or "all" in which:
        from t7_surface import t7
        items += t7(OUT, gate)
    with open(os.path.join(OUT, "gates.json"), "w") as f:
        json.dump(RESULTS, f, indent=2)
    if items:
        save_sheet(os.path.join(OUT, "ALL_checkpoints_sheet.png"), items, cols=4, cell=(320, 320))
    print("gates:", sum(r["pass"] for r in RESULTS.values()), "/", len(RESULTS), "passed")


if __name__ == "__main__":
    main(sys.argv[1:] or ["all"])
