"""Export stroke lists to the oilcore corpus format (stroke file v2 prototype, little endian, 4-byte fields).
usage: python export_corpus.py sheet out.bin | storm strokes.npz out.bin"""
import sys, os, struct, json, ctypes
import numpy as np
EC = os.path.join(os.path.dirname(os.path.abspath(__file__)), "eval_copy")
sys.path.insert(0, EC)
from oilpaint import mix
from oilpaint.canvas import make_params, MODES
from oilpaint._build import BrushParams, LAT
from oilpaint.render import load_strokes
from oilpaint.scene import load_scene

def write(path, strokes, layers, ground_rgb, aspect):
    """layers: list of dict(start, end, needs_hblur, hblur_sigma, dry_after)"""
    g = mix.rgb_to_latent(np.asarray(ground_rgb, np.float32)).astype(np.float32)
    grgb = mix.latent_to_rgb(g).astype(np.float32)
    pts = np.concatenate([s.pts for s in strokes]).astype(np.float32)
    off = np.zeros(len(strokes) + 1, np.uint32); off[1:] = np.cumsum([len(s.pts) for s in strokes])
    params = (BrushParams * len(strokes))()
    for i, s in enumerate(strokes):
        params[i] = make_params(s.seed, **s.params)
    with open(path, "wb") as f:
        f.write(struct.pack("<6I", 0x3143504F, 1, len(strokes), len(pts), len(layers), LAT))
        f.write(struct.pack("<f", aspect)); f.write(g.tobytes()); f.write(grgb.tobytes())
        for l in layers:
            f.write(struct.pack("<3I2f", l["start"], l["end"], int(l["needs_hblur"]), l["hblur_sigma"],
                                float("nan") if l["dry_after"] is None else float(l["dry_after"])))
        f.write(off.tobytes()); f.write(pts.tobytes())
        for k in ("zcol", "zcol2", "dz"):
            f.write(np.stack([getattr(s, k) for s in strokes]).astype(np.float32).tobytes())
        f.write(bytes(params))
    print(f"wrote {path}: {len(strokes)} strokes, {len(pts)} points, {len(layers)} layers, {os.path.getsize(path)/1e6:.1f} MB")

if sys.argv[1] == "sheet":
    from oilpaint import eval as E
    sh = E.sheet_layers(E.swatches_module().SEED if hasattr(E.swatches_module(), "SEED") else 1907)
    swm = E.swatches_module()
    strokes, layers = [], []
    for l in sh.layers:
        a = len(strokes); strokes += l["strokes"]
        layers.append(dict(start=a, end=len(strokes), needs_hblur=E._needs_hblur(l), hblur_sigma=l["hblur_sigma"], dry_after=l["dry_after"]))
    write(sys.argv[2], strokes, layers, mix.color_spec_to_rgb(swm.GROUND), swm.ASPECT[1] / swm.ASPECT[0])
else:
    S = load_scene(os.path.join(EC, "scenes/storm_v3.py"))
    strokes, ranges = load_strokes(sys.argv[2])
    layers = []
    for li, (a, b) in enumerate(ranges):
        L = S.layer_list[li]
        layers.append(dict(start=a, end=b, needs_hblur=any(s.params.get("mode") == "scumble" for s in strokes[a:b]),
                           hblur_sigma=L["hblur_sigma"], dry_after=L.get("dry_after")))
    write(sys.argv[3], strokes, layers, S.ground, 1.25)
