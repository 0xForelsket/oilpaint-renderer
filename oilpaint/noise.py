"""Resolution-independent value noise: a seeded lattice sampled at continuous canvas-width coordinates."""
import numpy as np
import cv2


def lattice_noise(X, Y, cells, seed=0, octaves=1, persistence=0.5, lacunarity=2.0):
    """X, Y: float arrays in canvas-width units (X in [0,1], Y in [0, H/W]). Returns values in ~[0,1].
    `cells` is the number of lattice cells across the width; the same (cells, seed) gives the same field at
    any resolution."""
    X = np.asarray(X, np.float32); Y = np.asarray(Y, np.float32)
    out = np.zeros(X.shape, np.float32)
    amp, tot, c = 1.0, 0.0, float(cells)
    rng = np.random.default_rng(seed)
    ymax = float(Y.max()) + 1e-3
    for _ in range(octaves):
        nx = int(np.ceil(c)) + 3
        ny = int(np.ceil(c * ymax)) + 3
        lat = rng.random((ny, nx), np.float32)
        mx = (X * c + 1.0).astype(np.float32)
        my = (Y * c + 1.0).astype(np.float32)
        out += amp * cv2.remap(lat, mx, my, cv2.INTER_CUBIC, borderMode=cv2.BORDER_REFLECT)
        tot += amp; amp *= persistence; c *= lacunarity
    return out / tot


def fbm(X, Y, scale=0.2, octaves=4, seed=0, persistence=0.5):
    """scale = size of the largest feature as a fraction of the canvas width."""
    return lattice_noise(X, Y, 1.0 / max(scale, 1e-3), seed, octaves, persistence)


def grid(W, H):
    """Coordinate grids in canvas-width units for a W x H raster (pixel centres)."""
    y, x = np.mgrid[0:H, 0:W].astype(np.float32)
    return (x + 0.5) / W, (y + 0.5) / W
