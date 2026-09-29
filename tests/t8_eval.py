"""T8: the evaluation harness itself (python -m oilpaint eval ...).  A measuring stick is only useful if it is itself checked:
the metrics must move when the flaw they name is present, be deterministic, and the compare / reference commands must behave.
Everything here is offline and takes under a minute."""
import json
import os
import subprocess
import sys
import tempfile
import time
import numpy as np
import cv2

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
sys.path.insert(0, ROOT)
from oilpaint import mix, light as Lt
from oilpaint import eval as EV
from oilpaint import eval_metrics as EM
from oilpaint import eval_swatch as ES


def _sheet_result(W, mixer="mixbox", **light_kw):
    """Paint the swatch sheet at width W and evaluate it under the given light.  Returns (result, unlit, h, layers, manifest)."""
    swm = EV.swatches_module()
    sh = EV.sheet_layers(swm.SEED, mixer)
    man = swm.manifest_dict(sh)
    man["mixer"] = mixer
    H = int(round(W * swm.ASPECT[1] / swm.ASPECT[0]))
    cv, _ = EV.replay(sh.layers, W, H, mix.color_spec_to_rgb(swm.GROUND))
    return cv.rgb, cv.h, sh.layers, man


def _eval_doc(unlit, h, layers, man, W, name, **light_kw):
    lit = Lt.relight(unlit, h, **light_kw)
    res = EV.evaluate_sheet(lit, unlit, h, layers, man, W)
    doc = dict(harness=dict(version=EV.HARNESS_VERSION, git="test"), config=dict(name=name, width=W, light_name=name, seed=1907, mixer=man["mixer"]),
               sheet=res)
    return doc, lit


def _synthetic_paintings(dirpath, n=3, W=900, H=650):
    """Clearly synthetic stand-ins (no real painting): colour fields with directional streak texture.  For testing `eval reference` only."""
    rng = np.random.default_rng(5)
    for i in range(n):
        y, x = np.mgrid[0:H, 0:W].astype(np.float32)
        base = np.stack([0.35 + 0.25 * np.sin(x / (170 + 40 * i)), 0.40 + 0.2 * np.cos(y / 140), 0.55 + 0.2 * np.sin((x + y) / 200)], -1)
        streak = cv2.GaussianBlur(rng.normal(0, 1, (H, W)).astype(np.float32), (0, 0), sigmaX=14 + 6 * i, sigmaY=1.6)
        img = np.clip(base + 0.06 * streak[..., None] / max(streak.std(), 1e-6), 0, 1)
        cv2.imwrite(os.path.join(dirpath, f"synthetic_{i}.png"), cv2.cvtColor((img * 255).astype(np.uint8), cv2.COLOR_RGB2BGR))
    with open(os.path.join(dirpath, "notes.txt"), "w") as f:
        f.write("not an image; must be ignored\n")


def t8(OUT, gate):
    print("T8 evaluation harness")
    t_start = time.perf_counter()
    items = []
    out = os.path.join(OUT, "t8_eval")
    os.makedirs(out, exist_ok=True)
    swm = EV.swatches_module()
    prev_backend = mix.backend()
    try:
        # ---- 1. the swatch sheet: content, manifest, determinism ------------------------------------------------------
        a, b, c = swm.build_sheet(swm.SEED), swm.build_sheet(swm.SEED), swm.build_sheet(swm.SEED + 1)
        man = swm.manifest_dict(a)
        names = [s["name"] for s in man["swatches"]]
        need = ["mode_paint", "mode_scumble", "mode_smudge", "mode_glaze", "dabs", "dry_tail", "long_straight", "long_curved", "blend_wet_in_wet",
                "blend_wet_on_dry", "blend_complementary", "gradient", "strata_sky", "cylinder", "sphere"]
        okbox = all(0 <= s["box"][0] < s["box"][2] <= 1.0 and 0 <= s["box"][1] < s["box"][3] and s["tests"] for s in man["swatches"])
        gate("T8 swatch sheet has every required swatch, boxes in canvas-width units, each says what it tests",
             all(n in names for n in need) and len(set(names)) == len(names) and okbox, f"{len(names)} swatches, missing {[n for n in need if n not in names]}")
        sig = lambda sh: [(s.pts.tobytes(), s.zcol.tobytes(), s.seed) for l in sh.layers for s in l["strokes"]]
        gate("T8 same seed -> bit-identical strokes; other seed -> different strokes", sig(a) == sig(b) and sig(a) != sig(c),
             f"{sum(len(l['strokes']) for l in a.layers)} strokes")

        # ---- 2. flaws 1 and 2 are detected: same strokes, two shade-blur settings --------------------------------------
        W = 1600
        unlit, h, layers, man = _sheet_result(W)
        d_hi, lit_hi = _eval_doc(unlit, h, layers, man, W, "shade_blur=0.0006", shade_blur=0.0006)
        d_lo, lit_lo = _eval_doc(unlit, h, layers, man, W, "shade_blur=0.002", shade_blur=0.002)
        sa_hi, sa_lo = d_hi["sheet"]["strata_sky"], d_lo["sheet"]["strata_sky"]
        gate("T8 hairline metric: strata sky at shade_blur 0.0006 >= 3x the 0.002 value (flaw 1)",
             sa_hi["hairline"] >= 3 * max(sa_lo["hairline"], 1e-4) and d_hi["sheet"]["_overall"]["hairline"] > 1.5 * d_lo["sheet"]["_overall"]["hairline"],
             f"strata {sa_hi['hairline']:.3f} vs {sa_lo['hairline']:.3f}; whole sheet {d_hi['sheet']['_overall']['hairline']:.3f} vs {d_lo['sheet']['_overall']['hairline']:.3f}")
        gate("T8 seam metrics: outline hairlines of overlapping strokes clearly higher at 0.0006 (deep: >2x and > 0.03 vs ~0)",
             sa_hi["seam_hairline_deep"] > 0.03 and sa_hi["seam_hairline_deep"] > 2 * sa_lo["seam_hairline_deep"] + 0.01 and sa_hi["seam_hairline"] > 1.8 * sa_lo["seam_hairline"],
             f"seam {sa_hi['seam_hairline']:.3f}/{sa_hi['seam_hairline_deep']:.3f} vs {sa_lo['seam_hairline']:.3f}/{sa_lo['seam_hairline_deep']:.3f}")
        gate("T8 bristle metrics: the blurred (silky) light has clearly less bristle texture (flaw 2), height-based bristle is light-independent",
             d_lo["sheet"]["_overall"]["bristle_L"] < 0.75 * d_hi["sheet"]["_overall"]["bristle_L"]
             and d_lo["sheet"]["_overall"]["bristle_relief"] < 0.5 * d_hi["sheet"]["_overall"]["bristle_relief"]
             and abs(d_lo["sheet"]["_overall"]["bristle_h"] - d_hi["sheet"]["_overall"]["bristle_h"]) < 1e-6,
             f"bristle_L {d_hi['sheet']['_overall']['bristle_L']:.2f} -> {d_lo['sheet']['_overall']['bristle_L']:.2f}, relief {d_hi['sheet']['_overall']['bristle_relief']:.2f} -> {d_lo['sheet']['_overall']['bristle_relief']:.2f}")
        errs = [k for k, v in d_hi["sheet"].items() if isinstance(v, dict) and "error" in v]
        gate("T8 no swatch metric raised an error", not errs, f"errors in {errs}")
        # resolution independence: the same strokes at 1250 px give about the same numbers
        u2, h2, l2, m2 = _sheet_result(1250)
        d2, _ = _eval_doc(u2, h2, l2, m2, 1250, "w1250")
        rel = lambda x, y: abs(x - y) / max(abs(x), 1e-6)
        pairs = [(d_hi["sheet"]["_overall"]["hairline"], d2["sheet"]["_overall"]["hairline"]),
                 (d_hi["sheet"]["strata_sky"]["seam_hairline"], d2["sheet"]["strata_sky"]["seam_hairline"]),
                 (d_hi["sheet"]["_overall"]["C_mean"], d2["sheet"]["_overall"]["C_mean"]),
                 (d_hi["sheet"]["blend_wet_in_wet"]["pk060"]["green_excursion"], d2["sheet"]["blend_wet_in_wet"]["pk060"]["green_excursion"])]
        gate("T8 metrics are resolution independent: hairline, seams, chroma, blend green agree within 12% between 1250 and 1600 px",
             all(rel(x, y) < 0.12 for x, y in pairs), ", ".join(f"{x:.3f}/{y:.3f}" for x, y in pairs))

        # ---- 3. blend correctness: Mixbox green vs an RGB mixer ---------------------------------------------------------
        ur, hr, lr, mr = _sheet_result(1250, mixer="rgb")
        dr, _ = _eval_doc(ur, hr, lr, mr, 1250, "rgb")
        mix.set_backend("mixbox")
        z_mb, z_rgb = d2["sheet"]["blend_wet_in_wet"]["pk060"], dr["sheet"]["blend_wet_in_wet"]["pk060"]
        gate("T8 blend correctness: blue over wet yellow goes green with Mixbox (a* < both parents, on the mixing curve) and not with an RGB mixer",
             z_mb["green_excursion"] > 15 and z_mb["off_curve_dE"] < 6 and z_rgb["green_excursion"] < 5 and z_rgb["off_curve_dE"] > 3 * z_mb["off_curve_dE"],
             f"mixbox green {z_mb['green_excursion']:.1f} off-curve {z_mb['off_curve_dE']:.1f}; rgb green {z_rgb['green_excursion']:.1f} off-curve {z_rgb['off_curve_dE']:.1f}")
        dv = d2["sheet"]["_derived"]
        gate("T8 wet-in-wet mixes more than wet-on-dry (dry_after), and more pickup mixes more",
             dv["wet_dry_mix_contrast"] > 0.08 and dv["pickup_monotonic"], f"contrast {dv['wet_dry_mix_contrast']:.2f}, pk060 contrast {dv['wet_dry_mix_contrast_pk060']:.2f}")

        # ---- 4. metrics on synthetic images: they must move when the thing they name is there --------------------------
        rng = np.random.default_rng(3)
        Hs, Ws = 260, 1600
        base = np.full((Hs, Ws, 3), 0.55, np.float32)
        grain = cv2.GaussianBlur(rng.normal(0, 1, (Hs, Ws)).astype(np.float32), (0, 0), 0.0007 * Ws)
        grain = grain / grain.std()
        rough = np.clip(base + 0.03 * grain[..., None], 0, 1)
        rough2 = np.clip(base + 0.06 * grain[..., None], 0, 1)
        m_s, m_r, m_r2 = (EM.image_metrics(x, detail=False) for x in (base, rough, rough2))
        gate("T8 bristle_L: 0 on a silky field, positive on a bristle-scale grain and proportional to its amplitude",
             m_s["bristle_L"] < 0.05 and m_r["bristle_L"] > 0.8 and 1.7 < m_r2["bristle_L"] / m_r["bristle_L"] < 2.3,
             f"silky {m_s['bristle_L']:.3f}, grain {m_r['bristle_L']:.2f}, double grain {m_r2['bristle_L']:.2f}")
        un = np.full((Hs, Ws, 3), 0.5, np.float32)
        hh = np.ones((Hs, Ws), np.float32)
        lit_line = un.copy()
        for y0 in range(30, 250, 45):
            lit_line[y0:y0 + 2, :, :] *= 0.8                 # five 2 px dark lines, 20% down, right across the painting
        plain, line = EM.image_metrics(un.copy(), un, hh, detail=False), EM.image_metrics(lit_line, un, hh, detail=False)
        gate("T8 hairline: ~0 on an even lighting term, jumps for a 2 px dark line; deep-line share appears",
             plain["hairline"] < 0.001 and line["hairline"] > 0.02 and line["hairline_deep"] > 0.02, f"{plain['hairline']:.4f} -> {line['hairline']:.4f}, deep {line['hairline_deep']:.4f}")
        yy, xx = np.mgrid[0:200, 0:200]
        circ = ((xx - 100) ** 2 + (yy - 100) ** 2) < 45 ** 2
        hexa = np.zeros((200, 200), np.uint8)
        cv2.fillPoly(hexa, [np.array([[100 + 50 * np.cos(2 * np.pi * k / 6 + 0.3), 100 + 50 * np.sin(2 * np.pi * k / 6 + 0.3)] for k in range(6)], np.int32)], 1)
        dc, dh = ES._ellipse_dev(circ), ES._ellipse_dev(hexa > 0)
        gate("T8 dab faceting: round dab ~0, hexagon > 0.05 (flaw 3)", dc < 0.02 and dh > 0.05, f"circle {dc:.3f}, hexagon {dh:.3f}")

        # ---- 5. compare: regression exit codes, thresholds file ---------------------------------------------------------
        td = tempfile.mkdtemp(prefix="oilpaint_t8_")
        pa, pb, pw = (os.path.join(td, n) for n in ("a.json", "b.json", "w.json"))
        json.dump(EV._clean(d_lo), open(pa, "w"))
        json.dump(EV._clean(d_hi), open(pb, "w"))
        dw = json.loads(json.dumps(EV._clean(d_lo)))
        dw["config"]["width"] = 1234
        json.dump(dw, open(pw, "w"))
        with open(os.devnull, "w") as dn:
            old = sys.stdout
            sys.stdout = dn
            try:
                rc_reg = EV.main(["compare", pa, pb])               # hairlines appear: regression
                rc_same = EV.main(["compare", pa, pa])
                rc_better = EV.main(["compare", pb, pa, "--thresholds", os.path.join(ROOT, "eval", "thresholds.json"), "--json", os.path.join(td, "r.json")])
                rc_width = EV.main(["compare", pa, pw])
            finally:
                sys.stdout = old
        th = json.load(open(os.path.join(ROOT, "eval", "thresholds.json")))
        okth = all(r["direction"] in ("lower", "higher", "match", "true", "info") and "match" in r for r in th["rules"]) and "_doc" in th
        gate("T8 compare: exit 1 on regression (hairlines appear), 0 for identical runs, 2 for different widths; thresholds file is well formed",
             rc_reg == 1 and rc_same == 0 and rc_width == 2 and okth, f"exit codes {rc_reg}, {rc_same}, {rc_width} (reverse direction: {rc_better}); {len(th['rules'])} rules")
        strict = os.path.join(td, "strict.json")
        json.dump(dict(rules=[dict(match="sheet._overall.C_mean", direction="match", fail=dict(abs=0.0))]), open(strict, "w"))
        dc2 = json.loads(json.dumps(EV._clean(d_lo)))
        dc2["sheet"]["_overall"]["C_mean"] += 0.5
        pc = os.path.join(td, "c.json")
        json.dump(dc2, open(pc, "w"))
        with open(os.devnull, "w") as dn:
            old = sys.stdout
            sys.stdout = dn
            try:
                rc_custom = EV.main(["compare", pa, pc, "--thresholds", strict])
            finally:
                sys.stdout = old
        gate("T8 compare honours a custom thresholds file", rc_custom == 1, f"exit {rc_custom}")

        # ---- 6. determinism and memory --------------------------------------------------------------------------------
        det = EV.check_determinism(1907, "mixbox")
        gate("T8 determinism: identical strokes, near-identical image, reproducible planner",
             det["strokes_identical"] and det["seed_changes_strokes"] and det["image_near_identical"] and det["planner_identical"],
             f"max |diff| rgb {det['replay_max_abs_diff_rgb']:.1e}, height {det['replay_max_abs_diff_h']:.1e}")
        mem = EV.probe_memory(400, "mixbox")
        gate("T8 peak-memory probe runs in a fresh process", "engine_mb" in mem and mem["engine_mb"] > 5, f"{mem.get('engine_mb', float('nan')):.0f} MB for {mem.get('mpx', 0):.2f} Mpx")

        # ---- 7. the CLI end to end + reference on synthetic stand-ins ---------------------------------------------------------
        r = subprocess.run([sys.executable, "-m", "oilpaint", "eval", "run", "--width", "700", "--no-checks", "--no-speed", "--quiet", "--out", os.path.join(out, "cli_run")],
                           cwd=ROOT, capture_output=True, text=True, timeout=300)
        files = [os.path.exists(os.path.join(out, "cli_run", f)) for f in ("eval.json", "contact_sheet.png", "summary.md")]
        gate("T8 `eval run` writes eval.json, contact_sheet.png and summary.md", r.returncode == 0 and all(files), f"exit {r.returncode} {files} {r.stderr.strip()[-200:]}")
        sd = os.path.join(td, "synthetic_refs")
        os.makedirs(sd)
        _synthetic_paintings(sd)
        ref_out = os.path.join(out, "reference_synthetic.json")
        rr = subprocess.run([sys.executable, "-m", "oilpaint", "eval", "reference", sd, "--out", ref_out, "--synthetic"], cwd=ROOT, capture_output=True, text=True, timeout=300)
        ok_ref = False
        if rr.returncode == 0:
            rj = json.load(open(ref_out))
            ok_ref = rj["n_images"] == 3 and rj["synthetic"] and all(k in rj["suggested_calib_targets"] for k in ("L_p50", "chroma_mean", "chroma_p90", "L_p01", "L_p99")) \
                and "bristle_L" in rj["summary"] and "SYNTHETIC" in rj["notes"]
        gate("T8 `eval reference DIR` computes image metrics and calibration targets (synthetic stand-ins, offline)", ok_ref, f"exit {rr.returncode} {rr.stderr.strip()[-200:]}")
        Lt.save_png(os.path.join(out, "sheet_shade_blur_0.0006.png"), lit_hi)
        Lt.save_png(os.path.join(out, "sheet_shade_blur_0.002.png"), lit_lo)
    finally:
        mix.set_backend(prev_backend)
    print(f"  T8 took {time.perf_counter() - t_start:.1f}s")
    return items
