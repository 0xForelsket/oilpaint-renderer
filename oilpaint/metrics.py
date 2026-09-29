"""Objective checks on a finished render.  Mechanical checks are gates; calibration-dependent numbers are
diagnostics compared with calib.TARGETS (guesses, see calib.py)."""
import numpy as np
import cv2

from . import mix
from .calib import TARGETS, GATES


def _hue_deg(lab):
    return np.degrees(np.arctan2(lab[..., 2], lab[..., 1])) % 360


def compute(rgb_unlit, rgb_lit, h, cover, region_id=None, silhouette_regions=(), strokes=None, names=None,
            stroke_colors_lab=None):
    m = {"gates": {}, "diagnostics": {}}
    lab = mix.rgb_to_lab(rgb_unlit)
    L = lab[..., 0]
    C = np.hypot(lab[..., 1], lab[..., 2])
    # ---- gates
    lmin = float(L.min()); frac18 = float((L < 18).mean())
    m["gates"]["no_black"] = dict(ok=bool(lmin > GATES["black_L_floor"] and frac18 < GATES["black_frac_L18"]),
                                  L_min=lmin, frac_L_below_18=frac18)
    m["gates"]["no_clipping"] = dict(ok=bool((rgb_unlit >= 0.999).all(-1).mean() < 0.02 and (rgb_unlit <= 0.001).all(-1).mean() < 0.001),
                                     white_frac=float((rgb_unlit >= 0.999).all(-1).mean()), black_frac=float((rgb_unlit <= 0.001).all(-1).mean()))
    # ---- diagnostics
    d = m["diagnostics"]
    d["L"] = dict(min=lmin, p01=float(np.percentile(L, 1)), p50=float(np.percentile(L, 50)), p99=float(np.percentile(L, 99)),
                  target=dict(p01=TARGETS["L_p01"], p50=TARGETS["L_p50"], p99=TARGETS["L_p99"]))
    d["chroma"] = dict(mean=float(C.mean()), p90=float(np.percentile(C, 90)), target=dict(mean=TARGETS["chroma_mean"], p90=TARGETS["chroma_p90"]))
    chrom = C > 12
    hue = _hue_deg(lab)[chrom]
    hist, edges = np.histogram(hue, bins=24, range=(0, 360))
    hist = hist / max(1, hist.sum())
    lo, hi = TARGETS["hue_lobe_cool"]; cool = float(((hue >= lo) & (hue < hi)).mean()) if hue.size else 0.0
    lo, hi = TARGETS["hue_lobe_warm"]; warm = float(((hue >= lo) & (hue < hi)).mean()) if hue.size else 0.0
    green = float(((hue >= 90) & (hue < 180)).mean()) if hue.size else 0.0
    d["hue"] = dict(chromatic_frac=float(chrom.mean()), cool_lobe=cool, warm_lobe=warm, green=green,
                    histogram_15deg=[float(x) for x in hist], target_lobe_mass_min=TARGETS["lobe_mass_min"])
    # edge softness at silhouettes vs stroke edges
    if region_id is not None and silhouette_regions:
        gray = L / 100.0
        gx = cv2.Sobel(gray, cv2.CV_32F, 1, 0, ksize=3); gy = cv2.Sobel(gray, cv2.CV_32F, 0, 1, ksize=3)
        gm = np.hypot(gx, gy)
        sil = np.zeros_like(region_id, bool)
        for r in silhouette_regions:
            mm = (region_id == r).astype(np.uint8)
            sil |= (cv2.dilate(mm, np.ones((5, 5), np.uint8)) - cv2.erode(mm, np.ones((5, 5), np.uint8))) > 0
        # stroke edges: strong gradients of the cover map (where strokes end/overlap) away from silhouettes
        cg = np.hypot(cv2.Sobel(cover, cv2.CV_32F, 1, 0, ksize=3), cv2.Sobel(cover, cv2.CV_32F, 0, 1, ksize=3))
        se = (cg > np.percentile(cg, 90)) & ~sil
        ratio = float(gm[sil].mean() / max(1e-6, gm[se].mean())) if sil.any() and se.any() else float("nan")
        d["edge_softness"] = dict(silhouette_over_stroke_edge=ratio, target_max=TARGETS["edge_ratio_max"])
    # paint body
    lum_u = rgb_unlit.mean(-1); lum_l = rgb_lit.mean(-1)
    relief = float((np.abs(lum_l - lum_u) > 0.03).mean())
    d["paint_body"] = dict(relief_coverage=relief, h_p50=float(np.percentile(h, 50)), h_p95=float(np.percentile(h, 95)),
                           target_relief=TARGETS["relief_coverage"])
    # mud: chroma of the canvas in overlapped areas vs chroma of the colours that were laid there
    if stroke_colors_lab is not None and cover is not None:
        overl = cover > 1.5
        if overl.any():
            canvas_c = C[overl].mean()
            laid_c = float(np.hypot(stroke_colors_lab[:, 1], stroke_colors_lab[:, 2]).mean())
            d["mud"] = dict(score=float(max(0.0, 1 - canvas_c / max(laid_c, 1e-6))), canvas_chroma_overlapped=float(canvas_c),
                            laid_chroma_mean=laid_c, target_max=TARGETS["mud_score_max"])
    # stroke statistics
    if strokes:
        by_layer = {}
        for s in strokes:
            p = s.pts
            w = float(p[:, 2].max()); seg = np.hypot(np.diff(p[:, 0]), np.diff(p[:, 1])); ln = float(seg.sum())
            v = p[-1, :2] - p[0, :2]; ang = float(np.degrees(np.arctan2(v[1], v[0])))
            by_layer.setdefault(s.layer, {"w": [], "l": [], "a": [], "n": 0, "regions": {}})
            e = by_layer[s.layer]; e["w"].append(w); e["l"].append(ln); e["a"].append(ang); e["n"] += 1
            rn = names[s.region] if names else str(s.region)
            e["regions"].setdefault(rn, []).append(ang)
        out = {}
        for li, e in sorted(by_layer.items()):
            a = np.array(e["a"]); a = np.abs(((a + 90) % 180) - 90)   # 0 = horizontal, 90 = vertical
            out[str(li + 1)] = dict(n=e["n"], width_med=float(np.median(e["w"])), length_med=float(np.median(e["l"])),
                                    angle_from_horizontal_med=float(np.median(a)),
                                    per_region={k: dict(n=len(v), frac_within_20deg_horizontal=float((np.abs(((np.array(v) + 90) % 180) - 90) < 20).mean()))
                                                for k, v in e["regions"].items()})
        d["strokes"] = out
    return m
