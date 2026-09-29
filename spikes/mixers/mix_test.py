import sys, numpy as np, cv2
sys.path.insert(0, "../eval_copy"); sys.path.insert(0, ".")
from oilpaint import mix
from open_mixers import wgm_rgb_to_latent, wgm_latent_to_rgb, KM
km38, km12 = KM(38), KM(12)
MIXERS = {"mixbox": (mix.rgb_to_latent, mix.latent_to_rgb), "wgm10": (wgm_rgb_to_latent, wgm_latent_to_rgb),
          "km38": (km38.rgb_to_latent, km38.latent_to_rgb), "km12": (km12.rgb_to_latent, km12.latent_to_rgb),
          "rgb": (lambda c: np.asarray(c, np.float32), lambda z: np.clip(z, 0, 1))}
T = mix.tube
PAIRS = [("cobalt_blue", "cadmium_yellow"), ("ultramarine", "cadmium_yellow"), ("phthalo_blue", "hansa_yellow"), ("vermilion", "viridian"),
         ("madder", "emerald"), ("cobalt_violet", "yellow_ochre"), ("cobalt_blue", "lead_white"), ("madder", "lead_white"), ("ultramarine", "cadmium_orange")]
def hue(lab): return float(np.degrees(np.arctan2(lab[2], lab[1])) % 360)
rows = []
print(f"{'pair':34s} " + " ".join(f"{m:>22s}" for m in MIXERS))
for a, b in PAIRS:
    line = f"{a+'+'+b:34s} "
    for m, (f, g) in MIXERS.items():
        za, zb = f(T(a)), f(T(b))
        mid = g(0.5 * za + 0.5 * zb); lab = mix.rgb_to_lab(mid)
        rt = np.abs(g(za) - T(a)).max()
        line += f"   L{lab[0]:4.0f} C{np.hypot(lab[1], lab[2]):4.0f} h{hue(lab):4.0f} rt{rt:.0e}"
    print(line)
# swatch image: 9 pairs x 5 mixers, 11-step ramps
H = 22; img = []
for a, b in PAIRS:
    band = []
    for m, (f, g) in MIXERS.items():
        za, zb = f(T(a)), f(T(b)); ts = np.linspace(0, 1, 11, dtype=np.float32)[:, None]
        cols = g(za[None] * (1 - ts) + zb[None] * ts)
        band.append(np.repeat(np.repeat(cols[None], H, 0), 12, 1)); band.append(np.ones((H, 8, 3), np.float32))
    img.append(np.concatenate(band, 1)); img.append(np.ones((4, img[-1].shape[1], 3), np.float32))
img = np.concatenate(img, 0)
lab = np.full((24, img.shape[1], 3), 255, np.uint8)
for i, m in enumerate(MIXERS):
    cv2.putText(lab, m, (i * 140 + 10, 17), cv2.FONT_HERSHEY_SIMPLEX, 0.5, (0, 0, 0), 1, cv2.LINE_AA)
out = np.concatenate([lab, (img * 255 + 0.5).astype(np.uint8)[..., ::-1]], 0)
cv2.imwrite("mix_ramps.png", cv2.resize(out, None, fx=1.4, fy=1.4, interpolation=cv2.INTER_NEAREST))
# mud stack: 8 alternating complementary deposits at alpha 0.5 onto white -> chroma
for m, (f, g) in MIXERS.items():
    z = f(T("lead_white")); seq = ["vermilion", "viridian", "cadmium_yellow", "cobalt_violet", "cobalt_blue", "cadmium_orange", "madder", "emerald"]
    for s in seq: z = z + 0.5 * (f(T(s)) - z)
    l = mix.rgb_to_lab(g(z)); print(f"mud stack {m:7s}: L {l[0]:5.1f} C {np.hypot(l[1], l[2]):5.1f}")
