"""Storm Light - a banded lighthouse on a rock in a breaking storm, one lamp lit (Fl(2) W 12s).

Fifteen layers.  Coordinates are canvas-width units; the canvas is 1.0 wide x 1.25 high (portrait 4:5).
Sizes (width, length) are fractions of the canvas width.  See docs/STYLE_REFERENCE.md for every parameter.
"""
from oilpaint.scene import *   # noqa: F401,F403  (shape/flow helpers, gradient_v, Layer)

# ---------------------------------------------------------------- composition
HZ = 0.575                       # horizon (46 % of the height)
LANTERN = (0.67, 0.39)
BEAM_ANGLE = 176                 # degrees, 180 = straight left, <180 = slightly upward (image y is down)
BEAM_LEN = 0.48
TOWER = [(0.615, 0.73), (0.725, 0.73), (0.708, 0.42), (0.632, 0.42)]           # base wider than the top
BAND1 = [(0.623, 0.575), (0.717, 0.575), (0.714, 0.525), (0.626, 0.525)]       # red bands (polygons)
BAND2 = [(0.617, 0.685), (0.723, 0.685), (0.720, 0.635), (0.620, 0.635)]
ROCK = [(0.47, 0.90), (0.50, 0.83), (0.545, 0.79), (0.58, 0.775), (0.61, 0.745), (0.645, 0.725), (0.70, 0.72),
        (0.745, 0.73), (0.78, 0.755), (0.83, 0.77), (0.87, 0.80), (0.92, 0.815), (0.97, 0.845), (1.02, 0.87),
        (1.02, 0.95), (0.86, 0.93), (0.70, 0.94), (0.58, 0.925)]
STORM = [(0.20, 0.09, 0.36, 0.13, "#232a58"), (0.74, 0.05, 0.34, 0.11, "#232a58"), (0.45, 0.22, 0.30, 0.10, "#2c3266"),
         (0.06, 0.30, 0.26, 0.09, "#3a3b78"), (0.88, 0.25, 0.24, 0.09, "#3a3b78"), (0.30, 0.37, 0.28, 0.07, "#5a5a95"),
         (0.78, 0.35, 0.26, 0.07, "#66679e")]
BREAKS = [(0.55, 0.12, 0.10, 0.04, "#9d97c4"), (0.12, 0.20, 0.09, 0.035, "#b1a8cf"), (0.62, 0.31, 0.10, 0.03, "#c9c1e0"),
          (0.36, 0.30, 0.08, 0.03, "#e8a08f")]

# palettes (light to dark), used as Mixbox snapping targets
SKY_DARK = ["#5a5a95", "#484886", "#3a3b78", "#2c3266"]
SKY_MID = ["#b1a8cf", "#9d97c4", "#8a86b8", "#7676ab", "#66679e"]
SKY_GLOW = ["#fbe5b8", "#f6d09a", "#f0b98f", "#e8a08f", "#d98f9a"]
SKY_HAZE = ["#efe1d6", "#e6d3d0", "#dccbd3", "#cdc3d6"]
SEA_FAR = ["#f3dcc4", "#e3c3c4", "#c9b6d3", "#b4b7d8", "#a3b6d8"]
SEA_MID = ["#a3b6d8", "#86a6c9", "#6fa0bb", "#8288b8", "#6086b4"]
SEA_NEAR = ["#4a76ae", "#3a67a0", "#2f5490", "#3f88a0", "#2f7590", "#2a4a80", "#1f3a6c", "#8fb4d4"]
ROCK_LIT = ["#f0d3a8", "#dcb98f", "#c7a37f", "#a98c86"]
ROCK_MID = ["#8d7d9a", "#7a6f98", "#68628f"]
ROCK_DEEP = ["#4a4a7c", "#3c4270", "#2f3862", "#403a6a"]
TOWER_W = ["#fbf0dc", "#f7e2bd", "#e9d8d6", "#c9c0dd", "#a79fca"]
TOWER_R = ["#ee8a6f", "#d8695a", "#c8503f", "#a94358", "#8a3d62"]
FOAM = ["#f3e9e0", "#e6e3f2", "#d3d9ee", "#bcc9e6", "#f0d3b8"]


def build(S):
    S.canvas(aspect=(4, 5), ground="#e9e1d6")      # pale warm grey with a little violet

    # ------------------------------------------------------------ target: soft fields
    T = S.target
    T.fill(gradient_v([(0.0, "#2c3266"), (0.22, "#4f4f8c"), (0.42, "#8a86b8"), (0.50, "#c9b8c8"), (HZ, "#eccfb8")]))
    T.blob(0.25, HZ - 0.02, 0.48, 0.11, "#f4bd90", softness=0.7, strength=0.8, seed=31)      # horizon glow
    T.blob(0.12, HZ - 0.06, 0.25, 0.07, "#e8a0a0", softness=0.7, strength=0.4, seed=32)
    for i, (cx, cy, rx, ry, c) in enumerate(STORM):
        T.blob(cx, cy, rx, ry, c, softness=0.55, noise=0.8, strength=0.9, seed=40 + i)
    for i, (cx, cy, rx, ry, c) in enumerate(BREAKS):                                            # lighter breaks
        T.blob(cx, cy, rx, ry, c, softness=0.8, noise=0.6, strength=0.6, seed=60 + i)
    T.fill(gradient_v([(HZ, "#dcc8cc"), (HZ + 0.05, "#b4b7d8"), (HZ + 0.14, "#7aa0c4"), (HZ + 0.3, "#3a67a0"), (1.25, "#243f74")]), mask=below(HZ))
    for i, (cx, cy, rx, ry, c) in enumerate([(0.15, 0.95, 0.20, 0.03, "#1f3a6c"), (0.55, 1.05, 0.25, 0.035, "#1c3566"),
                                             (0.85, 0.98, 0.18, 0.03, "#243f74"), (0.30, 1.12, 0.22, 0.03, "#4f9ab0"),
                                             (0.10, 1.05, 0.15, 0.025, "#9ab8d8"), (0.70, 1.15, 0.2, 0.03, "#7fb0c8"),
                                             (0.45, 0.88, 0.2, 0.025, "#8fb4d4"), (0.80, 1.08, 0.15, 0.03, "#1f3a6c")]):
        T.blob(cx, cy, rx, ry, c, softness=0.8, noise=0.6, strength=0.8, seed=70 + i)             # troughs and crests
    T.blob(0.22, HZ + 0.03, 0.30, 0.04, "#f6cf98", softness=0.8, strength=0.6, seed=33)       # glow reflected
    T.blob(0.24, HZ + 0.16, 0.14, 0.09, "#d9b98a", softness=0.9, strength=0.3, seed=34)
    T.polygon(ROCK, "#4a4a7c", softness=0.012, noise=0.5, seed=35)
    T.blob(0.56, 0.80, 0.08, 0.04, "#c7a37f", softness=0.8, strength=0.55, seed=36)            # lit facets, left
    T.blob(0.87, 0.84, 0.10, 0.04, "#2f3862", softness=0.8, strength=0.6, seed=37)             # deep shadow, right
    T.polygon(TOWER, "#e9d8d6", softness=0.006)
    T.polygon(BAND1, "#c8503f", softness=0.005); T.polygon(BAND2, "#c8503f", softness=0.005)
    T.blob(0.60, 0.55, 0.05, 0.25, "#f7e2bd", softness=0.9, strength=0.4, seed=38)             # lit left edge
    T.blob(0.735, 0.58, 0.05, 0.25, "#8a86b8", softness=0.9, strength=0.55, seed=39)           # shadow right edge
    T.blob(0.67, 0.42, 0.06, 0.02, "#4a4270", softness=0.5, strength=0.9)                      # gallery / roof
    T.blob(0.36, 0.375, 0.34, 0.055, "#66679e", softness=0.7, noise=0.5, strength=0.75, seed=45)   # darker strip the beam crosses
    T.blob(0.30, 0.42, 0.30, 0.03, "#7676ab", softness=0.8, noise=0.5, strength=0.5, seed=46)
    T.beam(LANTERN, BEAM_ANGLE, 9, BEAM_LEN, "#fbe3b8", strength=0.7, softness=0.5)
    T.glow(LANTERN, 0.035, "#fff0b8", strength=1.2)
    T.blob(0.62, 0.86, 0.22, 0.035, "#d3d9ee", softness=0.9, strength=0.4, seed=41)            # foam base
    T.blob(0.58, 0.79, 0.10, 0.06, "#dcd6ea", softness=0.9, strength=0.35, seed=42)            # spray

    # ------------------------------------------------------------ regions (soft masks); later ones override the id map
    beam_axis = [LANTERN, (LANTERN[0] - BEAM_LEN, LANTERN[1] - 0.034)]
    S.region("rain", lambda X, Y: (Y < 0.98).astype("float32"), edge=0.02)          # whole picture above the near sea
    S.region("haze", ellipse(0.35, HZ + 0.02, 0.75, 0.16), edge=0.08)                # far distance band
    S.region("sky", above(HZ), edge=0.03)
    S.region("storm", union(*[ellipse(*m[:4]) for m in STORM[:5]]), edge=0.06)
    S.region("glow", ellipse(0.25, HZ - 0.02, 0.45, 0.11), edge=0.08)
    S.region("sea_far", intersect(below(HZ), above(HZ + 0.13)), edge=0.02)
    S.region("sea_near", below(HZ + 0.13), edge=0.03)
    S.region("rock", polygon(ROCK), edge=0.012)
    S.region("foam", intersect(band_around(ROCK, 0.03), below(0.80)), edge=0.02)
    S.region("spray", ellipse(0.58, 0.80, 0.09, 0.05), edge=0.04)
    S.region("tower", polygon(TOWER), edge=0.008)
    S.region("band1", polygon(BAND1), edge=0.004)
    S.region("band2", polygon(BAND2), edge=0.004)
    S.region("beam", wedge(LANTERN, BEAM_ANGLE, 9, BEAM_LEN), edge=0.03)
    S.region("halo", disc(LANTERN, 0.07), edge=0.03)
    S.region("lantern", disc(LANTERN, 0.026), edge=0.01)

    # ------------------------------------------------------------ flow fields
    S.flow("rain", constant(115, noise=0.04))
    S.flow("haze", constant(0, noise=0.05))
    S.flow("sky", sweep(angle_deg=-14, curl=0.5, noise=0.3, scale=0.5))
    S.flow("storm", swirl_around(STORM[:5], strength=0.35, noise=0.45))
    S.flow("glow", sweep(angle_deg=-4, curl=0.2, noise=0.2, scale=0.4, seed=9))
    S.flow("sea_far", waves(base_angle=0, amplitude_deg=4, wavelength=0.08, noise=0.08, horizon=HZ))
    S.flow("sea_near", waves(base_angle=0, amplitude_deg=9, wavelength=0.18, noise=0.15, horizon=HZ))
    S.flow("foam", waves(base_angle=0, amplitude_deg=35, wavelength=0.05, noise=0.5, horizon=HZ, seed=12))
    S.flow("spray", upward(noise=0.7))
    S.flow("rock", contour(ROCK, noise=0.35))
    S.flow("tower", constant(90, noise=0.05))
    S.flow("band1", constant(0, noise=0.03)); S.flow("band2", constant(0, noise=0.03))
    S.flow("beam", radial_from(LANTERN, noise=0.04))
    S.flow("halo", swirl_around([(LANTERN[0], LANTERN[1], 0.07, 0.07)], strength=1.0, noise=0.25))
    S.flow("lantern", swirl_around([(LANTERN[0], LANTERN[1], 0.03, 0.03)], strength=1.0, noise=0.3))

    S.light(glow=(0.25, HZ - 0.02, 0.5, 0.7), beam=(LANTERN, BEAM_ANGLE, 9, BEAM_LEN, 0.8), lamp=(LANTERN, 0.07, 1.0))

    # ------------------------------------------------------------ styles
    S.style("sky", colors=SKY_MID + SKY_DARK[:2] + SKY_HAZE[:2], flecks=[("#f0b98f", 0.06)],
            width=(0.025, 0.040), length=(0.10, 0.28), curvature=0.35, align=0.85, pickup=0.15, hgain=0.7,
            warmth=0.5, priority=0, spill=0.2, jitter=(6.0, 4.0))
    S.style("storm", colors=SKY_DARK + SKY_MID[3:] + ["#e8a08f"], flecks=[("#e8a08f", 0.05), ("#9d97c4", 0.05)],
            width=(0.018, 0.030), length=(0.04, 0.10), curvature=0.6, align=0.6, pickup=0.2, hgain=0.6, priority=1,
            jitter=(7.0, 4.0))
    S.style("glow", colors=SKY_GLOW + SKY_HAZE[:2], flecks=[("#9d97c4", 0.06)],
            width=(0.022, 0.040), length=(0.08, 0.22), curvature=0.3, align=0.85, pickup=0.3, hgain=0.8, priority=2,
            warmth=0.6)
    S.style("haze", colors=SKY_HAZE + SEA_FAR[:2], width=(0.04, 0.06), length=(0.15, 0.3), curvature=0.1, align=0.95,
            pickup=0.0, hgain=0.0, priority=2, snap=0.5, opacity=(0.04, 0.08))
    S.style("sea_far", colors=SEA_FAR + SEA_MID[:2], flecks=[("#f6d09a", 0.10)], width=(0.008, 0.014),
            length=(0.025, 0.06), curvature=0.15, align=0.95, pickup=0.2, hgain=0.7, priority=3, warmth=0.6)
    S.style("sea_near", colors=SEA_NEAR + SEA_MID + SEA_FAR[:1], flecks=[("#f6d09a", 0.07), ("#e34234", 0.015)],
            width=(0.014, 0.024), length=(0.06, 0.16), curvature=0.2, align=0.92, pickup=0.2, hgain=1.0, priority=3,
            warmth=0.5, size_by_y=(HZ + 0.13, 1.25, 0.7, 1.6), end_width=0.7, jitter=(6.0, 3.0))
    S.style("rock", colors=ROCK_DEEP + ROCK_MID + ROCK_LIT, flecks=[("#f0d3a8", 0.06)], width=(0.014, 0.028),
            length=(0.03, 0.08), curvature=0.8, align=0.8, pickup=0.3, hgain=1.0, priority=4, warmth=0.5)
    S.style("foam", colors=FOAM + SEA_MID[:2], width=(0.007, 0.013), length=(0.03, 0.08), curvature=0.5, align=0.6,
            pickup=0.3, hgain=1.1, priority=6, warmth=0.5, jitter=(4.0, 3.0))
    S.style("spray", colors=FOAM[:4] + SKY_HAZE[:2], width=(0.006, 0.012), length=(0.02, 0.06), curvature=0.4,
            align=0.55, pickup=0.25, hgain=0.6, priority=6, opacity=(0.5, 0.85), warmth=0.4)
    S.style("tower", colors=TOWER_W + ["#f0d3a8", "#8a86b8"], flecks=[("#40826d", 0.02), ("#f0b98f", 0.05)],
            width=(0.008, 0.016), length=(0.03, 0.07), curvature=0.02, align=0.97, pickup=0.25, hgain=0.8,
            priority=5, spill=0.2, jitter=(3.0, 2.0), end_width=0.8)
    S.style("band1", colors=TOWER_R, width=(0.006, 0.011), length=(0.02, 0.05), curvature=0.02, align=0.97,
            pickup=0.25, hgain=0.8, priority=5, spill=0.15, jitter=(3.0, 2.0), end_width=0.8, flecks=[("#40826d", 0.03)])
    S.style("band2", colors=TOWER_R, width=(0.006, 0.011), length=(0.02, 0.05), curvature=0.02, align=0.97,
            pickup=0.25, hgain=0.8, priority=5, spill=0.15, jitter=(3.0, 2.0), end_width=0.8, flecks=[("#40826d", 0.03)])
    S.style("beam", colors=["#fff3d0", "#fbe5b8", "#f6d09a", "#f0c9a0"], width=(0.022, 0.036), length=(0.14, 0.30),
            curvature=0.05, align=0.98, pickup=0.2, hgain=0.4, priority=2, warmth=0.3, opacity=(0.65, 0.85),
            opacity_by_light=0.45, body=0.7, load=0.8, deplete=0.03, end_width=0.7, snap=0.7, jitter=(3.0, 2.0))
    S.style("halo", colors=SKY_GLOW[:2] + ["#fff0b8"], width=(0.012, 0.022), length=(0.04, 0.09), curvature=0.6,
            align=0.9, pickup=0.2, hgain=0.3, priority=6, opacity=(0.2, 0.35))
    S.style("lantern", colors=["#fff8e0", "#fff0b8", "#ffe08a", "#fbd070"], width=(0.006, 0.010), length=(0.014, 0.028),
            curvature=0.2, align=0.5, pickup=0.05, hgain=2.5, priority=7, jitter=(2.0, 2.0), end_width=0.9,
            end_pressure=0.5, opacity=(0.95, 1.0), L_floor=80.0)
    S.style("rain", colors=SKY_HAZE[:2] + ["#cfcbe6"], width=(0.004, 0.007), length=(0.08, 0.18), curvature=0.0,
            align=0.98, pickup=0.0, hgain=0.2, priority=8, opacity=(0.15, 0.3), mode="scumble", body=0.0, snap=0.3)

    # ------------------------------------------------------------ fifteen layers
    S.layers([
        Layer("Toned ground", regions=["haze"], placement="density", width=(0.08, 0.10), length=(0.3, 0.5),
              spacing=1.2, opacity=(0.4, 0.55), relief=0.1, streak=0.2, snap=0.0, jitter=(2.0, 1.5), flecks=(),
              colors=["#e9e1d6", "#e4dcd8", "#ded7d4", "#e6ded0"], color_from="palette", pickup=0.5, curvature=0.0,
              align=1.0, stop_at_edge=0.0, spill=1.0, coverage=0.6),
        Layer("Ebauche dark masses", regions=["storm", "sky", "sea_near", "sea_far", "rock"], placement="error",
              width=(0.04, 0.06), length=(0.12, 0.3), T=16, fs=0.7, fg=1.0, relief=0.25, load=0.7, opacity=(0.7, 0.9),
              pickup=0.3, streak=0.3, flecks=(), curvature=0.15),
        Layer("Ebauche light masses", regions=["glow", "sky", "sea_far", "sea_near", "foam"], placement="error",
              width=(0.04, 0.06), length=(0.12, 0.3), T=14, fs=0.7, relief=0.25, load=0.7, opacity=(0.7, 0.9),
              pickup=0.35, streak=0.3, flecks=(), curvature=0.15, dry_after=0.3),
        Layer("Sky long strokes", regions=["sky", "glow"], placement="error", T=12, fs=0.5, fg=1.2),
        Layer("Storm swirl", regions=["storm"], placement="error", T=10, fs=0.45, fg=1.0),
        Layer("Sea underpainting", regions=["sea_far", "sea_near"], placement="error", T=12, fs=0.5, fg=1.2,
              width=(0.016, 0.028), length=(0.06, 0.16)),
        Layer("Rock", regions=["rock", "foam"], placement="error", T=10, fs=0.5, fg=1.0),
        Layer("Tower masses", regions=["tower", "band1", "band2", "lantern"], placement="error", T=6, fs=0.4, fg=1.0,
              dry_after=0.4),
        Layer("Broken colour", regions=["sea_near", "sea_far", "sky", "storm", "glow"], placement="error", T=14, fs=0.35,
              fg=1.6, width=(0.007, 0.014), length=(0.025, 0.07), relief=0.9, coverage=0.5),
        Layer("Glow and halo scumble", regions=["glow", "haze", "halo", "beam"], placement="density", mode="scumble",
              spacing={"glow": 1.6, "haze": 2.2, "halo": 1.6, "beam": 1.4}, coverage={"haze": 0.35, "halo": 0.5}, width=(0.022, 0.036),
              length=(0.08, 0.2), opacity=(0.25, 0.4), relief=0.5, body=0.0,
              load=0.5, vdry=0.6, deplete=0.0, dry_thresh=0.01, dry_width=0.12, colors=SKY_GLOW[:3] + SKY_HAZE[:2],
              color_from="palette", snap=0.0, pickup=0.0, max_cover=None),
        Layer("Beam and band edges", regions=["beam", "band1", "band2"], placement="curve",
              curve={"beam": beam_axis, "band1": "boundary", "band2": "boundary"}, curve_spacing=0.25,
              curve_jitter=0.9, T=0, relief=0.6),
        Layer("Spray and foam", regions=["spray", "foam"], placement="density", spacing=1.8, coverage=0.4, relief=1.0,
              opacity=(0.55, 0.9)),
        Layer("Lost edges", regions=["tower", "sea_far"], placement="curve",
              curve={"tower": "boundary", "sea_far": [(0.0, HZ), (0.6, HZ)]}, curve_spacing=1.2,
              curve_jitter=0.5, mode="smudge", width=(0.008, 0.013), length=(0.02, 0.04), opacity=(0.2, 0.35),
              pickup=0.3, flatten=0.2, body=0.5, align=0.5, curvature=0.3, spill=1.0, stop_at_edge=0.0,
              colors=SKY_HAZE, coverage={"tower": 0.5, "sea_far": 0.12}),
        Layer("Haze glaze and rain", regions=["haze", "rain"], placement="density", mode="glaze",
              spacing={"haze": 1.5, "rain": 6.0}, coverage={"haze": 0.25, "rain": 0.4}, relief=0.0),
        Layer("Lantern and impasto touches", regions=["lantern", "foam", "sea_near", "rock"], placement="density",
              spacing={"lantern": 1.2, "foam": 2.5, "sea_near": 4.0, "rock": 5.0},
              coverage={"lantern": 1.0, "foam": 0.2, "sea_near": 0.02, "rock": 0.04},
              width=(0.005, 0.009), length=(0.015, 0.035), relief=2.0, opacity=(0.9, 1.0),
              colors=["#fff8e0", "#fbe5b8", "#f6d09a", "#f3e9e0"], color_from="palette", snap=0.0, flecks=(),
              pickup=0.05, dab=True),
    ])


# ---- crisp variant: less wet-in-wet blending, harder paint edges (for comparison)
_orig_build = build
def build(S):
    _orig_build(S)
    for name, st in S.styles.items():
        st["pickup"] = min(st["pickup"], 0.06)
        st["hardness"] = 0.85
        st["streak_mix"] = 0.5
    for L in S.layer_list:
        if "pickup" in L and L["mode"] == "paint":
            L["pickup"] = min(L["pickup"], 0.12)
