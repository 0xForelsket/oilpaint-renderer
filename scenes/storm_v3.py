"""Storm Light v3 - a banded lighthouse on a rock in a breaking storm, one lamp lit (Fl(2) W 12s).

Composition notes (Monet-ward): the tower is set against the dark storm bank on its right and a
counter-lit, lower glow on its left; the horizon glow, the beam and the sea reflections carry the warm light.
Coordinates are canvas-width units; the canvas is 1.0 wide x 1.25 high.
"""
from oilpaint.scene import *   # noqa: F401,F403

HZ = 0.60
CX = 0.68                                   # tower axis
T_TOP, T_BOT = 0.400, 0.738
HALF_TOP, HALF_BOT = 0.034, 0.058
LANTERN = (CX, 0.362)
BEAM_ANGLE, BEAM_LEN = 178, 0.52


def tw(y):
    h = HALF_TOP + (HALF_BOT - HALF_TOP) * (y - T_TOP) / (T_BOT - T_TOP)
    return CX - h, CX + h


def band(y0, y1):
    (a, _), (_, b) = tw(y0), tw(y1)
    return [(tw(y0)[0], y0), (tw(y0)[1], y0), (tw(y1)[1], y1), (tw(y1)[0], y1)]


TOWER = band(T_TOP, T_BOT)
BAND1 = band(0.500, 0.556)
BAND2 = band(0.628, 0.686)
ROCK = [(0.45, 0.90), (0.48, 0.83), (0.52, 0.79), (0.57, 0.772), (0.61, 0.752), (0.645, 0.738), (0.71, 0.735),
        (0.75, 0.742), (0.79, 0.765), (0.84, 0.780), (0.89, 0.805), (0.94, 0.822), (1.02, 0.850),
        (1.02, 0.955), (0.86, 0.938), (0.70, 0.948), (0.56, 0.932)]

STORM = [(0.18, 0.09, 0.36, 0.13, "#232a58"), (0.74, 0.06, 0.34, 0.11, "#232a58"), (0.45, 0.22, 0.30, 0.10, "#2c3266"),
         (0.06, 0.29, 0.26, 0.09, "#3a3b78"), (0.90, 0.26, 0.24, 0.09, "#3a3b78"), (0.30, 0.365, 0.28, 0.06, "#5a5a95"),
         (0.84, 0.44, 0.30, 0.075, "#3a3b78")]                       # last: squall bank right of the tower

SKY_DARK = ["#5a5a95", "#484886", "#3a3b78", "#2c3266"]
SKY_MID = ["#b1a8cf", "#9d97c4", "#8a86b8", "#7676ab", "#66679e"]
SKY_GLOW = ["#fbe5b8", "#f6d09a", "#f0b98f", "#e8a08f", "#d98f9a"]
SKY_HAZE = ["#efe1d6", "#e6d3d0", "#dccbd3", "#cdc3d6"]
SEA_FAR = ["#f3dcc4", "#e3c3c4", "#c9b6d3", "#b4b7d8", "#a3b6d8"]
SEA_MID = ["#a3b6d8", "#86a6c9", "#6fa0bb", "#8288b8", "#6086b4"]
SEA_NEAR = ["#4a76ae", "#3a67a0", "#2f5490", "#3f88a0", "#2f7590", "#2a4a80", "#1f3a6c", "#5a8fc0"]
ROCK_LIT = ["#f0d3a8", "#dcb98f", "#c7a37f", "#a98c86", "#c9785a", "#e0a070"]
ROCK_MID = ["#8d7d9a", "#7a6f98", "#68628f"]
ROCK_DEEP = ["#4a4a7c", "#3c4270", "#2f3862", "#403a6a"]
TOWER_W = ["#fbf0dc", "#f7e2bd", "#e9d8d6", "#c9c0dd", "#a79fca"]
TOWER_R = ["#ee8a6f", "#d8695a", "#c8503f", "#a94358", "#8a3d62"]
FOAM = ["#f3e9e0", "#e6e3f2", "#d3d9ee", "#bcc9e6", "#f0d3b8"]
CREST = ["#5a8fc0", "#5a8fc0", "#7fb0c8", "#7fb0c8", "#9ab8d8", "#e6e3f2"]


def build(S):
    S.canvas(aspect=(4, 5), ground="#e9e1d6")

    def _style(name, **kw):
        kw.setdefault("splay", 0.35)              # fewer stray hairs: they read as pencil lines in pale areas
        S.style(name, **kw)

    T = S.target
    # ---- sky: dark above, a pale-gold gap low on the left, a dark squall on the right at tower height
    T.fill(gradient_v([(0.0, "#232a58"), (0.24, "#3a3b78"), (0.40, "#66679e"), (0.51, "#b4a6c6"), (HZ, "#e8cbb8")]))
    T.blob(0.22, HZ - 0.03, 0.40, 0.09, "#f4bd90", softness=0.7, strength=0.85, seed=31)      # horizon glow
    T.blob(0.20, HZ - 0.04, 0.20, 0.045, "#f9dcae", softness=0.8, strength=0.7, seed=32)      # its hot core
    T.blob(0.08, HZ - 0.09, 0.22, 0.06, "#e8a0a0", softness=0.7, strength=0.45, seed=33)      # rose above it
    for i, (cx, cy, rx, ry, c) in enumerate(STORM):
        T.blob(cx, cy, rx, ry, c, softness=0.55, noise=0.8, strength=0.92, seed=40 + i)
    for i, (cx, cy, rx, ry, c, s) in enumerate([(0.55, 0.13, 0.10, 0.035, "#9d97c4", 0.6), (0.12, 0.20, 0.09, 0.03, "#b1a8cf", 0.6),
                                                (0.40, 0.285, 0.13, 0.022, "#e8a08f", 0.75), (0.13, 0.355, 0.11, 0.02, "#f0b98f", 0.75),
                                                (0.52, 0.415, 0.10, 0.02, "#f6d09a", 0.6), (0.92, 0.10, 0.08, 0.025, "#c9a6b8", 0.5)]):
        T.blob(cx, cy, rx, ry, c, softness=0.8, noise=0.6, strength=s, seed=60 + i)              # light breaks in the storm
    # ---- sea
    T.fill(gradient_v([(HZ, "#dcc8cc"), (HZ + 0.05, "#b4b7d8"), (HZ + 0.14, "#7aa0c4"), (HZ + 0.30, "#3a67a0"), (1.25, "#20386c")]), mask=below(HZ))
    for i, (cx, cy, rx, ry, c) in enumerate([(0.15, 0.95, 0.20, 0.03, "#1f3a6c"), (0.55, 1.05, 0.25, 0.035, "#1c3566"),
                                             (0.85, 0.99, 0.18, 0.03, "#243f74"), (0.30, 1.12, 0.22, 0.03, "#4f9ab0"),
                                             (0.10, 1.04, 0.15, 0.025, "#9ab8d8"), (0.70, 1.15, 0.2, 0.03, "#7fb0c8"),
                                             (0.45, 0.88, 0.2, 0.025, "#8fb4d4"), (0.80, 1.08, 0.15, 0.03, "#1f3a6c")]):
        T.blob(cx, cy, rx, ry, c, softness=0.8, noise=0.6, strength=0.85, seed=70 + i)
    T.blob(0.22, HZ + 0.03, 0.28, 0.04, "#f6cf98", softness=0.8, strength=0.65, seed=34)      # the glow, reflected
    for i, (cx, cy, rx, ry, s) in enumerate([(0.22, HZ + 0.10, 0.16, 0.02, 0.55), (0.24, HZ + 0.17, 0.20, 0.022, 0.5),
                                             (0.20, HZ + 0.25, 0.24, 0.025, 0.42), (0.23, HZ + 0.34, 0.26, 0.028, 0.35)]):
        T.blob(cx, cy, rx, ry, "#e8c48c", softness=0.85, noise=0.6, strength=s, seed=90 + i)   # gold path on the water
    T.polygon(ROCK, "#4a4a7c", softness=0.012, noise=0.5, seed=35)
    T.blob(0.55, 0.805, 0.08, 0.04, "#c7a37f", softness=0.8, strength=0.6, seed=36)           # lit facets, left
    T.blob(0.88, 0.85, 0.10, 0.04, "#2f3862", softness=0.8, strength=0.6, seed=37)            # deep shadow, right
    T.blob(0.64, 0.765, 0.07, 0.022, "#d9a883", softness=0.8, noise=0.6, strength=0.55, seed=48)  # warm light on the top plane
    T.blob(0.78, 0.792, 0.06, 0.02, "#a98c96", softness=0.8, noise=0.6, strength=0.45, seed=49)
    T.blob(0.66, 0.85, 0.10, 0.03, "#6b5f92", softness=0.8, noise=0.6, strength=0.5, seed=50)
    # ---- tower, gallery, roof
    T.polygon(TOWER, "#f0e3d3", softness=0.005)
    T.polygon(BAND1, "#c8503f", softness=0.004); T.polygon(BAND2, "#c8503f", softness=0.004)
    T.blob(tw(0.57)[0] + 0.012, 0.57, 0.02, 0.17, "#fbecc8", softness=0.9, strength=0.55, seed=38)   # lit left edge
    T.blob(tw(0.57)[1] - 0.012, 0.57, 0.03, 0.17, "#8a86b8", softness=0.9, strength=0.65, seed=39)   # shadow right edge
    T.blob(0.36, 0.378, 0.32, 0.05, "#66679e", softness=0.7, noise=0.5, strength=0.7, seed=45)   # darker strip the beam crosses
    T.blob(0.30, 0.425, 0.28, 0.028, "#7676ab", softness=0.8, noise=0.5, strength=0.45, seed=46)
    T.beam(LANTERN, BEAM_ANGLE, 9, BEAM_LEN, "#fadd9c", strength=0.8, softness=0.5)
    T.glow(LANTERN, 0.085, "#f9dcae", strength=0.6)                                            # wide warm halo
    T.glow(LANTERN, 0.05, "#ffe9a8", strength=0.75)                                            # inner halo
    T.polygon([(CX - 0.052, 0.394), (CX + 0.052, 0.394), (CX + 0.052, 0.407), (CX - 0.052, 0.407)], "#4a4270", softness=0.003)   # gallery
    T.polygon([(CX - 0.030, 0.335), (CX + 0.030, 0.335), (CX, 0.288)], "#352e5e", softness=0.003)                                 # roof
    T.polygon([(CX - 0.024, 0.338), (CX + 0.024, 0.338), (CX + 0.024, 0.392), (CX - 0.024, 0.392)], "#ffd870", softness=0.004)   # lantern room
    T.glow(LANTERN, 0.018, "#fffbe0", strength=1.0)                                            # hot core
    T.blob(0.61, 0.875, 0.20, 0.03, "#d3d9ee", softness=0.9, strength=0.4, seed=41)           # foam at the base
    T.blob(0.57, 0.80, 0.09, 0.05, "#dcd6ea", softness=0.9, strength=0.35, seed=42)           # spray

    # ---- regions
    beam_axis = [(LANTERN[0] - 0.05, LANTERN[1] + 0.002), (LANTERN[0] - BEAM_LEN, LANTERN[1] + 0.018)]   # starts off-axis: radial flow is undefined at the lamp
    S.region("ground", lambda X, Y: (Y > -1).astype("float32"), edge=0.02)
    S.region("rain", lambda X, Y: (Y < 0.98).astype("float32"), edge=0.02)
    S.region("sky", above(HZ), edge=0.03)
    S.region("storm", union(*[ellipse(*m[:4]) for m in STORM[:5] + STORM[6:]]), edge=0.06)
    S.region("glow", ellipse(0.22, HZ - 0.03, 0.40, 0.09), edge=0.08)
    S.region("sea_far", intersect(below(HZ), above(HZ + 0.13)), edge=0.02)
    NOT_ROCK = lambda X, Y: (1.0 - polygon(ROCK)(X, Y)).clip(0, 1)
    S.region("sea_near", intersect(below(HZ + 0.13), NOT_ROCK), edge=0.03)
    S.region("rock", polygon(ROCK), edge=0.012)
    S.region("foam", intersect(band_around(ROCK, 0.03), below(0.80)), edge=0.02)
    S.region("spray", ellipse(0.57, 0.80, 0.09, 0.05), edge=0.04)
    S.region("tower", polygon(TOWER), edge=0.006)
    S.region("band1", polygon(BAND1), edge=0.004)
    S.region("band2", polygon(BAND2), edge=0.004)
    S.region("beam", wedge(LANTERN, BEAM_ANGLE, 9, BEAM_LEN), edge=0.03)
    S.region("halo", disc(LANTERN, 0.07), edge=0.03)
    S.region("lantern", polygon([(CX - 0.024, 0.338), (CX + 0.024, 0.338), (CX + 0.024, 0.392), (CX - 0.024, 0.392)]), edge=0.004)
    S.region("roof", polygon([(CX - 0.030, 0.335), (CX + 0.030, 0.335), (CX, 0.288)]), edge=0.003)
    FOOT = [(CX - 0.085, 0.736), (CX + 0.085, 0.736), (CX + 0.095, 0.758), (CX - 0.095, 0.758)]
    S.region("foot", polygon(FOOT), edge=0.004)
    S.flow("foot", constant(-3, noise=0.15))
    S.region("gallery", polygon([(CX - 0.052, 0.394), (CX + 0.052, 0.394), (CX + 0.052, 0.407), (CX - 0.052, 0.407)]), edge=0.003)

    # ---- flow
    S.flow("ground", constant(0, noise=0.05))
    S.flow("rain", constant(115, noise=0.04))
    S.flow("sky", sweep(angle_deg=-14, curl=0.5, noise=0.3, scale=0.5))
    S.flow("storm", swirl_around(STORM[:5] + STORM[6:], strength=0.22, noise=0.4))
    S.flow("glow", sweep(angle_deg=-4, curl=0.2, noise=0.2, scale=0.4, seed=9))
    S.flow("sea_far", waves(base_angle=0, amplitude_deg=4, wavelength=0.08, noise=0.08, horizon=HZ))
    S.flow("sea_near", waves(base_angle=0, amplitude_deg=9, wavelength=0.18, noise=0.15, horizon=HZ))
    S.flow("foam", waves(base_angle=0, amplitude_deg=35, wavelength=0.05, noise=0.5, horizon=HZ, seed=12))
    S.flow("spray", upward(noise=0.7))
    S.flow("rock", contour(ROCK, noise=0.8))
    S.flow("tower", constant(90, noise=0.05))
    S.flow("band1", constant(0, noise=0.03)); S.flow("band2", constant(0, noise=0.03))
    S.flow("beam", constant(BEAM_ANGLE, noise=0.02))          # parallel: radial flow fans strokes that start off-axis near the lamp
    S.flow("halo", radial_from(LANTERN, noise=0.9))
    S.flow("lantern", constant(80, noise=0.25))
    S.flow("roof", radial_from((CX, 0.288), noise=0.05)); S.flow("gallery", constant(0, noise=0.02))
    S.light(glow=(0.22, HZ - 0.03, 0.5, 0.75), beam=(LANTERN, BEAM_ANGLE, 9, BEAM_LEN, 0.8), lamp=(LANTERN, 0.07, 1.0))

    # ---- styles (lane engine: hardness/ridge/levee/marble per the recommended table)
    _style("sky", colors=SKY_MID + SKY_DARK[:2] + SKY_HAZE[:2], flecks=[("#f0b98f", 0.03)],
            width=(0.025, 0.040), length=(0.10, 0.26), curvature=0.35, align=0.80, pickup=0.10, hgain=0.7,
            warmth=0.5, priority=0, spill=0.2, jitter=(6.0, 4.0), marble=0.4, load2="#c9b8d8", ridge=0.4, levee=0.0, furrow=0.05, blob=0.15, stiff=0.3, hardness=0.6)
    _style("storm", colors=SKY_DARK + SKY_MID[3:] + ["#e8a08f"], flecks=[("#e8a08f", 0.06), ("#9d97c4", 0.06)],
            width=(0.020, 0.034), length=(0.06, 0.14), curvature=0.4, align=0.68, pickup=0.10, hgain=0.6, priority=1,
            jitter=(7.0, 4.0), marble=0.4, load2="#5a5a95", ridge=0.4, levee=0.0, furrow=0.05, blob=0.15, stiff=0.3, hardness=0.6)
    _style("glow", colors=SKY_GLOW + SKY_HAZE[:2], flecks=[("#9d97c4", 0.03), ("#e8a08f", 0.04), ("#f0d090", 0.04)],
            width=(0.022, 0.040), length=(0.08, 0.22), curvature=0.3, align=0.85, pickup=0.12, hgain=0.8, priority=2,
            warmth=0.6, marble=0.3, load2="#f6d09a", ridge=0.35, levee=0.0, furrow=0.03, blob=0.1, stiff=0.25, hardness=0.55)
    _style("sea_far", colors=SEA_FAR + SEA_MID[:2], flecks=[("#f6d09a", 0.12)], width=(0.008, 0.014),
            length=(0.025, 0.06), curvature=0.15, align=0.95, pickup=0.10, hgain=0.7, priority=3, warmth=0.6,
            marble=0.4, load2="#f3dcc4", ridge=0.3, levee=0.1, furrow=0.05, hardness=0.65)
    _style("sea_near", colors=SEA_NEAR + SEA_MID + SEA_FAR[:1], flecks=[("#f6d09a", 0.08), ("#e8944a", 0.03)],
            width=(0.014, 0.024), length=(0.06, 0.16), curvature=0.2, align=0.92, pickup=0.10, hgain=1.0, priority=3,
            warmth=0.5, size_by_y=(HZ + 0.13, 1.25, 0.7, 1.5), end_width=0.7, jitter=(6.0, 3.0), marble=0.4, load2="#5a8fc0",
            ridge=0.5, levee=0.35, stiff=0.25, hardness=0.75)
    _style("rock", colors=ROCK_DEEP + ROCK_MID + ROCK_LIT, flecks=[("#f0d3a8", 0.04), ("#e8944a", 0.02)],
            width=(0.012, 0.024), length=(0.02, 0.055), curvature=0.35, align=0.5, pickup=0.10, hgain=1.0, priority=4,
            warmth=0.5, marble=0.4, load2="#8d7d9a", ridge=0.5, levee=0.35, hardness=0.75)
    _style("foam", colors=FOAM + SEA_MID[:2], width=(0.008, 0.016), length=(0.03, 0.08), curvature=0.5, align=0.6,
            pickup=0.10, hgain=1.1, priority=6, warmth=0.5, jitter=(4.0, 3.0), marble=0.3, hardness=0.7)
    _style("spray", colors=FOAM[:4] + SKY_HAZE[:2], width=(0.006, 0.012), length=(0.02, 0.06), curvature=0.4,
            align=0.55, pickup=0.10, hgain=0.6, priority=6, opacity=(0.5, 0.85), warmth=0.4, hardness=0.5)
    _style("tower", colors=TOWER_W + ["#f0d3a8", "#8a86b8"], flecks=[("#f0b98f", 0.05)],
            width=(0.008, 0.016), length=(0.03, 0.07), curvature=0.02, align=0.97, pickup=0.10, hgain=0.8,
            priority=5, spill=0.0, stop_at_edge=0.98, jitter=(3.0, 2.0), end_width=0.8, hardness=0.7, marble=0.3, load2="#c9c0dd", ridge=0.4)
    for nm in ("band1", "band2"):
        _style(nm, colors=TOWER_R, width=(0.006, 0.011), length=(0.02, 0.05), curvature=0.02, align=0.97,
                pickup=0.10, hgain=0.8, priority=5, spill=0.0, stop_at_edge=0.98, jitter=(3.0, 2.0), end_width=0.8,
                flecks=[], hardness=0.7, marble=0.3, load2="#a94358", ridge=0.4)
    _style("beam", colors=["#fff0c0", "#fde3a8", "#f9d38c", "#f6c878"], width=(0.018, 0.030), length=(0.14, 0.30),
            curvature=0.05, align=0.98, pickup=0.02, hgain=0.4, priority=2, warmth=0.3, opacity=(0.85, 0.97),
            opacity_by_light=0.3, body=0.9, load=1.0, deplete=0.02, end_width=0.7, snap=0.7, jitter=(3.0, 2.0),
            hardness=0.3, ridge=0.2, levee=0.0, stiff=0.1, splay=0.0)
    _style("halo", colors=["#fff3c8", "#ffe9a8", "#fbe5b8", "#f6d09a", "#f9dcae"], width=(0.006, 0.012), length=(0.010, 0.022), curvature=0.2,
            align=0.45, pickup=0.12, hgain=0.3, priority=3, opacity=(0.35, 0.65), hardness=0.4, ridge=0.1, levee=0.0, furrow=0.0, splay=0.0,
            snap=0.6, body=0.8)
    _style("lantern", colors=["#fff2c0", "#ffe9a0", "#ffdf7a", "#ffd060", "#ffe9b0", "#ffc850", "#fffbe0"], width=(0.005, 0.008), length=(0.014, 0.026),
            curvature=0.05, align=0.85, pickup=0.0, hgain=0.7, priority=7, jitter=(1.5, 1.5), end_width=0.9,
            end_pressure=0.5, opacity=(0.97, 1.0), L_floor=80.0, blob=0.2, levee=0.15, stiff=0.1, ridge=0.25, marble=0.0, hardness=0.6, splay=0.0)
    _style("roof", colors=["#352e5e", "#3c3565", "#4a4270", "#5d4f82"], width=(0.004, 0.007), length=(0.010, 0.022), curvature=0.0,
            align=0.95, pickup=0.05, hgain=0.8, priority=7, jitter=(2.0, 2.0), hardness=0.8, splay=0.0, ridge=0.3, L_floor=25.0)
    _style("gallery", colors=["#5d4f82", "#6a5a8c", "#4a4270", "#8a86b8"], width=(0.004, 0.006), length=(0.02, 0.05), curvature=0.0,
            align=1.0, pickup=0.05, hgain=0.8, priority=7, jitter=(2.0, 2.0), hardness=0.8, splay=0.0, ridge=0.3, L_floor=25.0)
    _style("foot", colors=ROCK_DEEP + ROCK_MID + ["#c7a37f", "#a98c86"], width=(0.006, 0.010), length=(0.03, 0.07), curvature=0.1,
            align=0.9, pickup=0.05, hgain=0.9, priority=7, jitter=(3.0, 2.0), hardness=0.75, ridge=0.35, levee=0.15, splay=0.0, spill=0.0)
    _style("crest", colors=CREST, flecks=[("#f6d09a", 0.06)], width=(0.007, 0.012), length=(0.05, 0.12), curvature=0.3,
            align=0.92, pickup=0.08, hgain=1.0, priority=6, opacity=(0.7, 0.95), marble=0.3, hardness=0.7,
            size_by_y=(HZ + 0.13, 1.25, 0.7, 1.3))
    _style("rain", colors=SKY_HAZE[:2] + ["#cfcbe6"], width=(0.004, 0.007), length=(0.08, 0.18), curvature=0.0,
            align=0.98, pickup=0.0, hgain=0.2, priority=8, opacity=(0.08, 0.18), mode="scumble", body=0.0, snap=0.3,
            hardness=0.5, ridge=0.3)

    S.region("crest", intersect(below(HZ + 0.16), NOT_ROCK, lambda X, Y: (Y < 1.2).astype("float32")), edge=0.03)
    S.flow("crest", waves(base_angle=0, amplitude_deg=14, wavelength=0.14, noise=0.2, horizon=HZ, seed=15))

    S.layers([
        Layer("Toned ground", regions=["ground"], placement="density", width=(0.08, 0.10), length=(0.3, 0.5),
              spacing=2.0, opacity=(0.3, 0.45), relief=0.1, streak=0.2, snap=0.0, jitter=(2.0, 1.5), flecks=(),
              colors=["#e9e1d6", "#e4dcd8", "#ded7d4", "#e6ded0", "#e2d6cc"], color_from="palette", pickup=0.3,
              curvature=0.0, align=1.0, stop_at_edge=0.0, spill=1.0, coverage=0.7, hardness=0.4, ridge=0.1, levee=0.0),
        Layer("Ebauche dark masses", regions=["storm", "sky", "sea_near", "sea_far", "rock"], placement="error",
              width=(0.04, 0.06), length=(0.12, 0.3), T=16, fs=0.7, fg=1.0, relief=0.3, load=0.7, opacity=(0.7, 0.9),
              pickup=0.15, streak=0.3, flecks=(), curvature=0.15, body=1.0, ridge=0.15, levee=0.05, furrow=0.05, blob=0.1, stiff=0.15, hardness=0.5),
        Layer("Ebauche light masses", regions=["glow", "sky", "sea_far", "sea_near", "foam"], placement="error",
              width=(0.04, 0.06), length=(0.12, 0.3), T=14, fs=0.7, relief=0.3, load=0.7, opacity=(0.7, 0.9),
              pickup=0.15, streak=0.3, flecks=(), curvature=0.15, dry_after=0.3, body=1.0, ridge=0.15, levee=0.05, furrow=0.05, blob=0.1, stiff=0.15, hardness=0.5),
        Layer("Sky long strokes", regions=["sky", "glow"], placement="error", T=12, fs=0.5, fg=1.2),
        Layer("Storm swirl", regions=["storm"], placement="error", T=10, fs=0.45, fg=1.0),
        Layer("Sea underpainting", regions=["sea_far", "sea_near"], placement="error", T=12, fs=0.5, fg=1.2,
              width=(0.016, 0.028), length=(0.06, 0.16)),
        Layer("Rock", regions=["rock", "foam"], placement="error", T=10, fs=0.5, fg=1.0),
        Layer("Tower and lamp", regions=["halo", "tower", "band1", "band2", "roof", "gallery", "lantern"], placement="error", T=6, fs=0.25, fg=1.0,
              dry_after=0.4),
        Layer("Broken colour", regions=["sea_near", "sea_far", "glow"], placement="error", T=14, fs=0.35,
              fg=1.6, width=(0.007, 0.014), length=(0.025, 0.07), relief=0.9, coverage=0.5, marble=0.5, body=0.9, blob=0.4),
        Layer("Glow and halo scumble", regions=["glow", "beam"], placement="density", mode="scumble",
              spacing={"glow": 1.8, "beam": 1.4}, coverage={"glow": 0.7}, width=(0.022, 0.036),
              length=(0.08, 0.2), opacity=(0.25, 0.4), relief=0.5, body=0.0,
              load=0.5, vdry=0.6, deplete=0.0, dry_thresh=0.01, dry_width=0.12, colors=SKY_GLOW[:3] + SKY_HAZE[:2],
              color_from="palette", snap=0.0, pickup=0.0, max_cover=None, ridge=0.2, hardness=0.5),
        Layer("Beam and band edges", regions=["beam", "band1", "band2"], placement="curve",
              curve={"beam": beam_axis, "band1": "boundary", "band2": "boundary"}, curve_spacing=0.25,
              curve_jitter=0.9, T=0, relief=0.6),
        Layer("Wave crests and foam", regions=["crest", "spray", "foam"], placement="density",
              spacing={"crest": 2.6, "spray": 2.2, "foam": 1.6}, coverage={"crest": 0.2, "spray": 0.25, "foam": 0.5},
              relief=1.0, opacity=(0.55, 0.9), color_from="palette", snap=0.0),
        Layer("Lost edges", regions=["tower", "sea_far"], placement="curve",
              curve={"tower": "boundary", "sea_far": [(0.0, HZ), (0.6, HZ)]}, curve_spacing=1.2,
              curve_jitter=0.5, mode="smudge", width=(0.008, 0.013), length=(0.02, 0.04), opacity=(0.15, 0.3),
              pickup=0.3, flatten=0.2, body=0.5, align=0.5, curvature=0.3, spill=1.0, stop_at_edge=0.0,
              colors=SKY_HAZE, coverage={"tower": 0.10, "sea_far": 0.10}, hardness=0.4),
        Layer("Glaze and rain", regions=["rain", "halo"], placement="density", mode="glaze",
              spacing={"rain": 6.0, "halo": 0.7}, coverage={"rain": 0.22, "halo": 0.45}, relief=0.0, color_from="palette"),
        Layer("Lantern and impasto touches", regions=["roof", "gallery", "lantern", "foot", "foam"], placement="density",
              spacing={"roof": 0.9, "gallery": 0.9, "lantern": 1.0, "foot": 1.2, "foam": 2.5},
              coverage={"roof": 1.0, "gallery": 1.0, "lantern": 1.0, "foot": 0.9, "foam": 0.2},
              width=(0.005, 0.009), length=(0.015, 0.035), relief=2.0, opacity=(0.9, 1.0),
              color_from="reference", snap=0.85, flecks=(),
              pickup=0.05, dab=True, blob=0.3, levee=0.3, stiff=0.2, marble=0.0),
    ])
