# oilpaint style reference

Everything you can set in a scene, what it does, and which way to turn it.  Units: **cw** = fraction of the
canvas width (the canvas is 1.0 wide; a 4:5 portrait is 1.25 high, so y runs 0..1.25).  A Monet-size flat
brush is about 0.015-0.025 cw; the block-in brush 0.04-0.07 cw; final touches 0.005-0.01 cw.

Colour mixing is Mixbox ((c) 2022 Secret Weapons, CC BY-NC 4.0, https://scrtwpns.com/mixbox): colours mix like
pigments (blue + yellow = green, tints stay clean).  Non-commercial use only; attribution is written into every
`run.json`.  The permissive WGM (libmypaint) mixer backend from the plan was **dropped** (time); `--mixer rgb`
remains as the plain-RGB baseline for A/B tests.

## 1. How a scene is authored

A scene is a Python module with `build(S)`.  `S` is a `SceneBuilder`; everything below is a call on it.
Read `scenes/storm_light.py` alongside this: it is the worked example.

```python
from oilpaint.scene import *          # shape helpers, flow helpers, gradient_v, Layer

def build(S):
    S.canvas(aspect=(4, 5), ground="#e9e1d6")           # 1. canvas and ground colour
    T = S.target                                         # 2. the soft "target image" the planner paints toward
    T.fill(gradient_v([(0.0, "#2c3266"), (0.575, "#eccfb8")]))
    T.blob(0.25, 0.55, 0.48, 0.11, "#f4bd90", softness=0.7, strength=0.8)
    T.polygon(TOWER_PTS, "#e9d8d6", softness=0.006)
    T.beam((0.67, 0.39), 176, 9, 0.48, "#fbe3b8", strength=0.7)
    T.glow((0.67, 0.39), 0.035, "#fff0b8", strength=1.2)
    S.region("sky", above(0.575), edge=0.03)             # 3. regions: soft masks, in painting order
    S.region("tower", polygon(TOWER_PTS), edge=0.008)
    S.flow("sky", sweep(angle_deg=-14, curl=0.5))        # 4. a flow field per region (stroke directions)
    S.flow("tower", constant(90, noise=0.05))
    S.light(glow=(0.25, 0.55, 0.5, 0.7), beam=((0.67, 0.39), 176, 9, 0.48, 0.8), lamp=((0.67, 0.39), 0.07, 1.0))
    S.style("sky", colors=[...], width=(0.025, 0.04), length=(0.10, 0.28), align=0.85, ...)   # 5. per-region style
    S.layers([Layer("Toned ground", ...), Layer("Ebauche dark masses", ...), ...])            # 6. the layer schedule
```

The **target** is what the picture should roughly look like: a soft, blurry version.  The planner never copies
it; error-driven layers place strokes where the canvas differs from a blurred copy of it, and every stroke
colour is snapped to the region's palette.  Keep it soft: sharp shapes in the target become sharp shapes in
the painting.  **Regions** give each part of the picture its own palette, brush and flow; their soft masks
(`edge` = blur radius in cw) decide how "lost" the boundary is.  Later regions override earlier ones in the
id map, but every region keeps its own mask and flow, so a big region defined first (like `rain`) can still be
painted by a layer even though other regions sit on top of it.

### Target operations (`S.target.*`)

| Call | What it draws | Notes |
|---|---|---|
| `fill(color_fn, mask=None)` | a full-canvas fill, e.g. `gradient_v([(y, colour), ...])` | `mask=below(y)` etc. restricts it |
| `blob(cx, cy, rx, ry, colour, softness, noise, strength)` | soft ellipse | `softness` 0.1 crisp .. 1 very soft; `noise` breaks the outline; `strength` = max opacity |
| `polygon(pts, colour, softness, noise, strength)` | filled polygon | `softness` in cw (0.005 = crisp, 0.03 = misty) |
| `bands(pts, [(f0, f1, colour)], softness)` | horizontal bands inside a polygon | fractions of the polygon's height |
| `glow(c, r, colour, strength, power)` | additive light spot | use for lamps |
| `beam(apex, angle_deg, spread_deg, length, colour, strength, softness)` | additive soft wedge | angle 180 = left, <180 slightly up (image y is down) |

Colours: `"#hex"`, a tube name from `oilpaint.mix.TUBES` (`"cobalt_blue"`, `"lead_white"`, ...), or a
mixture `("tube_a", "tube_b", t)` (Mixbox lerp).

### Mask helpers (for `S.region`)
`above(y)`, `below(y)`, `ellipse(cx, cy, rx, ry, softness=0)`, `disc(c, r)`, `polygon(pts)`, `wedge(apex, angle,
spread, length)`, `band_around(pts, dist)` (a band along a polygon's outline), `union(...)`, `intersect(...)`,
`noisy(mask, amount, scale)`.  Any `f(X, Y) -> array` works.

### Flow helpers (for `S.flow`)
| Helper | Strokes go... |
|---|---|
| `constant(angle_deg, noise)` | one direction (0 = right, 90 = down, -90 = up) with noise |
| `sweep(angle_deg, curl, noise, scale)` | long arcs bending slowly (sky) |
| `waves(base_angle, amplitude_deg, wavelength, noise, horizon)` | near-horizontal undulation, shorter waves toward the horizon (water) |
| `swirl_around(centres, strength, noise)` | tangentially around ellipses (storm cells, halos) |
| `radial_from(c, noise)` | outward from a point (beam) |
| `contour(pts, noise)` | along a polygon's outline (rock facets) |
| `upward(noise)` | up with noise (spray) |

Regions without a flow use the structure tensor of the target (along local edges).

### Light map (`S.light(glow=(cx, cy, r, strength), beam=(apex, angle, spread, length, strength), lamp=(c, r, strength))`)
A 0..1 map of warm light.  Styles use it through `warmth` (colour shifts toward `warm_color`) and
`opacity_by_light` (strokes fade away from the light).

## 2. Style parameters (`S.style(region, ...)`)

Defaults in `oilpaint/planner.py::STYLE_DEFAULTS`.  A layer may override most of them for all its regions.

| Parameter | Default | Meaning | Raise it -> | Lower it -> |
|---|---|---|---|---|
| `colors` | None | palette the region's colours are snapped to (list of colour specs). Mixtures of pairs and tints with white are candidates. | more hues available | tighter, more unified colour |
| `flecks` | () | `[(colour, p)]`: with probability p a stroke takes this colour instead (complementary touches) | more sparkle/vibration | calmer |
| `width` | (0.015, 0.03) | stroke width range, cw | fatter marks, fewer strokes | finer marks |
| `length` | (0.04, 0.10) | stroke length range, cw | long dragged strokes | dabs |
| `min_aspect` | 2.5 | length is at least this x width unless the layer sets `dab=True` | no blobs | allows round dabs |
| `curvature` | 0.3 | how tightly the path follows turns of the flow (0 straight, 1 every turn) | curly, swirling | straight |
| `align` | 0.85 | 1 = along the flow, 0 = random directions | combed, uniform | scattered, choppy |
| `reverse_p` | 0 | probability a stroke runs against the flow | | |
| `opacity` | (0.85, 1.0) | per-stroke opacity range | solid | translucent, washy |
| `pickup` | 0.12 | share of the canvas colour under a wet stroke that each bristle LANE blends into its deposit (lane-wise, so pick-up shows as parallel streaks, not a smooth drag) | wet-in-wet streaks, risk of mud | crisp juxtaposed colour (Monet blended little) |
| `release` | 0.3 | how fast picked-up paint leaves a lane over dry paint (per segment, ~0.5 width) | short trails (1 width) | long trails |
| `load` / `deplete` / `vdry` | 1 / 0.02 / 0.25 | initial paint load, loss per width travelled, dry threshold | `deplete` up: strokes run dry and break up sooner | |
| `body` | 0.9 | paint body filling between the bristle lanes at full load (0 = lanes only, broken coverage) | solid touches | dry-brush lanes with the layer below showing between them |
| `ridge` | 0.5 | lane ridge/furrow relief across the stroke (x hgain): parallel ridges 1-3 px wide at 2400 | corduroy relief under raking light | smoother body |
| `levee` | 0.35 | raised paint pushed to both stroke edges (x hgain) | strong edge rims | flat-topped strokes |
| `furrow` | 0.15 | slight trough along the stroke centre (x hgain) | | |
| `blob` | 0.3 | extra paint where the stroke lands (x hgain, first ~0.5 width) | fat starts | even thickness |
| `stiff` | 0.25 | multi-scale roughness of stiff paint (x hgain), resolution independent | gritty, broken peaks | smooth paint |
| `marble` | 0 | two-colour load: runs of adjacent lanes carry colour B (`load2`) with this share; 0.4-0.8 gives marbled strokes | parallel colour streaks A/B | single colour |
| `load2` | None | colour B for marbling (colour spec). None = automatic: a lighter-warmer or darker-cooler variant of the stroke colour, chosen per stroke | | |
| `splay` | 1.0 | stray hairs at the outline (0..3): short broken lanes just outside the body | hairy, ragged outline | clean outline |
| `hgain` | 1.0 | paint thickness per stroke | impasto ridges, stronger lighting | thin, flat |
| `hgain_jitter` | 0.25 | +- variation of thickness between strokes | more varied relief | uniform |
| `flatten` | 0.6 | how much a stroke levels the paint under it (0 = pile up, 1 = replace) | smooth surface | ridges accumulate |
| `streak` | 0.25 | light/dark colour variation between lanes (tube inhomogeneity) | stripy | flat colour |
| `streak_mix` | 0.8 | how unevenly lanes show the picked-up colour | streaky blends | uniform blends |
| `hardness` | 0.75 | crispness of the paint-body outline (0.75 = 1-2 px transition at 2400; 0.1 = soft, misty edge) | crisp paint edges | soft edges |
| `grain` | 0.10 | fine per-pixel alpha noise (resolution dependent, shows at 2400) | grittier | smoother |
| `nb_base`, `nb_per_cw` | 5, 450 | bristle lanes = nb_base + nb_per_cw * width (0.02 cw -> 14 lanes, ~3.4 px pitch at 2400) | finer lanes | coarser lanes |
| `dropout`, `ragged` | 0.02, 0.5 | bristle gaps per along-sample; ragged start length (widths) | broken, dry look | full, wet look |
| `end_width`, `end_pressure` | 0.5, 0.15 | width and pressure at the very end | 1.0 = blunt flat-brush end | pointed, fading end |
| `snap` | 0.85 | how far the sampled target colour moves toward the nearest palette mixture | palette discipline | follows the target exactly |
| `jitter` | (5, 4) | Lab (L, a/b) random variation between strokes | lively broken colour | flat planes (avoid < 3) |
| `L_floor` | 20 | stroke colours never darker than this L* | | allow darks |
| `warmth` | 0 | colour shifts toward `warm_color` by light-map value x warmth | glow spreads into the region | |
| `opacity_by_light` | 0 | opacity x (1 - k(1 - light)): strokes fade away from the light (beams) | | |
| `size_by_y` | None | `(y0, y1, s0, s1)`: scale width/length from s0 at y0 to s1 at y1 (perspective) | | |
| `spill` | 0.15 | fraction of strokes that ignore the region mask and run into neighbours (lost edges) | boundaries dissolve | boundaries hold |
| `stop_at_edge` | 0.9 | probability the path stops where the soft mask fades | | strokes overrun |
| `priority` | 0 | painting order among regions in one layer (low first) | | |
| `mode` | None | per-region override of the layer mode (`paint`, `scumble`, `smudge`, `glaze`) | | |

## 3. Layer parameters (`Layer(name, ...)`)

Defaults in `oilpaint/planner.py::LAYER_DEFAULTS`.  Layers run in order; each can be disabled (`enabled=False`),
reordered, or rendered alone with `--only N --from <run>`.

| Parameter | Default | Meaning |
|---|---|---|
| `regions` | "all" | list of region names this layer paints (each with its own style) |
| `placement` | "error" | `error`: strokes where the canvas differs from the blurred target (Hertzmann); `density`: jittered grid over the region; `curve`: along a curve |
| `mode` | "paint" | `paint` (normal), `scumble` (dry brush that catches ridges only), `smudge` (no paint, drags what is there), `glaze` (thin transparent tint, no height) |
| `T` | 18 | error threshold (Lab distance): lower = more strokes, closer to the target |
| `fs` | 0.5 | blur of the reference = fs x brush radius (Hertzmann): lower = more detail |
| `fg` | 1.0 | grid spacing = fg x brush radius: lower = denser |
| `spacing` | 1.6 | density placement: grid spacing in stroke widths (dict per region allowed) |
| `coverage` | 1.0 | keep this fraction of the sites (dict per region allowed) |
| `max_cover` | None | skip sites already covered more than this (leave lower layers visible) |
| `gap_fill`, `gap_cover` | True, 0.15 | error layers add strokes where cover < gap_cover (no bare ground slivers) |
| `dab` | False | allow length < min_aspect x width (round touches) |
| `relief` | 1.0 | multiplies `hgain` for this layer (ebauche 0.25, touches 2) |
| `dry_after` | None | multiply wetness after the layer (0.3 = mostly dry before the next; None = stays wet) |
| `color_from` | "reference" | `reference` (target colour under the stroke) or `palette` (random colour from `colors`) |
| `order` | "sweep" | `sweep` (painterly, top to bottom in bands) or `random` |
| `curve` | None | for `placement="curve"`: `"boundary"` (region outline) or a polyline `[(x, y), ...]`; dict per region allowed |
| `curve_spacing`, `curve_offset`, `curve_jitter` | 1.0, 0, 0.5 | spacing along the curve, normal offset and jitter, in stroke widths |
| `hblur_sigma` | 0.01 | blur (cw) of the height map that scumble strokes compare against |
| `max_strokes` | None | cap |
| `seed_offset` | 0 | change to re-roll this layer only |
| any style key | | overrides that style key for every region in the layer (`width`, `length`, `opacity`, `pickup`, `hgain`, `flecks`, `colors`, `snap`, ...) |

Brush kernel defaults (`oilpaint/canvas.py::BRUSH_DEFAULTS`) and lighting defaults
(`oilpaint/light.py::LIGHT_DEFAULTS`) can be changed from the CLI (`--bump`, `--contrast`, `--spec`, `--cavity`,
`--weave-amp`, `--light x,y,z`, `--matte`) or via `relight` without repainting.  Lighting parameters:

| Parameter | Default | Meaning |
|---|---|---|
| `bump` | 1.4 | normal strength of the broad paint form (stroke bodies, levees); 2-3 = strong raking light |
| `bump_fine` | 1.8 | normal strength of the fine detail (lanes, roughness) = height minus its `broad` blur |
| `broad` | 0.004 cw | blur that separates broad form from fine detail |
| `contrast` | 0.45 | diffuse contrast; a flat surface always keeps its albedo |
| `spec`, `shininess` | 0.10, 22 | white satin highlight; 22 is oil paint, 60+ is gel |
| `gloss_h` | (0.25, 1.4) | highlight fades in with paint thickness: thin/dry paint is matte |
| `tint` | 0.06 | ridges a touch lighter and more saturated, furrows darker |
| `cavity` | 0.06 | capped, blurred valley darkening (no contour lines) |
| `hsmooth`, `shade_blur` | 0.0004, 0.0006 cw | anti-aliasing blur of the height / softening of the diffuse term |
| `weave_amp`, `weave_h0` | 0.18, 1.2 | canvas weave amplitude and the paint thickness that hides it |

### Recommended values for a Monet-like surface (v2 lane engine)

| Where | Settings | Why |
|---|---|---|
| ébauche layers (L2, L3) | `pickup` 0.15, `relief` 0.3, `body` 1.0, `ridge` 0.3, `stiff` 0.15, `hardness` 0.5 | thin, fairly smooth block-in; still a little tooth |
| main stroke layers (sky L4, storm L5, sea L6, rock L7) | `pickup` 0.08-0.12, `marble` 0.4, `ridge` 0.5, `levee` 0.35, `stiff` 0.25, `hardness` 0.75, `jitter` (6, 4) | distinct loaded touches with parallel colour streaks |
| tower (L8) | `hardness` 0.7, `marble` 0.3 (`load2` a pale violet), `ridge` 0.4 | ribbed but not shredded |
| broken colour (L9) | `marble` 0.5, `body` 0.9, `blob` 0.4 | small fat touches with two colours |
| scumbles (L10, rain) | `body` 0, `ridge` 0.6, `hardness` 0.6 | lanes only, catching ridges |
| beam (L11) | `hardness` 0.3, `ridge` 0.2, `levee` 0, `stiff` 0.1, `body` 0.7 | soft light, not scratched lines |
| smudge (L13) | `hardness` 0.4, `flatten` 0.2 | soft drags |
| impasto touches (L15) | `relief` 2, `blob` 0.6, `levee` 0.5, `stiff` 0.4, `marble` 0.3 | fat peaks |
| lighting | default; for a stronger surface `--bump 2 --contrast 0.55`; never above `--spec 0.18` | satin, not gel |

## 4. The storm_light schedule, annotated

| # | Layer | What it does | Knobs that matter |
|---|---|---|---|
| 1 | Toned ground | wide pale horizontal priming strokes over the haze band, relief 0.1 | `coverage`, ground colour in `S.canvas` |
| 2 | Ebauche dark masses | thin block-in of storm, sky, sea, rock with a 4-6 % brush | `T` (16), `relief` 0.25, `pickup` |
| 3 | Ebauche light masses | same for glow, sky, sea, foam; then `dry_after=0.3` | |
| 4 | Sky long strokes | error-driven 2.5-4 % strokes along the sky sweep | sky style `align`, `jitter`, `flecks` |
| 5 | Storm swirl | shorter, looser strokes around the storm cells | storm `align` 0.6, `curvature` |
| 6 | Sea underpainting | far: small pale near-horizontal; near: longer, larger toward the viewer (`size_by_y`) | sea styles |
| 7 | Rock | contour-following strokes, violet shadow, ochre facets | rock `colors`, `warmth` |
| 8 | Tower masses | short vertical strokes, bands as separate regions; `dry_after=0.4` | tower `width`, `spill`; `T` 6 |
| 9 | Broken colour | small error-driven dabs with flecks over sea and sky | `coverage`, `flecks` |
| 10 | Glow and halo scumble | dry-brush peach over the glow band, the haze, the halo and the beam | `opacity`, `spacing`, `dry_thresh` |
| 11 | Beam and band edges | curve placement: dragged strokes along the beam axis; short strokes along the band edges | beam `opacity`, `opacity_by_light`, `curve_jitter` |
| 12 | Spray and foam | flicks around the rock base | `coverage`, foam `colors` |
| 13 | Lost edges | smudge strokes along the tower outline and the horizon | `coverage`, `opacity`, `pickup` |
| 14 | Haze glaze and rain | thin glazes over the distance; sparse diagonal dry veils (rain style, mode scumble) | `spacing`, `coverage` |
| 15 | Lantern and impasto touches | heavy warm-white touches on the lantern, a few glints | `relief` 2, per-region `coverage` |

## 5. Iterating

```
python -m oilpaint render scenes/storm_light.py --preview --out out/preview          # ~35 s
python -m oilpaint render scenes/storm_light.py --preview --only 11 --from out/preview --out out/try  # re-plan one layer (~3 s + replay)
python -m oilpaint render scenes/storm_light.py --preview --upto 8                  # stop after layer 8
python -m oilpaint relight out/preview --bump 2 --contrast 0.5 --spec 0.15          # lighting only
python -m oilpaint crop out/final --box 0.52,0.30,0.82,0.62                          # native-resolution crop
python -m oilpaint render scenes/storm_light.py --size 2400x3000 --strokes out/preview/strokes.npz --out out/final --timelapse out/final/timelapse.mp4
```
Every run prints one summary line (size, strokes, timings, gates, L*, chroma, hue lobes, relief, mud) and
writes `run.json`, `metrics.json`, `layers_sheet.png` (all layers), `lit_unlit.png`, `final_900.png`.
The numbers in `metrics.json` under `diagnostics` compare against guesses in `oilpaint/calib.py`; only
`gates` (no black, no clipping) are pass/fail.
