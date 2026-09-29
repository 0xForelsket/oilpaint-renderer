"""Scene DSL (minimal, stage 2a).  A scene module defines `build(S)` and calls the SceneBuilder.

All coordinates are canvas-width units: x in [0, 1], y in [0, H/W] (portrait 4:5 -> y in [0, 1.25]).
Shapes and fields are callables f(X, Y) -> array evaluated on pixel grids at any resolution, so the same
scene rasterises identically at 600 or 2400 px wide.
"""
import math
import numpy as np
import cv2

from . import mix
from .noise import fbm, grid
from .maps import GuideMaps, structure_flow
from .planner import Style, Layer


# ------------------------------------------------------------------ shape / mask helpers (return f(X,Y) -> 0..1)
def above(y0):
    return lambda X, Y: (Y < y0).astype(np.float32)


def below(y0):
    return lambda X, Y: (Y >= y0).astype(np.float32)


def ellipse(cx, cy, rx, ry, softness=0.0):
    def f(X, Y):
        d = np.sqrt(((X - cx) / rx) ** 2 + ((Y - cy) / ry) ** 2)
        if softness <= 0:
            return (d < 1).astype(np.float32)
        return np.clip((1 + softness - d) / softness, 0, 1).astype(np.float32)
    return f


def disc(c, r, softness=0.0):
    return ellipse(c[0], c[1], r, r, softness)


def polygon(pts):
    def f(X, Y):
        H, W = X.shape
        scale = W  # X in [0,1] -> pixels
        p = np.array([[int(round(x * scale)), int(round(y * scale))] for x, y in pts], np.int32)
        m = np.zeros((H, W), np.uint8)
        cv2.fillPoly(m, [p], 255)
        return (m > 0).astype(np.float32)
    return f


def wedge(apex, angle_deg, spread_deg, length):
    """A soft-edged sector from `apex` pointing at angle_deg (0 = +x, 90 = +y, i.e. down)."""
    def f(X, Y):
        dx, dy = X - apex[0], Y - apex[1]
        d = np.sqrt(dx * dx + dy * dy)
        a = np.degrees(np.arctan2(dy, dx))
        da = (a - angle_deg + 180) % 360 - 180
        ang = np.clip(1 - np.abs(da) / spread_deg, 0, 1)
        rad = np.clip((length - d) / (0.5 * length), 0, 1)
        return (ang * rad).astype(np.float32)
    return f


def union(*fs):
    return lambda X, Y: np.clip(sum(f(X, Y) for f in fs), 0, 1).astype(np.float32)


def intersect(*fs):
    def f(X, Y):
        out = np.ones_like(X)
        for g in fs:
            out = out * g(X, Y)
        return out.astype(np.float32)
    return f


def band_around(pts, dist):
    """Pixels within `dist` (cw units) of a polygon's boundary."""
    poly = polygon(pts)
    def f(X, Y):
        m = poly(X, Y)
        W = X.shape[1]
        k = max(1, int(dist * W))
        dil = cv2.dilate(m, np.ones((2 * k + 1, 2 * k + 1), np.uint8))
        ero = cv2.erode(m, np.ones((2 * k + 1, 2 * k + 1), np.uint8))
        return np.clip(dil - ero, 0, 1).astype(np.float32)
    return f


def noisy(f, amount=0.3, scale=0.08, seed=11):
    """Perturb a mask's boundary with noise (breaks straight edges)."""
    def g(X, Y):
        m = f(X, Y)
        n = fbm(X, Y, scale, 3, seed) - 0.5
        return np.clip(m + amount * n * (m > 0.02) * (m < 0.98) * 4 + amount * n * 0.5, 0, 1).astype(np.float32)
    return g


# ------------------------------------------------------------------ flow-field helpers (return f(X,Y) -> (dx, dy))
def _unit(dx, dy):
    n = np.sqrt(dx * dx + dy * dy) + 1e-6
    return (dx / n).astype(np.float32), (dy / n).astype(np.float32)


def constant(angle_deg, noise=0.0, seed=1):
    def f(X, Y):
        a = np.full(X.shape, math.radians(angle_deg), np.float32)
        if noise > 0:
            a = a + (fbm(X, Y, 0.15, 3, seed) - 0.5) * 2 * noise
        return _unit(np.cos(a), np.sin(a))
    return f


def sweep(angle_deg=-12, curl=0.4, noise=0.25, scale=0.5, seed=2):
    """Long arcs: base direction plus a slowly varying bend."""
    def f(X, Y):
        a = math.radians(angle_deg) + curl * (fbm(X, Y, scale, 2, seed) - 0.5) * 2 + noise * (fbm(X, Y, scale * 0.35, 3, seed + 1) - 0.5) * 2
        return _unit(np.cos(a), np.sin(a))
    return f


def waves(base_angle=0.0, amplitude_deg=18.0, wavelength=0.12, noise=0.3, perspective=True, horizon=0.5, seed=3):
    """Near-horizontal with an undulation whose wavelength shrinks toward the horizon."""
    def f(X, Y):
        depth = np.clip((Y - horizon) / max(1e-3, Y.max() - horizon), 0, 1) if perspective else np.ones_like(Y)
        wl = wavelength * (0.3 + 0.7 * depth)
        a = math.radians(base_angle) + math.radians(amplitude_deg) * np.sin(2 * np.pi * X / wl + 6 * fbm(X, Y, 0.3, 2, seed)) \
            + noise * (fbm(X, Y, 0.08, 3, seed + 1) - 0.5) * 2 * (0.3 + 0.7 * depth)
        return _unit(np.cos(a), np.sin(a))
    return f


def swirl_around(centres, strength=0.7, noise=0.3, seed=4):
    """Tangential flow around ellipse centres (cx, cy, rx, ry, ...)."""
    def f(X, Y):
        dx = np.zeros_like(X); dy = np.zeros_like(Y)
        for c in centres:
            cx, cy, rx, ry = c[:4]
            ex, ey = (X - cx) / rx, (Y - cy) / ry
            d = np.sqrt(ex * ex + ey * ey) + 1e-6
            w = np.exp(-d * d * 0.7)
            dx += w * (-ey / d); dy += w * (ex / d)
        base = constant(-8, noise, seed)(X, Y)
        return _unit(strength * dx + (1 - strength) * base[0], strength * dy + (1 - strength) * base[1])
    return f


def radial_from(c, noise=0.1, seed=5):
    def f(X, Y):
        dx, dy = X - c[0], Y - c[1]
        a = np.arctan2(dy, dx) + (fbm(X, Y, 0.1, 2, seed) - 0.5) * 2 * noise
        return _unit(np.cos(a), np.sin(a))
    return f


def upward(noise=0.6, seed=6):
    return constant(-90, noise * 1.2, seed)


def contour(pts, noise=0.35, seed=7):
    """Tangent of the distance transform of a polygon: strokes follow the shape's outline."""
    poly = polygon(pts)
    def f(X, Y):
        m = poly(X, Y)
        W = X.shape[1]
        d = cv2.distanceTransform((m > 0.5).astype(np.uint8), cv2.DIST_L2, 5) - cv2.distanceTransform((m <= 0.5).astype(np.uint8), cv2.DIST_L2, 5)
        d = cv2.GaussianBlur(d.astype(np.float32), (0, 0), max(1, 0.01 * W))
        gx = cv2.Sobel(d, cv2.CV_32F, 1, 0, ksize=3); gy = cv2.Sobel(d, cv2.CV_32F, 0, 1, ksize=3)
        a = np.arctan2(gy, gx) + np.pi / 2 + (fbm(X, Y, 0.1, 3, seed) - 0.5) * 2 * noise
        return _unit(np.cos(a), np.sin(a))
    return f


# ------------------------------------------------------------------ target painter
class Target:
    """Accumulates soft procedural fills into an RGB image at raster time."""
    def __init__(self):
        self.ops = []

    def fill(self, color_fn, mask=None):
        self.ops.append(("fill", color_fn, mask))

    def blob(self, cx, cy, rx, ry, color, softness=0.5, noise=0.0, strength=1.0, seed=21):
        def alpha(X, Y):
            d = np.sqrt(((X - cx) / rx) ** 2 + ((Y - cy) / ry) ** 2)
            if noise > 0:
                d = d * (1 + noise * (fbm(X, Y, rx * 0.8, 3, seed) - 0.5))
            return strength * np.clip((1 - d) / max(softness, 1e-3), 0, 1) ** 1.0
        self.ops.append(("blend", color, alpha))

    def polygon(self, pts, color, softness=0.01, noise=0.0, strength=1.0, seed=22):
        poly = polygon(pts)
        def alpha(X, Y):
            m = poly(X, Y)
            W = X.shape[1]
            if noise > 0:
                m = noisy(poly, noise, 0.05, seed)(X, Y)
            if softness > 0:
                m = cv2.GaussianBlur(m, (0, 0), max(0.5, softness * W))
            return strength * m
        self.ops.append(("blend", color, alpha))

    def glow(self, c, r, color, strength=1.0, power=2.0):
        def alpha(X, Y):
            d = np.sqrt((X - c[0]) ** 2 + (Y - c[1]) ** 2) / r
            return strength * np.exp(-(d ** power))
        self.ops.append(("add", color, alpha))

    def beam(self, apex, angle_deg, spread_deg, length, color, strength=0.4, softness=0.3):
        w = wedge(apex, angle_deg, spread_deg, length)
        def alpha(X, Y):
            a = w(X, Y)
            W = X.shape[1]
            a = cv2.GaussianBlur(a, (0, 0), max(1, softness * spread_deg * 0.02 * W))
            return strength * a
        self.ops.append(("add", color, alpha))

    def bands(self, pts, bands, softness=0.008):
        """Horizontal colour bands inside a polygon; band positions are fractions of the polygon's height."""
        poly = polygon(pts)
        ys = [p[1] for p in pts]; y0, y1 = min(ys), max(ys)
        for (f0, f1, color) in bands:
            def alpha(X, Y, f0=f0, f1=f1):
                m = poly(X, Y) * ((Y >= y0 + f0 * (y1 - y0)) & (Y < y0 + f1 * (y1 - y0)))
                W = X.shape[1]
                return cv2.GaussianBlur(m.astype(np.float32), (0, 0), max(0.5, softness * W))
            self.ops.append(("blend", color, alpha))

    def render(self, X, Y):
        H, W = X.shape
        img = np.zeros((H, W, 3), np.float32)
        for op in self.ops:
            kind = op[0]
            if kind == "fill":
                col = op[1](X, Y)
                m = op[2]
                if m is None:
                    img = col
                else:
                    a = m(X, Y)[..., None]
                    img = img * (1 - a) + col * a
            elif kind == "blend":
                c = mix.color_spec_to_rgb(op[1])
                a = np.clip(op[2](X, Y), 0, 1)[..., None]
                img = img * (1 - a) + c * a
            elif kind == "add":
                c = mix.color_spec_to_rgb(op[1])
                a = np.clip(op[2](X, Y), 0, 2)[..., None]
                img = img + a * (c - img * 0.5)
        return np.clip(img, 0, 1)


def gradient_v(stops):
    """Vertical gradient from (y, colour) stops (y in cw units)."""
    ys = np.array([s[0] for s in stops], np.float32)
    cs = np.stack([mix.color_spec_to_rgb(s[1]) for s in stops])
    def f(X, Y):
        out = np.zeros(X.shape + (3,), np.float32)
        for k in range(3):
            out[..., k] = np.interp(Y, ys, cs[:, k])
        return out
    return f


# ------------------------------------------------------------------ scene builder
class SceneBuilder:
    def __init__(self):
        self.aspect = (4, 5)
        self.ground = np.array([0.95, 0.93, 0.89], np.float32)
        self.target = Target()
        self.regions = []       # (name, mask_fn, edge)
        self.flows = {}
        self.lights = []
        self.styles = {}
        self.layer_list = []
        self.meta = {}

    def canvas(self, aspect=(4, 5), ground=None):
        self.aspect = aspect
        if ground is not None:
            self.ground = np.asarray(mix.color_spec_to_rgb(ground) if not isinstance(ground, np.ndarray) else ground, np.float32)

    def region(self, name, mask, edge=0.02):
        self.regions.append((name, mask, edge))

    def flow(self, name, field):
        self.flows[name] = field

    def light(self, glow=None, beam=None, lamp=None):
        self.lights.append(dict(glow=glow, beam=beam, lamp=lamp))

    def style(self, name, **kw):
        self.styles[name] = Style(**kw)

    def layers(self, layers):
        self.layer_list = [l if isinstance(l, Layer) else Layer(**l) for l in layers]

    # ---- rasterise
    def size(self, W):
        return W, int(round(W * self.aspect[1] / self.aspect[0]))

    def guides(self, W):
        W, H = self.size(W)
        X, Y = grid(W, H)
        target = self.target.render(X, Y)
        names = [r[0] for r in self.regions]
        masks = {}
        region_id = np.zeros((H, W), np.uint8)
        hard = {}
        for i, (name, fn, edge) in enumerate(self.regions):
            m = np.clip(fn(X, Y), 0, 1).astype(np.float32)
            hard[name] = m
            region_id[m > 0.5] = i          # later regions override earlier ones
        for i, (name, fn, edge) in enumerate(self.regions):
            m = hard[name]
            e = edge(Y) if callable(edge) else edge
            if np.isscalar(e):
                sig = max(0.5, float(e) * W)
                soft = cv2.GaussianBlur(m, (0, 0), sig)
            else:
                # spatially varying edge: blend two blurs by the per-pixel edge width
                e = np.asarray(e, np.float32)
                lo, hi = float(e.min()), float(e.max())
                b_lo = cv2.GaussianBlur(m, (0, 0), max(0.5, lo * W)); b_hi = cv2.GaussianBlur(m, (0, 0), max(0.5, hi * W))
                t = (e - lo) / max(hi - lo, 1e-6)
                soft = b_lo * (1 - t) + b_hi * t
            masks[name] = soft.astype(np.float32)
        # flow: each region keeps its own authored field (strokes of that region follow it everywhere, so a
        # region hidden under later ones, like "rain", still works); the global field is the field of the
        # region that owns each pixel, structure-tensor fallback where no field is authored
        st_flow, aniso = structure_flow(target, sigma_px=0.02 * W)
        region_flows = {}
        flow = st_flow.copy()
        for i, name in enumerate(names):
            if name in self.flows:
                fx, fy = self.flows[name](X, Y)
                f = np.stack([fx, fy], -1).astype(np.float32)
                region_flows[name] = f
                own = region_id == i
                flow[own] = f[own]
        n = np.linalg.norm(flow, axis=-1, keepdims=True) + 1e-6
        flow = (flow / n).astype(np.float32)
        # light map
        lm = np.zeros((H, W), np.float32)
        for l in self.lights:
            if l["glow"]:
                cx, cy, r = l["glow"][:3]; s = l["glow"][3] if len(l["glow"]) > 3 else 1.0
                d = np.sqrt((X - cx) ** 2 + ((Y - cy) * 2.5) ** 2) / r
                lm += s * np.exp(-d * d)
            if l["beam"]:
                apex, ang, spread, length = l["beam"][:4]; s = l["beam"][4] if len(l["beam"]) > 4 else 1.0
                a = wedge(apex, ang, spread, length)(X, Y)
                lm += s * cv2.GaussianBlur(a, (0, 0), max(1, 0.02 * W))
            if l["lamp"]:
                c, r = l["lamp"][:2]; s = l["lamp"][2] if len(l["lamp"]) > 2 else 1.0
                d = np.sqrt((X - c[0]) ** 2 + (Y - c[1]) ** 2) / r
                lm += s * np.exp(-d * d)
        lm = np.clip(lm, 0, 1).astype(np.float32)
        return GuideMaps(W, H, target, region_id, masks, names, flow, lm, region_flows=region_flows)


def load_scene(path):
    import importlib.util
    spec = importlib.util.spec_from_file_location("scene_module", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    S = SceneBuilder()
    mod.build(S)
    return S
