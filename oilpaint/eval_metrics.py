"""Image-level and stroke-level metrics for the evaluation harness (see oilpaint/eval.py and README "Evaluating changes").

Pure functions of arrays: no rendering, no files.  Everything is defined in canvas-width (cw) units, so a value means
the same thing at any image width W as long as the feature is resolvable (`resolved` flags say when it is not):

  scale                          definition                                      resolved when
  hairline line / background     valley sigma 0.0007 / 0.0025 cw (~1.1 / 4 px @1600) W >= 1250 (line sigma >= 0.9 px)
  bristle band                   DoG sigma 0.0004 / 0.0012 cw  (periods 0.0015-0.008 cw) W >= 1250
  stroke scale (high-pass)       Gaussian sigma 0.010 cw
  structure tensor               gradient sigma 0.002 cw, integration sigma 0.012 cw (evaluated at <= 800 px wide)

Inputs: `lit` float RGB (H,W,3) 0..1 (what a viewer sees; the only thing a scanned painting has), optional `unlit` albedo
and `h` paint height from the renderer.  Metrics that need unlit+h (the lighting-term ones) are absent for reference images.
"""
import numpy as np
import cv2

from . import mix

BRISTLE_BAND = (0.0004, 0.0012)      # DoG sigmas, cw
HAIR_LINE, HAIR_BG = 0.0007, 0.0025  # DoG sigmas, cw
HAIR_DEPTH_SHADE = 3.0               # a thin dark line in the lighting term deeper than this (% of luminance) is a hairline pixel
HAIR_DEPTH_LIT = 2.0                 # same in lit L* units (includes albedo streaks)
HAIR_DEEP_SHADE = 8.0                # a deep dark line (the ones that read as a drawn outline rather than bristle grooves)
STROKE_SIGMA = 0.010                 # cw, separates bristle texture from stroke-scale tone
RESOLVED_W = 1250                    # below this width the bristle / hairline scales fall under ~1 px
PAINT_H = 0.05                       # paint height above which a pixel counts as painted (hgain units)
STEP_D = 0.0012                      # cw, half-baseline of the edge-step measurement
WORK_W = 800                         # structure tensor / autocorrelation working width


# ------------------------------------------------------------------ small helpers
def _g(img, sigma):
    return cv2.GaussianBlur(np.ascontiguousarray(img, np.float32), (0, 0), float(sigma))


def lum(rgb):
    return (0.2126 * rgb[..., 0] + 0.7152 * rgb[..., 1] + 0.0722 * rgb[..., 2]).astype(np.float32)


def lab_of(rgb, chunk=384):
    """rgb_to_lab in row chunks (keeps peak memory low on 7 Mpx images)."""
    rgb = np.asarray(rgb, np.float32)
    H = rgb.shape[0]
    out = np.empty(rgb.shape, np.float32)
    for y in range(0, H, chunk):
        out[y:y + chunk] = mix.rgb_to_lab(rgb[y:y + chunk])
    return out


def dog(img, W, a_cw, b_cw):
    sa = max(a_cw * W, 0.5)
    sb = max(b_cw * W, 2.0 * sa)
    return _g(img, sa) - _g(img, sb)


def valley(img, W, line=HAIR_LINE, bg=HAIR_BG):
    """Depth of thin dark lines: (wide blur - narrow blur)+ ; units of img."""
    sl = max(line * W, 0.7)
    sb = max(bg * W, 2.5 * sl)
    return np.clip(_g(img, sb) - _g(img, sl), 0, None)


def shade_valley(lit, unlit, W):
    """Depth (percent of luminance) of thin dark lines in the lighting term  log(lum(lit)) - log(lum(unlit)).  The albedo cancels,
    so a hit is relief the light draws, not paint colour.  Needs the renderer's unlit image."""
    shade = ((np.log(lum(lit) + 0.02) - np.log(lum(unlit) + 0.02)) * 100.0).astype(np.float32)
    return valley(shade, W), shade


def _disk(r):
    r = max(1, int(round(r)))
    return cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (2 * r + 1, 2 * r + 1))


def erode(mask, r_px):
    return cv2.erode(mask.astype(np.uint8), _disk(r_px)) > 0


def dilate(mask, r_px):
    return cv2.dilate(mask.astype(np.uint8), _disk(r_px)) > 0


def _rms(x):
    return float(np.sqrt(np.mean(np.square(x)))) if x.size else float("nan")


def _pct(x, q):
    return float(np.percentile(x, q)) if x.size else float("nan")


# ------------------------------------------------------------------ structure tensor + autocorrelation
def structure_maps(lab, wwork=WORK_W):
    """Smoothed structure tensor (Jxx, Jxy, Jyy) of the Lab image at <= wwork px wide (colour edges count)."""
    H, W = lab.shape[:2]
    if W > wwork:
        lab = cv2.resize(lab, (wwork, max(1, int(round(H * wwork / W)))), interpolation=cv2.INTER_AREA)
    Wn = lab.shape[1]
    sd, si = max(0.002 * Wn, 1.0), max(0.012 * Wn, 3.0)
    Jxx = Jxy = Jyy = 0.0
    for k, wk in enumerate((1.0, 0.35, 0.35)):
        c = _g(lab[..., k], sd)
        gx = cv2.Sobel(c, cv2.CV_32F, 1, 0, ksize=3)
        gy = cv2.Sobel(c, cv2.CV_32F, 0, 1, ksize=3)
        Jxx = Jxx + wk * gx * gx; Jxy = Jxy + wk * gx * gy; Jyy = Jyy + wk * gy * gy
    return _g(Jxx, si), _g(Jxy, si), _g(Jyy, si), si


def _mask_work(mask, shape):
    return cv2.resize(mask.astype(np.uint8), (shape[1], shape[0]), interpolation=cv2.INTER_NEAREST) > 0


def struct_stats(maps, mask):
    """Direction coherence inside a mask.  coh_local: energy-weighted mean of the local coherence (1 = every neighbourhood
    has one clear direction); dir_R: consistency of that direction across the whole region (1 = all the same);
    orient_deg: dominant stroke direction, 0 = horizontal, 90 = vertical."""
    Jxx, Jxy, Jyy, si = maps
    m = erode(_mask_work(mask, Jxx.shape), 0.5 * si)
    if m.sum() < 50:
        return {}
    a, b, c = Jxx[m], Jxy[m], Jyy[m]
    tr = a + c + 1e-9
    coh = np.sqrt((a - c) ** 2 + 4 * b * b) / tr
    w = tr / tr.sum()
    theta = 0.5 * np.arctan2(2 * b, a - c)          # gradient orientation
    R = np.hypot((w * np.cos(2 * theta)).sum(), (w * np.sin(2 * theta)).sum())
    mean_t = 0.5 * np.arctan2((w * np.sin(2 * theta)).sum(), (w * np.cos(2 * theta)).sum())
    return dict(coh_local=float((w * coh).sum()), dir_R=float(R), orient_deg=float((np.degrees(mean_t) + 90.0) % 180.0))


def acf_scales(lab_L, mask, W_full):
    """Characteristic patch size of the tonal texture from the masked autocorrelation of L*: the equivalent ellipse of the
    ACF >= 0.5 contour.  Returns (minor, major, orient_deg) in cw.  A proxy for stroke width / length when no stroke list exists."""
    H, W = lab_L.shape
    f = W / 260.0 if W > 260 else 1.0                      # work at <= ~260 px per cw
    if f > 1.0:
        L = cv2.resize(lab_L, (int(W / f), int(H / f)), interpolation=cv2.INTER_AREA)
        m = cv2.resize(mask.astype(np.uint8), (L.shape[1], L.shape[0]), interpolation=cv2.INTER_NEAREST) > 0
    else:
        L, m = lab_L, mask
    ys, xs = np.where(m)
    if len(ys) < 200:
        return None
    y0, y1, x0, x1 = ys.min(), ys.max() + 1, xs.min(), xs.max() + 1
    L, m = L[y0:y1, x0:x1], m[y0:y1, x0:x1]
    cwpx = 1.0 / (W / f)                                    # cw per work pixel
    x = L - _g(L, max(0.05 / cwpx, 3.0))
    x = np.where(m, x, 0.0).astype(np.float64)
    mf = m.astype(np.float64)
    sh = (2 * x.shape[0], 2 * x.shape[1])
    F = np.fft.rfft2(x, s=sh)
    ac = np.fft.irfft2(F * np.conj(F), s=sh)
    Fm = np.fft.rfft2(mf, s=sh)
    nm = np.fft.irfft2(Fm * np.conj(Fm), s=sh)
    ac = np.fft.fftshift(ac); nm = np.fft.fftshift(nm)
    ok = nm > 0.25 * nm.max()
    acn = np.where(ok, ac / np.maximum(nm, 1e-9), 0.0)
    cy, cx = sh[0] // 2, sh[1] // 2
    if acn[cy, cx] <= 1e-9:
        return None
    acn = acn / acn[cy, cx]
    lab_cc = cv2.connectedComponents((acn >= 0.5).astype(np.uint8), connectivity=8)[1]
    comp = lab_cc == lab_cc[cy, cx]
    yy, xx = np.nonzero(comp)
    if len(yy) < 3:
        return (cwpx * 1.5, cwpx * 1.5, 0.0)
    cov = np.cov(np.stack([xx - cx, yy - cy]))
    ev, evec = np.linalg.eigh(cov)
    minor, major = 4 * np.sqrt(max(ev[0], 1 / 12)) * cwpx, 4 * np.sqrt(max(ev[1], 1 / 12)) * cwpx   # a 1-px line has variance 1/12
    orient = float(np.degrees(np.arctan2(evec[1, 1], evec[0, 1])) % 180.0)
    return float(minor), float(major), orient


# ------------------------------------------------------------------ image-level metrics
def image_metrics(lit, unlit=None, h=None, mask=None, lab=None, maps=None, detail=True):
    """All image-level metrics of `lit` inside `mask`.  Returns a flat dict of floats / bools (missing = not computable)."""
    H, W = lit.shape[:2]
    if mask is None:
        mask = np.ones((H, W), bool)
    m = dict(width_px=int(W))
    npx = int(mask.sum())
    m["area_cw2"] = npx / float(W * W)
    if npx < 300:
        return m
    if lab is None:
        lab = lab_of(lit)
    L, a, b = lab[..., 0], lab[..., 1], lab[..., 2]
    C = np.hypot(a, b)
    Lm, Cm = L[mask], C[mask]
    hue = np.degrees(np.arctan2(b[mask], a[mask])) % 360.0
    # ---- colour statistics (of the lit image, so references are comparable)
    m.update(L_min=float(Lm.min()), L_p01=_pct(Lm, 1), L_p05=_pct(Lm, 5), L_p50=_pct(Lm, 50), L_p95=_pct(Lm, 95), L_p99=_pct(Lm, 99),
             C_mean=float(Cm.mean()), C_p50=_pct(Cm, 50), C_p90=_pct(Cm, 90), C_p99=_pct(Cm, 99))
    chrom = Cm > 12
    m["chromatic_frac"] = float(chrom.mean())
    hc = hue[chrom]
    if hc.size > 100:
        hist, _ = np.histogram(hc, bins=24, range=(0, 360))
        hist = hist / hist.sum()
        sm = (np.roll(hist, 1) + hist + np.roll(hist, -1)) / 3.0
        peaks = [i for i in range(24) if sm[i] >= sm[(i - 1) % 24] and sm[i] > sm[(i + 1) % 24] and sm[i] >= 0.06]
        m.update(hue_hist24=[round(float(x), 4) for x in hist], hue_lobes=len(peaks), hue_dominant_deg=float((np.argmax(hist) + 0.5) * 15.0),
                 hue_entropy=float(-(hist[hist > 0] * np.log2(hist[hist > 0])).sum() / np.log2(24)),
                 cool_frac=float(((hc >= 230) & (hc < 300)).mean()), warm_frac=float(((hc >= 30) & (hc < 75)).mean()),
                 green_frac=float(((hc >= 90) & (hc < 180)).mean()))
    m["mud_frac"] = float(((Lm > 20) & (Lm < 65) & (Cm < 12) & (hue >= 20) & (hue < 110)).mean())
    # ---- bristle-scale texture of the lit luminance
    inner = erode(mask, max(2.0, 0.002 * W))
    if inner.sum() > 200:
        band = dog(L, W, *BRISTLE_BAND)
        hp = L - _g(L, max(STROKE_SIGMA * W, 2.0))
        bi, hi = band[inner], hp[inner]
        m["bristle_L"] = _rms(bi)
        m["bristle_frac"] = float(np.var(bi) / max(np.var(hi), 1e-6))
        m["bristle_resolved"] = bool(W >= RESOLVED_W)
        maskd = dilate(mask, max(2.0, 0.003 * W))
        m["hairline_lit"] = float((valley(L, W)[maskd] > HAIR_DEPTH_LIT).mean())
    # ---- relief and lighting-term metrics (need the renderer's albedo and height)
    if h is not None:
        painted = mask & (h > PAINT_H)
        if painted.sum() > 200:
            hp_ = h[painted]
            m.update(h_p50=_pct(hp_, 50), h_p95=_pct(hp_, 95), h_max=float(hp_.max()), painted_frac=float(painted.sum() / npx))
            d = max(1, int(round(STEP_D * W)))
            ex = np.zeros_like(h); ey = np.zeros_like(h)
            ex[:, d:-d] = np.abs(h[:, 2 * d:] - h[:, :-2 * d]); ey[d:-d] = np.abs(h[2 * d:] - h[:-2 * d])
            e = np.maximum(ex, ey)
            maskd = dilate(painted, max(2.0, 0.003 * W))
            ev = e[maskd]
            m.update(edge_step_p90=_pct(ev, 90), edge_step_p99=_pct(ev, 99), edge_step_rel=_pct(ev, 99) / max(m["h_p50"], 1e-3))
            innerp = erode(painted, max(2.0, 0.002 * W))
            if innerp.sum() > 200:
                m["bristle_h"] = _rms(dog(h, W, *BRISTLE_BAND)[innerp])
    if unlit is not None and h is not None:
        painted = mask & (h > PAINT_H)
        if painted.sum() > 200:
            V, shade = shade_valley(lit, unlit, W)                                    # lighting term, % of luminance
            maskd = dilate(painted, max(2.0, 0.003 * W))
            Vm = V[maskd]
            m["hairline"] = float((Vm > HAIR_DEPTH_SHADE).mean())
            m["hairline_deep"] = float((Vm > HAIR_DEEP_SHADE).mean())
            m["hairline_p99"] = _pct(Vm, 99)
            innerp = erode(painted, max(2.0, 0.002 * W))
            if innerp.sum() > 200:
                m["bristle_relief"] = _rms(dog(shade.astype(np.float32), W, *BRISTLE_BAND)[innerp])
            m["relief_cov"] = float((np.abs(lum(lit) - lum(unlit))[mask] > 0.03).mean())
    # ---- direction coherence, patch size
    if detail:
        if maps is None:
            maps = structure_maps(lab)
        m.update(struct_stats(maps, mask))
        ac = acf_scales(L, mask, W)
        if ac is not None:
            m.update(acf_minor=ac[0], acf_major=ac[1], acf_aspect=ac[1] / max(ac[0], 1e-6), acf_orient_deg=ac[2])
    return m


# ------------------------------------------------------------------ stroke lists
def stroke_stats(strokes, names=None):
    """Width / length / direction statistics of a stroke list (canvas-width units), overall and per region."""
    rows = []
    for s in strokes:
        p = np.asarray(s.pts, np.float64)
        w = float(p[:, 2].max())
        ln = float(np.hypot(np.diff(p[:, 0]), np.diff(p[:, 1])).sum())
        v = p[-1, :2] - p[0, :2]
        rows.append((int(s.region), w, ln, float(np.arctan2(v[1], v[0]))))
    if not rows:
        return {}
    arr = np.array(rows)

    def summ(a):
        w, ln, ang = a[:, 1], a[:, 2], a[:, 3]
        z = (ln * np.exp(2j * ang)).sum() / max(ln.sum(), 1e-9)
        return dict(n=int(len(a)), width_med=float(np.median(w)), width_p10=_pct(w, 10), width_p90=_pct(w, 90), length_med=float(np.median(ln)),
                    aspect_med=float(np.median(ln / np.maximum(w, 1e-6))), dir_R=float(abs(z)), orient_deg=float((np.degrees(0.5 * np.angle(z))) % 180.0))

    out = dict(overall=summ(arr), regions={})
    for r in sorted(set(arr[:, 0].astype(int))):
        sel = arr[arr[:, 0] == r]
        if len(sel) >= 5:
            out["regions"][names[r] if names is not None and r < len(names) else str(r)] = summ(sel)
    return out
