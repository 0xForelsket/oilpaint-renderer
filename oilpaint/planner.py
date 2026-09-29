"""Stroke planner: turns guide maps + per-region styles + a layer schedule into an ordered stroke list.

Planning runs once at the plan resolution on a proxy canvas (the same kernel), so error-driven placement
sees what earlier strokes did.  Strokes are emitted in canvas-width units and replay at any resolution.
"""
import math
import numpy as np
import cv2
from scipy.spatial import cKDTree

from . import mix
from .canvas import Canvas, Stroke
from .strokes import profile, streak_vector

STYLE_DEFAULTS = dict(
    colors=None,            # list of colour specs the region is painted with (palette snapping targets)
    flecks=(),              # [(colour spec, probability)] complementary touches
    width=(0.015, 0.03),    # stroke width range, fraction of canvas width
    length=(0.04, 0.10),    # stroke length range, fraction of canvas width
    curvature=0.3,          # 0 = straight, 1 = follows every turn of the flow
    align=0.85,             # 1 = strictly along the flow, 0 = random directions
    opacity=(0.85, 1.0),
    pickup=0.12, load=1.0, deplete=0.02, vdry=0.25, hgain=1.0, flatten=0.6, streak=0.25, streak_mix=0.8, body=0.9,
    hardness=0.75, grain=0.10, nb_per_cw=450, nb_base=5,  # bristle lanes = nb_base + nb_per_cw * width (0.02 cw -> 14)
    release=0.3, dropout=0.02, ragged=0.5,
    ridge=0.5,              # lane ridge/furrow relief (x hgain)
    levee=0.35,             # raised paint along both stroke edges (x hgain)
    furrow=0.15,            # slight trough along the stroke centre (x hgain)
    blob=0.3,               # extra paint where the stroke starts (x hgain)
    stiff=0.25,             # multi-scale roughness of stiff paint (x hgain)
    marble=0.0,             # two-colour load: share of colour B in the lanes assigned to it (0 = single colour)
    load2=None,             # colour B for marbling (colour spec); None = an automatic lighter/warmer variant
    splay=1.0,              # stray hairs at the outline (0..3)
    snap=0.85,              # how far to move the sampled colour toward the nearest palette mixture
    jitter=(5.0, 4.0),      # Lab jitter (L, a/b) between strokes
    warmth=0.0,             # how much the light map warms the colour toward `warm_color`
    warm_color="#f6d09a",
    priority=0,             # painting order among regions in one layer (low first)
    reverse_p=0.0,          # probability to run a stroke against the flow direction
    spill=0.15,             # probability that a stroke keeps painting when it crosses into another region
    stop_at_edge=0.9,       # probability a path stops where the region's soft mask fades out
    min_aspect=2.5,         # length >= min_aspect * width unless the layer sets dab=True
    end_width=0.5,          # width factor at the very end of a stroke (1 = blunt flat brush)
    end_pressure=0.15,      # pressure at the very end (higher = blunter, more opaque end)
    hgain_jitter=0.25,      # +- relative variation of paint thickness between strokes
    mode=None,              # per-region override of the layer mode: paint | scumble | smudge | glaze
    size_by_y=None,         # (y0, y1, s0, s1): scale width and length by s0 at y0 .. s1 at y1 (perspective)
    opacity_by_light=0.0,   # >0: opacity multiplied by (1 - k*(1-light)) so strokes fade away from the light
    L_floor=20.0,           # stroke colours never darker than this L* (Monet: no real darks)
)

LAYER_DEFAULTS = dict(
    regions="all", placement="error", mode="paint", T=18.0, fg=1.0, fs=0.5, spacing=1.6, coverage=1.0,
    dry_after=None, color_from="reference", order="sweep", jitter_pos=0.5, max_strokes=None, enabled=True,
    seed_offset=0, hblur_sigma=0.01, relief=1.0, max_cover=None, gap_fill=True, gap_cover=0.15, dab=False,
    curve=None, curve_offset=0.0, curve_spacing=1.0, curve_jitter=0.5,
)


class Style(dict):
    def __init__(self, **kw):
        super().__init__(STYLE_DEFAULTS); self.update(kw)


class Layer(dict):
    def __init__(self, name, **kw):
        super().__init__(LAYER_DEFAULTS); self.update(kw); self["name"] = name


def _rgb_spec(c):
    return mix.color_spec_to_rgb(c)


class Palette:
    """Nearest-mixture snapping in Lab over Mixbox mixtures of a region's colours (+ white)."""
    def __init__(self, colors, steps=9, white="lead_white"):
        cols = [np.asarray(_rgb_spec(c), np.float32) for c in colors]
        if not cols:
            cols = [np.array([0.5, 0.5, 0.5], np.float32)]
        lats = [mix.rgb_to_latent(c) for c in cols]
        zw = mix.rgb_to_latent(mix.tube(white))
        cands = []
        ts = np.linspace(0, 1, steps, dtype=np.float32)
        for i, zi in enumerate(lats):
            for t in ts:
                cands.append((1 - t) * zi + t * zw)                 # tints
            for zj in lats[i + 1:]:
                for t in ts:
                    cands.append((1 - t) * zi + t * zj)             # pairs
                    cands.append(0.7 * ((1 - t) * zi + t * zj) + 0.3 * zw)   # lighter pairs
        self.lat = np.stack(cands).astype(np.float32)
        self.rgb = mix.latent_to_rgb(self.lat)
        self.lab = mix.rgb_to_lab(self.rgb)
        self.tree = cKDTree(self.lab)

    def snap(self, rgb, amount=1.0):
        rgb = np.asarray(rgb, np.float32)
        lab = mix.rgb_to_lab(rgb)
        _, idx = self.tree.query(lab.reshape(-1, 3))
        target = self.lat[idx].reshape(rgb.shape[:-1] + (7,))
        src = mix.rgb_to_latent(rgb)
        return mix.latent_to_rgb(src + amount * (target - src))


class Planner:
    def __init__(self, guides, styles, layers, seed=1907, proxy=None, ground_rgb=None, verbose=False):
        self.g = guides
        self.styles = {k: (v if isinstance(v, Style) else Style(**v)) for k, v in styles.items()}
        self.layers = [l if isinstance(l, Layer) else Layer(**l) for l in layers]
        self.seed = int(seed)
        self.verbose = verbose
        W, H = guides.W, guides.H
        self.proxy = proxy or Canvas(W, H, ground_rgb if ground_rgb is not None else (0.96, 0.94, 0.90))
        self.proxy.set_region(guides.region_id)
        self.palettes = {}
        self.warm_lat = {}
        self.strokes = []
        self.layer_ranges = []   # (start, end) indices into self.strokes per layer

    # ------------------------------------------------------------ helpers
    def _style(self, name):
        return self.styles.get(name, self.styles.get("default", Style()))

    def _palette(self, name):
        if name not in self.palettes:
            st = self._style(name)
            self.palettes[name] = Palette(st["colors"] or [])
        return self.palettes[name]

    def _regions_of(self, layer):
        regs = layer["regions"]
        if regs == "all":
            return list(range(len(self.g.names)))
        return [self.g.index(r) for r in regs]

    def _flow_at(self, x, y, rid=None):
        f = self.g.flow
        if rid is not None:
            f = self.g.region_flows.get(self.g.names[rid], f)
        xi = min(max(int(x), 0), self.g.W - 1); yi = min(max(int(y), 0), self.g.H - 1)
        return f[yi, xi]

    def _mask_at(self, name_idx, x, y):
        m = self.g.masks[self.g.names[name_idx]]
        xi = min(max(int(x), 0), self.g.W - 1); yi = min(max(int(y), 0), self.g.H - 1)
        return m[yi, xi]

    def _path(self, x0, y0, width_px, length_px, st, rid, rng, ignore_mask=False):
        """Follow the flow field from (x0, y0). Returns (n, 2) pixel points."""
        step = max(1.0, 0.5 * width_px)
        n = max(3, int(length_px / step) + 1)
        pts = [(x0, y0)]
        d = self._flow_at(x0, y0, rid).copy()
        if np.linalg.norm(d) < 1e-3:
            d = np.array([1.0, 0.0], np.float32)
        d = d / (np.linalg.norm(d) + 1e-9)
        # per-stroke angle jitter, more when align is low
        jit = (1 - st["align"]) * rng.normal(0, 0.7)
        c, s_ = math.cos(jit), math.sin(jit)
        d = np.array([c * d[0] - s_ * d[1], s_ * d[0] + c * d[1]], np.float32)
        if rng.random() < st["reverse_p"]:
            d = -d
        fc = float(st["curvature"])
        # back up so that the stroke's full-pressure part passes over the site (the ramp-in deposits little)
        back = rng.uniform(0.5, 1.0) * width_px
        x0, y0 = x0 - d[0] * back, y0 - d[1] * back
        n += int(back / step) + 1
        pts = [(x0, y0)]
        x, y = x0, y0
        W, H = self.g.W, self.g.H
        for i in range(1, n):
            f = self._flow_at(x, y, rid)
            if np.dot(f, d) < 0:
                f = -f
            nd = (1 - fc) * d + fc * f
            nd = nd / (np.linalg.norm(nd) + 1e-9)
            # small wobble
            wob = rng.normal(0, 0.08 * (1 - st["align"]) + 0.02)
            c, s_ = math.cos(wob), math.sin(wob)
            d = np.array([c * nd[0] - s_ * nd[1], s_ * nd[0] + c * nd[1]], np.float32)
            x, y = x + d[0] * step, y + d[1] * step
            if x < -width_px or y < -width_px or x > W + width_px or y > H + width_px:
                break
            if not ignore_mask and self._mask_at(rid, x, y) < rng.random() * st["stop_at_edge"]:
                break
            pts.append((x, y))
        return np.array(pts, np.float32)

    def _edge_start(self, x, y, width, rng, rid=None):
        W, H = self.g.W, self.g.H
        f = self._flow_at(x, y, rid)
        m = 1.2 * width
        if x < m and f[0] > 0.2:
            x = x - rng.uniform(0.5, 1.0) * width - x * rng.random()
        elif x > W - m and f[0] < -0.2:
            x = x + rng.uniform(0.5, 1.0) * width + (W - x) * rng.random()
        if y < m and f[1] > 0.2:
            y = y - rng.uniform(0.5, 1.0) * width - y * rng.random()
        elif y > H - m and f[1] < -0.2:
            y = y + rng.uniform(0.5, 1.0) * width + (H - y) * rng.random()
        return x, y

    def _color(self, ref, x, y, rpx, st, rid, layer, rng):
        """Colour for a stroke starting at (x, y): reference sample -> palette snap -> jitter -> fleck -> warmth."""
        H, W = ref.shape[:2]
        r = max(1, int(rpx))
        x = min(max(x, 0), W - 1); y = min(max(y, 0), H - 1)
        x0, x1 = max(0, int(x) - r), min(W, int(x) + r + 1); y0, y1 = max(0, int(y) - r), min(H, int(y) + r + 1)
        if layer["color_from"] == "palette" and st["colors"]:
            rgb = _rgb_spec(st["colors"][rng.integers(len(st["colors"]))])
        else:
            rgb = ref[y0:y1, x0:x1].reshape(-1, 3).mean(0)
        pal = self._palette(self.g.names[rid])
        if st["colors"] and st["snap"] > 0:
            rgb = pal.snap(rgb, st["snap"])
        jl, jab = st["jitter"]
        lab = mix.rgb_to_lab(rgb) + np.array([rng.normal(0, jl), rng.normal(0, jab), rng.normal(0, jab)], np.float32)
        lab[0] = max(float(lab[0]), float(st["L_floor"]))
        rgb = mix.lab_to_rgb(lab)
        for spec, p in st["flecks"]:
            if rng.random() < p:
                rgb = mix.mix_rgb(rgb, _rgb_spec(spec), 0.75)
                break
        if st["warmth"] > 0:
            lm = self.g.light[min(max(int(y), 0), H - 1), min(max(int(x), 0), W - 1)]
            if lm > 0.02:
                rgb = mix.mix_rgb(rgb, _rgb_spec(st["warm_color"]), float(np.clip(lm * st["warmth"], 0, 0.8)))
        return np.asarray(rgb, np.float32)

    def _end_profile(self, st):
        return dict(end_width=st["end_width"], end_pressure=st["end_pressure"])

    def _opacity_range(self, st):
        o = st["opacity"]
        return tuple(o) if isinstance(o, (tuple, list)) else (o, o)

    def _nb(self, st, width_px, Wp):
        return int(max(3, min(40, st["nb_base"] + st["nb_per_cw"] * width_px / Wp)))

    def _emit(self, li, layer, st, r, ref, rpx, params, x, y, width, length, rng, pressure_floor=0.0, ignore_mask=None):
        """Build, render on the proxy and store one stroke starting at plan-pixel (x, y). Returns the stroke or None."""
        Wp = self.g.W
        if st["size_by_y"] is not None:
            y0, y1, s0, s1 = st["size_by_y"]
            sc = float(np.interp(y / Wp, [y0, y1], [s0, s1]))
            width *= sc; length *= sc
        sx, sy = self._edge_start(x, y, width, rng, r)
        if ignore_mask is None:
            ignore_mask = rng.random() < st["spill"]      # `spill`: fraction of strokes that run into neighbours
        path = self._path(sx, sy, width, length, st, r, rng, ignore_mask=ignore_mask)
        if len(path) < 2:
            return None
        seg = np.hypot(np.diff(path[:, 0]), np.diff(path[:, 1]))
        sarr = np.concatenate([[0], np.cumsum(seg)])
        wf, pr = profile(sarr / max(sarr[-1], 1e-6), **self._end_profile(st))
        pr = np.maximum(pr, pressure_floor)
        pts4 = np.stack([path[:, 0] / Wp, path[:, 1] / Wp, width * wf / Wp, pr], 1).astype(np.float32)
        rgb = self._color(ref, x, y, rpx, st, r, layer, rng)
        z = mix.rgb_to_latent(rgb)
        z2 = None
        if st["marble"] > 0 or st["load2"] is not None:
            if st["load2"] is not None:
                z2 = mix.rgb_to_latent(mix.color_spec_to_rgb(st["load2"]))
            else:   # automatic second colour: lighter and warmer, or darker and cooler (per stroke)
                lab = mix.rgb_to_lab(rgb)
                if rng.random() < 0.6:
                    lab = lab + np.array([9.0, 3.0, 8.0], np.float32)
                else:
                    lab = lab + np.array([-7.0, 1.0, -6.0], np.float32)
                z2 = mix.rgb_to_latent(mix.lab_to_rgb(lab))
        p = dict(params)
        p["opacity"] = rng.uniform(*self._opacity_range(st))
        if st["opacity_by_light"] > 0:
            H = self.g.H
            lm = self.g.light[min(max(int(y), 0), H - 1), min(max(int(x), 0), Wp - 1)]
            p["opacity"] *= max(0.05, 1 - st["opacity_by_light"] * (1 - lm))
        p["nb"] = self._nb(st, width, Wp)
        p["hgain"] = p.get("hgain", st["hgain"]) * layer["relief"] * rng.uniform(1 - st["hgain_jitter"], 1 + st["hgain_jitter"])
        stk = Stroke(pts4, z, streak_vector(z, 1.0), p, seed=int(rng.integers(1, 2 ** 31 - 1)), layer=li, region=r, zcol2=z2)
        self.proxy.render_stroke(stk)
        self.strokes.append(stk)
        return stk

    def _curve_points(self, layer, wpx, region_mask, rng, region_name=None):
        """Start points along a curve: either an explicit list of (x, y) in canvas-width units (layer['curve'])
        or, when curve == 'boundary', the boundary of the region mask.  Spacing in stroke widths."""
        Wp, H = self.g.W, self.g.H
        curve = layer["curve"]
        if isinstance(curve, dict):
            curve = curve.get(region_name, "boundary")
        if curve == "boundary":
            m8 = (region_mask > 0.5).astype(np.uint8)
            cs, _ = cv2.findContours(m8, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_NONE)
            pts_px = np.concatenate([c.reshape(-1, 2) for c in cs]) if cs else np.zeros((0, 2))
        else:
            arr = np.asarray(curve, np.float32) * Wp
            # resample the polyline densely
            out = []
            for a, b in zip(arr[:-1], arr[1:]):
                n = max(2, int(np.hypot(*(b - a)) / 2))
                out.append(np.linspace(a, b, n, endpoint=False))
            pts_px = np.concatenate(out) if out else arr
        if len(pts_px) == 0:
            return []
        step = max(1.0, layer["curve_spacing"] * wpx)
        # walk the point list by arc length
        d = np.concatenate([[0], np.cumsum(np.hypot(*np.diff(pts_px, axis=0).T))]) if len(pts_px) > 1 else np.zeros(1)
        targets = np.arange(0, d[-1], step) + rng.uniform(0, step)
        idx = np.searchsorted(d, targets).clip(0, len(pts_px) - 1)
        pts = []
        for i in idx:
            x, y = pts_px[i]
            # normal offset (outward from the region for boundary curves)
            j = min(i + 3, len(pts_px) - 1); k = max(i - 3, 0)
            tx, ty = pts_px[j] - pts_px[k]; nrm = np.hypot(tx, ty) + 1e-6
            nx, ny = -ty / nrm, tx / nrm
            off = (layer["curve_offset"] + rng.uniform(-1, 1) * layer["curve_jitter"]) * wpx
            pts.append((float(x + nx * off), float(y + ny * off), 1.0))
        return pts

    def _params(self, st, layer, allow_mask):
        p = dict(mode=st["mode"] or layer["mode"], pickup=st["pickup"], load=st["load"], deplete=st["deplete"], vdry=st["vdry"],
                 hgain=st["hgain"], flatten=st["flatten"], streak=st["streak"], streak_mix=st["streak_mix"], body=st["body"],
                 hardness=st["hardness"], grain=st["grain"], release=st["release"], dropout=st["dropout"],
                 ragged=st["ragged"], ridge=st["ridge"], levee=st["levee"], furrow=st["furrow"], blob=st["blob"],
                 stiff=st["stiff"], marble=st["marble"], splay=st["splay"], allow_mask=0xFFFFFFFF, override_p=1.0)
        for k in ("pickup", "load", "deplete", "vdry", "hgain", "flatten", "streak", "streak_mix", "body", "hardness",
                  "grain", "release", "dropout", "ragged", "dry_thresh", "dry_width", "opacity", "ridge", "levee",
                  "furrow", "blob", "stiff", "marble", "splay"):
            if k in layer:
                p[k] = layer[k]
        return p

    # ------------------------------------------------------------ placement
    def _error_cells(self, layer, ref, rpx, region_mask, rng):
        E = np.linalg.norm(mix.rgb_to_lab(self.proxy.rgb) - mix.rgb_to_lab(ref), axis=-1)
        E = E * region_mask
        g = max(2, int(layer["fg"] * rpx))
        H, W = E.shape
        gh, gw = max(1, H // g), max(1, W // g)
        cell = cv2.resize(E, (gw, gh), interpolation=cv2.INTER_AREA)
        # max-pooled error so thin gaps (a sliver of bare ground) also trigger a stroke
        Ep = E[:gh * g, :gw * g].reshape(gh, g, gw, g).max(axis=(1, 3))
        cell = 0.6 * cell + 0.4 * Ep
        cover = cv2.resize(region_mask, (gw, gh), interpolation=cv2.INTER_AREA)
        ys, xs = np.where((cell > layer["T"]) & (cover > 0.3))
        pts = []
        for cy, cx in zip(ys, xs):
            y0, x0 = cy * g, cx * g
            patch = E[y0:y0 + g, x0:x0 + g]
            if patch.size == 0:
                continue
            iy, ix = np.unravel_index(np.argmax(patch), patch.shape)
            jx, jy = rng.uniform(-0.5, 0.5, 2) * g * layer["jitter_pos"]
            pts.append((x0 + ix + jx, y0 + iy + jy, float(cell[cy, cx])))
        return pts

    def _density_points(self, layer, wpx, region_mask, rng, region_name=None):
        sp = layer["spacing"]
        if isinstance(sp, dict):
            sp = sp.get(region_name, 1.6)
        g = max(2, int(sp * wpx))
        H, W = region_mask.shape
        pts = []
        for y0 in range(0, H, g):
            for x0 in range(0, W, g):
                x = x0 + rng.uniform(0, g); y = y0 + rng.uniform(0, g)
                xi, yi = min(int(x), W - 1), min(int(y), H - 1)
                if region_mask[yi, xi] > rng.random():
                    pts.append((x, y, 1.0))
        return pts

    def _order(self, pts, layer, rng, wpx):
        if not pts:
            return pts
        if layer["order"] == "random":
            rng.shuffle(pts); return pts
        # painterly sweep: bands top to bottom, alternating direction, with jitter
        band = max(4.0, 3.0 * wpx)
        keyed = []
        for (x, y, e) in pts:
            b = int((y + rng.uniform(-0.3, 0.3) * band) / band)
            kx = x if b % 2 == 0 else -x
            keyed.append((b, kx + rng.uniform(-band, band), (x, y, e)))
        keyed.sort(key=lambda k: (k[0], k[1]))
        return [k[2] for k in keyed]

    # ------------------------------------------------------------ main
    def plan_layer(self, li, layer, progress=None):
        rng = np.random.default_rng(self.seed * 1000 + li * 17 + layer.get("seed_offset", 0))
        regs = self._regions_of(layer)
        start = len(self.strokes)
        Wp = self.g.W
        allow_mask = 0
        for r in regs:
            allow_mask |= 1 << (r & 31)
        # per-region masks, but strokes are placed over the union so that painterly order interleaves regions by priority
        for r in sorted(regs, key=lambda r: self._style(self.g.names[r])["priority"]):
            name = self.g.names[r]
            st = Style(**self._style(name))
            for k in ("width", "length", "curvature", "align", "opacity", "colors", "flecks", "snap", "warmth", "jitter",
                      "end_width", "end_pressure", "spill", "min_aspect", "reverse_p", "stop_at_edge", "hgain_jitter",
                      "size_by_y", "opacity_by_light", "pickup", "hgain", "streak", "body", "load", "L_floor",
                      "ridge", "levee", "furrow", "blob", "stiff", "marble", "load2", "splay", "hardness", "release"):
                if k in layer:
                    st[k] = layer[k]
            wlo, whi = st["width"]; llo, lhi = st["length"]
            wmean_px = 0.5 * (wlo + whi) * Wp
            rpx = 0.5 * wmean_px
            ref = self.g.reference(layer["fs"] * rpx)
            m = self.g.masks[name]
            if layer["placement"] == "error":
                pts = self._error_cells(layer, ref, rpx, m, rng)
            elif layer["placement"] == "curve":
                pts = self._curve_points(layer, wmean_px, m, rng, name)
            else:
                pts = self._density_points(layer, wmean_px, m, rng, name)
            cov = layer["coverage"]
            if isinstance(cov, dict):
                cov = cov.get(name, 1.0)
            if cov < 1.0:
                keep = rng.random(len(pts)) < cov
                pts = [p for p, k in zip(pts, keep) if k]
            pts = self._order(pts, layer, rng, wmean_px)
            if layer["max_strokes"]:
                pts = pts[:layer["max_strokes"]]
            params = self._params(st, layer, allow_mask)
            if layer["mode"] == "scumble" or st["mode"] == "scumble":
                sig = max(1.0, layer["hblur_sigma"] * Wp)
                self.proxy.set_hblur(cv2.GaussianBlur(self.proxy.h, (0, 0), sig))
            n_emitted = 0
            for (x, y, e) in pts:
                xi, yi = min(max(int(x), 0), Wp - 1), min(max(int(y), 0), self.g.H - 1)
                if layer["placement"] == "error":
                    # re-check the error here now that earlier strokes of this layer have been painted
                    d = np.linalg.norm(mix.rgb_to_lab(self.proxy.rgb[yi, xi]) - mix.rgb_to_lab(ref[yi, xi]))
                    if d < 0.5 * layer["T"]:
                        continue
                if layer["max_cover"] is not None and self.proxy.cover[yi, xi] > layer["max_cover"]:
                    continue     # leave the lower layers / ground visible here
                width = rng.uniform(wlo, whi) * Wp
                length = rng.uniform(llo, lhi) * Wp
                if not layer["dab"]:
                    length = max(length, st["min_aspect"] * width)
                if self._emit(li, layer, st, r, ref, rpx, params, x, y, width, length, rng) is not None:
                    n_emitted += 1
            # gap fill: uncovered pixels inside the region get an extra stroke (bare ground only where wanted)
            if layer["placement"] == "error" and layer.get("gap_fill", True):
                bare = (self.proxy.cover < layer.get("gap_cover", 0.15)) & (m > 0.5)
                gg = max(2, int(0.5 * layer["fg"] * rpx))
                ys, xs = np.where(bare[::gg, ::gg])
                n_gap = 0
                for cy, cx in zip(ys * gg, xs * gg):
                    if self.proxy.cover[cy, cx] >= layer.get("gap_cover", 0.15):
                        continue
                    width = rng.uniform(wlo, whi) * Wp * 0.8
                    length = max(rng.uniform(llo, lhi) * Wp * 0.7, 2.0 * width)
                    if self._emit(li, layer, st, r, ref, rpx, params, cx + rng.uniform(-1, 1), cy + rng.uniform(-1, 1),
                                  width, length, rng, pressure_floor=0.6, ignore_mask=False) is not None:
                        n_emitted += 1; n_gap += 1
                if self.verbose and n_gap:
                    print(f"    layer {li+1} gap fill in {name}: {n_gap} strokes")
            if self.verbose:
                print(f"    layer {li+1} {layer['name']!r} region {name}: {len(pts)} sites -> {n_emitted} strokes")
        if layer["dry_after"] is not None:
            self.proxy.dry(layer["dry_after"])
        self.layer_ranges.append((start, len(self.strokes)))
        return self.strokes[start:]

    def plan(self, upto=None, layer_hook=None):
        for li, layer in enumerate(self.layers):
            if upto is not None and li >= upto:
                break
            if not layer.get("enabled", True):
                self.layer_ranges.append((len(self.strokes), len(self.strokes)))
                continue
            self.plan_layer(li, layer)
            if layer_hook:
                layer_hook(li, layer, self)
        return self.strokes
