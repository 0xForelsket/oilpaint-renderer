"""Per-swatch metrics for the evaluation harness.  The swatches are defined in scenes/swatches.py; this module measures them.

Geometry comes from the explicit stroke list of each swatch (nominal outlines rasterised from the polyline and width
profile), so "the overlap of the blue and the yellow stroke" or "the last 15 % of a stroke" is known exactly and the
measurements do not depend on how the picture happens to look.  Colours are measured on the unlit albedo (paint colour);
relief and lighting metrics use the height field and the lit image.
"""
import os
import numpy as np
import cv2

from . import mix
from . import eval_metrics as EM

DE_PAINT = 6.0        # Delta E76 from the ground above which a pixel counts as painted
EDGE_COV = 0.5        # coverage level that defines the stroke edge


class Ctx:
    """One swatch cut out of the sheet: crop of lit / unlit / h, ground colour, strokes by step, coordinate helpers."""

    def __init__(self, name, box, W, lit, unlit, h, steps, ground_rgb, margin=0.004):
        H = lit.shape[0]
        x0, y0, x1, y1 = box
        self.name, self.W = name, W
        self.ox = max(0, int(np.floor((x0 - margin) * W)))
        self.oy = max(0, int(np.floor((y0 - margin) * W)))
        ex = min(lit.shape[1], int(np.ceil((x1 + margin) * W)))
        ey = min(H, int(np.ceil((y1 + margin) * W)))
        sl = (slice(self.oy, ey), slice(self.ox, ex))
        self.lit, self.unlit, self.h = lit[sl], unlit[sl], h[sl]
        self.steps = steps
        self.ground_lab = mix.rgb_to_lab(np.asarray(ground_rgb, np.float32))
        self.lab = EM.lab_of(self.unlit)
        self.lab_lit = EM.lab_of(self.lit)
        self.dE = np.linalg.norm(self.lab - self.ground_lab, axis=-1)
        self.shape = self.h.shape

    def px(self, stroke):
        p = np.asarray(stroke.pts, np.float64).copy()
        p[:, 0] = p[:, 0] * self.W - self.ox
        p[:, 1] = p[:, 1] * self.W - self.oy
        p[:, 2] *= self.W
        return p

    def rgb_of(self, stroke):
        return mix.latent_to_rgb(stroke.zcol)

    def lab_of_stroke(self, stroke):
        return mix.rgb_to_lab(self.rgb_of(stroke))

    def all_strokes(self):
        return [s for v in self.steps.values() for s in v]


# ------------------------------------------------------------------ geometry
def _param(P):
    seg = np.hypot(np.diff(P[:, 0]), np.diff(P[:, 1]))
    s = np.concatenate([[0.0], np.cumsum(seg)])
    return s, s[-1]


def footprint(shape, pts, t0=0.0, t1=1.0, scale=1.0):
    """Nominal footprint (bool mask) of a stroke between arc-length fractions t0..t1; `scale` scales the width."""
    P, w = pts[:, :2], pts[:, 2]
    s, tot = _param(P)
    s = s / max(tot, 1e-9)
    T = np.gradient(P, axis=0)
    T = T / (np.linalg.norm(T, axis=1, keepdims=True) + 1e-9)
    N = np.stack([-T[:, 1], T[:, 0]], 1)
    hw = 0.5 * w * scale
    Lp, Rp = P + N * hw[:, None], P - N * hw[:, None]
    m = np.zeros(shape, np.uint8)
    for i in range(len(P) - 1):
        if s[i + 1] < t0 or s[i] > t1:
            continue
        q = np.array([Lp[i], Lp[i + 1], Rp[i + 1], Rp[i]])
        cv2.fillConvexPoly(m, np.round(q * 16).astype(np.int32), 1, cv2.LINE_8, shift=4)
    return m > 0


def profiles(img, pts, t0=0.15, t1=0.85, umax=1.6, step_frac=0.35, nu=None):
    """Sample img across the stroke: returns (prof[n_s, n_u], u[n_u], hw_px[n_s], centre_xy[n_s,2]).  u is in units of the local
    half width (u = +-1 is the nominal outline)."""
    P, w = pts[:, :2], pts[:, 2]
    s, tot = _param(P)
    wmax = float(w.max())
    n = max(8, int((t1 - t0) * tot / max(step_frac * wmax, 0.7)))
    sq = np.linspace(t0 * tot, t1 * tot, n)
    x = np.interp(sq, s, P[:, 0]); y = np.interp(sq, s, P[:, 1]); wq = np.interp(sq, s, w)
    dx = np.gradient(x); dy = np.gradient(y)
    nn = np.hypot(dx, dy) + 1e-9
    tx, ty = dx / nn, dy / nn
    # smooth the tangent over a few samples so polyline corners do not jitter the normal
    k = 3
    tx = np.convolve(np.pad(tx, k, mode="edge"), np.ones(2 * k + 1) / (2 * k + 1), mode="valid")
    ty = np.convolve(np.pad(ty, k, mode="edge"), np.ones(2 * k + 1) / (2 * k + 1), mode="valid")
    nn = np.hypot(tx, ty) + 1e-9
    nx, ny = -ty / nn, tx / nn
    hw = 0.5 * wq
    if nu is None:
        nu = max(21, int(2 * umax * float(np.median(hw))) + 1)
    u = np.linspace(-umax, umax, nu)
    mx = (x[:, None] + nx[:, None] * hw[:, None] * u[None, :]).astype(np.float32)
    my = (y[:, None] + ny[:, None] * hw[:, None] * u[None, :]).astype(np.float32)
    prof = cv2.remap(np.ascontiguousarray(img, np.float32), mx, my, cv2.INTER_LINEAR, borderMode=cv2.BORDER_REPLICATE)
    return prof, u, hw, np.stack([x, y], 1)


def edge_metrics(ctx, pts, t0=0.2, t1=0.8):
    """Edge quality of one stroke from the coverage profile (Delta E from the ground, normalised by the stroke body)."""
    prof, u, hw, _ = profiles(ctx.dE, pts, t0, t1, 1.8)
    ns = prof.shape[0]
    c = len(u) // 2
    body = np.median(prof[:, np.abs(u) < 0.4], axis=1)
    ok = body > 8.0
    cov = prof / np.maximum(body[:, None], 1e-6)
    uR = np.full(ns, np.nan); uL = np.full(ns, np.nan)
    for i in range(ns):
        if not ok[i]:
            continue
        r = np.where(cov[i, c:] < EDGE_COV)[0]
        if len(r) and r[0] > 0:
            j = c + r[0]
            uR[i] = np.interp(EDGE_COV, [cov[i, j], cov[i, j - 1]], [u[j], u[j - 1]])
        l = np.where(cov[i, :c + 1][::-1] < EDGE_COV)[0]
        if len(l) and l[0] > 0:
            j = c - l[0]
            uL[i] = np.interp(EDGE_COV, [cov[i, j], cov[i, j + 1]], [u[j], u[j + 1]])
    good = np.isfinite(uR) & np.isfinite(uL)
    out = {}
    if good.sum() < 6:
        return out
    wpx = (uR - uL)[good] * hw[good]
    nominal = 2.0 * hw[good]
    out["width_ratio"] = float(np.median(wpx / nominal))
    out["width_cv"] = float(np.std(wpx) / max(np.mean(wpx), 1e-6))
    # edge raggedness: departure of each edge from its own running mean (window ~ 3 widths), in widths
    eR = (uR * hw)[good]; eL = (uL * hw)[good]
    win = max(5, int(3 * float(np.median(hw)) * 2 / max(0.35 * float(pts[:, 2].max()), 0.7)))
    win = min(win, len(eR) // 2 * 2 - 1) if len(eR) > 6 else 3
    win = max(3, win // 2 * 2 + 1)                      # odd, so pad + 'valid' convolution keeps the length
    ker = np.ones(win) / win
    rough = []
    kink = []
    for e in (eR, eL):
        tr = np.convolve(np.pad(e, win // 2, mode="edge"), ker, mode="valid")
        rough.append(np.std(e - tr))
        d2 = np.abs(e[2:] - 2 * e[1:-1] + e[:-2])
        kink.append(np.percentile(d2, 99) if len(d2) else 0.0)
    wmed = float(np.median(nominal))
    out["edge_rough"] = float(np.mean(rough) / wmed)
    out["edge_kink"] = float(np.mean(kink) / wmed)
    return out


def height_lane_metrics(ctx, pts, t0=0.3, t1=0.6):
    """Bristle lanes across a stroke: mean height profile over a stretch of the stroke."""
    prof, u, hw, _ = profiles(ctx.h, pts, t0, t1, 1.4, step_frac=0.5)
    hp = prof.mean(0)
    inn = np.abs(u) < 0.85
    hm = float(np.mean(hp[inn]))
    amp = (np.percentile(hp[inn], 90) - np.percentile(hp[inn], 10)) / max(hm, 1e-6)
    k = max(1, int(round(len(u) / 60)))
    sm = np.convolve(hp, np.ones(2 * k + 1) / (2 * k + 1), mode="same")
    mx = [i for i in range(1, len(sm) - 1) if sm[i] > sm[i - 1] and sm[i] >= sm[i + 1] and sm[i] > 0.2 * sm.max() and abs(u[i]) < 0.95]
    # step across the outline: height just inside (u 0.7..0.85) vs just outside (u 1.15..1.3), hgain units
    ins = np.mean(hp[(np.abs(u) > 0.7) & (np.abs(u) < 0.85)])
    out_ = np.mean(hp[(np.abs(u) > 1.15) & (np.abs(u) < 1.3)])
    return dict(ridge_amp=float(amp), lanes=int(len(mx)), h_body=hm, outline_step=float(ins - out_))


def _stroke_med(ctx, mask, arr=None):
    arr = ctx.lab if arr is None else arr
    return np.median(arr[mask], axis=0) if mask.sum() > 20 else None


def _nan_to_none(d):
    return {k: (None if (isinstance(v, float) and not np.isfinite(v)) else v) for k, v in d.items()}


# ------------------------------------------------------------------ kind: single strokes, long strokes
def m_single_strokes(ctx):
    out = {}
    name = ctx.name
    if name == "mode_paint":
        st = ctx.steps["3 widths"]
        for s, tag in zip(st, ("w012", "w024", "w048")):
            pts = ctx.px(s)
            out[tag] = dict(**edge_metrics(ctx, pts), **height_lane_metrics(ctx, pts))
    elif name == "mode_scumble":
        patch = np.zeros(ctx.shape, bool)
        for s in ctx.steps["patch"]:
            patch |= footprint(ctx.shape, ctx.px(s), 0.05, 0.95)
        patch = EM.erode(patch, 0.004 * ctx.W)
        sc = ctx.steps["scumble"]
        base_p = _stroke_med(ctx, patch)
        res = {}
        for s, tag in zip(sc, ("w012", "w024", "w048")):
            fp = footprint(ctx.shape, ctx.px(s), 0.1, 0.9, 0.8)
            on_p = fp & patch
            on_g = fp & ~EM.dilate(patch, 0.01 * ctx.W)
            r = {}
            if on_p.sum() > 30 and base_p is not None:
                r["on_paint_frac"] = float((np.linalg.norm(ctx.lab[on_p] - base_p, axis=-1) > 8).mean())
            if on_g.sum() > 30:
                r["on_ground_frac"] = float((ctx.dE[on_g] > 3).mean())
            res[tag] = r
        out.update(res)
    elif name == "mode_smudge":
        sm = ctx.steps["smudge"]
        xs = [ctx.px(s)[:, 0] for s in ctx.steps["fields"]]
        xm = float(np.median([p.mean() for p in xs[:2]]))
        # untouched yellow / blue: the field rows outside the smudge footprints
        touched = np.zeros(ctx.shape, bool)
        for s in sm:
            touched |= footprint(ctx.shape, ctx.px(s), 0.0, 1.0, 1.4)
        fy = np.zeros(ctx.shape, bool)
        for s in ctx.steps["fields"][1::2]:
            fy |= footprint(ctx.shape, ctx.px(s), 0.1, 0.9, 0.9)
        yellow = fy & ~touched
        y_lab = _stroke_med(ctx, yellow)
        res = {}
        for s, tag in zip(sm, ("w016", "w024", "w036")):
            pts = ctx.px(s)
            fp = footprint(ctx.shape, pts, 0.0, 1.0, 0.6)
            xs_ = np.where(fp.any(0))[0]
            if y_lab is None or len(xs_) == 0:
                continue
            # carry: how far right of the seam the smudged colour differs from clean yellow (Delta E > 8), in cw
            far = 0.0
            for x in range(int(xm) + 2, xs_.max() + 1):
                col = fp[:, x]
                if col.sum() < 3:
                    continue
                if np.linalg.norm(np.median(ctx.lab[col, x], axis=0) - y_lab) > 8:
                    far = (x - xm) / ctx.W
            res[tag] = dict(carry_cw=float(far))
            inside = fp & (np.arange(ctx.shape[1])[None, :] > xm + 4)
            outside = yellow & (np.arange(ctx.shape[1])[None, :] > xm + 4)
            if inside.sum() > 30 and outside.sum() > 30:
                res[tag]["flatten_dh"] = float(np.mean(ctx.h[outside]) - np.mean(ctx.h[inside]))
        out.update(res)
    elif name == "mode_glaze":
        patch = np.zeros(ctx.shape, bool)
        for s in ctx.steps["patch"]:
            patch |= footprint(ctx.shape, ctx.px(s), 0.05, 0.95)
        patch = EM.erode(patch, 0.004 * ctx.W)
        gl = ctx.steps["glaze"]
        base = _stroke_med(ctx, patch)
        res = {}
        for s, tag in zip(gl, ("w012", "w024", "w048")):
            fp = footprint(ctx.shape, ctx.px(s), 0.1, 0.9, 0.7) & patch
            others = patch.copy()
            for s2 in gl:
                others &= ~footprint(ctx.shape, ctx.px(s2), 0.0, 1.0, 1.3)
            if fp.sum() > 30 and others.sum() > 30:
                res[tag] = dict(tint_dE=float(np.linalg.norm(np.median(ctx.lab[fp], axis=0) - np.median(ctx.lab[others], axis=0))),
                                dh=float(np.mean(ctx.h[fp]) - np.mean(ctx.h[others])))
        out.update(res)
    return out


def m_long_strokes(ctx):
    out = {}
    st = ctx.all_strokes()
    keys = ("w012", "w024", "w048")
    for s, tag in zip(st, keys):
        pts = ctx.px(s)
        out[tag] = dict(**edge_metrics(ctx, pts, 0.15, 0.85), **height_lane_metrics(ctx, pts, 0.25, 0.75))
    # roll up (median over the three widths) for a compact headline
    for k in ("width_ratio", "width_cv", "edge_rough", "edge_kink", "outline_step"):
        v = [out[t][k] for t in keys if t in out and k in out[t]]
        if v:
            out[k] = float(np.median(v))
    return out


# ------------------------------------------------------------------ kind: dry tail
def m_dry_tail(ctx):
    out = {}
    for s, tag in zip(ctx.steps["dry"], ("viridian", "violet")):
        pts = ctx.px(s)
        mid = footprint(ctx.shape, pts, 0.30, 0.45, 0.8)
        tail = footprint(ctx.shape, pts, 0.85, 0.97, 0.8)
        cm = float((ctx.dE[mid] > DE_PAINT).mean()) if mid.sum() > 20 else float("nan")
        ct = float((ctx.dE[tail] > DE_PAINT).mean()) if tail.sum() > 20 else float("nan")
        out[tag] = dict(mid_cov=cm, tail_cov=ct, tail_over_mid=ct / max(cm, 1e-6), tail_gap_frac=1.0 - ct)
    out["tail_over_mid"] = float(np.mean([out[t]["tail_over_mid"] for t in ("viridian", "violet") if t in out]))
    return out


# ------------------------------------------------------------------ kind: dabs
def _contour_straight_frac(mask, w_px):
    """Fraction of the outline whose curvature is below 0.6 / width (straight runs) - a circle scores 0, a polygon ~ its edge share."""
    cs, _ = cv2.findContours(mask.astype(np.uint8), cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_NONE)
    if not cs:
        return None
    c = max(cs, key=cv2.contourArea).reshape(-1, 2).astype(np.float64)
    if len(c) < 20:
        return None
    # resample by arc length at ~0.5 px, smooth with sigma ~ 0.07 width, then curvature
    d = np.concatenate([[0], np.cumsum(np.hypot(*np.diff(np.vstack([c, c[:1]]), axis=0).T))])
    n = max(32, int(d[-1] / 0.5))
    t = np.linspace(0, d[-1], n, endpoint=False)
    cx = np.interp(t, d, np.append(c[:, 0], c[0, 0])); cy = np.interp(t, d, np.append(c[:, 1], c[0, 1]))
    sig = max(1.0, 0.07 * w_px / (d[-1] / n))
    k = int(4 * sig)
    g = np.exp(-0.5 * (np.arange(-k, k + 1) / sig) ** 2); g /= g.sum()
    sm = lambda a: np.convolve(np.pad(a, k, mode="wrap"), g, mode="valid")
    x, y = sm(cx), sm(cy)
    ds = d[-1] / n
    dx, dy = np.gradient(x, ds), np.gradient(y, ds)
    ddx, ddy = np.gradient(dx, ds), np.gradient(dy, ds)
    kappa = np.abs(dx * ddy - dy * ddx) / np.maximum((dx * dx + dy * dy) ** 1.5, 1e-9)
    return float((kappa * w_px < 0.6).mean())


def _ellipse_dev(comp):
    """1 - IoU between a silhouette and its best-fit ellipse: ~0 for a round dab, 0.1-0.2 for a hexagon / octagon."""
    cnt = max(cv2.findContours(comp.astype(np.uint8), cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_NONE)[0], key=cv2.contourArea)
    if len(cnt) < 8:
        return None
    el = cv2.fitEllipse(cnt)
    em = np.zeros(comp.shape, np.uint8)
    cv2.ellipse(em, el, 1, -1, cv2.LINE_8)
    em = em > 0
    return float(1.0 - (em & comp).sum() / max((em | comp).sum(), 1))


def m_dabs(ctx):
    out = {}
    st = ctx.steps["dabs"]
    fr = {}
    for i, s in enumerate(st):
        ri, ci = divmod(i, 4)
        pts = ctx.px(s)
        cx, cy = pts[:, 0].mean(), pts[:, 1].mean()
        w = float(pts[:, 2].max())
        R = int(1.3 * max(w, np.hypot(np.ptp(pts[:, 0]), np.ptp(pts[:, 1]))) + 6)
        y0, y1, x0, x1 = max(0, int(cy) - R), min(ctx.shape[0], int(cy) + R), max(0, int(cx) - R), min(ctx.shape[1], int(cx) + R)
        sil = ctx.dE[y0:y1, x0:x1] > DE_PAINT
        n, lab_, stats, _ = cv2.connectedComponentsWithStats(sil.astype(np.uint8), connectivity=8)
        if n < 2:
            continue
        j = 1 + int(np.argmax(stats[1:, cv2.CC_STAT_AREA]))
        comp = lab_ == j
        if comp.sum() < 30:
            continue
        cnt = cv2.findContours(comp.astype(np.uint8), cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_NONE)[0]
        hull = cv2.convexHull(max(cnt, key=cv2.contourArea))
        solid = float(comp.sum() / max(cv2.contourArea(hull), 1.0))
        sf = _contour_straight_frac(comp, min(w, np.ptp(np.nonzero(comp)[1]) + 1))
        fr[(ri, ci)] = dict(straight_frac=sf, solidity=min(solid, 1.0), ellipse_dev=_ellipse_dev(comp))
    # the compact dabs (aspect 1.2, 1.8) at the two larger widths carry the faceting score
    vals = [fr[k]["straight_frac"] for k in fr if k[0] >= 1 and k[1] <= 1 and fr[k]["straight_frac"] is not None]
    if vals:
        out["facet_straight_frac"] = float(np.mean(vals))
    ed = [fr[k]["ellipse_dev"] for k in fr if k[0] >= 1 and fr[k]["ellipse_dev"] is not None]
    if ed:
        out["facet_ellipse_dev"] = float(np.mean(ed))
    sol = [fr[k]["solidity"] for k in fr if k[0] >= 1]
    if sol:
        out["solidity"] = float(np.mean(sol))
    out["n_measured"] = len(fr)
    return out


# ------------------------------------------------------------------ kind: blends
def _mix_curve(rgb_top, rgb_under, n=101):
    """Lab of the pigment (Mixbox) mixing curve top -> under, always Mixbox whatever backend painted the sheet."""
    prev = mix.backend()
    mix.set_backend("mixbox")
    try:
        t = np.linspace(0, 1, n, dtype=np.float32)
        lat = mix.lerp_latent(mix.rgb_to_latent(rgb_top)[None, :], mix.rgb_to_latent(rgb_under)[None, :], t)
        lab = mix.rgb_to_lab(mix.latent_to_rgb(lat))
    finally:
        mix.set_backend(prev)
    return t, lab


def blend_zone(ctx, under, over, t0=0.15, t1=0.85):
    """Overlap zone of two strokes: where does its colour sit relative to the pigment mixing curve top -> under?"""
    pu, po = ctx.px(under), ctx.px(over)
    fu = footprint(ctx.shape, pu, t0, t1)
    fo = footprint(ctx.shape, po, t0, t1)
    zone = EM.erode(fu & fo, max(1.5, 0.0015 * ctx.W))
    top_only = EM.erode(fo & ~EM.dilate(fu, 1), max(2.0, 0.003 * ctx.W))
    und_only = EM.erode(fu & ~EM.dilate(fo, 1), max(2.0, 0.003 * ctx.W))
    if zone.sum() < 30 or top_only.sum() < 30 or und_only.sum() < 30:
        return {}
    med_rgb = lambda m: np.median(ctx.unlit[m], axis=0).astype(np.float32)
    rt, ru = med_rgb(top_only), med_rgb(und_only)
    t, curve = _mix_curve(rt, ru)
    zl = np.median(ctx.lab[zone], axis=0)
    d = np.linalg.norm(curve - zl, axis=1)
    j = int(np.argmin(d))
    Cz = float(np.hypot(zl[1], zl[2])); Cc = float(np.hypot(curve[j, 1], curve[j, 2]))
    lt, lu = mix.rgb_to_lab(rt), mix.rgb_to_lab(ru)
    out = dict(mix_t=float(t[j]), off_curve_dE=float(d[j]), chroma_vs_curve=Cz / max(Cc, 1e-6), zone_C=Cz, zone_L=float(zl[0]),
               zone_a=float(zl[1]), zone_hue=float(np.degrees(np.arctan2(zl[2], zl[1])) % 360),
               green_excursion=float(min(lt[1], lu[1]) - zl[1]), zone_px=int(zone.sum()))
    # how much of the overlap looks like grey-brown mud
    zL, zC = ctx.lab[zone][:, 0], np.hypot(ctx.lab[zone][:, 1], ctx.lab[zone][:, 2])
    out["zone_mud_frac"] = float(((zL > 20) & (zL < 65) & (zC < 12)).mean())
    return out


def m_blend(ctx):
    out = {}
    n = ctx.name
    if n in ("blend_wet_in_wet", "blend_wet_on_dry"):
        us, os_ = ctx.steps["yellow"], ctx.steps["blue"]
        for u, o, tag in zip(us, os_, ("pk012", "pk030", "pk060")):
            out[tag] = blend_zone(ctx, u, o)
    else:
        us, os_ = ctx.steps["under"], ctx.steps["over"]
        for u, o, tag in zip(us, os_, ("orange_blue_pk012", "orange_blue_pk030", "yellow_violet_pk012", "yellow_violet_pk030")):
            out[tag] = blend_zone(ctx, u, o)
    return out


def m_mud_stack(ctx):
    st = ctx.steps["stacks"]
    out = {}
    i = 0
    for k in (2, 4, 6, 8):
        grp = st[i:i + k]; i += k
        top = grp[-1]
        z = EM.erode(footprint(ctx.shape, ctx.px(top), 0.2, 0.8, 0.8), max(2.0, 0.003 * ctx.W))
        if z.sum() < 30:
            continue
        zl = np.median(ctx.lab[z], axis=0)
        nom = ctx.lab_of_stroke(top)
        Cn = float(np.hypot(nom[1], nom[2]))
        zz = ctx.lab[z]
        zC = np.hypot(zz[:, 1], zz[:, 2])
        out[f"k{k}"] = dict(chroma_ratio=float(np.hypot(zl[1], zl[2]) / max(Cn, 1e-6)), dE_from_top=float(np.linalg.norm(zl - nom)),
                            zone_C=float(np.hypot(zl[1], zl[2])), zone_hue=float(np.degrees(np.arctan2(zl[2], zl[1])) % 360),
                            zone_mud_frac=float(((zz[:, 0] > 20) & (zz[:, 0] < 65) & (zC < 12)).mean()))
    ks = [out[f"k{k}"]["chroma_ratio"] for k in (2, 4, 6, 8) if f"k{k}" in out]
    if len(ks) >= 2:
        out["chroma_k8_ratio"] = ks[-1]
        out["mud_frac_k8"] = out["k8"]["zone_mud_frac"]
        out["chroma_drop_2_to_8"] = ks[0] - ks[-1]
    return out


# ------------------------------------------------------------------ kind: gradient
def m_gradient(ctx):
    st = ctx.steps["ramp"]
    ys = np.array([ctx.px(s)[:, 1].mean() for s in st])
    cols = np.stack([ctx.lab_of_stroke(s) for s in st])
    xs = ctx.px(st[0])[:, 0]
    x0, x1 = int(xs.min() + 0.25 * np.ptp(xs)), int(xs.min() + 0.75 * np.ptp(xs))
    ya, yb = int(ys[0]), int(ys[-1])
    rows = np.arange(ya, yb)
    P = np.stack([np.median(ctx.lab[y, x0:x1], axis=0) for y in rows])
    ideal = np.stack([np.interp(rows, ys, cols[:, k]) for k in range(3)], 1)
    d = max(1, int(round(0.003 * ctx.W)))
    step = np.linalg.norm(P[d:] - P[:-d], axis=1)
    return dict(ramp_rmse=float(np.sqrt(np.mean(np.sum((P - ideal) ** 2, axis=1)))), step_dE_p95=float(np.percentile(step, 95)),
                step_dE_max=float(step.max()))


# ------------------------------------------------------------------ kind: form studies
def _intended_L(ctx, strokes):
    """The albedo the painter meant: nominal footprints filled with each stroke's colour, later strokes over earlier."""
    L = np.full(ctx.shape, np.nan, np.float32)
    for s in strokes:
        fp = footprint(ctx.shape, ctx.px(s))
        L[fp] = float(ctx.lab_of_stroke(s)[0])
    return L


def _mnorm_blur(img, mask, sigma):
    num = EM._g(np.where(mask, img, 0.0), sigma)
    den = EM._g(mask.astype(np.float32), sigma)
    return num / np.maximum(den, 1e-3)


def m_form(ctx):
    st = ctx.all_strokes()
    Li = _intended_L(ctx, st)
    have = np.isfinite(Li)
    mask = EM.erode(have, max(3.0, 0.006 * ctx.W))
    if mask.sum() < 200:
        return {}
    sg = max(0.008 * ctx.W, 2.0)
    I = _mnorm_blur(np.nan_to_num(Li), mask, sg)
    Ll = _mnorm_blur(ctx.lab_lit[..., 0], mask, sg)
    Lu = _mnorm_blur(ctx.lab[..., 0], mask, sg)
    c = lambda a, b: float(np.corrcoef(a[mask], b[mask])[0, 1])
    lit_d = ctx.lab_lit[..., 0] - ctx.lab[..., 0]
    sd = float(np.std(np.nan_to_num(Li)[mask]))
    return dict(form_corr_lit=c(I, Ll), form_corr_unlit=c(I, Lu), relief_leak=float(np.std(lit_d[mask]) / max(sd, 1e-6)),
                tone_range_ratio=float((np.percentile(Ll[mask], 95) - np.percentile(Ll[mask], 5)) / max(np.percentile(I[mask], 95) - np.percentile(I[mask], 5), 1e-6)))


def m_pile(ctx):
    z = np.zeros(ctx.shape, bool)
    for s in ctx.steps["pile"]:
        z |= footprint(ctx.shape, ctx.px(s), 0.2, 0.8, 0.8)
    z = EM.erode(z, max(2.0, 0.004 * ctx.W))
    if z.sum() < 100:
        return {}
    hz = ctx.h[z]
    return dict(pile_h_p50=float(np.median(hz)), pile_h_p95=float(np.percentile(hz, 95)), pile_h_max=float(hz.max()))


def m_strata(ctx):
    """Seams between overlapping long strokes: how much of the stroke outlines is drawn as a thin dark line by the light
    (the 'geological strata' flaw), against the same measure inside the strokes (bristle grooves)."""
    st = ctx.steps["sky strokes"]
    union = np.zeros(ctx.shape, bool)
    band = np.zeros(ctx.shape, bool)
    r = max(1.5, 0.0025 * ctx.W)
    for s in st:
        fp = footprint(ctx.shape, ctx.px(s), 0.03, 0.97)
        union |= fp
        band |= EM.dilate(fp, r) & ~EM.erode(fp, r)
    inside = EM.erode(union, 2.5 * r)
    seam = band & EM.dilate(ctx.h > EM.PAINT_H, 2)
    V, _ = EM.shade_valley(ctx.lit, ctx.unlit, ctx.W)
    out = {}
    if seam.sum() > 200 and inside.sum() > 200:
        vs, vi = V[seam], V[inside]
        out.update(seam_hairline=float((vs > EM.HAIR_DEPTH_SHADE).mean()), seam_hairline_deep=float((vs > EM.HAIR_DEEP_SHADE).mean()),
                   inside_hairline=float((vi > EM.HAIR_DEPTH_SHADE).mean()), seam_depth_p90=float(np.percentile(vs, 90)),
                   seam_over_inside=float((vs > EM.HAIR_DEPTH_SHADE).mean() / max((vi > EM.HAIR_DEPTH_SHADE).mean(), 1e-3)))
    return out


KIND_FUNCS = dict(single_strokes=m_single_strokes, long_strokes=m_long_strokes, dry_tail=m_dry_tail, dabs=m_dabs, blend=m_blend,
                  mud_stack=m_mud_stack, gradient=m_gradient, form=m_form, pile=m_pile, strata=m_strata)


def swatch_metrics(ctx, kind, lit, unlit, h):
    """Generic image metrics on the swatch crop (painted area) + the kind-specific ones."""
    paint = EM.erode(EM.dilate(ctx.h > EM.PAINT_H, 2), 1)
    if paint.sum() < 200:      # glaze / scumble on bare ground: fall back to Delta E from the ground
        paint = ctx.dE > DE_PAINT
    m = EM.image_metrics(ctx.lit, ctx.unlit, ctx.h, mask=paint, lab=ctx.lab_lit, detail=False, cw_px=ctx.W)
    m["kind"] = kind
    fn = KIND_FUNCS.get(kind)
    if fn:
        try:
            m.update(fn(ctx))
        except Exception as e:            # a broken swatch metric must not kill the whole run: report and continue
            if os.environ.get("OILPAINT_EVAL_DEBUG"):
                raise
            m["error"] = f"{type(e).__name__}: {e}"
    return m
