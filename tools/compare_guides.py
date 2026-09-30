"""Compare the new engine's guide maps (oil guides --npy) with v1's Python SceneBuilder.guides for the same scene.

The engine defines its own rasterisation and noise (hashed lattice instead of numpy's PCG64 lattices), so the maps
are not expected to be identical: this measures how close they are, per plane and per region, and writes a
side-by-side sheet. L2 acceptance (docs/plans/LIBRARY_PLAN.md) is Storm Light's guide sheets from TS.

  .venv\\Scripts\\python.exe tools\\compare_guides.py --scene scenes/storm_v3.py --rust out/l2/guides_native --out out/l2/compare
"""
import argparse
import json
import os
import sys
import time

import cv2
import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from oilpaint.maps import flow_preview, regions_preview  # noqa: E402
from oilpaint.scene import load_scene  # noqa: E402
from oilpaint.sheet import sheet  # noqa: E402


def to8(img):
    return (np.clip(img, 0, 1) * 255 + 0.5).astype(np.uint8)


def angle_deg(a, b):
    """Angle between unit direction fields (H, W, 2), degrees in [0, 180]."""
    dot = np.clip((a * b).sum(-1), -1, 1)
    return np.degrees(np.arccos(dot))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--scene", default="scenes/storm_v3.py")
    ap.add_argument("--rust", required=True, help="directory written by `oil guides ... --npy`")
    ap.add_argument("--out", default="out/l2/compare")
    ap.add_argument("--repeat", type=int, default=3, help="timing repeats for the Python compile")
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)

    rs = json.load(open(os.path.join(a.rust, "guides.json")))
    W, H = rs["size"]
    S = load_scene(a.scene)
    times = []
    for _ in range(a.repeat):
        t0 = time.perf_counter()
        g = S.guides(W)
        times.append((time.perf_counter() - t0) * 1000)
    assert (g.W, g.H) == (W, H), ((g.W, g.H), (W, H))

    r_target = np.load(os.path.join(a.rust, "target.npy"))
    r_rid = np.load(os.path.join(a.rust, "region_id.npy"))
    r_masks = np.load(os.path.join(a.rust, "masks.npy"))
    r_flow = np.load(os.path.join(a.rust, "flow.npy"))
    r_light = np.load(os.path.join(a.rust, "light.npy"))
    names = [r["name"] for r in rs["regions"]]
    assert names == g.names, (names, g.names)

    d_target = np.abs(r_target - g.target) * 255
    d_light = np.abs(r_light - g.light)
    ang = angle_deg(r_flow, g.flow)
    rep = {
        "size": [W, H],
        "pythonGuidesMs": {"min": min(times), "runs": times},
        "engineGuidesMs": rs["timings"],
        "target": {"meanAbs255": float(d_target.mean()), "p99Abs255": float(np.percentile(d_target, 99)), "maxAbs255": float(d_target.max()),
                    "psnr": float(10 * np.log10(255 ** 2 / max(1e-12, (d_target ** 2).mean())))},
        "regionId": {"agreement": float((r_rid == g.region_id).mean())},
        "light": {"meanAbs": float(d_light.mean()), "maxAbs": float(d_light.max())},
        "flow": {"meanAngleDeg": float(ang.mean()), "medianAngleDeg": float(np.median(ang)), "within10Deg": float((ang < 10).mean())},
        "regions": [],
    }
    for i, n in enumerate(names):
        rp, pp = r_rid == i, g.region_id == i
        union = (rp | pp).sum()
        row = {"name": n, "iou": float((rp & pp).sum() / union) if union else 1.0, "pixelsEngine": int(rp.sum()), "pixelsV1": int(pp.sum()),
               "softMaskMeanAbs": float(np.abs(r_masks[i] - g.masks[n]).mean())}
        own = pp & rp
        if own.any():
            row["flowMedianAngleDeg"] = float(np.median(ang[own]))
        rf = os.path.join(a.rust, f"flow_{n}.npy")
        if os.path.exists(rf) and n in g.region_flows:
            af = angle_deg(np.load(rf), g.region_flows[n])
            row["regionFlowMedianAngleDeg"] = float(np.median(af))
            row["regionFlowP90AngleDeg"] = float(np.percentile(af, 90))
        rep["regions"].append(row)

    # side-by-side sheet: v1 on top, engine below, then difference maps
    rs_img = lambda f: cv2.cvtColor(cv2.imread(os.path.join(a.rust, f)), cv2.COLOR_BGR2RGB)
    v1 = [("v1 target", to8(g.target)), ("v1 regions", regions_preview(g.region_id, g.names)),
          ("v1 flow", flow_preview(g.flow, W, H, bg=g.target * 0.5 + 0.5)), ("v1 light", to8(g.light))]
    eng = [("engine target", rs_img("guide_target.png")), ("engine regions", rs_img("guide_regions.png")),
           ("engine flow", rs_img("guide_flow.png")), ("engine light", rs_img("guide_light.png"))]
    heat = lambda d, s: cv2.cvtColor(cv2.applyColorMap(to8(np.clip(d / s, 0, 1)), cv2.COLORMAP_INFERNO), cv2.COLOR_BGR2RGB)
    diffs = [("|target| x4", heat(d_target.max(-1), 64)), ("region id differs", to8(np.repeat((r_rid != g.region_id)[..., None], 3, -1).astype(np.float32))),
             ("flow angle / 90deg", heat(ang, 90)), ("|light| x4", heat(d_light, 0.25))]
    cv2.imwrite(os.path.join(a.out, "compare_sheet.png"), cv2.cvtColor(sheet(v1 + eng + diffs, cols=4, cell=(360, 450)), cv2.COLOR_RGB2BGR))
    json.dump(rep, open(os.path.join(a.out, "compare.json"), "w"), indent=1)
    print(json.dumps({k: rep[k] for k in ("pythonGuidesMs", "target", "regionId", "light", "flow")}, indent=1))
    worst = sorted(rep["regions"], key=lambda r: r["iou"])[:6]
    print("lowest IoU:", [(r["name"], round(r["iou"], 4)) for r in worst])
    print("region flows (median/p90 deg):", [(r["name"], round(r.get("regionFlowMedianAngleDeg", -1), 1), round(r.get("regionFlowP90AngleDeg", -1), 1)) for r in rep["regions"] if "regionFlowMedianAngleDeg" in r])


if __name__ == "__main__":
    main()
