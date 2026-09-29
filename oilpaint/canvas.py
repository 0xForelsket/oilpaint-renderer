"""Canvas state and stroke replay through the C kernel.

Strokes are stored in canvas-width units: x, y, width all divided by the canvas width W, so a stroke list
replays at any resolution.  `Canvas.render(strokes)` scales them to pixels and calls the kernel.
"""
import ctypes
import numpy as np

from . import mix
from ._build import BrushParams, CCanvas, OchrellCanvas, build_and_load

MODES = {"paint": 0, "scumble": 1, "smudge": 2, "glaze": 3}

BRUSH_DEFAULTS = dict(
    mode="paint", opacity=0.95, pickup=0.12, load=1.0, deplete=0.02, vdry=0.25, hgain=1.0, flatten=0.6,
    streak=0.25, hardness=0.75, grain=0.10, dry_thresh=0.0, dry_width=0.15, nb=14, allow_mask=0xFFFFFFFF,
    override_p=1.0, dropout=0.02, ragged=0.5, body=0.9, release=0.3, streak_mix=0.8,
    ridge=0.5, levee=0.35, furrow=0.15, blob=0.3, stiff=0.25, marble=0.0, splay=1.0,
)


def make_params(seed, **kw):
    p = dict(BRUSH_DEFAULTS)
    p.update({k: v for k, v in kw.items() if k in p})
    bp = BrushParams()
    bp.mode = MODES[p["mode"]] if isinstance(p["mode"], str) else int(p["mode"])
    for k in ("opacity", "pickup", "load", "deplete", "vdry", "hgain", "flatten", "streak", "hardness", "grain",
              "dry_thresh", "dry_width", "override_p", "dropout", "ragged", "body", "release", "streak_mix",
              "ridge", "levee", "furrow", "blob", "stiff", "marble", "splay"):
        setattr(bp, k, float(p[k]))
    bp.nb = int(p["nb"])
    bp.allow_mask = int(p["allow_mask"]) & 0xFFFFFFFF
    bp.seed = int(seed) & 0xFFFFFFFF
    return bp


class Stroke:
    """pts: (n, 4) float32 in width units [x, y, w, pressure]; mixer-sized zcol/zcol2/dz; params: dict.
    zcol2 is the second colour on the brush (two-colour load, used by `marble`); defaults to zcol."""
    __slots__ = ("pts", "zcol", "zcol2", "dz", "params", "seed", "layer", "region", "backend")

    def __init__(self, pts, zcol, dz=None, params=None, seed=0, layer=0, region=0, zcol2=None):
        self.pts = np.ascontiguousarray(pts, np.float32)
        self.zcol = np.ascontiguousarray(zcol, np.float32)
        self.zcol2 = self.zcol.copy() if zcol2 is None else np.ascontiguousarray(zcol2, np.float32)
        self.dz = np.zeros(mix.LAT, np.float32) if dz is None else np.ascontiguousarray(dz, np.float32)
        self.backend = mix.backend()
        self.params = dict(params or {})
        self.seed = int(seed)
        self.layer = int(layer)
        self.region = int(region)

    def validate(self, backend):
        if self.backend != backend:
            raise ValueError(f"stroke uses {self.backend}, canvas uses {backend}; rebuild strokes from RGB inputs")
        nlat = 85 if mix.is_material(backend) else 7
        if self.pts.ndim != 2 or self.pts.shape[1] != 4 or len(self.pts) < 2:
            raise ValueError("stroke points must have shape (n>=2,4)")
        for a in (self.pts, self.zcol, self.zcol2, self.dz):
            if a.dtype != np.float32 or not a.flags.c_contiguous or not np.isfinite(a).all():
                raise ValueError("stroke arrays must be contiguous finite float32")
        if any(a.shape != (nlat,) for a in (self.zcol, self.zcol2, self.dz)):
            raise ValueError("stroke state width does not match mixer")
        if np.any(self.pts[:, 2] <= 0) or np.any(self.pts[:, 3] < 0) or np.any(self.pts[:, 3] > 1):
            raise ValueError("width must be positive and pressure in [0,1]")
        p = dict(BRUSH_DEFAULTS)
        p.update({k: v for k, v in self.params.items() if k in p})
        if p["mode"] not in MODES and p["mode"] not in range(4):
            raise ValueError("invalid brush mode")
        for k in ("opacity", "pickup", "marble", "release", "flatten", "body", "hardness", "dropout"):
            if not np.isfinite(p[k]) or not 0 <= p[k] <= 1:
                raise ValueError(f"{k} must be in [0,1]")
        for k, v in p.items():
            if k != "mode" and not np.isfinite(v):
                raise ValueError(f"{k} must be finite")
        if p["vdry"] <= 0 or p["dry_width"] <= 0 or p["load"] < 0 or p["deplete"] < 0 or not 0 <= p["splay"] <= 3:
            raise ValueError("invalid load/depletion, dry width or splay")
        if mix.is_material(backend):
            from .ochrell import validate_state
            validate_state(self.zcol, backend); validate_state(self.zcol2, backend)


class Canvas:
    def __init__(self, W, H, ground_rgb=(0.96, 0.94, 0.90), weave=None):
        self.W, self.H = int(W), int(H)
        if self.W <= 0 or self.H <= 0:
            raise ValueError("canvas dimensions must be positive")
        self.backend = mix.backend()
        self.lib = build_and_load(self.backend)
        n = self.W * self.H
        g = mix.rgb_to_latent(np.asarray(ground_rgb, np.float32))
        self.lat = np.broadcast_to(g, (self.H, self.W, mix.LAT)).copy()
        self.rgb = np.tile(mix.latent_to_rgb(g), (n, 1)).astype(np.float32).reshape(self.H, self.W, 3)
        self.h = np.zeros((self.H, self.W), np.float32) if weave is None else np.ascontiguousarray(weave, np.float32).copy()
        if self.h.shape != (self.H, self.W) or not np.isfinite(self.h).all():
            raise ValueError("weave must be a finite canvas-sized height array")
        self.wet = np.zeros((self.H, self.W), np.float32)
        self.cover = np.zeros((self.H, self.W), np.float32)
        self.hblur = np.zeros((self.H, self.W), np.float32)
        self.region = np.zeros((self.H, self.W), np.uint8)
        self.amount = np.zeros((self.H, self.W), np.float32) if mix.is_material(self.backend) else None
        self._cc = OchrellCanvas() if self.amount is not None else CCanvas()
        self._bind()

    def _bind(self):
        fp = ctypes.POINTER(ctypes.c_float)
        c = self._cc
        c.W, c.H = self.W, self.H
        for name in ("lat", "rgb", "h", "wet", "cover", "hblur"):
            arr = getattr(self, name)
            assert arr.flags["C_CONTIGUOUS"] and arr.dtype == np.float32
            setattr(c, name, arr.ctypes.data_as(fp))
        assert self.region.flags["C_CONTIGUOUS"]
        c.region = self.region.ctypes.data_as(ctypes.POINTER(ctypes.c_uint8))
        if self.amount is not None:
            from .ochrell import MODES as MIXING_MODES
            c.amount = self.amount.ctypes.data_as(fp)
            c.mixing_mode = MIXING_MODES[self.backend]

    def set_region(self, region_u8):
        region = np.ascontiguousarray(region_u8, np.uint8)
        if region.shape != (self.H, self.W):
            raise ValueError("region shape mismatch")
        self.region = region
        self._bind()

    def set_hblur(self, hblur):
        hblur = np.ascontiguousarray(hblur, np.float32)
        if hblur.shape != (self.H, self.W) or not np.isfinite(hblur).all():
            raise ValueError("blurred height shape mismatch")
        self.hblur = hblur
        self._bind()

    def dry(self, factor):
        if not np.isfinite(factor) or not 0 <= factor <= 1:
            raise ValueError("dry factor must be in [0,1]")
        self.wet *= float(factor)

    def render_stroke(self, st: Stroke, stats=False):
        st.validate(self.backend)
        pts = st.pts.copy()
        pts[:, :3] *= self.W
        bp = make_params(st.seed, **st.params)
        fp = ctypes.POINTER(ctypes.c_float)
        out = np.zeros(3, np.float32)
        n = self.lib.render_stroke(ctypes.byref(self._cc), pts.ctypes.data_as(fp), pts.shape[0],
                                   st.zcol.ctypes.data_as(fp), st.zcol2.ctypes.data_as(fp), st.dz.ctypes.data_as(fp),
                                   ctypes.byref(bp), out.ctypes.data_as(fp) if stats else None)
        if n < 0:
            raise RuntimeError("native brush failed; discard this partially painted canvas")
        return (n, out) if stats else n

    def render(self, strokes, progress=None, every=0):
        """Render a list of strokes in order. If progress is given it is called as progress(i, n) every `every` strokes."""
        if not strokes:
            return 0
        for st in strokes:
            st.validate(self.backend)
        # batch through render_strokes to avoid per-call overhead
        pts_all = np.concatenate([s.pts for s in strokes]).astype(np.float32)
        pts_all[:, :3] *= self.W
        offsets = np.zeros(len(strokes) + 1, np.int32)
        offsets[1:] = np.cumsum([s.pts.shape[0] for s in strokes])
        zc = np.stack([s.zcol for s in strokes]).astype(np.float32)
        zc2 = np.stack([s.zcol2 for s in strokes]).astype(np.float32)
        dz = np.stack([s.dz for s in strokes]).astype(np.float32)
        params = (BrushParams * len(strokes))()
        for i, s in enumerate(strokes):
            params[i] = make_params(s.seed, **s.params)
        fp = ctypes.POINTER(ctypes.c_float)
        total = 0
        if progress is None or every <= 0:
            count = self.lib.render_strokes(ctypes.byref(self._cc), pts_all.ctypes.data_as(fp),
                                           offsets.ctypes.data_as(ctypes.POINTER(ctypes.c_int)), len(strokes),
                                           zc.ctypes.data_as(fp), zc2.ctypes.data_as(fp), dz.ctypes.data_as(fp), params)
            if count < 0:
                raise RuntimeError("native brush failed; discard this partially painted canvas")
            return count
        i = 0
        n = len(strokes)
        while i < n:
            j = min(n, i + every)
            sub_off = (offsets[i:j + 1] - offsets[i]).astype(np.int32)
            sub_pts = np.ascontiguousarray(pts_all[offsets[i]:offsets[j]])
            sub_params = (BrushParams * (j - i))()
            for k in range(i, j):
                sub_params[k - i] = params[k]
            count = self.lib.render_strokes(ctypes.byref(self._cc), sub_pts.ctypes.data_as(fp),
                                             sub_off.ctypes.data_as(ctypes.POINTER(ctypes.c_int)), j - i,
                                             np.ascontiguousarray(zc[i:j]).ctypes.data_as(fp),
                                             np.ascontiguousarray(zc2[i:j]).ctypes.data_as(fp),
                                             np.ascontiguousarray(dz[i:j]).ctypes.data_as(fp), sub_params)
            if count < 0:
                raise RuntimeError("native brush failed; discard this partially painted canvas")
            total += count
            progress(j, n)
            i = j
        return total

    def rgb8(self):
        return (np.clip(self.rgb, 0, 1) * 255 + 0.5).astype(np.uint8)
