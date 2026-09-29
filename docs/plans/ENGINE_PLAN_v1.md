# ENGINE_PLAN: oilpaint, oil-only engine phase

Scratchpad root `SP=/tmp/claude-0/-home-claude/cd4531f6-3314-512b-9953-4f74d55c5a55/scratchpad`. The repo was not changed.
Evidence: `SP/work_engine/evidence/` (images), `SP/work_engine/logs/measurements.txt` (all numbers),
`SP/work_engine/exp/` (prototypes). The 2 vCPUs were shared with another agent, so timings are noisy; I quote clean
samples (other processes under ~1 CPU-s) and ratios.

## 1. Current state (measured on Storm Light v3, 12,495 strokes)

| Stage | Time | Where it goes |
|---|---|---|
| Preview 600x750 | ~41 s | plan 32 s, replay 5 s, guides 2 s |
| Planner Python overhead | ~20 s of the 32 | ~70k scalar numpy colour conversions (`mix.rgb_to_latent`, `rgb_to_lab`, ~0.13 ms each) ~12 s; `render.save_state` zlib x15 ~7 s |
| Full 2400x3000 replay | 100-110 s | 57 s with `--no-layers` |
| Paint kernel (`brush.c render_strokes`) | 36 s | 234 Mpx painted, 6.6 Mpx/s, ~150 ns per painted pixel, one core |
| 15 per-layer lit PNGs | ~50 s | `light.relight` 1.7 s per call at full size + PNG + resize |

Surprises:
1. At full size the per-layer PNG lighting costs more than the kernel.
2. The painterly sweep order makes each stroke overlap the previous one: stroke-level parallelism in the real list is
   at most 1.5x even with unlimited cores; big-stroke layers L03-L06 only 1.1-1.2x (`logs/parallelism_dag_bsp.txt`).
3. The block-in layer L02 paints 14.6 canvas areas and takes 44% of kernel time.
4. "Replays identically at any size" holds only at 600·2^k widths. `make_bristles` draws xorshift numbers per
   along-sample, ns = ceil(total_s·6)+3, and the planner's 0.5-width steps put total_s·6 on an integer for 12,181 of
   12,495 strokes; a rounding difference flips ns and reshuffles that stroke's bristles. At 1800 px ~28% flip: 1800 vs
   2400 (downsampled to 600) differ >8/255 on 10.3% of pixels vs 0.5% for 1200 vs 2400. A strict-FP build differs from
   the fast-math one by up to 124/255 on 9.9% of pixels. Same size + same binary is bit-exact.
5. The strata are geometry, not lighting: each stroke lays a plateau at the local mean plus its thickness (`brush.c`
   height update), so every outline is a cliff. `--shade-blur 0.002` hides the cliffs and the bristle texture alike:
   the "silky" sky (`evidence/light_cmp_sky_strata.png`).

## 2. M0: determinism hardening and stroke file v2 (1.5 agent-days)

- Counter-based RNG: `make_bristles` draws `hash(seed, lane, sample, purpose)` instead of a sequential stream; stroke
  length is computed once in canvas-width units in double and stored (`arclen`). The sample count then only moves the
  array end, never the pattern.
- Pick-up sums (`segZ`, `segA`, `segWet`) accumulate in int64 fixed point (2^-32): associative, so any row chunking,
  thread count or GPU atomic order gives the same bits.
- `strokes.npz` gains `format_version` (absent = 1) and `kernel_version`; per-point columns become
  (x, y, w, p[, angle, speed]); per-stroke `arclen`, `brush_id`, `shape`. v1 files replay through a retained legacy
  path, identical to today at the same size and build (question 1).
- Build adds `-ffp-contract=off` (keeps -O3).
- Acceptance (harness determinism block, new keys): `threads_identical` (1 vs N threads bit-identical);
  `resolution_consistency` (replay at 1800 and 2400, downsample to 600: pixels >8/255 at most 1%, today 10.3%);
  `v1_replay_identical` (Storm Light v1 file gives today's digest).

## 3. Items

### 3.1 Speed (M1, 4 agent-days)

In order of value per effort:
- a. Lighting off the critical path: a worker process lights per-layer snapshots while the kernel paints on; cache
  `light.weave` (rebuilt 16 times now); layer PNGs at most 1200 px by default. Deterministic. Full size: 100 → ~45-55 s.
- b. Planner (measured prototype, `logs/planner_fast.diff`): memoised colour specs, scalar single-colour conversions and
  background state compression: plan 32 → 12-13 s, same stroke count, image within 1/255 (plans re-baselined once).
  Then `_path` in C (2.7 s), error map on the region bbox (2 s), guide cache (2 s): target ≤8 s.
- c. Kernel threads: each segment's pixel rows are split into fixed 16-row chunks on a small pthread pool (short spin,
  then block) and reduced in fixed order (int64 after M0). Prototype `exp/brush_mt.c`: 1 vs 2 threads are bit-identical.
  Clean samples: 1.3-1.6x on 120-px strokes, 1.2x at 60 px, 1.0x at 20 px; full replay 36 → 28-30 s. Optional: a
  stroke-dependency scheduler for small-stroke layers (L09, L12-L15 offer 2-6x). Rejected: canvas tiles/bands (a stroke's
  pick-up reads its whole segment) and batching non-overlapping strokes (1.5x ceiling).
- d. Single-thread pixel path: exact row spans tried (bit-identical, no gain); next suspects are 2 value-noise lookups,
  3 `expf` per pixel and the lane loop. Target 1.3x, unmeasured.
- e. Optional, Sean's call (question 2): footprint-aware block-in placement with about half the L02 strokes saves ~8 s at
  full size but changes the underpainting.

Files: `render.py`, `light.py`, `planner.py`, `mix.py`, `canvas.py`, `brush.c`; no stroke-file change. Risk: pool
portability (MinGW, macOS): pool behind `set_threads`, serial path identical by construction.
Acceptance (harness `speed`): preview at most 20 s, full size at most 50 s on 2 quiet cores, kernel Mpix/s reported per
thread count, determinism block green.

### 3.2 Paint that settles (M2, 3 agent-days)

Symptoms: strata hairlines, a silky sky under the workaround, faceted dabs. All five parts were prototyped at 2400 px on
the real strokes (`exp/brush_proto.c`, `exp/level2.c`, `exp/replay_level.py`):
1. Height-only outline taper: `ah = a·smoothstep(1.2, 1−τ, |u|)` with τ = `edge_taper` ≈ 0.3 half-widths. The colour
   edge is unchanged.
2. Wet displacement of the stroke body: `body *= 1 − D·wet_under` with D = `displace` ≈ 0.7; ridge and roughness texture
   are untouched. Displacing everything flattened the texture, so that variant was rejected.
3. Settle pass after each layer, on that layer's fresh paint only (yield-slope, Bingham-like levelling). For each pair of
   neighbours, `q = r·m·sign(Δh)·max(0, |Δh| − s_y·d)/d`, applied as a Jacobi update. Mobility `m = fresh·wet·(0.1 + 0.9·edge)`;
   `s_y = s_cw/W` with s_cw ≈ 300 per cw (a per-style `flow` knob); iterations `ceil(12·W/2400)`, so the relaxation
   distance is fixed in cw. Newtonian thin-film levelling (Orchard: t ∝ η·λ⁴/(σh³)) erases fine marks first; the yield
   slope is what keeps the bristle ridges.
4. Outline smoothing: at replay, densify each polyline with a centripetal Catmull-Rom spline (subdivision count from
   arc length in cw) and add rounded caps. `evidence/dabs_dense_detail.png`: facets gone; caps alone did little.
5. Return to the default lighting (drop `--shade-blur 0.002`).

Results (my crop proxies): hairline share −65% pale sky, −81% storm sky, −39% rock and sea; texture proxy −20-40%
(partly the removed edge lines). `evidence/BEFORE_AFTER_settle_900.png`: panel C keeps the sharp-light texture without
most strata; faint lines remain in the pale band. Taper alone: −35-50% in the sky, ~−16% rock/sea. Levelling all wet paint over-softened (paint
stayed mobile for 8 layers), hence fresh-only (`evidence/cmp7_all.png`).

Where: taper, displacement, caps in the kernel; settle is a new threaded C Jacobi pass. Canvas gains `edge` and `fresh`
planes (+58 MB at 2400). New BrushParams `edge_taper`, `displace`, `flow`, `smooth_path` default to 0 for v1 (old
strokes unchanged). Order-free, all units cw or half-widths. Cost: taper/displacement ≈0; settle 6.5 s in the prototype,
~2 s bbox-limited and threaded. Risk: "plastic" over-smoothing; per-region `flow` controls it.

Acceptance: swatch `strata.seam_hairline` −50% and `seam_over_inside` −40% against baseline, with `bristle_L` within
−15% and `inside_hairline` about unchanged; `dabs.facet_straight_frac` −40% and `facet_ellipse_dev` −30%; Storm Light
region `hairline` −40% under default lighting. New metric needed: `hairline` at 900-px display scale, because strata read
at whole-picture scale.

### 3.3 Brush memory, shapes, richer inputs (M3, 4 agent-days)

- Memory: the planner assigns strokes to a few physical brushes per layer by width class (`brush_id`); the kernel keeps
  per brush a residual colour in 16 across-bins and a residual load. Stroke start: lane load = lerp(residual(bin), palette
  colour, `reload`); stroke end: lane tips (`Ztip`, dirt) written back. `wipe` resets a brush; the table resets per layer
  (keeps `--only N` valid). Precedents: Stuyck's reservoir refill, libmypaint's smudge bucket (ISC, ideas only).
- Shapes (`shape` enum): flat has a square tip with rounded corners and even lanes; filbert has an elliptical tip and
  shorter edge lanes; round has an elliptical thickness profile across the stroke, width ∝ p^γ, and a pointed tail. Each
  is implemented as cap geometry, a cross profile and a pressure-to-width response.
- Inputs: per-point twist θ (blade vs path normal): half-width hw·(|cos θ| + 0.15|sin θ|), lanes compressed; speed v:
  deposit ∝ 1/(1+v/v0), more dropout. Planner derives θ from flow curvature, v from stroke phase.

Memory chains one brush's strokes (row threads unaffected). Bins in cw. v1 = no memory, flat, θ=0, v=1. Cost: a small
brush table; stroke file +2 columns. Risk: mud from carried colour, handled with a high
reload and watched with `mud_stack`. Acceptance: new `brush_memory` swatch (the start of stroke n+1 moves ≥5 ΔE toward
stroke n at reload 0.6 and drops below 2 ΔE within one stroke length); `shapes` swatch (tip `ellipse_dev` ordered
flat > filbert > round); twist swatch (width edge-on over face-on at most 0.25).

### 3.4 Wet/dry stages and optical glazes (M4, 3 agent-days)

- A per-pixel `age` (layers since deposit) and a per-style `open_time` give three stages. Wet: pick-up and settling.
  Tacky: no settling, pick-up ×0.3, and drag (alpha catches ridges, as in scumble). Dry: no pick-up; glazes act optically.
  `dry_after` stays.
- Glaze = optical film, not a colour lerp: per pixel absorbance A (3 ch, linear RGB) and scatter X; display
  R = R_g + T²·R_body/(1 − R_g·R_body) with Kubelka-Munk layer R_g, T (a = 1+K/S, b = √(a²−1), tanh/sech form). Glaze
  given as tint over white C (K = −ln C / 2), strength d, opacity (sets S). A later opaque stroke bakes the film into
  the body latent by its alpha; the body stays Mixbox.
- Prototype (`evidence/glaze_km_vs_alpha.png`): a madder glaze over a dark takes L* from 18.9 to 16.6 (today's lerp:
  20.9, lighter); over a light it gives L* 95→54 and C* 11→45 (lerp: 71 and 34, a pastel). Today a yellow glaze turns dark
  blue green, because it mixes like opaque paint.

Cost +16 B/px (+115 MB at 2400). Per pixel, so deterministic and resolution-free. v1 glaze mode keeps the lerp; v2 uses
`glaze_model=km`. Risk: glaze colours need tints, not tube masstones (the ultramarine row shows why). Acceptance: `glaze`
swatch (ΔL* over darks at most +1, chroma gain over lights at least +15, hue within 15° of the tint), `blend` hue error
unchanged, wet-over-dry trail below 15% of wet-in-wet.

### 3.5 Palette knife (M5, 3 agent-days)

Mode 4 is a rigid blade of length Lk at an angle to its motion:
- A per-segment pre-pass sets the blade rest level B to the maximum of h along the blade line (sampled in cw), smoothed
  along the path.
- The film thickness is t = t0·load. Contact happens where h ≥ B − t, so valleys are skipped; the surface becomes the
  plane B.
- Scraped paint plus a share of the film forms sharp ridges at the blade ends and a lip where the knife lifts.
- Colour: 2-3 broad, unmixed streaks across the blade plus a smear of the wet paint passed over.
- A `scrape` variant removes paint instead (sgraffito).

Prototype `evidence/knife_proto.png`: reads as knife work (flat streaked planes, crisp edges, skips) but too
geometric; needs curved paths, varying angle/pressure, blade tilt. Prior art: Okaichi et al. 2008 (metadata only).
Where: kernel mode 4 + planner `knife` layers; BrushParams `blade_len`, `blade_angle`, `film`; v2 only. Deterministic,
cw units; cost ≈ a paint stroke. Risk: looks pasted-on if the blade ignores the surface.
Acceptance: `knife` swatch (at least 60% of the knife footprint planar at |∇h| < 20 per cw, edge ridge at least 1.5x the
film, skip share adjustable over 10-50% with t0).

### 3.6 Planner fixes for the Storm Light flaws (M3b, 3 agent-days)

- Tower-base comb (`evidence/flaw_tower_base_comb.png`): strokes end raggedly at edges. Cut-in: where flow meets the edge
  at >45°, start at the edge and pull inward; add a short edge-following pass. Metric: stroke ends per cm of boundary.
- Rock ribbons (`flaw_rock_rings.png`): the `contour` flow makes rings. A new `facets(poly, cell, contour_weight)` flow
  gives one plane per Voronoi cell blended with the contour; rock `marble` goes down. Metrics: `coh_local` and the share
  of rock strokes within 15° of the contour tangent.
- Halo petals (`flaw_halo_petals.png`): replace the radial strokes with a warm KM glaze with radial falloff (needs M4)
  plus tangential broken touches.
- Palette: regions list tubes (≤3 + white) instead of hex lists (Mixbox mixture lattice as in `Palette`), plus a paint
  "pile" reused by consecutive strokes with slow drift.

Planner-only: saved strokes replay unchanged; new plans differ (same seed + version is reproducible). Risk: scene retuning.

## 4. Milestones (each shippable, each ends with a Storm Light before/after)

| M | Content | Agent-days | Before/after shown |
|---|---|---|---|
| M0 | determinism, format v2 skeleton | 1.5 | 1800 vs 2400 consistency; v1 digest unchanged |
| M1 | speed a-c (d, e optional) | 4 | same painting, timing table; 1 vs 2 threads bit-identical |
| M2 | paint that settles + default light | 3 | A/B/C strip plus native crops (sky, rock, dabs) |
| M3b | planner fixes (recommended here) | 3 | tower base, rock, halo crops |
| M3 | brush memory, shapes, inputs | 4 | sky and sea crops |
| M4 | wet/dry stages, KM glazes | 3 | horizon glow, halo, haze |
| M5 | palette knife | 3 | rock facets and foam crests |

Total ≈ 21.5 agent-days. M3b before brush memory departs from Sean's order: cheapest visible gain (question 5).

## 5. Not now, and what to keep stable

Parked: Rust port, wgpu/WebGPU, browser player, watercolour, pencil. To keep them possible:
1. The stroke file stays the seam: versioned, canvas-width units only.
2. One C kernel, no hidden globals (prototype setters move into BrushParams), canvas as separate float planes.
3. Counter-based RNG (portable to WGSL/Rust); order-independent integer reductions (GPU atomics give the same bits).
4. Work split segment → row tiles → ordered reduction; settling as a Jacobi stencil.
5. Mixbox LUT + polynomial (official Rust/GLSL ports; CC BY-NC, keep attribution).
6. Mode enum and BrushParams append-only, defaults reproduce v1; threads behind `set_threads`, no OpenMP-only code.

Licences: all proposed code is our own, from published formulas; no GPL (Krita) code; libmypaint (ISC) ideas only.

## 6. Open questions for Sean

1. Old paintings: must old stroke files keep replaying pixel-identical forever? That means an "old engine" path kept
   inside the new one, which is small but permanent upkeep. Or is "looks the same, tiny pixel differences" acceptable?
2. The first block-in layer is painted about 15 times over and takes 44% of the paint time. May the planner use about
   half the strokes there? It is mostly covered later, but the underpainting will change.
3. Do you need every layer image at full resolution, or are 1200-px layer images enough? Full-size ones cost ~40-50 s
   per render.
4. How fluid should the sky paint be? I will show three settings, from crisp stroke edges to strokes melting together;
   the prototype sits in the middle.
5. May I do the cheap planner fixes (tower-base drips, rock rings, lamp-halo petals) right after "paint that settles",
   before brush memory?
6. Will renders stay on this 2-core machine? On 2 cores, moving lighting aside beats kernel threads; 4-8 cores favour threads.

Sources: Orchard levelling https://www.stevenabbott.co.uk/practical-coatings/levelling.php ; yield-stress levelling
https://link.springer.com/article/10.1007/s11998-019-00260-z (rate-limited, unread); knife https://doi.org/10.1007/s00371-008-0257-5
