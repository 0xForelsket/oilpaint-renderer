"""Pigment mixing: vectorised Mixbox 2.0 (CC BY-NC 4.0, (c) Secret Weapons, https://scrtwpns.com/mixbox),
an `rgb` backend that rides on the same latent layout, Lab conversion and the tube palette.

Latent layout (7 floats): c0..c3 pigment concentrations (c3 = 1 - c0 - c1 - c2) and an RGB residual.
Mixing is linear interpolation in latent space.  The `rgb` backend stores rgb in the residual with zero
concentrations, so the same kernel does plain RGB lerps: that is the A/B baseline.
"""
import numpy as np

try:
    import mixbox as _mixbox_ref
    _LUT = np.frombuffer(bytes(_mixbox_ref._lut), dtype=np.uint8).astype(np.float32)
except ImportError as e:  # pragma: no cover
    raise ImportError("pymixbox is required: pip install --break-system-packages pymixbox") from e

LAT = 7
_MODE = {"backend": "mixbox"}

COEF = np.array([
    [0.07717053, 0.02826978, 0.24832992], [0.95912302, 0.80256528, 0.03561839], [0.74683774, 0.04868586, 0.0],
    [0.99518138, 0.99978149, 0.99704802], [0.04819146, 0.83363781, 0.32515377], [-0.68146950, 1.46107803, 1.06980936],
    [0.27058419, -0.15324870, 1.98735057], [0.80478189, 0.67093710, 0.18424500], [-0.35031003, 1.37855826, 3.68865000],
    [1.05128046, 1.97815239, 2.82989073], [3.21607125, 0.81270228, 1.03384539], [2.78893374, 0.41565549, -0.04487295],
    [3.02162577, 2.55374103, 0.32766114], [2.95124691, 2.81201112, 1.17578442], [2.82677043, 0.79933038, 1.81715262],
    [2.99691099, 1.22593053, 1.80653661], [1.87394106, 2.05027182, -0.29835996], [2.56609566, 7.03428198, 0.62575374],
    [4.08329484, -1.40408358, 2.14995522], [6.00078678, 2.55552042, 1.90739502]], np.float32)


def set_backend(name):
    assert name in ("mixbox", "rgb")
    _MODE["backend"] = name


def backend():
    return _MODE["backend"]


def _eval_poly(c0, c1, c2, c3):
    c00, c11, c22, c33 = c0 * c0, c1 * c1, c2 * c2, c3 * c3
    c01, c02, c12 = c0 * c1, c0 * c2, c1 * c2
    terms = np.stack([c0 * c00, c1 * c11, c2 * c22, c3 * c33, c00 * c1, c01 * c1, c00 * c2, c02 * c2, c00 * c3,
                      c0 * c33, c11 * c2, c1 * c22, c11 * c3, c1 * c33, c22 * c3, c2 * c33, c01 * c2, c01 * c3,
                      c02 * c3, c12 * c3], -1)
    return terms @ COEF


def rgb_to_latent(rgb):
    """rgb float array (..., 3) in 0..1 -> latent (..., 7)."""
    rgb = np.clip(np.asarray(rgb, np.float32), 0, 1)
    if _MODE["backend"] == "rgb":
        out = np.zeros(rgb.shape[:-1] + (LAT,), np.float32)
        out[..., 4:7] = rgb
        return out
    x, y, z = rgb[..., 0] * 63, rgb[..., 1] * 63, rgb[..., 2] * 63
    ix = np.minimum(x.astype(np.int32), 62); iy = np.minimum(y.astype(np.int32), 62); iz = np.minimum(z.astype(np.int32), 62)
    tx, ty, tz = x - ix, y - iy, z - iz
    base = ix + iy * 64 + iz * 4096
    c = np.zeros(rgb.shape[:-1] + (3,), np.float32)
    for dx, dy, dz in [(0, 0, 0), (1, 0, 0), (0, 1, 0), (1, 1, 0), (0, 0, 1), (1, 0, 1), (0, 1, 1), (1, 1, 1)]:
        w = (tx if dx else 1 - tx) * (ty if dy else 1 - ty) * (tz if dz else 1 - tz)
        idx = base + dx + dy * 64 + dz * 4096
        c[..., 0] += w * _LUT[idx + 192]; c[..., 1] += w * _LUT[idx + 262336]; c[..., 2] += w * _LUT[idx + 524480]
    c /= 255.0
    c3 = 1 - c.sum(-1)
    mix = _eval_poly(c[..., 0], c[..., 1], c[..., 2], c3)
    return np.concatenate([c, c3[..., None], rgb - mix], -1).astype(np.float32)


def latent_to_rgb(lat):
    lat = np.asarray(lat, np.float32)
    rgb = _eval_poly(lat[..., 0], lat[..., 1], lat[..., 2], lat[..., 3]) + lat[..., 4:7]
    return np.clip(rgb, 0, 1).astype(np.float32)


def lerp_latent(la, lb, t):
    t = np.asarray(t, np.float32)[..., None]
    return (1 - t) * la + t * lb


def mix_rgb(rgb_a, rgb_b, t):
    return latent_to_rgb(lerp_latent(rgb_to_latent(rgb_a), rgb_to_latent(rgb_b), t))


# ---------------- colour spaces ----------------
def srgb_to_linear(c):
    c = np.asarray(c, np.float32)
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4).astype(np.float32)


def linear_to_srgb(c):
    c = np.clip(np.asarray(c, np.float32), 0, 1)
    return np.where(c <= 0.0031308, c * 12.92, 1.055 * c ** (1 / 2.4) - 0.055).astype(np.float32)


_M_RGB2XYZ = np.array([[0.4124564, 0.3575761, 0.1804375],
                       [0.2126729, 0.7151522, 0.0721750],
                       [0.0193339, 0.1191920, 0.9503041]], np.float32)
_WHITE = np.array([0.95047, 1.0, 1.08883], np.float32)


def rgb_to_lab(rgb):
    lin = srgb_to_linear(rgb)
    xyz = lin @ _M_RGB2XYZ.T / _WHITE
    f = np.where(xyz > 0.008856, np.cbrt(xyz), 7.787 * xyz + 16 / 116)
    L = 116 * f[..., 1] - 16
    a = 500 * (f[..., 0] - f[..., 1])
    b = 200 * (f[..., 1] - f[..., 2])
    return np.stack([L, a, b], -1).astype(np.float32)


_M_XYZ2RGB = np.linalg.inv(_M_RGB2XYZ).astype(np.float32)


def lab_to_rgb(lab):
    lab = np.asarray(lab, np.float32)
    fy = (lab[..., 0] + 16) / 116
    fx = fy + lab[..., 1] / 500
    fz = fy - lab[..., 2] / 200
    def finv(f):
        return np.where(f ** 3 > 0.008856, f ** 3, (f - 16 / 116) / 7.787)
    xyz = np.stack([finv(fx), finv(fy), finv(fz)], -1) * _WHITE
    lin = xyz @ _M_XYZ2RGB.T
    return linear_to_srgb(np.clip(lin, 0, 1))


def hex_to_rgb(s):
    s = s.lstrip("#")
    return np.array([int(s[i:i + 2], 16) / 255 for i in (0, 2, 4)], np.float32)


def rgb_to_hex(rgb):
    return "#%02x%02x%02x" % tuple(int(round(float(v) * 255)) for v in np.clip(rgb, 0, 1))


# ---------------- tube palette ----------------
# Mixbox's published pigment RGBs plus a few painter's tubes entered as sRGB swatches.
TUBES = {
    "lead_white": (250, 247, 240),
    "titanium_white": (255, 255, 255),
    "cadmium_yellow": (254, 236, 0),
    "hansa_yellow": (252, 211, 0),
    "cadmium_orange": (255, 105, 0),
    "cadmium_red": (255, 39, 2),
    "vermilion": (227, 66, 52),
    "madder": (160, 32, 60),
    "quinacridone_magenta": (128, 2, 46),
    "cobalt_violet": (78, 0, 66),
    "cobalt_violet_light": (146, 89, 163),
    "ultramarine": (25, 0, 89),
    "cobalt_blue": (0, 33, 133),
    "phthalo_blue": (13, 27, 68),
    "cerulean": (42, 120, 190),
    "phthalo_green": (0, 60, 50),
    "viridian": (64, 130, 109),
    "emerald": (60, 180, 110),
    "permanent_green": (7, 109, 22),
    "sap_green": (107, 148, 4),
    "yellow_ochre": (204, 153, 51),
    "burnt_sienna": (138, 54, 15),
}


def tube(name):
    if name.startswith("#"):
        return hex_to_rgb(name)
    return np.array(TUBES[name], np.float32) / 255.0


def mix_tubes(*parts):
    """mix_tubes("lead_white", ("cobalt_violet", 0.1), ("yellow_ochre", 0.05)) -> rgb.
    First entry is the base with weight 1 unless given as a tuple; weights are normalised."""
    names, ws = [], []
    for p in parts:
        if isinstance(p, tuple):
            names.append(p[0]); ws.append(float(p[1]))
        else:
            names.append(p); ws.append(1.0)
    ws = np.array(ws, np.float32); ws /= ws.sum()
    lat = sum(w * rgb_to_latent(tube(n)) for n, w in zip(names, ws))
    return latent_to_rgb(lat)


def color_spec_to_rgb(spec):
    """Accepts '#hex', 'tube_name', ('a', 'b', t) meaning lerp a->b by t, or an rgb triple."""
    if isinstance(spec, str):
        return tube(spec)
    if isinstance(spec, (tuple, list)) and len(spec) == 3 and isinstance(spec[0], str):
        return mix_rgb(tube(spec[0]), tube(spec[1]), float(spec[2]))
    return np.asarray(spec, np.float32)
