"""Palette-knife prototype (numpy, on a real 2400-px crop).  A rigid blade (length Lk, at angle to the motion) is swept
along a path.  The blade rides on the high points of the existing paint (plane B = high percentile of h under the
sweep); it carries a paint film of thickness t(tau) = t0 * load(tau).  Contact where h_old >= B - t: there the surface
becomes the flat plane B (scraped flat); valleys deeper than the film are skipped (broken, 'skipping' knife texture).
Paint scraped above B plus a share of the film is squeezed to the blade ends: sharp lateral ridges; a lip where the knife
lifts.  Colour: two unmixed colours on the knife in a few broad streaks across the blade, smeared with the wet paint
it passes over (pick-up along the motion)."""
import sys, numpy as np, cv2
sys.path.insert(0, "oilpaint-renderer")
from oilpaint import light as L
from oilpaint.noise import fbm
B = "look/lv_ramp03_2400/"
h_all = np.load(B + "height.npy"); rgb_all = np.load(B + "unlit.npy")
Hh, Ww = h_all.shape
y0, x0 = int(0.78 * Hh), int(0.08 * Ww)
h = h_all[y0:y0 + 380, x0:x0 + 700].copy(); rgb = rgb_all[y0:y0 + 380, x0:x0 + 700].copy()
h0, rgb0 = h.copy(), rgb.copy()
H, W = h.shape
yy, xx = np.mgrid[0:H, 0:W].astype(np.float32) + 0.5
rng = np.random.default_rng(5)

def knife(p0, ang_deg, D, Lk, blade_deg, colA, colB, t0=1.6, deplete=0.5, pick=0.35, ridge=1.2):
    global h, rgb
    m = np.array([np.cos(np.radians(ang_deg)), np.sin(np.radians(ang_deg))]); b = np.array([np.cos(np.radians(ang_deg + blade_deg)), np.sin(np.radians(ang_deg + blade_deg))])
    M = np.array([[m[0], b[0]], [m[1], b[1]]]); Mi = np.linalg.inv(M)
    dx, dy = xx - p0[0], yy - p0[1]
    tau = Mi[0, 0] * dx + Mi[0, 1] * dy; sig = Mi[1, 0] * dx + Mi[1, 1] * dy
    sw = (tau >= 0) & (tau <= D) & (np.abs(sig) <= Lk / 2)
    load = np.clip(1 - deplete * tau / D, 0, 1)
    # the rigid blade rests on the highest points ACROSS its length at each position along the path, and follows the
    # broad surface along the path (smoothed): Bk(tau)
    nb = int(D) + 1; ti = np.clip(tau, 0, D).astype(int)
    prof = np.full(nb, -1e9, np.float32)
    np.maximum.at(prof, ti[sw], h[sw] - 0.0)
    good = prof > -1e8; prof[~good] = np.interp(np.flatnonzero(~good), np.flatnonzero(good), prof[good])
    prof = cv2.GaussianBlur(prof[None], (0, 0), 12)[0] - 0.35   # blade bottom a little below the peaks: it scrapes them
    Bk = prof[ti]
    t = t0 * load
    contact = np.clip((h - (Bk - t)) / 0.08 + 0.5, 0, 1) * sw * np.clip(load * 3, 0, 1)
    scraped = np.clip(h - Bk, 0, None) * sw
    # colour: broad streaks across the blade (few, sharp-edged), smeared with the canvas upstream
    s_n = sig / Lk + 0.5
    streak = (np.sin(2 * np.pi * (1.7 * s_n + rng.random())) + 0.6 * np.sin(2 * np.pi * (4.3 * s_n + rng.random())) > 0.3).astype(np.float32)
    streak = cv2.GaussianBlur(streak, (0, 0), 1.2)
    knife_col = colA[None, None] * (1 - streak[..., None]) + colB[None, None] * streak[..., None]
    up = np.stack([cv2.remap(rgb[..., c], (xx - m[0] * 18 - 0.5).astype(np.float32), (yy - m[1] * 18 - 0.5).astype(np.float32), cv2.INTER_LINEAR, borderMode=cv2.BORDER_REFLECT) for c in range(3)], -1)
    k = pick * (1 - load)[..., None] + 0.1
    col = knife_col * (1 - k) + up * k
    a = contact[..., None]
    rgb = rgb * (1 - a) + col * a
    hn = np.where(contact > 0, h + contact * (Bk - h), h)       # flat scraped plane where the film touches
    # squeezed paint: ridges at the blade ends (|sig| ~ Lk/2) and a lip at the lift (tau ~ D)
    vol = (scraped.sum() + (contact * t).sum() * 0.25) / max(1.0, D)
    side = np.exp(-((np.abs(sig) - Lk / 2) / 2.2) ** 2) * ((tau >= 0) & (tau <= D))
    lip = np.exp(-((tau - D) / 3.0) ** 2) * (np.abs(sig) <= Lk / 2)
    hn = hn + ridge * min(1.5, vol / (Lk * 2)) * (0.8 * side + 0.6 * lip)
    h = hn.astype(np.float32)

knife((60, 120), -8, 330, 90, 78, np.array([0.93, 0.92, 0.97]), np.array([0.62, 0.74, 0.90]))
knife((300, 260), 4, 300, 70, 70, np.array([0.30, 0.50, 0.78]), np.array([0.96, 0.85, 0.62]))
knife((420, 90), -20, 220, 60, 82, np.array([0.98, 0.86, 0.60]), np.array([0.95, 0.95, 0.98]))
a = L.relight(rgb0, h0); b = L.relight(rgb, h)
out = np.concatenate([L.to8(a), np.full((H, 6, 3), 255, np.uint8), L.to8(b)], 1)
cv2.imwrite("look/knife_proto.png", cv2.cvtColor(out, cv2.COLOR_RGB2BGR))
hs = lambda z: L.to8(np.repeat(((z - z.min()) / (np.ptp(z) + 1e-6))[..., None], 3, -1))
cv2.imwrite("look/knife_proto_height.png", np.concatenate([hs(h0), np.full((H, 6, 3), 255, np.uint8), hs(h)], 1))
print("ok")
