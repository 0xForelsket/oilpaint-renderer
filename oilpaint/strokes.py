"""Stroke geometry helpers shared by the planner and the tests (all in canvas-width units)."""
import numpy as np

from . import mix
from .canvas import Stroke


def smoothstep(e0, e1, x):
    t = np.clip((x - e0) / (e1 - e0), 0, 1)
    return t * t * (3 - 2 * t)


def profile(t, start=0.12, end=0.20, end_width=0.5, end_pressure=0.15):
    """Width factor and pressure along a stroke parameter t in [0,1].
    A flat brush keeps its width; the start rounds in as the brush lands, the end loses pressure (and some
    width) as the paint runs out."""
    t = np.asarray(t, np.float32)
    wf = (0.35 + 0.65 * smoothstep(0, start, t)) * (1 - (1 - end_width) * smoothstep(1 - end, 1, t))
    pr = smoothstep(0, 0.05, t) * (1 - (1 - end_pressure) * smoothstep(1 - end, 1, t))
    return wf.astype(np.float32), pr.astype(np.float32)


def polyline(xs, ys, width, pressure_scale=1.0, taper=True):
    xs = np.asarray(xs, np.float32); ys = np.asarray(ys, np.float32)
    n = len(xs)
    seg = np.hypot(np.diff(xs), np.diff(ys))
    s = np.concatenate([[0], np.cumsum(seg)])
    t = s / max(s[-1], 1e-9)
    if taper:
        wf, pr = profile(t)
    else:
        wf, pr = np.ones(n, np.float32), np.ones(n, np.float32)
    return np.stack([xs, ys, width * wf, pr * pressure_scale], 1).astype(np.float32)


def streak_vector(zcol, amount=1.0, rng=None):
    """A latent direction for per-bristle colour variation: toward a slightly lighter/warmer and a slightly
    darker/cooler version of the colour.  Returned dz is the half-range; bristles use zcol + t*dz, t in [-1,1]."""
    rgb = mix.latent_to_rgb(zcol)
    light = np.clip(rgb * 1.10 + np.array([0.05, 0.035, 0.0], np.float32), 0, 1)
    dark = np.clip(rgb * 0.88 - np.array([0.0, 0.01, -0.02], np.float32), 0, 1)
    zl, zd = mix.rgb_to_latent(light), mix.rgb_to_latent(dark)
    return (0.5 * (zl - zd) * amount).astype(np.float32)


def straight(x0, y0, x1, y1, width, n=None, **params):
    """Convenience for tests: a straight stroke in width units."""
    L = float(np.hypot(x1 - x0, y1 - y0))
    if n is None:
        n = max(3, int(L / (0.35 * width)) + 1)
    t = np.linspace(0, 1, n)
    return polyline(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, width)


def arc(cx, cy, r, a0, a1, width, n=None):
    L = abs(a1 - a0) * r
    if n is None:
        n = max(4, int(L / (0.35 * width)) + 1)
    a = np.linspace(a0, a1, n)
    return polyline(cx + r * np.cos(a), cy + r * np.sin(a), width)


def make_stroke(pts, color, seed, streak_amount=1.0, color2=None, **params):
    """color: rgb triple / tube name / hex; color2: optional second colour on the brush; returns a Stroke."""
    rgb = mix.color_spec_to_rgb(color) if not isinstance(color, np.ndarray) else color
    z = mix.rgb_to_latent(np.asarray(rgb, np.float32))
    dz = streak_vector(z, streak_amount)
    z2 = None
    if color2 is not None:
        rgb2 = mix.color_spec_to_rgb(color2) if not isinstance(color2, np.ndarray) else color2
        z2 = mix.rgb_to_latent(np.asarray(rgb2, np.float32))
    return Stroke(pts, z, dz, params, seed, zcol2=z2)
