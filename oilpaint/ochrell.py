"""Small ctypes adapter; optics stay in the sibling Ochrell Rust library.

The 85-float wire format is K[41], S[41], residual[3], model v0.2/10 nm.
This is an explicit array format, never a transmute of Rust struct layout.
"""
import ctypes
import functools
import os
from pathlib import Path
import subprocess
import sys
import numpy as np

ROOT = Path(__file__).resolve().parents[1]
CRATE = ROOT / "native" / "ochrell-brush"
MODES = {"ochrell": 0, "ochrell-roundtrip": 1, "ochrell-srgb": 2, "mixbox-material": 3}
FORMAT = "ochrell-0.2-ks41-f32-v1"
FP = ctypes.POINTER(ctypes.c_float)


def library(comparison=False):
    return _library(bool(comparison))


@functools.lru_cache(maxsize=2)
def _library(comparison):
    # Cargo tracks the path dependency and shared kernel, so stale DLLs are not
    # silently reused. Build once per process, outside measured painting loops.
    crate = ROOT / "native" / "mixer-comparison" if comparison else CRATE
    target = crate / "target"
    subprocess.run([os.environ.get("CARGO", "cargo"), "build", "--release", "--offline",
                    "--manifest-path", str(crate / "Cargo.toml"), "--target-dir", str(target)], check=True)
    stem = "mixer_comparison" if comparison else "ochrell_brush"
    name = stem + ".dll" if sys.platform == "win32" else (
        "lib" + stem + (".dylib" if sys.platform == "darwin" else ".so"))
    lib = ctypes.CDLL(str(target / "release" / name))
    lib.ochrell_bridge_abi.restype = ctypes.c_uint32
    lib.ochrell_state_len.restype = ctypes.c_size_t
    if lib.ochrell_bridge_abi() != 1 or lib.ochrell_state_len() != 85:
        raise RuntimeError("Ochrell bridge ABI mismatch")
    for name in ("ochrell_encode", "ochrell_decode"):
        fn = getattr(lib, name)
        fn.argtypes = [FP, FP, ctypes.c_size_t, ctypes.c_int]
        fn.restype = ctypes.c_int
    return lib


def validate_state(z, backend):
    if z.shape[-1:] != (85,) or not np.isfinite(z).all():
        raise ValueError("expected finite (...,85) Ochrell states")
    if backend in ("ochrell", "ochrell-roundtrip") and (np.any(z[..., :41] < 0) or np.any(z[..., 41:82] <= 0)):
        raise ValueError("Ochrell requires K >= 0 and S > 0")
    if backend == "ochrell-srgb" and (np.any(z[..., :3] < 0) or np.any(z[..., :3] > 1)):
        raise ValueError("sRGB control requires bounded RGB")


def encode(rgb, backend):
    rgb = np.ascontiguousarray(rgb, np.float32)
    if rgb.shape[-1:] != (3,) or not np.isfinite(rgb).all():
        raise ValueError("expected finite (...,3) sRGB")
    out = np.empty(rgb.shape[:-1] + (85,), np.float32)
    status = library().ochrell_encode(rgb.ctypes.data_as(FP), out.ctypes.data_as(FP), rgb.size // 3, MODES[backend])
    if status:
        raise ValueError(f"Ochrell encode rejected input: {status}")
    return out


def decode(z, backend):
    z = np.ascontiguousarray(z, np.float32)
    validate_state(z, backend)
    out = np.empty(z.shape[:-1] + (3,), np.float32)
    status = library(backend == "mixbox-material").ochrell_decode(z.ctypes.data_as(FP), out.ctypes.data_as(FP), z.size // 85, MODES[backend])
    if status:
        raise ValueError(f"Ochrell decode rejected state: {status}")
    return out
