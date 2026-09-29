"""T7: bristle-lane surface model at native (2400-class) resolution: ridge amplitude, edge crispness,
two-colour marbling, edge levees, gloss/gel check."""
import os
import sys
import numpy as np
import cv2

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
sys.path.insert(0, ROOT)
from oilpaint import mix, light, strokes as S
from oilpaint.canvas import Canvas
from oilpaint.sheet import save_sheet


def t7(OUT, gate):
    print("T7 surface model (2400-class resolution)")
    W, H = 2400, 900          # a 2400-wide canvas strip: stroke widths in cw are the same as in the picture
    cv = Canvas(W, H)
    items = []
    w = 0.02                  # 2 % cw = 48 px, Monet's main flat brush
    nb = int(5 + 450 * w)
    # (a) a plain loaded stroke, (b) two-colour marbled, (c) running dry, (d) a soft-edge one for contrast
    sts = [S.make_stroke(S.straight(0.04, 0.06, 0.46, 0.06, w), "cobalt_blue", 11, nb=nb, pickup=0.0),
           S.make_stroke(S.straight(0.54, 0.06, 0.96, 0.06, w), ("cadmium_yellow", "lead_white", 0.3), 12, nb=nb,
                         color2="vermilion", marble=1.0, pickup=0.0, streak=0.1),
           S.make_stroke(S.straight(0.04, 0.16, 0.96, 0.16, w), "viridian", 13, nb=nb, deplete=0.06, vdry=0.5, pickup=0.0),
           S.make_stroke(S.straight(0.04, 0.26, 0.46, 0.26, w), "madder", 14, nb=nb, hardness=0.1, pickup=0.0),
           S.make_stroke(S.straight(0.54, 0.26, 0.96, 0.26, w), ("cobalt_violet_light", "lead_white", 0.4), 15, nb=nb,
                         ridge=1.0, levee=0.6, stiff=0.5, pickup=0.0)]
    cv.render(sts)
    unlit = cv.rgb.copy(); h = cv.h.copy()
    lit = light.relight(unlit, h)
    lit3 = light.relight(unlit, h, bump=3.0, contrast=0.6, spec=0.18)
    light.save_png(os.path.join(OUT, "t7_strokes_unlit.png"), unlit)
    light.save_png(os.path.join(OUT, "t7_strokes_lit.png"), lit)
    light.save_png(os.path.join(OUT, "t7_strokes_lit_bump3.png"), lit3)
    light.save_gray(os.path.join(OUT, "t7_strokes_height.png"), h)
    yc = int(0.06 * W); xa, xb = int(0.15 * W), int(0.35 * W)
    half = int(w * W * 0.75)
    # ---- ridge amplitude and lane count across the plain stroke (averaged over a short along-window)
    prof = h[yc - half:yc + half + 1, xa:xa + 12].mean(1)
    inside = prof > 0.15 * prof.max()
    pi = prof[inside]
    amp = (np.percentile(pi, 90) - np.percentile(pi, 10)) / max(pi.mean(), 1e-6)
    ps = np.convolve(prof, [0.25, 0.5, 0.25], mode="same")
    maxima = sum(1 for i in range(1, len(ps) - 1) if ps[i] > ps[i - 1] and ps[i] >= ps[i + 1] and ps[i] > 0.2 * ps.max())
    gate("T7 bristle ridges across a 2% brush at 2400: (p90-p10)/mean > 0.25 and >= 8 lanes",
         amp > 0.25 and maxima >= 8, f"amplitude {amp:.2f}, {maxima} maxima across {int(inside.sum())} px")
    # ---- edge crispness: cover 0.1 -> 0.9 across the boundary, median over columns
    widths = []
    for x in range(xa, xb, 7):
        col = cv.cover[yc - half:yc + half + 1, x]
        top = np.argmax(col > 0.9)
        lo = np.argmax(col > 0.1)
        widths.append(top - lo)
    wmed = float(np.median(widths))
    gate("T7 edge crispness: cover 0.1->0.9 within <= 3 px at 2400 (hardness 0.75)", wmed <= 3.0, f"median {wmed:.1f} px")
    # ---- marbling: hue alternations along a cross-section of the two-colour stroke
    yc2 = yc; xm = int(0.75 * W)
    lab = mix.rgb_to_lab(unlit[yc2 - half:yc2 + half + 1, xm:xm + 6].mean(1))
    hue = np.degrees(np.arctan2(lab[:, 2], lab[:, 1])) % 360
    ins = cv.cover[yc2 - half:yc2 + half + 1, xm] > 0.5
    hin = hue[ins]
    red = hin < 60   # vermilion ~ 40 deg, pale yellow ~ 90 deg
    switches = int(np.sum(red[1:] != red[:-1]))
    gate("T7 two-colour load: >= 3 colour switches across the stroke (marble 1.0)", switches >= 3, f"{switches} switches, {int(red.sum())}/{len(red)} px red")
    # ---- levees: edge zones higher than the centre on the plain stroke (averaged along 60 px)
    prof2 = h[yc - half:yc + half + 1, xa:xa + 60].mean(1)
    ins2 = np.where(prof2 > 0.15 * prof2.max())[0]
    a, b = ins2[0], ins2[-1]
    n = b - a + 1
    edge_l = prof2[a + int(0.04 * n): a + int(0.22 * n)].max(); edge_r = prof2[b - int(0.22 * n): b - int(0.04 * n)].max()
    centre = prof2[a + int(0.35 * n): b - int(0.35 * n)].mean()
    gate("T7 levees: both edge zones > 1.08 x centre height", edge_l > 1.08 * centre and edge_r > 1.08 * centre,
         f"left {edge_l:.2f} right {edge_r:.2f} centre {centre:.2f}")
    # ---- gel check: at bump 3 the mean specular over the stroke stays modest (satin, not wet plastic)
    _, comp = light.relight(unlit, h, bump=3.0, contrast=0.6, spec=0.18, components=True)
    painted = cv.cover > 0.5
    spec_mean = float(comp["spec"][painted].mean()); spec_p99 = float(np.percentile(comp["spec"][painted], 99))
    gate("T7 satin not gel: at bump 3 / spec 0.18 mean specular on paint < 0.02 and p99 < 0.12",
         spec_mean < 0.02 and spec_p99 < 0.12, f"mean {spec_mean:.4f} p99 {spec_p99:.3f}")
    # crops for the sheet (native pixels)
    y0, y1 = int(0.02 * W), int(0.10 * W)
    items += [("T7 loaded stroke 2400 unlit", light.to8(unlit[y0:y1, int(0.04 * W):int(0.30 * W)])),
              ("T7 same, lit", light.to8(lit[y0:y1, int(0.04 * W):int(0.30 * W)])),
              ("T7 same, lit bump 3", light.to8(lit3[y0:y1, int(0.04 * W):int(0.30 * W)])),
              ("T7 two-colour marble", light.to8(lit[y0:y1, int(0.54 * W):int(0.80 * W)])),
              ("T7 dry tail", light.to8(lit[int(0.12 * W):int(0.20 * W), int(0.62 * W):int(0.96 * W)])),
              ("T7 soft (hardness .1) vs ridge 1/levee .6", light.to8(lit[int(0.22 * W):int(0.30 * W), int(0.30 * W):int(0.70 * W)])),
              ("T7 height (plain + marble)", light.to8(np.repeat((h / max(h.max(), 1e-6))[y0:y1, int(0.04 * W):int(0.80 * W), None], 3, -1)))]
    save_sheet(os.path.join(OUT, "T7_sheet.png"), items, cols=3, cell=(420, 260))
    return items[:3]


if __name__ == "__main__":
    def gate(n, ok, d):
        print(f"  [{'PASS' if ok else 'FAIL'}] {n}: {d}")
    t7(os.path.join(ROOT, "out", "checkpoints"), gate)
