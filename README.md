# oilpaint

## Brush authoring preview

The Rust/WASM authoring tools now include seven **retained** brush presets, named stroke groups with stable IDs
and local editing, and an interactive paint preview. From this directory run `npm run build:wasm`, then
`npm run preview`, and open <http://127.0.0.1:4173>.

See [authoring API, format and controls](docs/BRUSH_AUTHORING.md), the
[native/browser review gallery](docs/reports/brush-review-3/index.html), and
[verification and measured latency](docs/reports/BRUSH_ITERATION_3.md).
All seven brushes are now held stable. Engine dev.7 integrates them into region/pass planning and adds a resolved `oil-composition` format for precise local edits. Open [the composition workbench](http://127.0.0.1:4173/packages/oilpaint/preview/composition.html) after launching the server above. See the [planner API and workflow](docs/PLANNER_AUTHORING.md), [still-life review](docs/reports/planner-integration/index.html), and [verification report](docs/reports/PLANNER_INTEGRATION.md).

The separate author 2 and composition 1 source formats keep existing paintings intact and enforce exact engine versions. Previous studies require their original engine. This work does not complete the remaining renderer roadmap.

**Ochrell integration:** `--mixer ochrell` now runs the existing planner, bristle
renderer and relighting with persistent synthetic K/S material states. It needs
Rust and the sibling `../ochrell` source directory, but no C compiler or Mixbox
installation. See [setup, architecture, limitations and reproduction](docs/OCHRELL.md).
The original default and its notices below remain in place.

An offline, stroke-based oil painting renderer aimed at a Monet-like look: a scene description (soft target
image + regions + flow fields + per-region styles + a layer schedule) is planned into an ordered list of
brush strokes, which a small C kernel paints onto a canvas that holds pigment-space colour, paint height and
wetness.  Strokes have bristle texture, pick up wet paint, leave ridges, and can be scumbled, smudged or
glazed.  A relighting pass adds canvas weave, raking light and gloss.  The same stroke list replays at any
resolution, so a 600 px preview is representative of the 2400 px final, and a time-lapse can be captured.

Colour mixing: **Mixbox** (c) 2022 Secret Weapons, CC BY-NC 4.0, https://scrtwpns.com/mixbox — non-commercial
use only; the attribution is written into every `run.json`.  The permissive WGM (libmypaint-derived) mixer that
the plan mentioned was dropped for time; `--mixer rgb` (plain RGB lerp) remains as an A/B baseline.

## Requirements

Python 3.11, numpy, scipy, opencv-python, scikit-image, imageio + imageio-ffmpeg, gcc or clang, ffmpeg, and
`pip install pymixbox`.  The C kernel (`oilpaint/csrc/brush.c`) is compiled automatically on first import
(`gcc -O3`, ~0.2 s) and cached next to the source.  Two CPU cores and < 1.5 GB RAM are enough for 2400x3000.

## Quick start (run from this directory)

```
python -m oilpaint test all                                          # checkpoints T1-T8 (T8 = the evaluation harness), gates -> out/checkpoints/
python -m oilpaint guides scenes/storm_light.py                      # target / regions / flow / light maps -> out/guides/
python -m oilpaint render scenes/storm_light.py --preview --out out/preview           # 600x750, ~35 s
python -m oilpaint render scenes/storm_light.py --size 2400x3000 --strokes out/preview/strokes.npz \
        --out out/final --timelapse out/final/timelapse.mp4          # replay the preview's strokes at full size + video
```

Without `--strokes` a full-size render re-plans (planning always happens at `--plan-width`, default 600, so
the result is the same strokes; `--strokes` just skips the 30 s).

### The iteration loop

```
python -m oilpaint render scenes/storm_light.py --preview --upto 8 --out out/p                  # stop after layer 8
python -m oilpaint render scenes/storm_light.py --preview --only 11 --from out/preview --out out/p11
        # re-plan layer 11 only, over the saved state of layers 1-10; layers 12-15 are replayed unchanged
python -m oilpaint relight out/preview --bump 2 --contrast 0.5 --spec 0.15 [--matte] [--light -0.5,-0.6,0.62]
python -m oilpaint crop out/final --box 0.52,0.30,0.82,0.62 [--unlit] [--layer 8]              # native-res crop
python -m oilpaint info out/final                                                              # summary + diagnostics
```

Every render prints one line: size, stroke count, timings, gates, L* min/median, mean chroma, cool/warm hue
lobe shares, relief coverage, mud score.  Other options: `--seed`, `--plan-width`, `--mixer mixbox|rgb`,
`--no-light`, `--no-layers`, `--video-size`, `--fps`, `--no-captions`, `--frames-per-layer`, `--hold`,
`--bump`, `--contrast`, `--spec`, `--cavity`, `--weave-amp`, `--light`, `--matte`, `--quiet`.

### Outputs of a run (`out/<run>/`)

`final.png` (lit), `final_unlit.png`, `final_900.png` (720x900 copy), `lit_unlit.png` (side by side),
`height.png` / `height.npy`, `normals.png`, `unlit.npy`, `layers/Lnn_name.png` (lit, after each layer),
`layers_sheet.png` (contact sheet of all layers), `guide_*.png` + `guides_sheet.png`, `strokes.npz` (the stroke
list, replayable at any size), `states/after_Lnn.npz` (plan-size canvas after each layer, for `--only`),
`run.json` (parameters, timings, attribution, metrics, summary), `metrics.json`, `timelapse.mp4` (if asked).

## Writing a scene

See `docs/STYLE_REFERENCE.md` for every parameter and an annotated walk through `scenes/storm_light.py`.
In short: a scene module defines `build(S)`; it paints a soft target image (`S.target.fill/blob/polygon/glow/
beam`), declares regions with soft masks (`S.region`), a flow field per region (`S.flow`), a light map
(`S.light`), a style per region (`S.style`: palette, brush width/length, direction rules, paint behaviour) and
the layer schedule (`S.layers([Layer(...), ...])`).  Coordinates and sizes are fractions of the canvas width.

## Layout

```
oilpaint/__main__.py   CLI            oilpaint/render.py    pipeline, states, --only, time-lapse capture
oilpaint/scene.py      scene DSL      oilpaint/maps.py      guide maps (target, regions, flow, light)
oilpaint/planner.py    stroke planner oilpaint/canvas.py    canvas state + ctypes wrapper of the kernel
oilpaint/csrc/brush.c  the kernel     oilpaint/mix.py       Mixbox (vectorised), Lab, tube palette
oilpaint/light.py      weave, normals, relighting            oilpaint/timelapse.py   frame capture + ffmpeg
oilpaint/metrics.py    gates and diagnostics                 oilpaint/calib.py       diagnostic targets (guesses)
scenes/storm_light.py  the picture   tests/checkpoints.py, tests/t5_coarse2fine.py   visual checkpoints + gates
oilpaint/eval*.py      evaluation harness (see "Evaluating changes")     scenes/swatches.py   the standard swatch sheet
tests/t8_eval.py       harness self-tests                                eval/                thresholds, baselines, reference calibration
docs/STYLE_REFERENCE.md   PLAN.md   RESEARCH.md
docs/plans/            plans for the next phase (library, Rust engine, physics)   spikes/   evidence experiments for those plans
```

## Engine notes and limits

- Planning runs in Python (~3 ms per stroke); a 9k-stroke preview plans in ~25 s.  Rendering is the C kernel
  (5-6 Mpix/s single core with the lane model); a 2400x3000 replay of 9k strokes takes ~27 s plus ~2 s per lit
  layer PNG; peak memory ~0.8 GB.
- Surface model (v2, "bristle lanes"): each stroke is a bundle of bristle lanes in stroke coordinates. Lanes
  define the ragged outline (plus stray hairs), the ridge/furrow relief, the edge levees, the start blob, the
  parallel colour streaks (two-colour load `marble`/`load2`, tube inhomogeneity `streak`, and lane-wise limited
  pick-up), and the broken coverage of a dry tail; the paint body fills between lanes while the brush is loaded
  and opens up as it runs dry. Multi-scale roughness (`stiff`) makes the paint read as stiff, not gel. The
  previous smooth-ribbon model (v1) was removed rather than kept behind a flag.
- Height: a stroke lays its own surface on the paint level under it (`flatten` = replace share), so thickness
  stays bounded over many layers. Lighting uses two-scale normals (broad form + fine lanes), a satin highlight
  that fades on thin paint, and a small relief tint; `relight` reruns it alone.
- Region boundaries: strokes stop at soft-mask fades in the planner (`stop_at_edge`) and a `spill` fraction runs
  through; the kernel never gates per segment (that produced combs of cliffs).
- Metrics: `gates` are mechanical (no black L* < 8, no clipping); everything under `diagnostics` is compared to
  guesses in `oilpaint/calib.py` and is advice, not a test.


## Reproducing "Storm Light" (scenes/storm_v3.py)

```
python -m oilpaint render scenes/storm_v3.py --preview --out out/v3          # ~40 s, 600x750, 15 layers
LIGHT="--bump 0.95 --bump-fine 1.0 --shade-blur 0.002 --contrast 0.30 --spec 0.10 --cavity 0.02"
OILPAINT_TITLE="Storm Light" python -m oilpaint render scenes/storm_v3.py --size 2400x3000 \
    --strokes out/v3/strokes.npz $LIGHT --out out/final \
    --timelapse out/final/timelapse.mp4 --video-size 1200x1500 --fps 24 --frames-per-layer 20 --hold 0.7
```

The lighting flags matter: with the engine defaults the raking light turns the long sky strokes into
dark hairline "strata".  A softer shadow term (`--shade-blur 0.002`, `--bump-fine 1.0`, `--contrast 0.30`) keeps the
bristle direction without those lines.  `--bump-fine`, `--shade-blur` and `--hsmooth` are available on `render`
and `relight`.  `OILPAINT_TITLE` sets the closing caption of the time-lapse.

Lessons from making this scene (details in `docs/STYLE_REFERENCE.md`): give each `curve` layer a flow that is
well defined at its start points (a radial flow is undefined at its centre, so beam strokes fanned out); keep the
sky's `levee` at 0 and `ridge` ~0.4; put a light source's halo in the same layer as the object in front of it so
the object paints over the halo; keep flecks low (<= 0.05) or small dabs turn into faceted crystals.

## Evaluating changes

A measuring stick for engine changes: paint a fixed swatch sheet (and optionally a real scene), measure it, and diff two
runs.  Everything is defined in canvas-width (cw) units, so a number means the same at any resolution once the feature
is resolvable (>= 1250 px wide; below that the bristle and hairline scales fall under one pixel and the run says so).

```
python -m oilpaint eval run --name mychange                          # sheet + checks: ~20 s -> out/eval/mychange/{eval.json,contact_sheet.png,summary.md}
python -m oilpaint eval run --name mychange --scene scenes/storm_v3.py --scene-width 600,1600    # + Storm Light: plan once, evaluate the preview and a 1600 px replay (~100 s)
python -m oilpaint eval compare eval/baseline_v1.json out/eval/mychange/eval.json               # per-metric deltas; exit 1 on regression, 2 if widths differ
python -m oilpaint eval run --sheet-run out/eval/mychange/sheet --shade-blur 0.002 --name m2    # re-light a saved sheet, no repainting (~6 s)
python -m oilpaint eval run --no-sheet --run out/storm_v3 --light run --name storm             # evaluate an existing render directory
python -m oilpaint eval reference DIR                                # metrics of real paintings -> calibration JSON (never edits calib.py)
python -m oilpaint test t8                                           # the harness's own checks (18 gates, ~30 s, offline)
```

**Read the numbers like this.** Compare like with like: same `--width`, same light (`--light default` = the engine defaults, which
show every flaw; `--light painting` = the flags of the real painting, `--bump 0.95 --bump-fine 1.0 --shade-blur 0.002 --contrast 0.30
--spec 0.10 --cavity 0.02`; any single light flag overrides the preset).  A change that helps under one light and hurts under the other
is a trade, not a win.  Timings on a shared machine are noisy: repeat before believing a speed WARN.  Run the same seed twice and every
number except speed is identical.

**The sheet** (`scenes/swatches.py`, 17 swatches, ~210 explicit strokes, renders in ~1 s at 1600 px) uses hand-placed strokes, so only
the kernel, the mixer and the lighting can move it, never the planner.  `python scenes/swatches.py` writes `strokes.npz` and
`manifest.json` (name, box in cw units, what it tests).  Swatches: the four modes (paint, scumble, smudge, glaze) at three widths on a toned ground;
dabs; a dry-brush tail; long straight and curved strokes; blue over yellow wet-in-wet (three pickups), the same wet-on-dry (`dry_after`),
complementary pairs and an 8-stroke mud stack; a smooth gradient; overlapping rows of long sky strokes (the hairline test); a cylinder
and a sphere (light on a form); ten strokes piled up (height).

| metric | what it measures (scale in cw) | flaw it watches |
|---|---|---|
| `hairline`, `hairline_deep`, `hairline_p99` | share of painted pixels where the *lighting term* (log lit/unlit luminance, albedo cancels) has a thin dark valley deeper than 3 % / 8 %; valley = blur 0.0025 minus blur 0.0007 | 1 strata hairlines |
| `seam_hairline`, `seam_hairline_deep` | the same, only on the outlines of the overlapping long strokes (strata swatch) | 1 |
| `bristle_L`, `bristle_relief`, `bristle_h` | RMS of the band 0.0004..0.0012 of lit L*, of the lighting term, of paint height | 2 silky look |
| `facet_ellipse_dev`, `facet_straight_frac` | dab silhouette against its best-fit ellipse (0.01 round, 0.11 pentagon); share of outline that is straight | 3 faceted dabs |
| `edge_step_p90/p99`, `width_ratio`, `edge_rough`, `edge_kink`, `outline_step` | height step across stroke edges; painted vs authored width; edge raggedness in stroke widths | edges |
| `mix_t`, `off_curve_dE`, `chroma_vs_curve`, `green_excursion`, `zone_mud_frac` | overlap colour against the Mixbox mixing curve between the measured top and under colours (works for any backend: an RGB mixer is off the curve); blue over wet yellow must go green | blends, mud |
| `wet_dry_mix_contrast`, `pickup_monotonic` | wet-in-wet mixes more than wet-on-dry; more pickup mixes more | wet vs dry |
| `C_*`, `L_*`, `hue_lobes`, `cool/warm/green_frac`, `mud_frac` | chroma and lightness percentiles, hue histogram (24 bins), grey-brown share | colour |
| `coh_local`, `dir_R`, `orient_deg`, `acf_minor/major`, `stroke_stats` | structure-tensor coherence (sigma 0.002/0.012), regional direction consistency, autocorrelation patch size, stroke width / length from the stroke list | direction, stroke size |
| `form_corr_lit`, `relief_leak`, `tone_range_ratio` | cylinder / sphere: lit tone against the intended form; relief noise on it | lighting on forms |
| `gradient.step_dE_p95`, `dry_tail.tail_over_mid`, `mode_*` | visible steps in a ramp; dry-tail coverage; scumble, smudge and glaze descriptors | modes |
| `system.*` | determinism (identical strokes and image for one seed, reproducible tiny plan), replay strokes/s, light seconds, peak MB per Mpx (fresh process) | speed, memory |

**Thresholds** live in `eval/thresholds.json` (copy it and pass `--thresholds mine.json`).  Each rule has an fnmatch `match` on the
flattened key (e.g. `sheet.strata_sky.hairline`, `scenes.storm_v3@1600.regions.sky.bristle_L`; `*` also matches dots; first matching rule wins),
a `direction` (`lower` / `higher` is better, `match` = any drift is bad, `true` = must hold, `info` = ignored) and `warn` / `fail` tolerances
`{abs, rel}` meaning how much worse than run A is tolerated, `max(abs, rel*|A|)`.  Keys with no rule are not compared.  A is the baseline, B the candidate.
`--strict` makes WARN fail; `--all` prints every covered metric; `--json FILE` keeps all rows.

**Baselines** (engine commit 8fad3fc): `eval/baseline_v1.json` (+ `baseline_v1_contact_sheet.png`) is the default light, sheet at 1600 px plus the Storm Light
preview at 600 and its replay at 1600; `eval/baseline_v1_painting.json` is the same strokes under the painting light.  Regenerate with the commands above.

Worked example, flaws 1 and 2 (same strokes, only `--shade-blur` differs; `eval compare` exits 1 for B = 0.0006 because hairlines appear):

| metric (default light otherwise) | shade_blur 0.002 | 0.0006 |
|---|---|---|
| strata swatch `hairline` / `seam_hairline_deep` | 0.033 / 0.000 | 0.180 / 0.087 |
| whole sheet `hairline` / `hairline_deep` | 0.119 / 0.009 | 0.247 / 0.100 |
| whole sheet `bristle_L` / `bristle_relief` (silkier when blurred) | 1.79 / 1.65 | 2.96 / 4.73 |
| Storm Light 1600, painting light: whole picture `hairline` / `hairline_deep`; sky `hairline` | 0.099 / 0.002; 0.044 | 0.182 / 0.044; 0.110 |

`bristle_h` (height) does not change with light, so it separates "the engine made less bristle relief" from "the light hides it".  Real
paintings (`eval/reference_calibration.json`, four Monet images from Wikimedia Commons, 1600 px, provenance in `eval/reference_provenance.json`,
`eval/fetch_reference.py` re-downloads them) span `bristle_L` 0.9 (Houses of Parliament in the Fog) to 2.7 (a Belle-Ile storm), mean 1.9, and are far duller and darker than
the guessed targets in `calib.py` (mean chroma 11..15, median L* 40..55).  `eval reference DIR` prints the values and `suggested_calib_targets`;
`--synthetic` marks a directory of stand-ins.  A scan carries varnish, glare and JPEG artefacts, so treat it as a range, not a target.

**What it cannot detect yet.** Flaw 3 (faceted flecks) is measured on the dab swatch only, not on the scene's flecks.  Flaw 4 (comb-like drips at region edges,
concentric ribbon rings on the rock) and flaw 5 (petal-like radial halos) come from the planner and its flow fields; the sheet cannot exercise
the planner, and on scenes there are only descriptors (`coh_local`, `dir_R`, `orient_deg`, `stroke_stats`, `edge_step_p99` per region) with no
detector for a ring or petal pattern.  Also missing: overall composition and colour harmony against a target, and any perceptual comparison to
a specific painting.  The reference set is four images: use it as a range.  Speed is the kernel replay only (planning time is recorded for scenes,
single run).

## Platform notes

Developed and tested on Linux (Python 3.11).  The C kernel is compiled on first use with `gcc` or `clang`
(`CC` overrides).  **Windows:** the easiest route is WSL2 (Ubuntu: `sudo apt install gcc ffmpeg python3-venv`,
then `pip install -r requirements.txt`); native Windows needs MSYS2/MinGW-w64 `gcc` on PATH and has not been tested.
`ffmpeg` is only needed for the time-lapse (`imageio-ffmpeg` bundles a copy).
