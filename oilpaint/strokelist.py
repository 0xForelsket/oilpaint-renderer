"""Write StrokeList v2 files (spec/STROKELIST_V2.md) from strokes planned by this Python renderer, so the new engine
can paint them: the eval harness's port checks (P1, P2), size-consistency tests and level E (L3). Writer only: the
engine never loads v1 `.npz` files, and this module does not convert them. The engine version comes from the new
kernel (crates/oil-shim), so a file is always stamped with the version that will paint it.
"""
import ctypes
import hashlib
import json
import os
import struct
import subprocess
import sys

import numpy as np

from . import mix

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
MAGIC = bytes([0x89, 0x4F, 0x49, 0x4C, 0x0D, 0x0A, 0x1A, 0x0A])
MODES = {"paint": 0, "scumble": 1, "smudge": 2, "glaze": 3}
NO_REGION = 0xFFFFFFFF
F_BEFORE_NB = ("opacity", "pickup", "load", "deplete", "vdry", "hgain", "flatten", "streak", "hardness", "grain",
               "dry_thresh", "dry_width")
F_AFTER_NB = ("dropout", "ragged", "body", "release", "streak_mix", "ridge", "levee", "furrow", "blob", "stiff", "marble",
              "splay")


def engine_version():
    """The engine version of the new kernel in this checkout (builds crates/oil-shim if needed)."""
    subprocess.run([os.environ.get("CARGO", "cargo"), "build", "--release", "-q", "-p", "oil-shim",
                    "--manifest-path", os.path.join(ROOT, "Cargo.toml")], check=True)
    name = "oil_shim.dll" if sys.platform == "win32" else ("liboil_shim.dylib" if sys.platform == "darwin" else "liboil_shim.so")
    lib = ctypes.CDLL(os.path.join(ROOT, "target", "release", name))
    lib.oil_engine_version.restype = ctypes.c_size_t
    lib.oil_engine_version.argtypes = [ctypes.c_char_p, ctypes.c_size_t]
    buf = ctypes.create_string_buffer(64)
    n = lib.oil_engine_version(buf, 64)
    return buf.raw[:n].decode("ascii")


def _section(out, tag, payload):
    out += tag + struct.pack("<I", len(payload)) + payload + b"\0" * ((4 - len(payload) % 4) % 4)


def to_bytes(strokes, layers, aspect, ground, layer_names, region_names=(), title=None, plan=None, version=None):
    """strokes: planned Stroke objects (mixer latents are decoded to authored sRGB with the current backend);
    layers: [dict(start, end, hblur_sigma or None, dry_after or None)]; aspect: (w, h) integers; ground: sRGB."""
    version = version or engine_version()
    head = bytearray(MAGIC + struct.pack("<HHB", 2, 0, len(version)) + version.encode("ascii"))
    head += b"\0" * (64 - len(head))
    meta = {"generator": f"oilpaint python planner (engine {version})", "title": title, "plan": plan,
            "inputs": {"sceneplan": None, "fields": {}, "hooks": []},
            "layers": list(layer_names), "regions": list(region_names)}
    out = bytearray(head)
    _section(out, b"META", json.dumps(meta, separators=(",", ":"), ensure_ascii=False).encode("utf-8"))
    _section(out, b"CANV", struct.pack("<II3fI", int(aspect[0]), int(aspect[1]), *[float(v) for v in ground], 0))
    layr = bytearray()
    for l in layers:
        flags = (l["hblur_sigma"] is not None) | ((l["dry_after"] is not None) << 1)
        layr += struct.pack("<3I2fI", l["start"], l["end"], flags, l["hblur_sigma"] or 0.0, l["dry_after"] or 0.0, 0)
    _section(out, b"LAYR", bytes(layr))
    off = np.zeros(len(strokes) + 1, np.uint32)
    off[1:] = np.cumsum([len(s.pts) for s in strokes])
    _section(out, b"OFFS", off.astype("<u4").tobytes())
    pts = np.concatenate([s.pts for s in strokes]).astype("<f4") if strokes else np.zeros((0, 4), "<f4")
    _section(out, b"PNTS", pts.tobytes())
    rgb = np.clip(mix.latent_to_rgb(np.stack([s.zcol for s in strokes])), 0, 1).astype(np.float32)
    rgb2 = np.clip(mix.latent_to_rgb(np.stack([s.zcol2 for s in strokes])), 0, 1).astype(np.float32)
    from .canvas import BRUSH_DEFAULTS
    strk = bytearray()
    for i, s in enumerate(strokes):
        p = dict(BRUSH_DEFAULTS)
        p.update({k: v for k, v in s.params.items() if k in p})
        mode = MODES[p["mode"]] if isinstance(p["mode"], str) else int(p["mode"])
        c2 = rgb[i] if np.array_equal(s.zcol2, s.zcol) else rgb2[i]
        region = NO_REGION if (s.region is None or not region_names) else int(s.region)
        strk += struct.pack("<4I", int(s.layer), region, int(s.seed) & 0xFFFFFFFF, mode)
        strk += struct.pack("<7f", *rgb[i], *c2, 1.0)
        strk += struct.pack("<12f", *[float(p[k]) for k in F_BEFORE_NB])
        strk += struct.pack("<I", int(p["nb"]))
        strk += struct.pack("<12f", *[float(p[k]) for k in F_AFTER_NB])
    _section(out, b"STRK", bytes(strk))
    _section(out, b"END ", hashlib.sha256(out).digest())
    return bytes(out)


def write(path, *a, **kw):
    data = to_bytes(*a, **kw)
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    with open(path, "wb") as f:
        f.write(data)
    return data


class ReadStroke:
    """One stroke of a read StrokeList: pts (n, 4) float32 in cw, region, layer, colour (sRGB), mode and params."""
    __slots__ = ("pts", "region", "layer", "color", "color2", "mode", "params")

    def __init__(self, pts, region, layer, color, color2, mode, params):
        self.pts, self.region, self.layer, self.color, self.color2, self.mode, self.params = pts, region, layer, color, color2, mode, params


def read(path):
    """Read a StrokeList v2 file (any engine version; no validation beyond the structure): returns a dict with
    engine, meta, aspect, ground, layers [dict(start, end, hblur_sigma, dry_after)] and strokes [ReadStroke]."""
    data = open(path, "rb").read()
    if data[:8] != MAGIC:
        raise ValueError(f"{path}: not a StrokeList v2 file")
    vlen = data[12]
    engine = data[13:13 + vlen].decode("ascii")
    pos, sec = 64, {}
    while pos < len(data):
        tag = data[pos:pos + 4]
        n = struct.unpack_from("<I", data, pos + 4)[0]
        sec[tag] = data[pos + 8:pos + 8 + n]
        pos += 8 + n + (4 - n % 4) % 4
        if tag == b"END ":
            break
    meta = json.loads(sec[b"META"].decode("utf-8"))
    aw, ah, g0, g1, g2, _ = struct.unpack("<II3fI", sec[b"CANV"])
    layers = []
    for k in range(0, len(sec[b"LAYR"]), 24):
        s, e, flags, hb, dry, _ = struct.unpack_from("<3I2fI", sec[b"LAYR"], k)
        layers.append(dict(start=s, end=e, hblur_sigma=hb if flags & 1 else None, dry_after=dry if flags & 2 else None))
    off = np.frombuffer(sec[b"OFFS"], "<u4")
    pts = np.frombuffer(sec[b"PNTS"], "<f4").reshape(-1, 4)
    strokes = []
    rec = struct.Struct("<4I7f12fI12f")
    for i, k in enumerate(range(0, len(sec[b"STRK"]), rec.size)):
        f = rec.unpack_from(sec[b"STRK"], k)
        layer, region, seed, mode = f[:4]
        params = dict(zip(F_BEFORE_NB, f[11:23]))
        params["nb"] = f[23]
        params.update(zip(F_AFTER_NB, f[24:36]))
        params["seed"] = seed
        strokes.append(ReadStroke(pts[off[i]:off[i + 1]], region, layer, np.array(f[4:7], np.float32), np.array(f[7:10], np.float32), mode, params))
    return dict(engine=engine, meta=meta, aspect=(aw, ah), ground=(g0, g1, g2), layers=layers, strokes=strokes)
