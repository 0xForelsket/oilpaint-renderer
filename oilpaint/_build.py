"""Compile csrc/brush.c with gcc/clang on first import and load it with ctypes (.so / .dylib / .dll)."""
import ctypes
import hashlib
import os
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.join(HERE, "csrc", "brush.c")

LAT = 7


class BrushParams(ctypes.Structure):
    _fields_ = [
        ("mode", ctypes.c_int), ("opacity", ctypes.c_float), ("pickup", ctypes.c_float),
        ("load", ctypes.c_float), ("deplete", ctypes.c_float), ("vdry", ctypes.c_float),
        ("hgain", ctypes.c_float), ("flatten", ctypes.c_float), ("streak", ctypes.c_float),
        ("hardness", ctypes.c_float), ("grain", ctypes.c_float), ("dry_thresh", ctypes.c_float),
        ("dry_width", ctypes.c_float), ("nb", ctypes.c_int), ("allow_mask", ctypes.c_uint32),
        ("override_p", ctypes.c_float), ("seed", ctypes.c_uint32), ("dropout", ctypes.c_float),
        ("ragged", ctypes.c_float), ("body", ctypes.c_float), ("release", ctypes.c_float), ("streak_mix", ctypes.c_float),
        ("ridge", ctypes.c_float), ("levee", ctypes.c_float), ("furrow", ctypes.c_float), ("blob", ctypes.c_float),
        ("stiff", ctypes.c_float), ("marble", ctypes.c_float), ("splay", ctypes.c_float),
    ]


class CCanvas(ctypes.Structure):
    _fields_ = [
        ("W", ctypes.c_int), ("H", ctypes.c_int),
        ("lat", ctypes.POINTER(ctypes.c_float)), ("rgb", ctypes.POINTER(ctypes.c_float)),
        ("h", ctypes.POINTER(ctypes.c_float)), ("wet", ctypes.POINTER(ctypes.c_float)),
        ("cover", ctypes.POINTER(ctypes.c_float)), ("hblur", ctypes.POINTER(ctypes.c_float)),
        ("region", ctypes.POINTER(ctypes.c_uint8)),
    ]


_lib = None


def build_and_load():
    global _lib
    if _lib is not None:
        return _lib
    with open(SRC, "rb") as f:
        digest = hashlib.sha1(f.read()).hexdigest()[:12]
    ext = ".dll" if sys.platform == "win32" else (".dylib" if sys.platform == "darwin" else ".so")
    so = os.path.join(HERE, "csrc", f"brush_{digest}{ext}")
    if not os.path.exists(so):
        cc = os.environ.get("CC") or next((c for c in ("gcc", "clang", "cc") if shutil.which(c)), None)
        if cc is None:
            raise RuntimeError("oilpaint needs a C compiler (gcc or clang) to build csrc/brush.c; "
                               "on Windows use WSL2 or MSYS2/MinGW-w64 (see README), or set CC")
        cmd = [cc, "-O3", "-march=native", "-ffast-math", "-fno-finite-math-only", "-shared"]
        if sys.platform != "win32":
            cmd.append("-fPIC")
        cmd += ["-o", so, SRC, "-lm"]
        subprocess.check_call(cmd)
    lib = ctypes.CDLL(so)
    fp = ctypes.POINTER(ctypes.c_float)
    lib.render_stroke.argtypes = [ctypes.POINTER(CCanvas), fp, ctypes.c_int, fp, fp, fp,
                                  ctypes.POINTER(BrushParams), fp]
    lib.render_stroke.restype = ctypes.c_int
    lib.render_strokes.argtypes = [ctypes.POINTER(CCanvas), fp, ctypes.POINTER(ctypes.c_int), ctypes.c_int,
                                   fp, fp, fp, ctypes.POINTER(BrushParams)]
    lib.render_strokes.restype = ctypes.c_int
    lib.latent_to_rgb_array.argtypes = [fp, fp, ctypes.c_int]
    lib.latent_to_rgb_array.restype = None
    _lib = lib
    return lib
