# ScenePlan v1 (draft)

The input to the planner: a declarative, versioned JSON description of a painting. The TypeScript DSL
(`S.region(...)`, `S.style(...)` ...) builds it; the Rust scene compiler turns it into guide maps (target image,
region masks, flow field, light map); the planner turns guides, styles and layers into a StrokeList.

- **Status:** draft written in L0, covering every call in the three v1 scenes (`scenes/storm_light.py`,
  `scenes/storm_v3.py`, `scenes/swatches.py`). L2 freezes it: Rust types (serde) generate the JSON Schema
  `spec/sceneplan-1.schema.json` with schemars, and that schema generates the TS types.
- **Reproducibility:** a ScenePlan, the plan options `{seed, planWidth, mixer}` and an engine version determine the
  StrokeList bit for bit on every host (see `README.md`).

## Top level

```json
{
  "$schema": "https://oilpaint.dev/schema/sceneplan-1.json",
  "sceneplan": 1,
  "engine": "2.0.0-dev.1",
  "title": "Storm Light",
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
| `engine` | no | engine version the scene was authored and tuned with. A different engine warns `ENGINE_VERSION_DIFFERS`; plan option `strictEngine: true` makes that an error. |
| `title` | no | copied into the StrokeList's `META` |
| `canvas` | yes | `aspect: [w, h]` positive integers; `ground`: a colour |
| `target` | yes | ordered target-image operations (see below); the reference the planner tries to paint |
| `regions` | yes | ordered list; later regions override earlier ones in the hard region map |
| `lights` | no | contributions to the light map (drives `warmth`, `opacityByLight`) |
| `styles` | yes | per-region style, keyed by region name |
| `layers` | yes | ordered layer schedule |
| `fields` | no | declarations of sampled fields (escape hatch 1) |

Plan options are not part of the spec: `seed` (u32, default 1907), `planWidth` (px, default 600), `mixer` (default
`ochrell`) and `strictEngine`. They are passed to `plan()` or the CLI.

Units follow `README.md`: cw coordinates, y down, angles in degrees (0 = +x, 90 = down), sRGB colours, u32 seeds. All
numbers must be finite.

## Colours

`"#rrggbb"` | `"tube_name"` (for example `"lead_white"`, `"cobalt_blue"`; the catalogue is shipped as data) | `[r, g, b]`
with each value in [0, 1] | `["a", "b", t]`, a mix of two colours at t in [0, 1] evaluated with the active mixer.

## Shapes (masks in [0, 1])

A shape is a single-key object (externally tagged):

| Shape | Form | v1 DSL |
|---|---|---|
| all | `{"all": true}` | `lambda X, Y: (Y > -1)` |
| above / below | `{"above": y}`, `{"below": y}` | `above(y)`, `below(y)` |
| ellipse | `{"ellipse": {"c": [x, y], "r": [rx, ry], "softness": 0}}` | `ellipse(cx, cy, rx, ry, softness)` |
| disc | `{"disc": {"c": [x, y], "r": r, "softness": 0}}` | `disc(c, r, softness)` |
| polygon | `{"polygon": [[x, y], ...]}` | `polygon(pts)` |
| wedge | `{"wedge": {"apex": [x, y], "angle": a, "spread": s, "length": l}}` | `wedge(...)` |
| bandAround | `{"bandAround": {"polygon": [[x, y], ...], "dist": d}}` | `band_around(pts, dist)` |
| union / intersect | `{"union": [shape, ...]}`, `{"intersect": [shape, ...]}` | `union`, `intersect` |
| not | `{"not": shape}` | `1 - polygon(...)` lambdas |
| noisy | `{"noisy": {"shape": shape, "amount": 0.3, "scale": 0.08, "seed": 11}}` | `noisy(...)` |
| field | `{"field": "name"}` | sampled mask (escape hatch) |

Rasterisation (anti-aliasing, polygon fill rule, noise lattice) is defined by the Rust compiler, not by OpenCV. It is
resolution-independent in cw, and exact per engine version.

## Target operations

Applied in order onto an RGB image (the reference colours):

| Op | Form | Composite |
|---|---|---|
| fill | `{"fill": {"color": c} \| {"gradientV": [[y, c], ...]}, "mask": shape?}` | replace (inside the mask) |
| blob | `{"blob": {"c": [x, y], "r": [rx, ry], "color": c, "softness": 0.5, "noise": 0, "strength": 1, "seed": 21}}` | alpha blend |
| polygon | `{"polygon": {"points": [[x, y], ...], "color": c, "softness": 0.01, "noise": 0, "strength": 1, "seed": 22}}` | alpha blend |
| bands | `{"bands": {"points": [[x, y], ...], "bands": [[f0, f1, c], ...], "softness": 0.008}}` | alpha blend per band |
| glow | `{"glow": {"c": [x, y], "r": r, "color": c, "strength": 1, "power": 2}}` | additive light |
| beam | `{"beam": {"apex": [x, y], "angle": a, "spread": s, "length": l, "color": c, "strength": 0.4, "softness": 0.3}}` | additive light |

## Regions

```json
{ "name": "sky", "shape": {"above": 0.6}, "edge": 0.03, "flow": {"sweep": {"angle": -14, "curl": 0.5, "noise": 0.3}} }
```

- `name` is unique (`DUPLICATE_NAME`).
- `edge` is the soft-mask blur in cw (default 0.02).
- `flow` is optional: without one the region follows the structure-tensor flow of the target (and L5 warns
  `REGION_NO_FLOW`).

## Flows (unit direction fields)

| Flow | Form (defaults shown) |
|---|---|
| constant | `{"constant": {"angle": 0, "noise": 0, "seed": 1}}` |
| sweep | `{"sweep": {"angle": -12, "curl": 0.4, "noise": 0.25, "scale": 0.5, "seed": 2}}` |
| waves | `{"waves": {"angle": 0, "amplitude": 18, "wavelength": 0.12, "noise": 0.3, "perspective": true, "horizon": 0.5, "seed": 3}}` |
| swirlAround | `{"swirlAround": {"centres": [[cx, cy, rx, ry], ...], "strength": 0.7, "noise": 0.3, "seed": 4}}` |
| radialFrom | `{"radialFrom": {"c": [x, y], "noise": 0.1, "seed": 5}}` |
| upward | `{"upward": {"noise": 0.6, "seed": 6}}` |
| contour | `{"contour": {"polygon": [[x, y], ...], "noise": 0.35, "seed": 7}}` |
| facets | `{"facets": {"polygon": [[x, y], ...], "cell": 0.05, "contourWeight": 0.4, "seed": 8}}` (planned for L9) |
| field | `{"field": "name"}` (sampled; escape hatch) |

## Lights

Each entry adds to the light map in [0, 1]:

- `{"glow": {"c": [x, y], "r": r, "strength": 1}}` (ellipse flattened 2.5x vertically, as in v1)
- `{"beam": {"apex": [x, y], "angle": a, "spread": s, "length": l, "strength": 1}}`
- `{"lamp": {"c": [x, y], "r": r, "strength": 1}}`

## Styles (per region; defaults from v1's planner)

Every key is optional. The v1 name is given where it differs.

| Key | Default | Meaning |
|---|---|---|
| `colors` | from the target | palette-snapping targets (colours) |
| `flecks` | `[]` | `[[colour, probability], ...]` complementary touches |
| `width`, `length` | `[0.015, 0.03]`, `[0.04, 0.10]` | stroke size ranges in cw |
| `curvature` | 0.3 | 0 straight, 1 follows every turn of the flow |
| `align` | 0.85 | 1 strictly along the flow |
| `opacity` | `[0.85, 1.0]` | |
| `pickup`, `load`, `deplete`, `vdry`, `hgain`, `flatten`, `streak`, `body` | 0.12, 1.0, 0.02, 0.25, 1.0, 0.6, 0.25, 0.9 | brush behaviour (StrokeList fields of the same name) |
| `streakMix` (`streak_mix`) | 0.8 | |
| `hardness`, `grain`, `release`, `dropout`, `ragged` | 0.75, 0.10, 0.3, 0.02, 0.5 | |
| `nbPerCw`, `nbBase` (`nb_per_cw`, `nb_base`) | 450, 5 | bristle lanes = nbBase + nbPerCw x width (3..40) |
| `ridge`, `levee`, `furrow`, `blob`, `stiff` | 0.5, 0.35, 0.15, 0.3, 0.25 | surface relief (x hgain) |
| `marble`, `load2` | 0, null | two-colour load share, and colour B (null: an automatic lighter/warmer variant) |
| `splay` | 1.0 | stray hairs, 0..3 |
| `snap` | 0.85 | pull toward the nearest palette mixture |
| `jitter` | `[5, 4]` | Lab jitter (L, a/b) between strokes |
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
| `lFloor` (`L_floor`) | 20 | stroke colours never darker than this L* |

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
| `spacing` | 1.6 | number or `{region: number}` (density placement), in stroke widths |
| `coverage` | 1.0 | number or `{region: number}`, share of the region to cover |
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
| `curve` | null | `{region: [[x, y], ...] \| "boundary"}` (curve placement) |
| `curveOffset`, `curveSpacing`, `curveJitter` | 0, 1.0, 0.5 | curve placement |
| any style key | – | overrides the region styles for this layer (for example `width`, `opacity`, `colors`, `hardness`) |

## Sampled fields (escape hatch)

```json
"fields": { "sky_flow": { "kind": "flow", "width": 300, "height": 375, "sha256": "…" } }
```

- The arrays (little-endian f32; 2 channels for `flow`, 1 for `mask`, 3 for `rgb`) are passed next to the spec, not
  inside it.
- The engine checks each array against its hash (`FIELD_HASH_MISMATCH`) and resamples it bilinearly to guide
  resolution.
- A plan that uses a field, or a hook, is marked `portable: false` in its StrokeList.

## Validation

Validation runs before compiling and reports every problem as a structured error (`ERRORS.md`). For example:

```json
{"code":"UNITS","path":"/styles/sky/width","got":[25,40],"expected":"[min, max] in cw, 0.001-0.2",
 "fix":"25 px at 1200 px is 0.021 cw: use [0.021, 0.033], or build the scene with units: 'px'"}
```

## Porting v1 scenes

The mapping is mechanical: each DSL call becomes the JSON object in the tables above, and snake_case keys become
the camelCase names listed.

Python lambdas in `storm_v3.py` map to declarative shapes:
- `(Y > -1)` becomes `{"all": true}`;
- `(Y < 0.98)` becomes `{"above": 0.98}`;
- `1 - polygon(ROCK)` becomes `{"not": {"polygon": ROCK}}`.

L2 ports `../storm-light-painting/scene/storm_v3.py` to TS, and its acceptance is that scene's guide sheets.
