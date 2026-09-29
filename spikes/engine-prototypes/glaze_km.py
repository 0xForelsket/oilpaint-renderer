"""Glaze: current mode 3 (lerp in Mixbox latent, alpha) vs Kubelka-Munk thin-layer composite (per linear-RGB channel).
KM layer (Kubelka 1948 hyperbolic solution): a = 1 + K/S, b = sqrt(a^2 - 1), X = S*d (optical thickness)
  R_g = sinh(bX) / (a sinh(bX) + b cosh(bX)),  T_g = b / (a sinh(bX) + b cosh(bX))
Composite over the body reflectance R_u:  R = R_g + T_g^2 R_u / (1 - R_g R_u)
K/S of the glaze pigment from its masstone reflectance R_inf (single-constant KM): K/S = (1 - R_inf)^2 / (2 R_inf)."""
import sys, numpy as np, cv2
sys.path.insert(0, "oilpaint-renderer")
from oilpaint import mix
def km_glaze(rgb_under, rgb_glaze, d, opacity=0.05):
    """Transparent glaze of strength d (0..1).  Absorption per channel from the glaze colour C (the colour a full-strength
    glaze gives over white: T^2 = C at d = 1, so K = -ln C / 2); scattering S = opacity * mean(K) (grey haze).
    General KM layer with numerically stable tanh/sech forms; S -> 0 gives Beer-Lambert R = T^2 R_u."""
    Ru = mix.srgb_to_linear(rgb_under).clip(1e-4, 1); C = mix.srgb_to_linear(rgb_glaze).clip(1e-3, 1)
    K = -np.log(C) / 2 * d; S = opacity * K.mean() + 1e-6
    a = 1 + K / S + 1e-6; b = np.sqrt(a * a - 1); x = b * S
    th = np.tanh(x); sech = 1 / np.cosh(np.minimum(x, 50))
    Rg = th / (a * th + b); Tg = b * sech / (a * th + b)
    R = Rg + Tg * Tg * Ru / (1 - Rg * Ru)
    return mix.linear_to_srgb(R)
def latent_glaze(rgb_under, rgb_glaze, alpha):
    zu = mix.rgb_to_latent(rgb_under); zg = mix.rgb_to_latent(np.broadcast_to(rgb_glaze, rgb_under.shape).copy())
    return mix.latent_to_rgb(zu + alpha * (zg - zu))
Wd = 720
stops = [mix.hex_to_rgb(h) for h in ("#232a58", "#5a5a95", "#b4a6c6", "#e8cbb8", "#f6d09a", "#fbf0dc")]
t = np.linspace(0, 1, Wd); seg = np.minimum((t * (len(stops) - 1)).astype(int), len(stops) - 2); f = (t * (len(stops) - 1) - seg)[:, None]
under = (np.array(stops)[seg] * (1 - f) + np.array(stops)[seg + 1] * f).astype(np.float32)
rows = [("underpainting", under)]
for name, g in (("madder", mix.tube("madder")), ("cadmium_yellow+white", mix.color_spec_to_rgb(("cadmium_yellow", "lead_white", 0.2))),
                ("ultramarine", mix.tube("ultramarine"))):
    rows.append((f"{name} latent alpha 0.3", latent_glaze(under, g, 0.3)))
    rows.append((f"{name} KM d=0.5", km_glaze(under, g, 0.5)))
img = np.concatenate([np.repeat(r[None], 46, 0) for _, r in rows], 0)
img8 = (img.clip(0, 1) * 255 + 0.5).astype(np.uint8)
lab = np.full((img8.shape[0], 230, 3), 255, np.uint8)
for i, (n, _) in enumerate(rows):
    cv2.putText(lab, n, (4, i * 46 + 28), cv2.FONT_HERSHEY_SIMPLEX, 0.42, (0, 0, 0), 1, cv2.LINE_AA)
cv2.imwrite("look/glaze_km_vs_alpha.png", cv2.cvtColor(np.concatenate([lab, img8], 1), cv2.COLOR_RGB2BGR))
# numbers: L* change in the darkest and lightest underpaint for madder
for n, fn in (("latent", lambda u: latent_glaze(u, mix.tube("madder"), 0.3)), ("KM", lambda u: km_glaze(u, mix.tube("madder"), 0.5))):
    for pos in (0, Wd - 1):
        u = under[pos:pos + 1]; o = fn(u)
        lu, lo = mix.rgb_to_lab(u)[0], mix.rgb_to_lab(o)[0]
        print(f"madder {n:6s} under L*{lu[0]:5.1f} C*{np.hypot(*lu[1:]):5.1f} -> L*{lo[0]:5.1f} C*{np.hypot(*lo[1:]):5.1f}")
