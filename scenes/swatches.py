"""Standard swatch sheet: the fixed test pattern the evaluation harness (oilpaint/eval.py) paints and measures.

Every swatch is a small set of EXPLICIT strokes (not planner output), so the sheet isolates the brush kernel and
the lighting pass: a planner change cannot move it, a kernel or light change moves it in a controlled way.  All
geometry is in canvas-width units (the sheet is 1.0 wide, 1.5 high), so the same stroke list replays at any width.

    python scenes/swatches.py                 # write out/swatches/{strokes.npz,manifest.json} (no rendering)
    python -m oilpaint eval run               # paint it at --width, light it, measure every swatch
    python -m oilpaint render scenes/swatches.py --strokes out/swatches/strokes.npz --size 1600x2400 --out out/sheet

`build(S)` declares the canvas and the layer schedule (names, `dry_after`, `hblur_sigma`) for that last command;
the strokes themselves come from `--strokes`.  The manifest lists, per swatch: name, box (x0, y0, x1, y1 in
canvas-width units), kind, what it tests, its layer indices and stroke count.

Swatches (col x row on a 3 x 7 grid; "wide" = two columns):
  mode_paint / mode_scumble / mode_smudge / mode_glaze   one stroke per mode at 3 widths (0.012, 0.024, 0.048 cw)
  dabs            short dabs at 3 widths, aspect 1.2 .. 3.5
  dry_tail        long strokes that run out of paint (dry-brush tail)
  long_straight   wide: three long straight strokes (edge quality, hairlines beside edges)
  long_curved     wide: three long arcs (edge quality on curves, polyline kinks)
  blend_wet_in_wet     blue dragged over yellow while wet, at pick-up 0.12 / 0.30 / 0.60 (should go green)
  blend_wet_on_dry     the same pairs after the yellow has dried (should stay crisp, no mixing)
  blend_complementary  orange/blue and violet/yellow pairs wet-in-wet (must not turn to grey mud too fast)
  mud_stack       alternating complementary strokes stacked 2 / 4 / 6 / 8 deep (chroma loss with overpainting)
  gradient        overlapping strokes stepping through a sky-like colour ramp (banding)
  strata_sky      wide: rows of long overlapping sky strokes (the "geological strata" hairline test)
  cylinder        vertical strokes, tone ramp across (lighting on a form)
  sphere          contour-following arcs shaded as a sphere (lighting on a form)
  pile_up         many strokes over the same place (height must stay bounded)
"""
import json
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from oilpaint import mix                                        # noqa: E402
from oilpaint import strokes as S_                              # noqa: E402
from oilpaint.canvas import BRUSH_DEFAULTS                      # noqa: E402

ASPECT = (2, 3)                      # sheet is 1.0 wide x 1.5 high (canvas-width units)
GROUND = "#e9e1d6"                   # the toned ground of Storm Light
SEED = 1907
WIDTHS = (0.012, 0.024, 0.048)       # the three brush widths used by the single-stroke swatches

# 3 x 7 grid of cells
_X0, _CW, _GX = 0.02, 0.31, 0.015
_Y0, _CH, _GY = 0.04, 0.19, 0.012


def cell(col, row, span=1):
    x0 = _X0 + col * (_CW + _GX)
    y0 = _Y0 + row * (_CH + _GY)
    return [round(x0, 4), round(y0, 4), round(x0 + span * _CW + (span - 1) * _GX, 4), round(y0 + _CH, 4)]


LAYOUT = {
    "mode_paint": cell(0, 0), "mode_scumble": cell(1, 0), "mode_smudge": cell(2, 0),
    "mode_glaze": cell(0, 1), "dabs": cell(1, 1), "dry_tail": cell(2, 1),
    "long_straight": cell(0, 2, 2), "blend_wet_in_wet": cell(2, 2),
    "long_curved": cell(0, 3, 2), "blend_wet_on_dry": cell(2, 3),
    "blend_complementary": cell(0, 4), "mud_stack": cell(1, 4), "gradient": cell(2, 4),
    "strata_sky": cell(0, 5, 2), "cylinder": cell(2, 5),
    "sphere": cell(0, 6), "pile_up": cell(1, 6),
}

TESTS = {
    "mode_paint": "paint strokes at 3 widths: width fidelity, bristle lanes, edge crispness on a toned ground",
    "mode_scumble": "dry-brush scumble over a ridged paint patch: catches ridges only, leaves bare ground alone",
    "mode_smudge": "smudge drags wet blue into yellow and decays; flattens relief",
    "mode_glaze": "glaze: thin transparent tint, colour shifts but no relief is added",
    "dabs": "short dabs, aspect 1.2-3.5: silhouette shape (faceting), bristle texture on small marks",
    "dry_tail": "strokes running out of paint: coverage falls along the stroke, tail breaks into bristle gaps",
    "long_straight": "long straight strokes at 3 widths: edge raggedness, width constancy, hairlines at the edges",
    "long_curved": "long arcs at 3 curvatures: edge kinks at polyline joins, width constancy",
    "blend_wet_in_wet": "blue over wet yellow at pick-up 0.12/0.30/0.60: overlap colour must lie on the pigment mixing curve (green), not grey",
    "blend_wet_on_dry": "blue over DRY yellow (dry_after): no mixing, crisp overlap; contrast with wet-in-wet",
    "blend_complementary": "orange/blue and violet/yellow wet-in-wet: chroma of the overlap vs the pigment mixing curve",
    "mud_stack": "alternating complementary strokes 2/4/6/8 deep: how fast overpainting turns to mud",
    "gradient": "overlapping strokes through a colour ramp: banding, hairlines inside a smooth passage",
    "strata_sky": "rows of long overlapping strokes with the Storm Light sky style: height steps at stroke edges give dark hairlines under raking light",
    "cylinder": "vertical strokes shaded as a cylinder: form must stay readable under the relief lighting",
    "sphere": "contour-following arcs shaded as a sphere: form readability, relief must not break the shading",
    "pile_up": "16 strokes over the same place: paint height must stay bounded (flatten)",
}

KINDS = {
    "mode_paint": "single_strokes", "mode_scumble": "single_strokes", "mode_smudge": "single_strokes",
    "mode_glaze": "single_strokes", "dabs": "dabs", "dry_tail": "dry_tail", "long_straight": "long_strokes",
    "long_curved": "long_strokes", "blend_wet_in_wet": "blend", "blend_wet_on_dry": "blend",
    "blend_complementary": "blend", "mud_stack": "mud_stack", "gradient": "gradient", "strata_sky": "strata",
    "cylinder": "form", "sphere": "form", "pile_up": "pile",
}

SKY = ["#b1a8cf", "#9d97c4", "#8a86b8", "#7676ab", "#66679e", "#5a5a95", "#484886"]     # Storm Light sky palette


# ------------------------------------------------------------------ stroke helpers
def _rng(seed, k):
    return np.random.default_rng([int(seed), int(k)])


def brush(width, **kw):
    """Brush parameters as the planner's default Style would give them for a stroke of this width."""
    p = dict(nb=int(max(3, min(40, 5 + 450 * width))))
    p.update(kw)
    return p


def stroke(rng, x0, y0, x1, y1, width, color, color2=None, streak=0.25, **kw):
    pts = S_.straight(x0, y0, x1, y1, width)
    return S_.make_stroke(pts, color, int(rng.integers(1, 2 ** 31 - 1)), streak_amount=1.0, color2=color2,
                          **brush(width, streak=streak, **kw))


def arc_stroke(rng, cx, cy, r, a0, a1, width, color, **kw):
    pts = S_.arc(cx, cy, r, a0, a1, width)
    return S_.make_stroke(pts, color, int(rng.integers(1, 2 ** 31 - 1)), **brush(width, **kw))


class Sheet:
    """Collects layers (an ordered list of stroke groups with the per-layer schedule) and the manifest."""

    def __init__(self, seed=SEED):
        self.seed = int(seed)
        self.layers = []      # dict(name, swatch, strokes, dry_after, hblur_sigma)
        self.manifest = []
        self._k = 0

    def rng(self):
        self._k += 1
        return _rng(self.seed, self._k)

    def layer(self, swatch, step, strokes, dry_after=None, hblur_sigma=0.01):
        self.layers.append(dict(name=f"{swatch}:{step}", swatch=swatch, strokes=list(strokes), dry_after=dry_after,
                                hblur_sigma=hblur_sigma))

    def finish(self):
        for name, box in LAYOUT.items():
            idx = [i for i, l in enumerate(self.layers) if l["swatch"] == name]
            n = sum(len(self.layers[i]["strokes"]) for i in idx)
            self.manifest.append(dict(name=name, box=box, kind=KINDS[name], tests=TESTS[name], layers=idx, n_strokes=n))


def _lab(L, a, b):
    return mix.lab_to_rgb(np.array([L, a, b], np.float32))


# ------------------------------------------------------------------ swatch builders
def sw_modes(sh):
    # ---- paint: 3 widths on the bare toned ground
    x0, y0, x1, y1 = LAYOUT["mode_paint"]
    r = sh.rng()
    col = ("cobalt_blue", "lead_white", 0.45)
    ys = [y0 + 0.035, y0 + 0.085, y0 + 0.145]
    sh.layer("mode_paint", "3 widths", [stroke(r, x0 + 0.025, y, x1 - 0.025, y + 0.004, w, col, pickup=0.0)
                                         for y, w in zip(ys, WIDTHS)])

    # ---- scumble: a ridged patch of thick paint, then pale dry-brush strokes across patch and bare ground
    x0, y0, x1, y1 = LAYOUT["mode_scumble"]
    r = sh.rng()
    patch = [stroke(r, x0 + 0.02, y0 + 0.02 + 0.017 * i, x0 + 0.20, y0 + 0.02 + 0.017 * i, 0.03, ("ultramarine", "lead_white", 0.45),
                    hgain=1.6, ridge=0.8, pickup=0.0) for i in range(9)]
    sh.layer("mode_scumble", "patch", patch)
    r = sh.rng()
    sc = [stroke(r, x0 + 0.02, y0 + yy, x1 - 0.02, y0 + yy + 0.006, w, ("cadmium_yellow", "lead_white", 0.6), mode="scumble",
                 opacity=0.6, dry_thresh=0.02, dry_width=0.08, load=0.5, vdry=0.6, deplete=0.0, pickup=0.0, streak=0.3)
          for yy, w in zip((0.035, 0.085, 0.145), WIDTHS)]
    sh.layer("mode_scumble", "scumble", sc)

    # ---- smudge: left half wet blue, right half wet yellow, drag pale smudges across the seam
    x0, y0, x1, y1 = LAYOUT["mode_smudge"]
    r = sh.rng()
    xm = 0.5 * (x0 + x1)
    fills = []
    for i in range(9):
        y = y0 + 0.02 + 0.0175 * i
        fills.append(stroke(r, x0 + 0.01, y, xm + 0.004, y, 0.03, "cobalt_blue", hgain=1.2, pickup=0.0))
        fills.append(stroke(r, xm - 0.004, y, x1 - 0.01, y, 0.03, "cadmium_yellow", hgain=1.2, pickup=0.0))
    sh.layer("mode_smudge", "fields", fills)
    r = sh.rng()
    sm = [stroke(r, x0 + 0.03, y0 + yy, x1 - 0.03, y0 + yy, w, "lead_white", mode="smudge", pickup=0.5, opacity=0.9, flatten=0.25,
                 body=0.5, streak=0.3) for yy, w in zip((0.045, 0.095, 0.15), (0.016, 0.024, 0.036))]
    sh.layer("mode_smudge", "smudge", sm)

    # ---- glaze: light patch, dark thin glazes over it and over bare ground
    x0, y0, x1, y1 = LAYOUT["mode_glaze"]
    r = sh.rng()
    patch = [stroke(r, x0 + 0.02, y0 + 0.02 + 0.017 * i, x0 + 0.20, y0 + 0.02 + 0.017 * i, 0.03, ("cadmium_yellow", "lead_white", 0.35),
                    hgain=1.0, pickup=0.0) for i in range(9)]
    sh.layer("mode_glaze", "patch", patch, dry_after=0.0)
    r = sh.rng()
    gl = [stroke(r, x0 + 0.02, y0 + yy, x1 - 0.02, y0 + yy + 0.006, w, "ultramarine", mode="glaze", opacity=0.35, pickup=0.0, streak=0.3)
          for yy, w in zip((0.035, 0.085, 0.145), WIDTHS)]
    sh.layer("mode_glaze", "glaze", gl)


def sw_dabs(sh):
    x0, y0, x1, y1 = LAYOUT["dabs"]
    r = sh.rng()
    cols = [("cadmium_orange", "lead_white", 0.3), ("cerulean", "lead_white", 0.2), ("viridian", "lead_white", 0.3), "vermilion"]
    aspects = (1.2, 1.8, 2.5, 3.5)
    dabs = []
    for ri, w in enumerate((0.010, 0.016, 0.024)):
        for ci, asp in enumerate(aspects):
            L = w * asp
            cx = x0 + 0.05 + ci * 0.072
            cy = y0 + 0.04 + ri * 0.055
            a = np.radians(float(r.uniform(-25, 25)))
            dabs.append(stroke(r, cx - 0.5 * L * np.cos(a), cy - 0.5 * L * np.sin(a), cx + 0.5 * L * np.cos(a), cy + 0.5 * L * np.sin(a),
                               w, cols[ci], pickup=0.0, marble=0.3))
    sh.layer("dabs", "dabs", dabs)


def sw_dry_tail(sh):
    x0, y0, x1, y1 = LAYOUT["dry_tail"]
    r = sh.rng()
    s = [stroke(r, x0 + 0.02, y0 + 0.05, x1 - 0.02, y0 + 0.055, 0.03, "viridian", deplete=0.06, vdry=0.5, pickup=0.0, streak=0.5),
         stroke(r, x0 + 0.02, y0 + 0.115, x1 - 0.02, y0 + 0.12, 0.03, ("cobalt_violet_light", "lead_white", 0.2), deplete=0.10, vdry=0.6,
                pickup=0.0, streak=0.5, hgain=1.3)]
    sh.layer("dry_tail", "dry", s)


def sw_long(sh):
    x0, y0, x1, y1 = LAYOUT["long_straight"]
    r = sh.rng()
    cols = [("cerulean", "lead_white", 0.15), ("madder", "lead_white", 0.35), ("viridian", "lead_white", 0.3)]
    ys = (0.035, 0.085, 0.145)
    st = [stroke(r, x0 + 0.03, y0 + y, x1 - 0.03, y0 + y + 0.012, w, c, pickup=0.0) for y, w, c in zip(ys, WIDTHS, cols)]
    sh.layer("long_straight", "straight", st)
    x0, y0, x1, y1 = LAYOUT["long_curved"]
    r = sh.rng()
    st = []
    # three gently undulating strokes ~0.56 cw long, bending like sky/sea strokes: (amplitude, wavelength, width, centre y)
    # tightest bend: radius of curvature A^-1 (2 pi / lam)^-2 = 0.31 cw
    for (amp, lam, w, yc, c) in ((0.010, 0.60, WIDTHS[0], 0.035, cols[0]), (0.016, 0.50, WIDTHS[1], 0.09, cols[1]), (0.020, 0.50, WIDTHS[2], 0.15, cols[2])):
        n = 80
        t = np.linspace(0.0, 1.0, n)
        xs = x0 + 0.03 + 0.56 * t
        ys = y0 + yc + amp * np.sin(2 * np.pi * (xs - x0) / lam + float(r.uniform(0, 1)))
        st.append(S_.make_stroke(S_.polyline(xs, ys, w), c, int(r.integers(1, 2 ** 31 - 1)), **brush(w, pickup=0.0)))
    sh.layer("long_curved", "waves", st)


def sw_blend(sh):
    """Pairs: first colour laid, second dragged over its lower half.  Two strokes 0.022 wide, 0.26 long."""
    def pair_set(name, first, second, pickups, y_start=0.03, dry_first=None):
        x0, y0, x1, y1 = LAYOUT[name]
        r = sh.rng()
        w = 0.022
        under, over = [], []
        for k, pk in enumerate(pickups):
            yc = y0 + y_start + 0.058 * k
            under.append(stroke(r, x0 + 0.03, yc, x1 - 0.03, yc + 0.003, w, first, pickup=0.0, hgain=1.0))
            over.append(stroke(r, x0 + 0.03, yc + 0.6 * w, x1 - 0.03, yc + 0.6 * w + 0.003, w, second, pickup=pk, hgain=1.0))
        return under, over

    # wet-in-wet: yellow first, blue over it, still wet
    u, o = pair_set("blend_wet_in_wet", "cadmium_yellow", "cobalt_blue", (0.12, 0.30, 0.60))
    sh.layer("blend_wet_in_wet", "yellow", u)
    sh.layer("blend_wet_in_wet", "blue", o)
    # wet-on-dry: the same, after the yellow has dried (dry_after = 0)
    u, o = pair_set("blend_wet_on_dry", "cadmium_yellow", "cobalt_blue", (0.12, 0.30, 0.60))
    sh.layer("blend_wet_on_dry", "yellow", u, dry_after=0.0)
    sh.layer("blend_wet_on_dry", "blue", o)
    # complementary pairs, wet-in-wet: orange under blue, yellow under violet, default and raised pick-up
    x0, y0, x1, y1 = LAYOUT["blend_complementary"]
    r = sh.rng()
    w = 0.022
    under, over = [], []
    specs = [(("cadmium_orange", "lead_white", 0.15), ("ultramarine", "lead_white", 0.25), 0.12),
             (("cadmium_orange", "lead_white", 0.15), ("ultramarine", "lead_white", 0.25), 0.30),
             (("hansa_yellow", "lead_white", 0.1), ("cobalt_violet_light", "lead_white", 0.0), 0.12),
             (("hansa_yellow", "lead_white", 0.1), ("cobalt_violet_light", "lead_white", 0.0), 0.30)]
    for k, (a, b, pk) in enumerate(specs):
        yc = y0 + 0.028 + 0.041 * k
        under.append(stroke(r, x0 + 0.03, yc, x1 - 0.03, yc + 0.002, 0.016, a, pickup=0.0))
        over.append(stroke(r, x0 + 0.03, yc + 0.6 * 0.016, x1 - 0.03, yc + 0.6 * 0.016 + 0.002, 0.016, b, pickup=pk))
    sh.layer("blend_complementary", "under", under)
    sh.layer("blend_complementary", "over", over)


def sw_mud(sh):
    x0, y0, x1, y1 = LAYOUT["mud_stack"]
    r = sh.rng()
    a, b = ("cadmium_orange", "lead_white", 0.15), ("ultramarine", "lead_white", 0.25)
    st = []
    for ci, k in enumerate((2, 4, 6, 8)):
        cx = x0 + 0.04 + ci * 0.07
        for j in range(k):
            dx = float(r.uniform(-0.004, 0.004))
            st.append(stroke(r, cx + dx, y0 + 0.03, cx + dx + 0.002, y0 + 0.16, 0.026, a if j % 2 == 0 else b, pickup=0.12, hgain=0.7))
    # each column is painted in a separate pass order: strokes are appended column by column
    sh.layer("mud_stack", "stacks", st)


def sw_gradient(sh):
    x0, y0, x1, y1 = LAYOUT["gradient"]
    r = sh.rng()
    n = 9
    top, bot = np.array([32.0, 18.0, -38.0]), np.array([86.0, 12.0, 24.0])      # deep blue-violet -> pale peach
    st = []
    for i in range(n):
        t = i / (n - 1)
        lab = top + (bot - top) * t + r.normal(0, 0.6, 3)
        yy = y0 + 0.02 + 0.017 * i
        st.append(stroke(r, x0 + 0.01, yy, x1 - 0.01, yy + 0.002, 0.03, _lab(*lab), pickup=0.12, hgain=0.7, ridge=0.4, levee=0.0, furrow=0.05,
                         blob=0.15, stiff=0.3, hardness=0.6))
    sh.layer("gradient", "ramp", st)


def sw_strata(sh):
    x0, y0, x1, y1 = LAYOUT["strata_sky"]
    r = sh.rng()
    st = []
    rows = 6
    for row in range(rows):
        y = y0 + 0.02 + row * 0.027
        x = max(x0 + 0.006, x0 + 0.01 - float(r.uniform(0.0, 0.05)))      # stay inside the box (the neighbouring swatch is at x1 + 0.015)
        while x < x1 - 0.08:
            ln = min(float(r.uniform(0.20, 0.26)), x1 - 0.006 - x)
            col = SKY[int(r.integers(len(SKY)))]
            lab = mix.rgb_to_lab(mix.color_spec_to_rgb(col)) + r.normal(0, [4.0, 2.0, 2.0])
            col = mix.lab_to_rgb(np.array([max(lab[0], 25.0), lab[1], lab[2]], np.float32))
            dy = float(r.uniform(-0.006, 0.006))
            # the sky style of scenes/storm_v3.py: width .025-.04, levee 0, ridge .4, hardness .6, marble .4, hgain .7
            st.append(stroke(r, x, y, x + ln, y + dy - 0.01, float(r.uniform(0.028, 0.038)), col, color2="#c9b8d8", marble=0.4,
                             pickup=0.10, hgain=0.7 * float(r.uniform(0.75, 1.25)), ridge=0.4, levee=0.0, furrow=0.05, blob=0.15,
                             stiff=0.3, hardness=0.6, opacity=float(r.uniform(0.85, 1.0))))
            x += ln * float(r.uniform(0.7, 0.9))
    sh.layer("strata_sky", "sky strokes", st)


def sw_forms(sh):
    # ---- cylinder: vertical strokes, light from the left
    x0, y0, x1, y1 = LAYOUT["cylinder"]
    r = sh.rng()
    cx, half, top, bot = 0.5 * (x0 + x1), 0.075, y0 + 0.012, y1 - 0.012
    st = []
    nstroke = 14
    for i in range(nstroke):
        u = -1 + 2 * (i + 0.5) / nstroke                     # -1 (left) .. 1 (right)
        col = _cyl_color(u)
        w = 2 * half / nstroke * 1.7
        st.append(stroke(r, cx + u * half, top, cx + u * half + float(r.uniform(-0.002, 0.002)), bot, w, col, pickup=0.08, hgain=0.9,
                         streak=0.2))
    sh.layer("cylinder", "form", st)
    # ---- sphere: arcs around rings concentric with the sphere, coloured by the shading at their midpoint
    x0, y0, x1, y1 = LAYOUT["sphere"]
    r = sh.rng()
    cx, cy, R = 0.5 * (x0 + x1), 0.5 * (y0 + y1), 0.083
    st = []
    ring_r = np.arange(0.012, R, 0.0115)
    for rho in ring_r:
        circ = 2 * np.pi * rho
        nseg = max(1, int(np.ceil(circ / 0.05)))
        a0 = float(r.uniform(0, 2 * np.pi))
        for s in range(nseg):
            aa = a0 + 2 * np.pi * s / nseg
            span = 2 * np.pi / nseg * 1.15
            am = aa + 0.5 * span
            mx, my = cx + rho * np.cos(am), cy + rho * np.sin(am)
            col = _sph_color((mx - cx) / R, (my - cy) / R)
            st.append(arc_stroke(r, cx, cy, rho, aa, aa + span, 0.016, col, pickup=0.08, hgain=0.9, streak=0.2))
    sh.layer("sphere", "form", st)


def _shade_color(t, warm):
    """t in 0 (shadow) .. 1 (light): Lab ramp from cool violet-blue shadow to warm cream light."""
    lo = np.array([34.0, 16.0, -26.0]); hi = np.array([88.0, 8.0, 26.0])
    lab = lo + (hi - lo) * np.clip(t, 0, 1)
    return _lab(*lab)


def _cyl_color(u):
    nz = np.sqrt(max(0.0, 1 - u * u))
    L = np.array([-0.75, 0.0, 0.66]); L /= np.linalg.norm(L)
    ndl = max(0.0, u * L[0] + nz * L[2])
    reflected = 0.12 * max(0.0, u)                                     # reflected light lifts the dark edge
    return _shade_color(0.15 + 0.85 * ndl + reflected, True)


def _sph_color(nx, ny):
    d2 = nx * nx + ny * ny
    nz = np.sqrt(max(0.0, 1 - d2))
    L = np.array([-0.5, -0.6, 0.62]); L /= np.linalg.norm(L)
    ndl = max(0.0, nx * L[0] + ny * L[1] + nz * L[2])
    return _shade_color(0.12 + 0.88 * ndl + 0.1 * max(0.0, nx * 0.5 + ny * 0.6) * (1 - nz), True)


def sw_pile(sh):
    x0, y0, x1, y1 = LAYOUT["pile_up"]
    r = sh.rng()
    cols = [("cobalt_blue", "lead_white", 0.4), ("cadmium_yellow", "lead_white", 0.3), ("madder", "lead_white", 0.4), ("viridian", "lead_white", 0.3)]
    st = []
    for j in range(16):
        dy = float(r.uniform(-0.015, 0.015)); dx = float(r.uniform(-0.01, 0.01))
        st.append(stroke(r, x0 + 0.05 + dx, y0 + 0.09 + dy, x1 - 0.05 + dx, y0 + 0.095 + dy, 0.045, cols[j % 4], pickup=0.10, hgain=1.0))
    sh.layer("pile_up", "pile", st)


def build_sheet(seed=SEED):
    """Build every swatch.  Returns a Sheet (layers + manifest).  Deterministic for a given seed and mixer backend."""
    sh = Sheet(seed)
    for fn in (sw_modes, sw_dabs, sw_dry_tail, sw_long, sw_blend, sw_mud, sw_gradient, sw_strata, sw_forms, sw_pile):
        fn(sh)
    sh.finish()
    return sh


def write_sheet(out_dir, seed=SEED):
    """Write strokes.npz (standard format, one layer per Sheet layer) and manifest.json into out_dir."""
    from oilpaint.render import save_strokes
    sh = build_sheet(seed)
    os.makedirs(out_dir, exist_ok=True)
    allst, ranges = [], []
    for li, l in enumerate(sh.layers):
        a = len(allst)
        for s in l["strokes"]:
            s.layer = li
        allst.extend(l["strokes"])
        ranges.append((a, len(allst)))
    save_strokes(os.path.join(out_dir, "strokes.npz"), allst, ranges)
    with open(os.path.join(out_dir, "manifest.json"), "w") as f:
        json.dump(manifest_dict(sh), f, indent=1)
    return sh


def manifest_dict(sh):
    return dict(sheet=dict(aspect=list(ASPECT), width_units=1.0, height_units=ASPECT[1] / ASPECT[0], ground=GROUND, seed=sh.seed),
                layers=[dict(name=l["name"], swatch=l["swatch"], n=len(l["strokes"]), dry_after=l["dry_after"], hblur_sigma=l["hblur_sigma"])
                        for l in sh.layers],
                swatches=sh.manifest)


# ------------------------------------------------------------------ standard-scene entry point (for `render --strokes`)
def build(S):
    """Canvas and layer schedule only: the sheet's strokes are explicit (see the module docstring)."""
    from oilpaint.planner import Layer
    from oilpaint.scene import gradient_v
    S.canvas(aspect=ASPECT, ground=GROUND)
    S.target.fill(gradient_v([(0.0, GROUND), (1.5, GROUND)]))
    S.region("all", lambda X, Y: np.ones_like(X))
    S.style("all", colors=[GROUND])
    sh = build_sheet()
    S.layers([Layer(l["name"], regions=["all"], placement="density", coverage=0.0, enabled=False, dry_after=l["dry_after"],
                    hblur_sigma=l["hblur_sigma"]) for l in sh.layers])


if __name__ == "__main__":
    out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "out", "swatches")
    sh = write_sheet(out)
    print(f"wrote {out}/strokes.npz and manifest.json: {len(sh.manifest)} swatches, {sum(m['n_strokes'] for m in sh.manifest)} strokes, "
          f"{len(sh.layers)} layers")
