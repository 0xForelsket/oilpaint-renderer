# PLAN: "oilpaint" - an offline Monet-style oil painting renderer

Status: Phase 1 plan for review. Nothing of the renderer is built yet. Measurements quoted here were run in this
container (see RESEARCH.md section 0); everything else is design.

## 1. Summary and recommended architecture

Build a **scene-guided stroke-based renderer with a reservoir brush, pigment-space (Mixbox) paint, and a height field**.
The scene is authored as a small Python module that produces *guide maps* (a soft target image, region masks, a
per-region flow field, a light map) plus a *style table* (per-region palette and stroke rules) and a *15-layer schedule*.
A planner walks the schedule and emits a stroke list: coarse-to-fine, error-driven placement in the spirit of Hertzmann
1998, but stroke paths follow the authored flow field instead of image gradients, and stroke colours are snapped to
mixtures of a Monet tube palette. A small C kernel (built with gcc, called through ctypes) replays the strokes onto a
canvas held in Mixbox latent space: each dab has a per-stroke bristle texture, picks up wet paint from the canvas into
the brush reservoir, deposits by lerp in latent space, and adds/levels a height field. Planning happens once at a fixed
low resolution; rendering replays the same stroke list at preview or final resolution, so previews are representative.
Finishing is a relighting pass (canvas weave + paint height -> normals -> Blinn-Phong with a raking light and a cavity
term). The time-lapse is captured from the render as frames per layer and encoded with ffmpeg.

Alternatives considered and rejected:

| Alternative | Why not |
|---|---|
| Image filters (anisotropic Kuwahara, "oil paint" filter) over a guide image | Fast, but no paint body, no layered build-up, no time-lapse; looks like a filter. Kept only as an optional flattening of the *target*. |
| Pure Hertzmann (strokes follow image gradients only) | Gradients are undefined in flat sky/sea; the result converges to the guide image (photo-like); no regional stroke rules; no pick-up. Its coarse-to-fine, error-driven placement is kept. |
| Full physical paint (IMPaSTo/Stuyck shallow water, Wetbrush bristles) | Designed for interactive GPU use; on 2 CPU cores a 2400x3000 fluid step is seconds each, and we never drag a brush interactively. The stamp + reservoir model (what Stuyck's shipping app and DAB do) gives pick-up, smear and ridges at a fraction of the cost. |
| Neural (Stylized Neural Painting, Paint Transformer, Gatys style transfer, diffusion) | Torch CPU and Hugging Face weights are reachable (verified), but the painting models' weights are not on reachable hosts, style transfer needs Monet images the user must supply, and CPU inference at 2400x3000 takes hours and still yields no paint body or layers. Not recommended; noted as a possible future guide-image step. |
| Keep p5.brush in a browser | Cannot read the canvas back for pick-up, no height map, uniform strokes; that is why we are here. |

## 2. Composition: what we paint

Subject kept from the first attempt: "Storm Light", a red-and-white banded lighthouse on a rock, right of centre, storm
breaking, one lamp lit (Fl(2) W 12s), warm glow low on the left horizon, blue-teal sea with gold reflections, foam, rain.

**Proposed change toward Monet** (say yes/no): make weather and sea the picture and the tower a "found" form.
- Horizon at ~46 % of height (was 42 %): more sea, which is where Monet's Belle-Ile pictures live; sky still dominant.
- Tower centre at x = 0.67, lantern at y = 0.31; the tower's *lit* left edge is warm and reads clearly, its right edge
  dissolves into the storm sky through pick-up and cross-region strokes; only the middle red band reads as red, the lower
  band is broken by spray and the upper by haze.
- The beam is not a wedge: it is a *lightening and warming of the sky colour* (a scumbled peach-over-violet band, wider
  and shorter than before, ~30 % of the width to the left of the lantern, then dissolving), with a few dragged strokes
  along its axis. Real Monet has almost no cast light rays; the light is in the colour temperature.
- Spray and foam rise from the rock base to roughly the tower's first third, painted in pale violet-white broken masses
  with upward flicks; this is what hides the tower's base and the rock's outline.
- Rain as diagonal *veils* (long, very low-opacity dry-brush drags at 70 deg) rather than lines; a few lines allowed
  only in the last layer.
- Rock: a dark violet-blue mass with warm ochre lit facets on the left, "flame-like" curved strokes.
Reason: the first attempt's main failure (a crisp geometric tower) was partly the engine, partly composition: the tower
was drawn as a shape. Monet's forms are built from adjacent strokes of different colour and later strokes cross the
boundary. Putting ~85 % of the picture into sky, sea and spray and letting those layers overrun the tower is the
composition-level fix; the engine-level fixes are in section 4.

## 3. Scene description and authoring

Recommendation: **a Python scene module** (like the previous `scene.js`, but declarative), because the target image
needs procedural gradients, noise and soft shapes that are painful in JSON, and you already write scenes this way.
A raster route also exists: `Scene.from_images(target.png, regions.png, flow.npy?)` for hand-painted guides.

A scene module defines `def build(S: SceneBuilder) -> None` and calls:

```python
# scenes/storm_light.py  (worked example, abbreviated; coordinates are normalised 0..1, sizes are fractions of width)
from oilpaint.scene import *

def build(S):
    S.canvas(aspect=(4, 5), ground=mix("lead_white", ("cobalt_violet", 0.06), ("yellow_ochre", 0.04)))
    S.palette("monet", tubes=["lead_white", "cadmium_yellow", "cadmium_orange", "vermilion", "madder",
                              "cobalt_blue", "ultramarine", "cobalt_violet", "viridian", "emerald", "yellow_ochre"])
    H = 0.46                                   # horizon (fraction of height)
    LANTERN = (0.67, 0.31)

    # --- target image: soft procedural fields, painted in the order they stack ---
    S.target.fill(gradient_v([(0.0, "#3a3b78"), (0.5, "#7676ab"), (H - 0.05, "#e5cdc2"), (H, "#f1d8b8")]))
    S.target.blob(0.25, H - 0.02, rx=0.45, ry=0.10, color="#f4bd90", softness=0.6, strength=0.7)   # horizon glow
    for cx, cy, rx, ry, c in STORM_MASSES:                                                          # dark violet masses
        S.target.blob(cx, cy, rx, ry, color=c, softness=0.5, noise=0.7, strength=0.85)
    S.target.beam(LANTERN, angle_deg=188, spread_deg=14, length=0.35, color="#fbe3b8", strength=0.35)
    S.target.fill(gradient_v([(H, "#d9c3c8"), (H + 0.15, "#7fa0c6"), (1.0, "#2f5490")]), below=H)   # sea
    S.target.reflection(LANTERN, glow_x=0.25, color="#f6cf98", strength=0.5)
    S.target.polygon(ROCK_PTS, color="#4a4a7c", softness=0.02, noise=0.4)
    S.target.polygon(TOWER_PTS, color="#e9d8d6", softness=0.015)
    S.target.bands(TOWER_PTS, [(0.36, 0.44, "#c8503f"), (0.54, 0.62, "#c8503f")])
    S.target.glow(LANTERN, r=0.06, color="#fff0b8", strength=1.0)

    # --- regions: soft masks (blur radius = how "lost" the edge is) ---
    S.region("sky",    mask=above(H), edge=0.05)
    S.region("storm",  mask=union(*[ellipse(*m[:4]) for m in STORM_MASSES]), edge=0.08)
    S.region("glow",   mask=ellipse(0.25, H - 0.02, 0.45, 0.12), edge=0.10)
    S.region("beam",   mask=wedge(LANTERN, 188, 14, 0.35), edge=0.06)
    S.region("sea",    mask=below(H), edge=0.02)
    S.region("foam",   mask=band_around(ROCK_PTS, 0.05) & below(H), edge=0.03)
    S.region("spray",  mask=ellipse(0.66, 0.62, 0.16, 0.10), edge=0.08)
    S.region("rock",   mask=polygon(ROCK_PTS), edge=0.015)
    S.region("tower",  mask=polygon(TOWER_PTS), edge=lambda y: 0.008 + 0.03 * (1 - y))  # softer toward the top
    S.region("lantern", mask=disc(LANTERN, 0.03), edge=0.02)

    # --- flow fields (unit vectors) ---
    S.flow("sky",   sweep(angle_deg=-12, curl=0.4, noise=0.25, scale=0.5))     # long arcs, left-high to right-low
    S.flow("storm", swirl_around(STORM_MASSES, strength=0.7, noise=0.3))
    S.flow("beam",  radial_from(LANTERN))
    S.flow("sea",   waves(base_angle=0, amplitude_deg=18, wavelength=0.12, noise=0.3, perspective=True))
    S.flow("foam",  waves(base_angle=0, amplitude_deg=35, wavelength=0.05, noise=0.5))
    S.flow("spray", upward(noise=0.6))
    S.flow("rock",  contour(ROCK_PTS, noise=0.35))            # tangent of the distance transform
    S.flow("tower", constant(90, noise=0.08))
    S.flow("lantern", radial_from(LANTERN))

    # --- light map: warm irradiance drives colour temperature and paint thickness ---
    S.light.add(glow=(0.25, H - 0.02, 0.5), beam=(LANTERN, 188, 14, 0.4), lamp=(LANTERN, 0.08))

    # --- per-region style: palettes, stroke rules ---
    S.style("sky",   colors=[("cobalt_blue", "lead_white", 0.55), ("cobalt_violet", "lead_white", 0.5),
                             ("ultramarine", "madder", 0.2)], flecks=[("cadmium_orange", 0.08)],
            width=(0.030, 0.050), length=(0.10, 0.30), curvature=0.3, align=0.85, opacity=(0.8, 1.0),
            pickup=0.25, load=1.0, height=0.7)
    S.style("storm", colors=[("ultramarine", "madder", 0.25), ("cobalt_violet", "ultramarine", 0.4)],
            flecks=[("cadmium_orange", 0.05)], width=(0.020, 0.035), length=(0.05, 0.14), curvature=0.6,
            align=0.7, pickup=0.35, height=0.6)
    S.style("sea",   colors=[("cobalt_blue", "viridian", 0.35), ("ultramarine", "lead_white", 0.3),
                             ("viridian", "lead_white", 0.5)], flecks=[("cadmium_yellow", 0.10), ("vermilion", 0.03)],
            width=(0.012, 0.028), length=(0.02, 0.06), curvature=0.2, align=0.9, pickup=0.2, height=0.9)
    S.style("rock",  colors=[("ultramarine", "madder", 0.3), ("cobalt_violet", "ultramarine", 0.5),
                             ("yellow_ochre", "lead_white", 0.4)], width=(0.015, 0.030), length=(0.03, 0.08),
            curvature=0.8, align=0.8, pickup=0.3, height=1.0)
    S.style("tower", colors=[("lead_white", "cobalt_violet", 0.15), ("vermilion", "madder", 0.25)],
            flecks=[("viridian", 0.04), ("cadmium_orange", 0.06)], width=(0.010, 0.020), length=(0.03, 0.09),
            curvature=0.05, align=0.95, pickup=0.35, height=0.8, edge_override=0.35)  # 35 % of neighbours may cross
    # ... foam, spray, glow, beam, lantern styles ...

    S.layers(MONET_15)   # the schedule from section 4.6, or a custom list of Layer(...) entries
```

Everything in `S.style` and in layers is in canvas-width units (`width=0.02` = 2 % of the canvas width; Monet's 1-1.5 cm
flats on a 66 cm canvas are 1.5-2.3 %). Colours are either hex (snapped to the palette at plan time) or
`("tube_a", "tube_b", t)` Mixbox mixtures. The engine rasterises all of this at the planning resolution and the
rendering resolution independently, so the scene never mentions pixels.

`python -m oilpaint guides scenes/storm_light.py` writes `guide_target.png`, `guide_regions.png`, `guide_flow.png`
(flow drawn as short lines), `guide_light.png` in a few seconds so you can check what the scene *means* before painting.

## 4. Rendering pipeline, stage by stage

### 4.1 Guide maps
From the scene: `target` (float RGB, soft), `region_id` (uint8) plus per-region soft masks (float), `flow` (2 x float,
unit vectors; where a region has no authored flow, use the structure-tensor orientation of the target, Kyprianidis-style),
`light` (float, 0..1), `size_scale` (float, default 1; lets you shrink strokes near detail), `wet` (updated during
painting). Optional target pre-flattening with an anisotropic Kuwahara pass (`--flatten`), off by default.

### 4.2 Canvas state (all float32, at render resolution)
- `lat[7]` Mixbox latent per pixel (202 MB at 2400x3000, fine), `rgb[3]` shadow copy kept in sync by the kernel,
  `h` paint height, `wet` wetness (0..1), `cover` (paint coverage over ground, for the ground-shows-through breaks).
- Ground: Mixbox mixture set by the scene; initial height = canvas weave (section 4.7) at low amplitude.

### 4.3 Planner (Python, runs once at plan resolution, default 600x750)
For each layer in the schedule:
1. `reference = blur(target, sigma = f_s * R)` with R = layer's mean stroke half-width in pixels (Hertzmann).
2. Placement, one of:
   - `error`: grid of spacing `f_g * R` with jitter; start a stroke at the max-error pixel of each cell whose mean
     |canvas - reference| exceeds T (in Lab). Canvas here is the planner's own proxy canvas (the same C kernel at
     plan resolution).
   - `density`: jittered grid / Poisson-disc samples inside the region mask with probability from a density map
     (used for decorative layers: broken colour, flecks, scumbles, rain).
   - `curve`: start points along a curve (region boundary, horizon, beam axis) for smudge and edge passes.
3. Path: from the start point, step `0.5 * width` along `flow` rotated by a per-stroke angle jitter
   (`(1 - align) * N(0, 40 deg)`), with an IIR curvature filter (`curvature`) and small per-step noise; stop at the sampled
   length, at a region boundary with probability from the neighbour's `edge_override`, or (in `error` mode) by
   Hertzmann's colour criterion. Width profile: ramp in over 15 % of the length, taper out over the last 25 %, plus
   load depletion (section 4.4).
4. Colour: sample `reference` at the start (or the mean along the path for long strokes); convert to Lab; **palette
   snap**: nearest of ~20k precomputed Mixbox mixtures of the region's listed colour pairs (+ white, + a little of each
   other tube), via a scipy cKDTree in Lab; then jitter (L +-4, a/b +-3 in Lab) and with probability `flecks[i].p`
   replace by that fleck colour. The `light` map warms the colour (toward the glow colour, weight = light * warmth).
5. Emit `Stroke(layer, region, pts[(x, y, w, p)], lat_color[7], brush params, seed)` in normalised coordinates, in
   *painterly order* (per region priority, then a coarse left-to-right/top-to-bottom sweep with jitter; Hertzmann
   randomises, but a painter does not, and the time-lapse should look like painting).
6. Render the stroke on the proxy canvas immediately (so `error` placement sees it). Save `strokes.json` (or .npz).

Expected stroke counts (design targets, to be measured at M4): block-in layers 300-800 strokes each, sky/sea
layers 1.5-3k, broken-colour and spray layers 3-6k, total **20-35k strokes** regardless of output resolution.

### 4.4 Brush model (C kernel, `csrc/brush.c`)
A stroke is a polyline of samples `(x, y, width, pressure)` in pixels. Rendering walks it with spacing
`spacing * width` (default 0.25) and splats an **oriented stamp** at each step.

- **Bristle texture per stroke**: at stroke start, generate `T[nb][ns]` (nb = 6..24 bristles across, ns = along
  samples): each bristle has a random across-offset, a random weight, a random dropout run-length (gaps that produce
  the ribbed, dry-brush look), and a load that decays along the stroke. The stamp alpha at a pixel is
  `T(u, s) * edge(u) * pressure(s) * opacity`, with `u` the across coordinate in [-1, 1], `s` the along coordinate,
  `edge(u)` a soft rectangle. Because T is fixed per stroke the ridges run the length of the stroke, which is what
  makes a mark read as a hog-bristle stroke rather than a blurred line. Flat-brush footprint: soft rectangle; round
  brush: soft ellipse; both use the same T.
- **Reservoir**: brush state `Z_b[7]` (latent colour) and `V` (load, 1.0 = full). Per dab:
  1. pick-up: `Z_avg` = alpha-weighted mean of canvas latent under the stamp; `w = pickup * mean(wet under stamp)`;
     `Z_b += w * (Z_avg - Z_b)` (libmypaint smudge rule; IMPaSTo's "transfer toward the side with more paint" collapses
     to this for a fixed-rate offline brush).
  2. deposit: `a = alpha * min(1, V / V_dry)` (V_dry = load below which the brush runs dry, default 0.2);
     `Z_c += a * (Z_b - Z_c)`; `rgb_c = mixbox_poly(Z_c)`; `wet = max(wet, a)`;
     `cover += a`.
  3. depletion: `V -= deplete * mean(alpha)`; at low V the stroke thins and breaks up (T dropouts scale with 1 - V).
  4. height: `h += a * hgain * thick(V) * T(u, s)`, and levelling `h -= a * flatten * (h - h_avg_under_stamp)` so a
     loaded stroke pushes and levels wet paint beneath it (cheap stand-in for IMPaSTo advection). This addresses
     Hertzmann's "hidden strokes surface" problem without giving up additive ridges.
- **Modes** (per layer or per stroke): `paint` (as above), `scumble`/dry-brush (`V` low, opacity low, alpha additionally
  multiplied by `smoothstep(h - h_blur)` so paint catches only ridges: the Pourville "skips across the high points"),
  `smudge` (`V = 0`, `pickup = 1`: drags existing paint along the path with a decaying trail; used to dissolve edges),
  `glaze` (very low alpha, no height, no pick-up: thin transparent tint over a dry layer, e.g. the haze).
- **Drying**: between layers `wet *= dry_factor` (0 = fully dry before the next layer, as in wet-over-dry sessions).
  Pick-up strength is proportional to `wet`, so wet-in-wet layers blend and wet-over-dry layers sit crisp on top.
- **Region gating**: the kernel receives the region-id map and a per-stroke list of allowed ids plus an override
  probability; a dab whose centre lands on a disallowed region is skipped with probability `1 - override`, which
  is how neighbouring layers cross the tower silhouette in a broken way.

### 4.5 Paint mixing
Mixbox latent space throughout (verified vectorised port; the C kernel gets the 786 kB LUT and the polynomial). Tube
colours are the 13 pigment RGBs published in the pymixbox docstring (cadmium yellow, hansa yellow, cadmium orange,
cadmium red, quinacridone magenta, cobalt violet, ultramarine, cobalt blue, phthalo blue, phthalo green, permanent
green, sap green, burnt sienna) plus lead white, vermilion, madder, viridian, emerald and yellow ochre entered as
standard sRGB swatch values of those paints (Mixbox accepts any RGB; only the mixing behaviour is pigment-based). Mixing rules: canvas deposit and pick-up are lerps in latent; palette snapping precomputes mixtures.
Switchable backends: `--mixer mixbox` (default), `wgm` (libmypaint's 10-band weighted geometric mean, ISC; ported from
the tables already saved in `exp/helpers.c`), `rgb` (for A/B tests only). Licence: Mixbox is CC BY-NC 4.0; fine for a
hobbyist's personal picture, attribution "Mixbox (c) Secret Weapons" goes in the README and the run metadata; the
`wgm` backend exists so that nothing breaks if the reviewer prefers a permissive stack.

### 4.6 Fifteen-layer schedule (design choice; each layer is a `Layer(...)` and can be edited in the scene)

| # | Name | Regions | Brush width (% cw) | Length (% cw) | Placement | Mode / blend | Notes |
|---|---|---|---|---|---|---|---|
| 1 | Toned ground | all | 8 | 20-40 | density, sparse | paint, opacity 0.5, no height | barely-visible priming strokes so the ground is not a flat fill; wet |
| 2 | Ebauche, dark masses | storm, rock, sea | 5-7 | 15-35 | error, T high | paint, thin (hgain 0.2), load 0.6 | dilute block-in, big soft brush, follows flow |
| 3 | Ebauche, light masses | sky, glow, beam, sea far | 5-7 | 15-35 | error, T high | paint, thin, pickup 0.5 | wet-into-wet with 2; edges melt. **Dry to 0.3 after** |
| 4 | Sky, long dragged strokes | sky, glow, beam | 3-5 | 10-30 | error | paint, pickup 0.25, hgain 0.7 | arcs along the sky flow; 8 % orange flecks |
| 5 | Storm masses, swirling | storm | 2-3.5 | 5-14 | error | paint, pickup 0.35 | curved strokes around the masses, edges blend into 4 |
| 6 | Sea underpainting strokes | sea | 3-4 | 6-14 | error | paint, pickup 0.2 | horizontal with wave undulation; far sea lighter, smaller |
| 7 | Rock | rock, foam | 1.5-3 | 3-8 | error | paint, hgain 1.0 | contour-following, violet/blue shadow, ochre lit facets |
| 8 | Tower masses | tower, lantern | 1-2 | 3-9 | error | paint, pickup 0.35 | vertical; white bands tinted warm left / violet right; red = vermilion+madder. **Dry to 0.4 after** |
| 9 | Sea, broken colour and wave forms | sea, foam | 1.2-2.5 | 2-6 | error + density | paint, hgain 0.9 | short choppy dabs, gold reflection flecks, dark troughs |
| 10 | Glow and beam scumble | glow, beam, sky near horizon | 4-5 | 8-20 | density | scumble, opacity 0.35 | peach/gold dry-brush over violet; the beam appears here |
| 11 | Spray, foam, mist | spray, foam, tower lower third | 1-2 | 1-4 | density | paint + 20 % smudge | pale violet-white masses and upward flicks; smudge drags from foam up the tower base |
| 12 | Broken colour over sky | sky, storm, glow | 1-1.5 | 1-3 | density | paint, pickup 0.1 | complementary flecks: orange in violet, violet in gold; the "vibration" layer |
| 13 | Lost edges and haze | tower silhouette, horizon, beam edges, far sea | 1.5-3 | 3-8 | curve | smudge (pickup 1, load 0) then glaze (pale violet, alpha 0.08) over the distance | dissolves every hard boundary; atmospheric perspective |
| 14 | Lantern, glints, final impasto | lantern, sea glints, foam crests, tower lit edge | 0.6-1.2 | 1-2.5 | density | paint, hgain 1.6, load 1 | single colour + white thick touches (Monet's last marks) |
| 15 | Rain and last accents | sky, sea | 0.6-1 | 6-15 (veils), 2-4 (lines) | density, sparse | scumble opacity 0.15 at 70 deg; a few thin drags | very light; must not read as hatching |

Every layer can be disabled or reordered; `--upto N` stops after layer N; `--only N` renders one layer on top of a
saved state for quick iteration.

### 4.7 Height map, canvas weave, lighting (Python/numpy/cv2, `light.py`)
- Weave: two orthogonal sine gratings at 19-20 threads/cm scaled to the canvas (a 66 cm canvas at 2400 px gives ~1.8 px
  per thread; at preview it collapses to fine noise) plus low-frequency slub noise; amplitude 0.15 of a typical stroke
  height. Weave visibility is attenuated where paint is thick: `h_total = h + weave * exp(-h / h0)`.
- Normals from Sobel of `h_total` with a bump gain (default 4); light from upper-left (`L = (-0.5, -0.6, 0.62)`, the
  usual gallery raking light); diffuse `0.55 + 0.45 * n.l`; Blinn-Phong specular `0.15 * (n.h)^40`, white (varnish
  gloss, not coloured); cavity darkening `0.25 * clamp(h_blur - h)` in the valleys; no vignette, no sharpening.
  Measured cost 0.43 s at full resolution. `relight` re-runs this on a saved run with new parameters without repainting.
- A `--matte` option reduces specular to 0.05 for a poppy-oil, unvarnished look (Monet's *Oatfields* was unvarnished).

### 4.8 Time-lapse (`timelapse.py`)
Frames are captured from the render at a fixed frame budget per layer (default 40, scaled by the layer's stroke count so
big layers get more), lit with the cheap path (diffuse only during the layer, full lighting at the layer's last frame),
downsampled to the video size (default 1200x1500, option 864x1080), with a 1 s hold and an optional layer caption at
each layer end. Encoded with ffmpeg (libx264, yuv420p, 24 fps, crf 20) from a raw pipe through imageio-ffmpeg. About
600-750 frames, 30-40 s. At preview size the same frames give a quick draft video.

### 4.9 Outputs of a run (`out/<name>/`)
`final.png` (lit), `final_unlit.png`, `height.png` (grey), `normals.png`, `layers/L01_toned_ground.png` ... `L15_*.png`
(lit, after each layer), `guide_*.png`, `strokes.npz`, `run.json` (all parameters, seed, timings, metrics), `timelapse.mp4`
(when `--timelapse`), `metrics.json` (section 7).

## 5. Tech stack and performance

| Component | Where | Why |
|---|---|---|
| Scene DSL, guide rasterisation, flow fields, noise | Python + numpy + cv2 (+ opensimplex, MIT, optional) | vectorised, easy to author |
| Planner (placement, paths, palette snap) | Python + numpy + scipy cKDTree | per-stroke logic at ~20-35k strokes is cheap |
| Stroke rasteriser (bristle texture, pick-up, deposit, height, region gating, Mixbox polynomial) | **C**, one file `csrc/brush.c` (~500 lines), `gcc -O3 -march=native -ffast-math -shared -fPIC`, loaded with **ctypes** (no build system; `_build.py` compiles on first import and caches the .so next to the source; cffi is available if we want it later) | measured 1.5x (r=40) to 23x (r=4) faster than a numpy loop per dab, and the numpy figure did not even include the pick-up read, bristle lookup or latent conversion, which cost far more in numpy than in C; per-dab pick-up cannot be vectorised across strokes because each dab reads what earlier dabs wrote |
| Mixbox vectorised (palette snapping, one-off conversions) | numpy (already written in `exp/t3_mixbox_np.py`) | 1.4-3.3 Mpix/s is plenty off the hot path |
| Lighting, weave, cavity | numpy + cv2 | 0.43 s at full size |
| Video | imageio-ffmpeg / ffmpeg | present |

Two cores: stroke rendering within a layer is sequential by nature (each dab reads the canvas that earlier dabs wrote),
so the painting thread is single-core. The second core runs a `multiprocessing` worker that receives frames and does
the downsample + light + encode for the time-lapse and writes the per-layer PNGs, which keeps the painter from
stalling. I do not recommend tiling the canvas across processes: the halo bookkeeping for strokes up to 8 % cw wide is
error-prone and the sequential cost is already small. Everything is seeded (`--seed`) and deterministic.

Performance estimate (from the measurements in RESEARCH.md; to be confirmed at M0/M4):
- Dab-pixel budget: 15 layers, average coverage 60 % of the canvas, overlap factor ~3 (spacing 0.25 width) -> ~27 canvas
  areas of dab pixels: at 600x750 that is 12 Mpix, at 2400x3000 it is 195 Mpix.
- Kernel throughput: 43-67 Mpix/s measured at 600x750 and 11-45 Mpix/s at 2400x3000 (random dab positions; real
  strokes are spatially coherent, so the large-canvas figure is pessimistic) for RGB pick-up + deposit + height; the
  latent (7-channel) version with the bristle lookup and the Mixbox polynomial per touched pixel is my estimate at
  3-6x slower, i.e. **~8-15 Mpix/s**. This is the first number to measure at M0.
- Planner: ~0.3 ms per stroke in Python -> ~10 s for 30k strokes, plus the proxy render at plan size (~1-2 s).
- **Preview 600x750: ~15-30 s** end to end (plan + render + light). **Final 2400x3000: ~15-45 s render + ~10 s
  planning + ~1-2 min time-lapse encoding**, i.e. a few minutes. If the kernel turns out 3x slower than estimated the
  final is still under 5 minutes. These are extrapolations from the micro-benchmarks, not measured end-to-end times.
- Memory: latent 202 MB + rgb 86 MB + height/wet/cover/region ~120 MB + guides ~150 MB: under 1 GB.

## 6. Public interface

```
oilpaint/                      (package root, inside the scratchpad oilpaint/ dir)
  README.md                    usage, scene format reference, layer schedule reference, licences
  PLAN.md  RESEARCH.md
  oilpaint/__init__.py  __main__.py (CLI)
  oilpaint/scene.py            SceneBuilder, Region, Style, Layer, palette tubes, shape/flow/noise helpers
  oilpaint/maps.py             rasterise scene -> guide maps at a resolution
  oilpaint/planner.py          placement, path following, palette snap, stroke list
  oilpaint/brush.py            ctypes wrapper, Canvas state, layer loop, drying
  oilpaint/csrc/brush.c        the kernel;  oilpaint/_build.py compiles it
  oilpaint/mix.py              Mixbox (numpy), WGM, palette tables, Lab conversions
  oilpaint/light.py            weave, normals, Blinn-Phong, cavity, relight
  oilpaint/timelapse.py        frame capture worker + ffmpeg
  oilpaint/metrics.py          objective checks (section 7)
  scenes/storm_light.py        the picture;  scenes/test_*.py  synthetic test scenes
  tests/run_checkpoints.py     produces the visual checkpoint images
  out/<run>/...                outputs
```

CLI (`python -m oilpaint ...`):
- `render scenes/storm_light.py --preview` : plan + render at 600x750 into `out/preview/`.
- `render scenes/storm_light.py --size 2400x3000 --out out/final --timelapse --video-size 1200x1500`.
- Options: `--seed 1907`, `--plan-size 600x750`, `--upto 9`, `--only 12 --from out/preview` (repaint one layer over a
  saved state), `--layers a,b,c`, `--frames-per-layer 40`, `--mixer mixbox|wgm|rgb`, `--light -0.5,-0.6`, `--spec 0.15`,
  `--bump 4`, `--matte`, `--no-light`, `--flatten`, `--strokes out/preview/strokes.npz` (skip planning, replay).
- `guides scenes/storm_light.py [--size]` : write the guide maps only.
- `relight out/final --light 0.4,-0.7 --spec 0.1` : re-light a saved run.
- `test strokes|mix|smear|light|coarse2fine|scumble` : the checkpoint renders of section 7.
- `info out/final` : print run.json and metrics.

Python API for programmatic use: `oilpaint.render(scene_path, size=(600, 750), **opts) -> RunResult` with
`.final`, `.layers`, `.height`, `.strokes`, `.metrics`.

## 7. Test plan and visual checkpoints

| Test | What is produced | What to look for (eye) | Objective check |
|---|---|---|---|
| T1 single strokes (`test strokes`) | 3 widths x {straight, curved}, flat and round, lit and unlit, height map | ribbed bristle marks along the stroke, tapered ends, dry break-up at the end of a long stroke, ridges catching light | across-stroke alpha profile has >= 4 local maxima at width 2 % cw; along-stroke alpha at the tail < 50 % of the middle; no aliasing: max alpha step between neighbours < 0.35 |
| T2 mixing (`test mix`) | swatch grid: blue+yellow, violet+peach, sea+white, red+green, each at 5 ratios, Mixbox vs WGM vs RGB | blue+yellow goes through green, tints stay clean, no grey mud | Mixbox blue+yellow midpoint hue in 100-150 deg; RGB midpoint hue outside it (documents why) |
| T3 pick-up and smear (`test smear`) | two colour fields; a loaded brush crosses wet and dry halves; a smudge stroke drags across | colour trail on the wet side, clean on the dry side; smudge stroke blends without erasing texture | Lab colour along the trail is monotone toward the brush colour; smear extent on dry side < 15 % of the wet side |
| T4 lighting (`test light`) | crossing impasto strokes over weave, three light directions, matte vs gloss | ridges highlight on the lit side, weave only in thin areas | mean specular on ridges (h > p90) > 3x valleys; weave modulation amplitude in thick areas < 20 % of that in bare ground |
| T5 coarse-to-fine (`test coarse2fine`) | synthetic photo-like target (sphere on a gradient, soft edge) painted with 5 error layers, flow = structure tensor | edge softens rather than sharpens, layers refine visibly, strokes follow the sphere's contour | mean Lab error vs target decreases every layer; stroke angle histogram near the sphere edge peaks at the tangent |
| T6 scumble | dry-brush over T4's ridges | paint catches ridges only | correlation(alpha_applied, h) > 0.5 |
| T7 full preview lighthouse (`render --preview`) | final + 15 layer PNGs + guides + metrics + a contact sheet of the 15 layers | the sheet reads as a painting being built up; the tower is found, not drawn; stroke size and direction differ visibly between sky, sea, rock and tower; spray hides the rock base; the beam is warm colour, not a wedge | the run metrics below, all in range |

Objective metrics computed for every run (`metrics.json`):
- **No black**: minimum L* and the fraction of pixels with L* < 10 (target 0 %) and < 18 (target < 0.5 %).
- **Hue structure**: hue histogram of pixels with chroma > 12; expect two dominant lobes, violet-blue (240-300 deg) and
  gold-peach (30-60 deg); report their mass and the mass in green.
- **Chroma**: mean and p90 chroma (Monet-like: mean 15-30, p90 40-60 in Lab units; to be calibrated).
- **Edge softness**: mean gradient magnitude along the tower and rock silhouettes (from the region map) divided by the
  mean gradient magnitude at stroke edges overall; target < 1.3 (the silhouette should not be sharper than the strokes).
- **Stroke statistics** (from the stroke list): per-layer width and length distributions (they must differ between
  layers), per-region angle histograms (sea within +-20 deg of horizontal >= 70 %; sky spread > 40 deg).
- **Paint body**: fraction of pixels whose lit/unlit luminance differ by > 3 % (relief coverage), height p50/p95.
What only eyes can judge: whether it reads as paint and not a filter, whether the light is convincing, the mood, and
Monet-ness. I will produce side-by-side sheets for those judgements.

## 8. Milestones (each ends with images you can approve or redirect)

| M | Deliverable | Done when |
|---|---|---|
| M0 Scaffold | package layout, `_build.py` compiling `brush.c`, Canvas state, one dab and one stroke, `test strokes` | T1 images look like bristle strokes; measured latent-kernel Mpix/s reported |
| M1 Paint model | latent canvas, pick-up/deposit, load depletion, height with levelling, scumble/smudge/glaze modes, drying, lighting | T2, T3, T4, T6 pass with their objective checks |
| M2 Planner | guide maps from images, error and density placement, flow-following paths, palette snap, plan-once/replay | T5 passes; the same strokes replayed at 2x size look the same |
| M3 Scene DSL + guides | `scene.py`, `guides` command, `scenes/storm_light.py` v1 | you approve `guide_target.png`, `guide_regions.png`, `guide_flow.png` |
| M4 Fifteen layers at preview | full `render --preview`, per-layer PNGs, metrics | all objective metrics in range; you judge the sheet of 15 layers |
| M5 Final + time-lapse | 2400x3000 render, mp4, README | plays; timing within the estimate; README lets you use it alone |
| M6 Look iteration | your loop on the scene (and my tuning of brush defaults) | you say it is done |

## 9. Risks, quality ceiling, open questions

Candid ceiling: with this design the output should read as a **convincing digital oil painting with real paint body**
(ribbed strokes, ridges catching light, wet-in-wet edges, dry scumbles), in the class of a good Rebelle/Painter oil
render, and not as an oil-paint filter, because every pixel comes from an ordered stroke with pick-up and thickness.
Whether it reads as *Monet* depends less on the engine than on the scene: the colour choices, the stroke rules per region,
and restraint. The first attempt's palette was already decent; the plan carries it over as Mixbox mixtures.
What would move it up a tier: (1) real Monet references to calibrate the metrics (see below); (2) a proper wet-paint
smear (Stuyck-style shallow water at preview resolution is feasible in numpy but is a week of tuning); (3) scanned brush
and canvas textures instead of procedural ones; (4) many rounds of looking. What could make it fall a tier: uniform-looking
strokes (mitigated by per-region styles, wide width/length ranges, painterly ordering), colour mud from too much pick-up
(Monet blended little on the canvas: defaults keep pickup at 0.1-0.35 and confine strong smudging to layer 13), and a
target image that is too clean (every shape in the target must be soft and noisy; the engine never draws a contour).

Technical risks: the latent kernel may be slower than estimated (mitigation: keep a float16 rgb-only path; measure at
M0); the Hertzmann error criterion can over-cover flat regions and leave the ground invisible (mitigation: coverage cap
per layer and the `cover` map); region gating may leave visible seams (mitigation: soft masks and override
probabilities); time-lapse frames can bottleneck (mitigation: worker process, fewer frames).

Licence: Mixbox is CC BY-NC 4.0 - acceptable for a hobbyist's personal picture with attribution; the `wgm` backend
(ISC-derived tables) is the drop-in if the reviewer wants a permissive stack only. libmypaint is ISC (only ideas and two
tables reused). spectral.js is MIT (not used unless chosen). Everything else is our own code or standard libraries.

Reference images: attaching 2-3 real Monet images (ideally a Belle-Ile storm sea, a Pourville/Etretat cliff, and a
Rouen Cathedral or Houses of Parliament for dissolved architecture, at 1000+ px) would change the *calibration*, not the
architecture. I would measure: L*/chroma histograms and the hue lobes (to set the metric targets and the fleck rates),
the autocorrelation length of luminance in sky vs water (stroke size ratio), the orientation histograms per region from
the structure tensor (to set `align` and `curvature`), the share of near-white and darkest pixels, and the edge-gradient
ratio at object silhouettes. None of that can be fetched programmatically (Wikimedia, AIC and the Met are blocked or
rate-limited from here, verified).

## 10. Decisions for the reviewer

1. Composition: adopt the section-2 proposal (higher sea, tower at 0.67/0.31, beam as colour not wedge, spray up the
   tower, rain as veils) or keep the first attempt's layout?
2. Mixbox (CC BY-NC 4.0, personal use, attribution) as default, or the permissive WGM mixer only?
3. Scene format: Python module (recommended) or JSON + a Python plugin for procedural parts?
4. Time-lapse: 1200x1500 at 24 fps, ~35 s, layer captions on/off?
5. Default light: upper-left raking light with gloss (`spec 0.15`) or the matte unvarnished look?
6. May I `pip install --break-system-packages opensimplex` (MIT, pure Python) or should I write value noise myself?
7. Will you attach 2-3 Monet reference images for calibration? If yes, which ones (a Belle-Ile storm sea is the most
   useful).
8. Priority when in conflict: quality of the final render, or short preview turnaround (drives the default stroke
   counts and frame counts)?
9. Fifteen layers as scheduled above, or would you rather see a 10-layer version first and split later?
10. Anisotropic Kuwahara pre-flattening of the target: implement in M2 (optional flag) or drop it?
