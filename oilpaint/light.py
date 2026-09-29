"""Canvas weave, normals from the paint height field, Blinn-Phong relighting with a cavity term."""
import numpy as np
import cv2

LIGHT_DEFAULTS = dict(light_dir=(-0.5, -0.6, 0.62), bump=1.4, bump_fine=1.8, contrast=0.45, spec=0.10, shininess=22.0,
                      gloss_h=(0.25, 1.4), cavity=0.06, hsmooth=0.0004, broad=0.004, shade_blur=0.0006,
                      tint=0.06, weave_amp=0.18, weave_px=None, weave_h0=1.2, matte=False)
"""
light_dir     direction to the light (x right, y down, z toward the viewer); default upper-left raking light
bump          normal strength of the broad paint form (stroke bodies, levees)
bump_fine     normal strength of the fine detail (bristle lanes, roughness) = height minus its broad blur
contrast      diffuse contrast: shade = 1 + contrast * (n.l - flat) / flat, clamped 0.55..1.35
spec          specular strength (white, Blinn-Phong); shininess 22 = satin oil, not gel
gloss_h       (h0, h1): specular fades in with paint thickness between h0 and h1 (thin/dry paint is matte)
cavity        capped, blurred occlusion darkening in valleys (no contour lines)
hsmooth       tiny blur of the height (cw) before the fine normals (anti-aliasing only)
broad         blur (cw) that separates broad form from fine detail
shade_blur    blur (cw) of the diffuse term (paint edges throw soft shadows)
tint          colour modulation by relief: ridges lighter and a touch more saturated, furrows darker
weave_amp, weave_h0   canvas weave amplitude and the thickness that hides it
"""


def value_noise(H, W, cells, rng, octaves=1, persistence=0.5):
    """Cheap fBm from upsampled random grids (resolution-independent when `cells` is fixed)."""
    out = np.zeros((H, W), np.float32)
    amp, tot = 1.0, 0.0
    c = cells
    for _ in range(octaves):
        g = rng.random((max(2, int(c * H / W)) + 1, max(2, int(c)) + 1)).astype(np.float32)
        out += amp * cv2.resize(g, (W, H), interpolation=cv2.INTER_CUBIC)
        tot += amp; amp *= persistence; c *= 2
    return out / tot


def weave(H, W, amp=0.12, threads_across=None, seed=7):
    """Two sine gratings (plain weave) plus slub noise.  threads_across: thread count across the width;
    default assumes a 66 cm canvas at 19.5 threads/cm (~1290 threads), which at 600 px collapses into
    fine noise and at 2400 px shows a ~1.9 px pitch."""
    if threads_across is None:
        threads_across = 1290.0
    pitch = W / threads_across
    rng = np.random.default_rng(seed)
    slub = value_noise(H, W, 40, rng, octaves=3)
    fine = rng.random((H, W)).astype(np.float32) - 0.5
    if pitch < 2.2:
        # the thread pitch is below what the pixel grid can hold: the grating would alias into moire.
        # Band-limit it: the weave averages to a fine, low-amplitude texture.
        w = 0.35 * (slub - 0.5)
        return (amp * (w + 0.35 * fine)).astype(np.float32)
    y, x = np.mgrid[0:H, 0:W].astype(np.float32)
    gx = np.sin(x * (2 * np.pi / pitch) + 0.8 * slub)
    gy = np.sin(y * (2 * np.pi / pitch) + 0.8 * slub)
    w = 0.5 * gx * gy + 0.25 * (gx + gy) * 0.3 + 0.35 * (slub - 0.5)
    return (amp * (w + 0.25 * fine)).astype(np.float32)


def blur(img, sigma):
    """Gaussian blur that downsamples first for large sigmas (cv2 is slow above sigma ~10)."""
    if sigma <= 0:
        return img.copy()
    if sigma < 8:
        return cv2.GaussianBlur(img, (0, 0), sigma)
    f = int(np.ceil(sigma / 4))
    H, W = img.shape[:2]
    small = cv2.resize(img, (max(2, W // f), max(2, H // f)), interpolation=cv2.INTER_AREA)
    small = cv2.GaussianBlur(small, (0, 0), sigma / f)
    return cv2.resize(small, (W, H), interpolation=cv2.INTER_LINEAR)


def relight(rgb, h, W_ref=None, components=False, **kw):
    """rgb float (H,W,3) unlit albedo; h paint height (H,W). Returns lit float rgb.
    Two-scale normals: the broad paint form (stroke bodies) lit with `bump`, the fine bristle detail with
    `bump_fine`.  A flat surface keeps its albedo; slopes toward the light brighten, away darken; a white
    satin highlight sits on slopes facing the half vector and only where the paint is thick enough; a small
    capped cavity term darkens valleys; ridges are tinted a touch lighter/more saturated."""
    p = dict(LIGHT_DEFAULTS); p.update({k: v for k, v in kw.items() if v is not None})
    H, W = h.shape
    if p["weave_amp"] > 0:
        wv = weave(H, W, p["weave_amp"], p["weave_px"])
        h_total = h + wv * np.exp(-h / max(1e-6, p["weave_h0"]))
    else:
        h_total = h
    scale = W / 600.0
    if p["hsmooth"] > 0:
        h_total = cv2.GaussianBlur(h_total, (0, 0), max(0.3, p["hsmooth"] * W))
    h_broad = blur(h_total, max(1.0, p["broad"] * W))
    h_fine = h_total - h_broad
    # gradients in height per canvas-width fraction so bump scales with resolution
    gxb = cv2.Sobel(h_broad, cv2.CV_32F, 1, 0, ksize=3) / 8.0 * scale
    gyb = cv2.Sobel(h_broad, cv2.CV_32F, 0, 1, ksize=3) / 8.0 * scale
    gxf = cv2.Sobel(h_fine, cv2.CV_32F, 1, 0, ksize=3) / 8.0 * scale
    gyf = cv2.Sobel(h_fine, cv2.CV_32F, 0, 1, ksize=3) / 8.0 * scale
    gx = p["bump"] * gxb + p["bump_fine"] * gxf
    gy = p["bump"] * gyb + p["bump_fine"] * gyf
    nz = 1.0 / np.sqrt(1 + gx * gx + gy * gy)
    nx, ny = -gx * nz, -gy * nz
    L = np.array(p["light_dir"], np.float32); L /= np.linalg.norm(L)
    V = np.array([0, 0, 1], np.float32)
    Hh = L + V; Hh /= np.linalg.norm(Hh)
    ndotl = np.clip(nx * L[0] + ny * L[1] + nz * L[2], 0, 1)
    ndoth = np.clip(nx * Hh[0] + ny * Hh[1] + nz * Hh[2], 0, 1)
    g0, g1 = p["gloss_h"]
    gloss = np.clip((h - g0) / max(1e-6, g1 - g0), 0, 1)
    gloss = gloss * gloss * (3 - 2 * gloss)
    spec = (0.03 if p["matte"] else p["spec"]) * gloss * ndoth ** p["shininess"]
    shade = np.clip(1 + p["contrast"] * (ndotl - L[2]) / L[2], 0.55, 1.35)
    shade = cv2.GaussianBlur(shade, (0, 0), max(0.5, p["shade_blur"] * W))
    hb = blur(h_total, 6.0 * scale)
    cav_raw = np.clip((hb - h_total) / 1.5, 0, 1)
    cav_raw = blur(cav_raw, 2.0 * scale)
    cav = 1 - p["cavity"] * cav_raw
    # relief tint: ridges lighter and a touch more saturated, furrows darker
    rel = np.clip(h_fine / 0.5, -1, 1)
    mean = rgb.mean(-1, keepdims=True)
    albedo = rgb * (1 + p["tint"] * rel)[..., None] + (0.6 * p["tint"] * rel)[..., None] * (rgb - mean)
    out = np.clip(albedo, 0, 1) * (shade * cav)[..., None] + spec[..., None]
    out = np.clip(out, 0, 1).astype(np.float32)
    if components:
        return out, dict(shade=shade, spec=spec, cav=cav, gx=gx, gy=gy, gloss=gloss)
    return out


def normals_preview(h, W_ref=None):
    scale = h.shape[1] / 600.0
    gx = cv2.Sobel(h, cv2.CV_32F, 1, 0, ksize=3) / 8.0 * scale
    gy = cv2.Sobel(h, cv2.CV_32F, 0, 1, ksize=3) / 8.0 * scale
    nz = 1.0 / np.sqrt(1 + 16 * (gx * gx + gy * gy))
    return np.stack([0.5 - 2 * gx * nz, 0.5 - 2 * gy * nz, nz], -1).clip(0, 1)


def to8(img):
    return (np.clip(img, 0, 1) * 255 + 0.5).astype(np.uint8)


def save_png(path, rgb_float):
    cv2.imwrite(path, cv2.cvtColor(to8(rgb_float), cv2.COLOR_RGB2BGR))


def save_gray(path, g, vmax=None):
    g = np.asarray(g, np.float32)
    vmax = float(g.max()) if vmax is None else vmax
    cv2.imwrite(path, to8(g / max(vmax, 1e-6)))
