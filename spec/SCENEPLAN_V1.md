# ScenePlan v1

The input to the planner: a declarative, versioned JSON description of a painting. The TypeScript DSL
(`packages/oilpaint/src/scene.ts`) builds it; the Rust scene compiler (`crates/oil-scene`) turns it into guide maps
(target image, region masks, flow field, light map); the planner (L3) turns guides, styles and layers into a
StrokeList.

- **Status:** frozen in L2.
  - The Rust types in `crates/oil-scene/src/spec.rs` are the definition.
  - They generate the JSON Schema [`sceneplan-1.schema.json`](sceneplan-1.schema.json) (draft-07; a test keeps it in
    sync), which generates the TS types (`packages/oilpaint/src/sceneplan.ts`, `npm run gen:types`).
  - The schema's descriptions are the parameter catalogue: every key, its unit and its default.
- **Example:** [`examples/storm_v3.sceneplan.json`](examples/storm_v3.sceneplan.json), written by
  `scenes/storm_v3.ts`.
- **Reproducibility:** a ScenePlan, the plan options `{seed, planWidth, mixer}` and an engine version determine the
  StrokeList bit for bit on every host (see `README.md`). The guide maps alone are checked across seven hosts by
  `ci/xhost` (cases `scene.*`).
- **Additions are compatible:** a new optional key or a new shape, flow or op kind keeps generation 1, since old
  plans stay valid. Changing the meaning of an existing key needs generation 2.

## Top level

```json
{
  "$schema": "https://oilpaint.dev/schema/sceneplan-1.json",
  "sceneplan": 1,
  "engine": "2.0.0-dev.2",
  "title": "Storm Light v3",
  "canvas": { "aspect": [4, 5], "ground": "#e9e1d6" },
  "target": [ ... ],
  "regions": [ ... ],
  "lights": [ ... ],
  "styles": { "sky": { ... } },
  "layers": [ ... ],
  "fields": { ... }
}
```

| Key | Required | Meaning |
|---|---|---|
| `sceneplan` | yes | schema generation, `1`; any other value is `UNSUPPORTED_SCENEPLAN` |
| `$schema` | no | ignored by the engine; lets editors validate |
| `engine` | no | engine version the scene was authored and tuned with. A different engine warns `ENGINE_VERSION_DIFFERS`; plan option `strictEngine: true` (L3) makes that an error. |
| `title` | no | copied into the StrokeList's `META` |
| `preset` | no | a style preset (`"impressionist"`); its style values sit between the engine's neutral defaults and the region styles (`UNKNOWN_PRESET` otherwise). Presets are data: `crates/oil-scene/presets/*.json` |
| `canvas` | yes | `aspect: [w, h]`, positive integers, at most 16:1; `ground`: a colour |
| `target` | yes | ordered target-image operations; the reference the planner paints toward |
| `regions` | yes | ordered list, 1 to 255 regions; later regions override earlier ones in the hard region map |
| `lights` | no | contributions to the light map (drives `warmth`, `opacityByLight`) |
| `styles` | yes | per-region style, keyed by region name (may be `{}`) |
| `layers` | yes | ordered layer schedule (may be `[]` for guides only) |
| `fields` | no | declarations of sampled fields (escape hatch) |

Unknown keys are errors (`SCHEMA`) everywhere, with a did-you-mean fix. snake_case spellings of camelCase keys and
v1's names (`T`, `fg`, `fs`, `L_floor`) get a fix naming the right key.

Plan options are not part of the spec: `seed` (u32, default 1907), `planWidth` (px, default 600), `mixer` (default
`ochrell`) and `strictEngine`. They are passed to `plan()` or the CLI.

Units follow `README.md`: cw coordinates, y down, angles in degrees (0 = +x, 90 = down), sRGB colours, u32 seeds. JSON
numbers are finite by construction.

## Colours

`"#rrggbb"` | `"tube_name"` | `[r, g, b]` with each channel in [0, 1] | `["a", "b", t]`: a mix of two named colours
at t in [0, 1], made by the active mixer (both encoded, their states lerped, the result decoded).

Tubes (v1's catalogue, 8-bit sRGB swatches): `lead_white`, `titanium_white`, `cadmium_yellow`, `hansa_yellow`,
`cadmium_orange`, `cadmium_red`, `vermilion`, `madder`, `quinacridone_magenta`, `cobalt_violet`,
`cobalt_violet_light`, `ultramarine`, `cobalt_blue`, `phthalo_blue`, `cerulean`, `phthalo_green`, `viridian`,
`emerald`, `permanent_green`, `sap_green`, `yellow_ochre`, `burnt_sienna`.

Mixes are the only place where guides depend on the mixer.

## The raster

Guides are compiled at a width W in pixels (the plan width); H = round(W x aspectH / aspectW), half up. Pixel (i, j)
is sampled at its centre ((i + 0.5) / W, (j + 0.5) / W) in cw. Blurs are Gaussian with a sigma given in cw and
converted to pixels (x W). The engine's blur downsamples first when sigma >= 8 px, as v1's `light.blur` did.

**Noise.** fbm(scale, octaves, seed) is value noise on a hashed lattice. It uses cells = 1 / scale per canvas width,
bicubic interpolation (Keys, a = -0.75), and octaves that double the frequency and halve the amplitude, normalised
by the sum of amplitudes. Values are about [0, 1]. Lattice values hash (seed, octave, i, j), so the field is the same
at every resolution and on every host. v1 drew its lattices from numpy's PCG64; the engine's fields are
statistically equivalent but a different realisation (L2 report).

## Shapes (masks in [0, 1])

A shape is a single-key object (externally tagged):

| Shape | Form | Value |
|---|---|---|
| all | `{"all": true}` | 1 |
| above / below | `{"above": y}`, `{"below": y}` | y_px < y, y_px >= y |
| ellipse | `{"ellipse": {"c": [x, y], "r": [rx, ry], "softness": 0}}` | d = elliptical distance; softness 0: d < 1, else clip((1 + s - d) / s) |
| disc | `{"disc": {"c": [x, y], "r": r, "softness": 0}}` | a circular ellipse |
| polygon | `{"polygon": [[x, y], ...]}` | 1 inside (even-odd rule, at pixel centres), 3 points or more |
| wedge | `{"wedge": {"apex": [x, y], "angle": a, "spread": s, "length": l}}` | clip(1 - \|da\| / s) x clip((l - d) / (l / 2)), da the angle off the axis |
| bandAround | `{"bandAround": {"polygon": [...], "dist": d}}` | dilate - erode of the polygon with a square of half-size k = max(1, floor(d W)) px |
| union / intersect | `{"union": [shape, ...]}`, `{"intersect": [...]}` | clip(sum), product |
| not | `{"not": shape}` | clip(1 - m) |
| noisy | `{"noisy": {"shape": shape, "amount": 0.3, "scale": 0.08, "seed": 11}}` | n = fbm(scale, 3, seed) - 0.5; clip(m + 4 a n [0.02 < m < 0.98] + a n / 2) (v1's formula, including its faint halo outside the shape) |
| field | `{"field": "name"}` | a sampled mask |

## Target operations

Applied in order onto an RGB image that starts black; the result is clipped to [0, 1].

| Op | Form (defaults shown) | Composite |
|---|---|---|
| fill | `{"fill": {"color": c}}` or `{"fill": {"gradientV": [[y, c], ...]}}`, optional `"mask": shape` | replace (blend by the mask). Gradient stops need increasing y; the gradient is clamped outside them |
| blob | `{"blob": {"c": [x, y], "r": [rx, ry], "color": c, "softness": 0.5, "noise": 0, "strength": 1, "seed": 21}}` | alpha = strength x clip((1 - d') / softness), d' = d x (1 + noise x (fbm(0.8 rx, 3, seed) - 0.5)) |
| polygon | `{"polygon": {"points": [...], "color": c, "softness": 0.01, "noise": 0, "strength": 1, "seed": 22}}` | the polygon, `noisy` (amount = noise, scale 0.05) if noise > 0, blurred by max(0.5 px, softness) if softness > 0 |
| bands | `{"bands": {"points": [...], "bands": [[f0, f1, c], ...], "softness": 0.008}}` | per band: polygon rows with y in [y0 + f0 h, y0 + f1 h), blurred by max(0.5 px, softness) |
| glow | `{"glow": {"c": [x, y], "r": r, "color": c, "strength": 1, "power": 2}}` | additive: alpha = strength x exp(-(d / r)^power) |
| beam | `{"beam": {"apex": [x, y], "angle": a, "spread": s, "length": l, "color": c, "strength": 0.4, "softness": 0.3}}` | additive: a wedge blurred by max(1 px, softness x spread x 0.02) |
| image | `{"image": {"field": "photo", "fit": "cover", "strength": 1, "mask": shape?}}` | replace (blend by strength x mask) with a picture: an `rgb` field, fitted by `cover` (fill, crop centred), `contain` (fit inside, centred; the target stays outside it) or `stretch`. Downscaling averages the picture's pixels under each guide pixel (exact areas); upscaling is bilinear |

The alpha blend is `img (1 - a) + c a` with a clipped to [0, 1]. The additive light is `img + a (c - img / 2)` with
a clipped to [0, 2].

## Regions

```json
{ "name": "sky", "shape": {"above": 0.6}, "edge": 0.03, "flow": {"sweep": {"angle": -14, "curl": 0.5, "noise": 0.3}} }
```

- **name:** unique (`DUPLICATE_NAME`), 1 to 64 characters.
- **Hard map:** a pixel belongs to the last region whose mask is > 0.5. Pixels in no region belong to region 0.
- **edge:** the soft mask is the clipped mask blurred by max(0.5 px, edge). Default 0.02 cw.
- **flow:** optional. Without one, the region follows the target's structure-tensor flow (and L5 warns
  `REGION_NO_FLOW`).

## Flows (unit direction fields)

| Flow | Form (defaults shown) | Angle |
|---|---|---|
| constant | `{"constant": {"angle": 0, "noise": 0, "seed": 1}}` | angle + 2 noise (fbm(0.15, 3, seed) - 0.5) |
| sweep | `{"sweep": {"angle": -12, "curl": 0.4, "noise": 0.25, "scale": 0.5, "seed": 2}}` | angle + 2 curl (fbm(scale, 2, seed) - 0.5) + 2 noise (fbm(0.35 scale, 3, seed + 1) - 0.5) |
| waves | `{"waves": {"angle": 0, "amplitude": 18, "wavelength": 0.12, "noise": 0.3, "perspective": true, "horizon": 0.5, "seed": 3}}` | see below |
| swirlAround | `{"swirlAround": {"centres": [[cx, cy, rx, ry], ...], "strength": 0.7, "noise": 0.3, "seed": 4}}` | strength x sum of tangents weighted exp(-0.7 d^2), plus (1 - strength) x constant(-8, noise, seed) |
| radialFrom | `{"radialFrom": {"c": [x, y], "noise": 0.1, "seed": 5}}` | away from c, + 2 noise (fbm(0.1, 2, seed) - 0.5) |
| upward | `{"upward": {"noise": 0.6, "seed": 6}}` | constant(-90, 1.2 noise, seed) |
| contour | `{"contour": {"polygon": [...], "noise": 0.35, "seed": 7}}` | along the outline: the gradient of the signed distance (exact EDT, blurred by max(1 px, 0.01)) + 90 degrees, + 2 noise (fbm(0.1, 3, seed) - 0.5) |
| facets | (planned for L9) | |
| field | `{"field": "name"}` | a sampled flow |

**waves:** depth = clip((y - horizon) / (canvas bottom - horizon)), or 1 without perspective. The wavelength is
wl = wavelength x (0.3 + 0.7 depth). The angle is angle + amplitude x sin(2 pi x / wl + 6 fbm(0.3, 2, seed)) +
2 noise (fbm(0.08, 3, seed + 1) - 0.5)(0.3 + 0.7 depth). v1 used the last pixel row, not the canvas bottom.

**Structure-tensor fallback:**
- It uses Sobel gradients of luma (0.299 R + 0.587 G + 0.114 B), with products blurred by max(1 px, 0.02).
- θ = ½ atan2(2 Jxy, Jxx - Jyy), and the flow is (-sin θ, cos θ).

**Evaluation (from engine `2.0.0-dev.3`).** Authored flows are functions of (x, y), evaluated where they are
needed, not rasters:
- A stroke of region R follows R's authored flow wherever it goes.
- Without one, it follows the global flow: the authored flow of the region owning that pixel, or else the
  structure-tensor fallback, which is the one flow kept as a raster (sampled bilinearly).
- `contour` uses the gradient of a smooth minimum of the distances to the polygon's edges: each edge's unit vector
  from its nearest point, weighted by exp(-(d - d_min) / 0.01 cw). It no longer blurs a distance raster.
- `waves` measures depth to the canvas bottom, aspect height / width, exactly.
- Soft masks are stored cropped to their non-zero box, as 16-bit fractions.

## Lights

Each entry adds to the light map, which is clipped to [0, 1]:

- `{"glow": {"c": [x, y], "r": r, "strength": 1}}`: strength x exp(-d²), d = |(dx, 2.5 dy)| / r (flattened
  vertically, as in v1).
- `{"beam": {"apex": [x, y], "angle": a, "spread": s, "length": l, "strength": 1}}`: a wedge blurred by
  max(1 px, 0.02).
- `{"lamp": {"c": [x, y], "r": r, "strength": 1}}`: strength x exp(-(d / r)²).

## Styles (per region)

Every key is optional. The defaults are the engine's neutral defaults: v1's planner defaults, except where they
carried Storm Light's taste (`lFloor`, `splay`), which now sits in the `impressionist` preset. Where v1's name
differs, it is given.

| Key | Default | Meaning |
|---|---|---|
| `colors` | from the target | palette-snapping targets (colours) |
| `flecks` | `[]` | `[[colour, probability], ...]` complementary touches |
| `width`, `length` | `[0.015, 0.03]`, `[0.04, 0.10]` | stroke size ranges in cw (above 0.5 and 2: `UNITS`) |
| `curvature` | 0.3 | 0 straight, 1 follows every turn of the flow |
| `align` | 0.85 | 1 strictly along the flow |
| `opacity` | `[0.85, 1.0]` | |
| `pickup`, `load`, `deplete`, `vdry`, `hgain`, `flatten`, `streak`, `body` | 0.12, 1.0, 0.02, 0.25, 1.0, 0.6, 0.25, 0.9 | brush behaviour (StrokeList fields of the same name) |
| `streakMix` (`streak_mix`) | 0.8 | |
| `hardness`, `grain`, `release`, `dropout`, `ragged` | 0.75, 0.10, 0.3, 0.02, 0.5 | |
| `nbPerCw`, `nbBase` (`nb_per_cw`, `nb_base`) | 450, 5 | bristle lanes = nbBase + nbPerCw x width (3..40) |
| `ridge`, `levee`, `furrow`, `blob`, `stiff` | 0.5, 0.35, 0.15, 0.3, 0.25 | surface relief (x hgain) |
| `marble`, `load2` | 0, null | two-colour load share, and colour B (null: an automatic lighter/warmer variant) |
| `splay` | 0.35 | stray hairs, 0..3 (v1's 1.0 read as pencil lines in pale areas) |
| `snap` | 0.85 | pull toward the nearest palette mixture |
| `jitter` | `[5, 4]` | Lab jitter [L, a/b] between strokes |
| `warmth`, `warmColor` (`warm_color`) | 0, `"#f6d09a"` | light-map warming |
| `priority` | 0 | order among regions within a layer (low first) |
| `reverseP` (`reverse_p`) | 0 | chance to run against the flow |
| `spill` | 0.15 | chance to keep painting across a region edge |
| `stopAtEdge` (`stop_at_edge`) | 0.9 | chance to stop where the soft mask fades |
| `minAspect` (`min_aspect`) | 2.5 | length ≥ minAspect x width unless the layer sets `dab` |
| `endWidth`, `endPressure` (`end_width`, `end_pressure`) | 0.5, 0.15 | stroke-end profile |
| `hgainJitter` (`hgain_jitter`) | 0.25 | ± relative thickness variation |
| `mode` | null | per-region override of the layer mode |
| `sizeByY` (`size_by_y`) | null | `[y0, y1, s0, s1]` perspective scaling |
| `opacityByLight` (`opacity_by_light`) | 0 | fade strokes away from the light |
| `lFloor` (`L_floor`) | 0 (impressionist: 20) | stroke colours never darker than this L* |
| `sizeByDetail` | 0 | 0..1: stroke width follows the reference's local detail (narrower where busy) within `width` |
| `lengthSkew` | 0 | ≥ 0: skews lengths toward the short end of `length` |
| `dabShare` | 0 | 0..1: share of strokes that are short dabs (1 to `minAspect` widths) |
| `endVariation` | 0 | 0..1: per-stroke variation of `endWidth` and `endPressure` |
| `flick` | 0 | 0..1: share of strokes ending in a curl of up to 40 degrees over their last third |
| `reliefByValue` | 0 | 0..1: paint thickness follows value: thick lights, thin darks |

## Layers

```json
{ "name": "Sky long strokes", "regions": ["sky", "glow"], "placement": "error", "errorThreshold": 12, "referenceBlur": 0.5, "gridFactor": 1.2 }
```

| Key | Default | Meaning |
|---|---|---|
| `name` | required | unique |
| `regions` | `"all"` | region names, or `"all"` |
| `placement` | `"error"` | `error` (where the canvas differs most from the target), `density` (even coverage), `curve` (along given curves) |
| `mode` | `"paint"` | `paint`, `scumble`, `smudge`, `glaze` |
| `errorThreshold` (`T`) | 18 | Lab error that triggers a stroke (error placement) |
| `gridFactor` (`fg`) | 1.0 | proposal grid cell, in stroke widths |
| `referenceBlur` (`fs`) | 0.5 | blur of the reference, in stroke widths |
| `spacing` | 1.6 | number or `{region: number}` (density placement), in stroke widths; regions not listed: 1.6 |
| `coverage` | 1.0 | number or `{region: number}`, share of the region to cover; regions not listed: 1.0 |
| `colorFrom` (`color_from`) | `"reference"` | `reference` (sample the target) or `palette` (pick from `colors`) |
| `order` | `"sweep"` | `sweep` (painterly sweeps) or `random` |
| `jitterPos` (`jitter_pos`) | 0.5 | position jitter within a grid cell |
| `maxStrokes` (`max_strokes`) | null | cap |
| `enabled` | true | |
| `seedOffset` (`seed_offset`) | 0 | vary one layer without changing the others |
| `hblurSigma` (`hblur_sigma`) | 0.01 | height blur before scumble, in cw |
| `relief` | 1.0 | thickness multiplier for the layer |
| `maxCover` (`max_cover`) | null | skip sites already covered more than this |
| `gapFill`, `gapCover` (`gap_fill`, `gap_cover`) | true, 0.15 | fill bare spots after the layer |
| `dab` | false | allow short, round dabs |
| `dryAfter` (`dry_after`) | null | wetness factor after the layer (replaced by open times in L11) |
| `dryThresh`, `dryWidth` (`dry_thresh`, `dry_width`) | 0, 0.15 | scumble: where paint catches on relief (height above its blur), smoothstep width |
| `curve` | null | `{region: [[x, y], ...] \| "boundary"}`; regions not listed, or no map at all: `"boundary"` |
| `curveOffset`, `curveSpacing`, `curveJitter` | 0, 1.0, 0.5 | curve placement |
| any style key | – | overrides the region styles for this layer (for example `width`, `opacity`, `colors`, `hardness`) |

## Sampled fields (escape hatch)

```json
"fields": { "sky_flow": { "kind": "flow", "width": 300, "height": 375, "sha256": "…" } }
```

- **Arrays:** the arrays are passed next to the spec, not inside it. They are little-endian f32, row-major, with
  channels interleaved: 1 channel for `mask`, 2 for `flow`, 3 for `rgb`.
  - The TS DSL samples a JS function at the pixel centres (`S.field(name, kind, fn, width)`).
  - The CLI reads raw files (`oil guides --field name=file.f32`).
- **Checks:**
  - Every declared field must be supplied (`UNKNOWN_FIELD`), with the right number of values (`RANGE`).
  - Its SHA-256 over those bytes must match (`FIELD_HASH_MISMATCH`).
  - Masks and flows are resampled bilinearly to the guide raster, and flows are renormalised.
- **Portability:** a plan that uses a field, or a hook, is marked `portable: false` in its StrokeList (L3).

## Validation

Validation runs before compiling:
- **Schema errors:** serde stops at the first one.
- **Semantic errors:** it reports all of them (ranges, cw units, colours, region and field references, duplicate
  names).

Each error is structured (`ERRORS.md`), for example:

```json
{"code":"UNITS","path":"/styles/sky/width/0","got":"25","expected":"(0, 0.5] cw",
 "message":"width[0] is too large for canvas widths",
 "fix":"coordinates and sizes are in canvas widths (cw): divide pixels by the canvas width in pixels: 25 px on a 600 px plan is 0.0417 cw"}
```

Messages and fixes are free text: they may improve without an engine-version bump. Agents branch on `code` and
`path`.

`oil scene validate FILE.json`, `oilpaint validate SCENE.ts` and `engine.validate(spec)` all run the same Rust
validator.

## Planning (from engine `2.0.0-dev.3`)

`oil plan`, `engine.plan()` and `oil_plan::plan` turn a ScenePlan into a StrokeList. The planner is a cutover
from v1's Python planner: the same ideas, restructured, with no parity requirement. Same inputs and engine version
give the same StrokeList bytes on every host (`ci/xhost`, cases `plan.*`).

**Plan options** (not part of the spec):

| Option | Default | Meaning |
|---|---|---|
| `seed` | 1907 | u32 |
| `planWidth` | 600 | width of the guides and of the proxy canvas, in pixels; strokes are stored in cw and replay at any size |
| `mixer` | `ochrell` | the mixer of the proxy canvas and of colour mixes; recorded in the StrokeList |
| `strictEngine` | false | refuse a plan whose `engine` differs from the running engine instead of warning |

**Per layer** (in order, disabled layers kept as empty ranges):
1. The layer's regions, sorted by resolved `priority` (stable).
2. If any of them paints in `scumble` mode, the proxy's height is blurred by `hblurSigma` once for the layer. The
   StrokeList layer records the sigma, so the painter does the same.
3. Per region, with its own random stream keyed by (seed, layer index, `seedOffset`, region index): editing one
   region leaves every other region's strokes unchanged.
   - **Sites.**
     - `error`: cells of `gridFactor` x half the mean stroke width, aligned to the canvas origin. A cell's error is
       0.6 x mean + 0.4 x max of the Lab difference between the proxy and the reference (the target blurred by
       `referenceBlur` x half the mean width), weighted by the soft mask. A cell with error above `errorThreshold`
       and mask cover above 0.3 gives one site at its worst pixel, jittered by `jitterPos`.
     - `density`: one candidate per `spacing` x mean-width cell, kept with probability equal to the soft mask.
     - `curve`: points every `curveSpacing` widths along the region's outline (soft mask > 0.5, outer boundaries)
       or a polyline, offset along the normal.
   - Then `coverage` thins the sites, `order` sorts them (painterly sweeps: bands of 3 widths, alternating
     direction, jittered; or random), and `maxStrokes` caps them.
   - **Per site:**
     - error placement re-checks the error (skip below half the threshold), and `maxCover` skips covered canvas;
     - width and length are drawn (`sizeByDetail`, `lengthSkew`, `dabShare`, `minAspect` unless `dab`);
     - one stroke is emitted.
   - **Gap fill** (error placement): canvas still bare inside the region (cover < `gapCover`, mask > 0.5) gets one
     more stroke per half-cell.
4. `dryAfter` multiplies wetness after the layer.

**A stroke:**
- **Path.** It starts at the site, pushed off-canvas when it lies at the edge and the flow runs inward. It follows
  the region's flow, evaluated at each step of half a width: per-stroke angle jitter (`align`), `curvature` toward
  the flow, wobble, an optional flick of up to 40 degrees over the last third (`flick`), and `reverseP`. It stops
  where the soft mask fades (`stopAtEdge`) unless the stroke spills (`spill`).
- **Profile.** Width and pressure follow v1's start and end ramps, with per-stroke `endVariation`.
- **Colour.**
  1. A reference sample (box of half a width), or a palette colour with `colorFrom: palette`.
  2. Snapped toward the nearest mixer-made palette mixture by `snap`.
  3. Lab jitter, floored at `lFloor`.
  4. `flecks` mixed in at 0.75.
  5. Warmed by the light map (`warmth`).
  6. `marble` or `load2` gives a second load.
- **Brush.**
  - `opacity` is drawn from its range, scaled by `opacityByLight`.
  - Lanes = `nbBase` + `nbPerCw` x width, clamped to 3..40.
  - Thickness is `hgain` x the layer's `relief` x jitter, x (1 + `reliefByValue` (L* - 50) / 50).
- **Painting.** It is painted on the proxy canvas exactly as `oil_paint` replays the StrokeList. A StrokeList
  painted at the plan width reproduces the proxy bit for bit (tested).

**The StrokeList** records its provenance in `META`:
- the generator;
- `plan` (`mixer`, `planWidth`, `seed`, `portable`: false when the plan uses sampled fields);
- `inputs`: the SHA-256 of the ScenePlan's canonical JSON, and each field's hash;
- the layer and region names.

**The plan report** (JSON, printed by `oil plan`):
- per layer and region: sites, strokes and gap strokes;
- per region: the share of its soft-mask area painted at all (proxy cover > 0.05), stroke count and width range;
- `bare`: the share of the canvas never painted;
- timings and warnings.

## Porting v1 scenes

The mapping is mechanical: each DSL call becomes the JSON object in the tables above, and snake_case keys become
the camelCase names listed. `scenes/storm_v3.ts` is the worked example.

Python lambdas map to declarative shapes:
- `(Y > -1)` becomes `{"all": true}`;
- `(Y < 0.98)` becomes `{"above": 0.98}`;
- `1 - polygon(ROCK)` becomes `{"not": {"polygon": ROCK}}`.
