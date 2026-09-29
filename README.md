# oilpaint

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
python -m oilpaint test all                                          # checkpoints T1-T6, gates -> out/checkpoints/
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
docs/STYLE_REFERENCE.md   PLAN.md   RESEARCH.md
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

## Platform notes

Developed and tested on Linux (Python 3.11).  The C kernel is compiled on first use with `gcc` or `clang`
(`CC` overrides).  **Windows:** the easiest route is WSL2 (Ubuntu: `sudo apt install gcc ffmpeg python3-venv`,
then `pip install -r requirements.txt`); native Windows needs MSYS2/MinGW-w64 `gcc` on PATH and has not been tested.
`ffmpeg` is only needed for the time-lapse (`imageio-ffmpeg` bundles a copy).
