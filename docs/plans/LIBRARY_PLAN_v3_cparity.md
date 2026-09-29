# LIBRARY_PLAN v3: Rust engine and planner, agent-first TypeScript

`SP=/tmp/claude-0/-home-claude/cd4531f6-3314-512b-9953-4f74d55c5a55/scratchpad`. Earlier versions:
`SP/plans/LIBRARY_PLAN_v2_jsplanner.md` (JS planner) and `SP/plans/ENGINE_PLAN_v1.md` (physics).
- Spikes: `SP/work_lib/` (`oilcore/` Rust crate, `planspike/`, `mixers/`).
- Numbers: `SP/work_lib/SPIKE_LOG.txt`; images: `SP/work_lib/evidence/`.
- Nothing is implemented; the repo and `work_eval` are untouched. Oil only.

**Changed from v2:** the planner and the scene compiler move into Rust ("policy in TS, mechanism in Rust"). TS becomes
the authoring, hosting and agent layer. The JS detmath and the JS-vs-Rust planner parity work disappear.

## 1. Architecture and layering

| Rust `oilcore` (WASM + native) | TypeScript `oilpaint` |
|---|---|
| scene-spec compiler: target, regions, flows, light map from primitives | scene authoring DSL that emits the spec; validation with fix hints |
| planner: incremental error planes, site proposal, path tracing, palette snap, ordering, layering | orchestration: default pipeline call or a user-written loop over planner stages |
| kernel (4 modes, later knife), mixers, lighting, image ops, metrics | Worker/canvas/OffscreenCanvas host, p5 add-on, progressive frames, time-lapse |
| StrokeList v2 codec, counter RNG, one math library | headless Node API, CLI, JSON feedback, agent docs and tooling |

**Canonical interface.**
- Input: `SceneSpec` JSON (versioned, `"$schema": "https://…/scene-spec/1.json"`), with optional raw guide maps
  (`Float32Array` target, masks, flows, light) as an escape hatch referenced by name.
- Output: `StrokeList v2` (binary: header, packed points, per-stroke params, layer table, engine version, input hashes).
- Everything between the two runs in Rust. A spec plus a seed is the whole reproducible description of a painting.

**Declarative primitives** (all seeded, in canvas-width units):
- shapes: `above/below/ellipse/disc/polygon/wedge/bandAround/union/intersect/noisy`
- target ops: `fill` (solid, `gradientV`), `blob`, `polygon`, `bands`, `glow`, `beam`
- flows: `constant/sweep/waves/swirlAround/radialFrom/upward/contour`, `facets` (new), and a structure-tensor fallback
- light: `glow/beam/lamp`
- palettes: tubes, hex and mixes; styles; layers; brush presets

These cover every call in the three existing scenes.

**Escape hatches:**
1. **Sampled fields.** A JS function `(x, y) => [dx, dy]`, `(x, y) => mask` or `(x, y) => rgb` is sampled by TS at
   guide resolution into a `Float32Array` before planning. Rust only ever sees numbers.
2. **Pure-function hooks** in a user-written TS loop (section 3), for example a stroke filter or a colour tweak.

**Determinism.**
- The Rust side is fully deterministic given its inputs. Sampled arrays and hook identities are hashed into the
  StrokeList header.
- Plans that used hooks or sampled fields carry `portable: false`: JS `Math.*` may differ between engines, so the same
  scene could plan differently in Safari. Authors may assert `portable: true` for arithmetic-only hooks.
- Painting a StrokeList is always portable.

## 2. API

Level 1 is p5.brush-like immediate painting: `set`, `line`, `flowLine`, `beginStroke/move/endStroke`, `fill`, `field`,
`mode`, `dry`, `render`. For agents:
- `createPainter({ width, height, units: "px" | "cw", seed })` returns an explicit object. Units and seed are
  required; the global `oil.*` is only a default instance for sketches.
- There is no hidden state: `p.state()` returns brush, field and mode as JSON, and `p.reset()` clears them.
- Every call returns a stroke id and is recorded, so `p.strokes()` returns a StrokeList that replays at any size.
- Rendering is explicit (no implicit frame loop), and errors use the structured format of section 5.

```ts
const p = await oil.createPainter({ width: 1200, height: 1500, units: "px", seed: 7, ground: "#e9e1d6" });
p.set("flat", ["cobalt_blue", "lead_white", 0.4], 24).paint({ pickup: 0.12, marble: 0.3 });
p.field("sea", { kind: "waves", angle: 0, amplitude: 9, wavelength: 0.18 }).flowLine(200, 900, 300);
await p.render({ light: "painting" });
```
Level 2 is the auto-painter. The TS DSL builds a ScenePlan whose `.spec` is plain JSON:
```ts
export default function build(S: Scene) {
  S.canvas({ aspect: [4, 5], ground: "#e9e1d6" });
  S.target.fill(gradientV([[0, "#232a58"], [0.4, "#66679e"], [HZ, "#e8cbb8"]]));
  S.region("sky", above(HZ), { edge: 0.03 }).flow(sweep({ angle: -14, curl: 0.5, noise: 0.3 }));
  S.style("sky", { colors: SKY, width: [0.025, 0.04], length: [0.10, 0.26], marble: 0.4 });
  S.layer("Sky long strokes", { regions: ["sky", "glow"], placement: "error", T: 12 });
}
// -> {"version":1,"canvas":{"aspect":[4,5],"ground":"#e9e1d6"},"regions":[{"name":"sky","mask":{"above":0.6},"edge":0.03,
//     "flow":{"sweep":{"angle":-14,"curl":0.5,"noise":0.3}}}], "styles":{...}, "layers":[...]}
const strokes = await oil.plan(build, { planWidth: 600, seed: 1907 });       // one Rust call
const job = oil.paint(strokes, { canvas, width: 2400, light: "painting" });   // Worker; frames(), done, toBlob(), timelapse()
```
The three Python scenes are hand-ported to TS modules (storm_v3.py is ~250 lines of DSL calls). The p5 add-on and the
standalone host are as in v2: p5 is a peer dependency, painting runs in a Worker, and the time-lapse uses WebCodecs
with mp4-muxer.

## 3. Planner as a toolbox

`oil.plan(spec)` is one composed Rust call. The same stages are exported through WASM, with a TS wrapper in
`oilpaint/plan`, so users can write their own loop:
```ts
const g = await plan.compile(spec, { width: 600 });            // Guides handle (+ g.debug() images)
const cv = plan.canvas(g); const rng = plan.rng(1907);
for (const [li, layer] of spec.layers.entries()) for (const region of layer.regions) {
  const sites = plan.sites(cv, g, { layer: li, region });        // error | density | curve placement
  for (const s of sites) {
    const path = plan.tracePath(g, region, s, rng);               // flow-following, stop at mask edges
    const color = plan.snapColor(g, region, plan.sampleRef(g, s), rng);
    if (myFilter(path, color)) plan.paintStroke(cv, plan.makeStroke(path, color, layer, region, rng));  // hook
  }
}
```
- **Stable in 1.0:** `compile`, `canvas`, `rng`, `errorPlane`, `sites`, `order`, `tracePath`, `sampleRef`,
  `snapColor`, `makeStroke`, `paintStroke`, `planLayer`, `plan`, `strokeList`.
- **Experimental:** gap fill, palette piles, brush-memory state, knife placement.
- **Internal:** lane evaluation and incremental error updates.

Handles and typed arrays cross the boundary, never JSON per call. A spike measured ≤0.5 µs per WASM call: 13,795
path traces took 16.5-20.9 ms as single calls vs 14-23 ms batched.

## 4. Rust core

```
oilcore/ (cargo workspace, MIT)
  oil-kernel  canvas planes, bristle-lane brush, StrokeList v2 codec, counter RNG, math (one pure-Rust implementation)
  oil-scene   ScenePlan spec types (serde + schemars -> JSON Schema -> TS types), spec compiler (guides)
  oil-plan    planner stages + composed plan(); incremental error planes
  oil-mix     km (default), rgb; mixbox in a separate crate/package (CC BY-NC)
  oil-image   blur, resize, Sobel, EDT, structure tensor, polygon fill, value noise, marching squares
  oil-light   relight, weave;  oil-metrics: agent feedback metrics (port of eval_metrics.py subset)
  oil-wasm    C-ABI exports + hand-written TS glue;  oil-native: rayon, CLI, ctypes shim for the Python harness
```
**Kernel spike** (`SP/work_lib/oilcore`, ~1,500 lines).
- It ports all of `brush.c`, the Mixbox polynomial, `relight` and the image ops, and builds native and to WASM
  (71-75 KB).
- Parity: Rust with platform libm is bit-identical to strict-built C on the swatch sheet and on all 12,495 Storm Light
  strokes at 600 and 1200 px. With `detmath`, native, Node and Chromium (scalar and SIMD128) give identical SHA-256 at
  600, 1200 and 2400 px.
- Today's fast-math C differs from strict C by up to 124/255 on 9.9% of pixels.
- Relight matches Python within 0.002/255 and is 2-6x faster.

| Storm Light kernel, single thread | 600x750 | 1200x1500 | 2400x3000 |
|---|---|---|---|
| C fast-math (today) | 2.7 s | 9.9 s | ~36 s |
| Rust native (libm / detmath) | 3.1-3.8 / 4.0 s | 11.9 / 13.3 s | – / ~50 s |
| WASM Node (detmath+SIMD) | 5.0 s | 15.7 s | 57 s, 647 MB |
| WASM headless Chromium 141 | 4.2-4.8 s | 16.2 s | 60 s, 561 MB |

- **Threads:** native uses rayon (fixed 16-row chunks and a stroke dependency scheduler; 1.3-1.6x measured in C on
  2 cores). In the browser, single-thread WASM in a Worker is the default. A threaded build (`wasm32-wasip1-threads` on
  stable or nightly `build-std`, both installable here) loads only when `crossOriginIsolated` (reached in the spike);
  results are identical either way.
- **Memory:** 56 B/px (403 MB at 2400x3000, ~580 MB with the new planes). Desktop browsers allow 4 GB, while iOS
  Safari kills tabs around 1-1.5 GB. Exact tiling is impossible, because pick-up reads whole segments across tiles, so
  small devices get a compact mode (f16 latent, ~34 B/px) or a size cap; big prints use the native CLI.

## 5. Determinism, parity, planner port

**Parity.** Rust owns all numerics, so there is no JS detmath. Inside Rust, every target must compile the same
pure-Rust maths (our `detmath` or the `libm` crate). Measured: platform glibc maths natively vs Rust's own maths in
WASM already differ on 0.23% of pixels. The rules are: no FMA or fast-math, no relaxed-SIMD, integer PCG32 streams per
layer/region, int64 pick-up sums, and `Vec`/`BTreeMap` iteration only.

| Level | Requirement | Status |
|---|---|---|
| A: Rust engine on every host | identical SHA-256 of StrokeList and canvas planes for the same spec, seed, size, version (native, Node, Chromium, Firefox, Safari) | kernel shown on native/Node/Chromium; planner by construction, CI to prove |
| B: legacy v1 replay | bit-identical to frozen strict C (Linux, platform libm) | shown |
| C: portable kernel vs C reference | rgb ≤ 1/255, h ≤ 1e-4 on the golden corpus | shown (h 2e-6) |
| D: size consistency (v2 RNG) | 1800 vs 2400 at 600 px: ≤ 1% of pixels > 8/255 | today 10.3%; expected fixed |
| E: Rust planner vs Python | metric-based, see below; host-to-host is level A, bit-exact | to do |

**Level E acceptance:**
- `eval run --scene` on Storm Light at 600 and 1600 stays within `eval/thresholds.json` of the Python baseline on every
  region metric (`hairline`, `bristle_L`, `edge_step_p99`, `C_mean`, `L_p50`, `coh_local`, `orient`, `acf_*`).
- Per-layer stroke count ±10%, width/length medians ±5%, angle-histogram distance ≤0.1.
- Sean signs off a side-by-side. The harness scores Rust StrokeLists through its ctypes shim.

| Python call | Replacement (all Rust) |
|---|---|
| `cv2.GaussianBlur`, `light.blur` | `oil-image` Gaussian, downsampled for sigma ≥ 8 (the 20 Storm Light mask blurs: Python 0.18 s, Rust 0.10-0.13 s) |
| `cv2.resize` (area, linear, nearest) | ported in the spike (matches cv2 within 0.002/255 in relight) |
| `cv2.remap` cubic + `np.random` lattice (`noise.fbm`) | hashed-lattice bicubic noise, evaluated per shape bbox (full-canvas 4-octave field: 0.065 s) |
| `cv2.distanceTransform` + `Sobel`, `skimage structure_tensor` | exact EDT + Sobel + Gaussian |
| `cv2.fillPoly`, `dilate/erode`, `findContours` | scanline fill, min/max filter, marching squares |
| `scipy cKDTree` | brute force over ≤3k Lab candidates (0.015 s for 12,500 snaps) |
| `np.random` uniform/normal/integers/shuffle | PCG32 + Box-Muller on the shared math |
| Mixbox LUT, Lab, `np.interp/cumsum` | Rust (`oil-mix`, `oil-plan`) |
| `cv2.line/putText` | TS Canvas2D debug views |

**Spike numbers** (600 px, Storm Light inputs):

| Stage | Python | JS | Rust → WASM | Rust native |
|---|---|---|---|---|
| error map ×27 | 1.52 s | 1.5 s (with LUT) | 1.3 s | 0.9 s |
| palette snap ×12,500 | 2.72 s | 0.04 s | 0.02 s | 0.015 s |
| path tracing ×13,795 | 1.30 s | 0.03 s | 0.015-0.026 s | 0.015 s |

- Spec compile: Python 1.9-2.8 s. Estimated Rust ~0.5-1 s native and 0.7-1.3 s WASM; not measured end to end.
- **Projected full planning at 600 px** in WASM, single thread: 5.5-7 s. That is proxy painting 4.2-5.0 s + compile
  ~1 s + error planes 0.2-1.3 s + planner logic ~0.1 s, against 32 s in Python.

## 6. Agent-first deliverables

1. **Headless Node API + CLI** (same WASM, no browser): `oilpaint render scene.ts --preview --json`. It writes PNGs and
   one JSON document an agent can read without looking at pixels:
   - size, seed, engine version and portable flag;
   - timing per stage;
   - per-layer strokes, coverage and metrics (`hairline`, `bristle_L`, `edge_step_p99`, `C_mean`, `L_p50`: the
     eval-harness subset ported to `oil-metrics`);
   - per-region coverage and stroke counts;
   - warnings with codes, for example `REGION_NO_FLOW` (falls back to structure tensor), `REGION_UNPAINTED`,
     `LAYER_OVERDRAW` (block-in >10 canvas areas), `BLACK_PIXELS`, `CLIPPING`;
   - paths to the layer contact sheet and the guide sheet.
2. **Short loops.**
   - Draft render: 600 px in about 6 s (projected), 300 px in about 2 s.
   - `--only-layer N` / `--only-region R` re-plans one layer from the cached canvas state before it and replays later
     layers (today's `--only`, extended to regions).
   - `--compare A.json B.json` reports metric deltas (the harness `compare` logic).
3. **Typed API and schema.**
   - Rust spec types generate the JSON Schema, which generates the TS types: one source of truth.
   - Brush presets, tubes, flows and style keys are shipped as a catalogue in data (name, description, units, range,
     "raise it → / lower it →", taken from STYLE_REFERENCE.md).
   - Validation errors are actionable. Example:
     `{code: "UNITS", path: "/styles/sky/width", got: 25, expected: "[min,max] in canvas widths 0.001-0.2", fix: "25 px at 1200 px = 0.021; or set units: 'px'"}`.
4. **Guide debug view:** `oilpaint guides scene.ts` renders region, flow, light and target sheets plus JSON stats
   (region areas, overlaps, flow coherence), so an agent can check the scene before painting.
5. **Docs for agents:**
   - `AGENTS.md` plus a skill file: the workflow, the edit-render loop, and reading the JSON.
   - A gallery of ~12 small scenes (5-15 lines each: sky gradient, sea, rock, tower, glow) plus Storm Light as the
     full example.
   - A recipe cookbook: lost edges, broken colour, glaze a glow, cut in an edge.
6. **No Rust toolchain for authors:** prebuilt WASM in npm (and optional prebuilt native binaries for the CLI).

## 7. Licence and packaging

MIT throughout. The default mixer is our Kubelka-Munk mixer on spectral.js data (MIT):
- Tested: blue+yellow gives a vivid green, tints stay clean, and Storm Light looks nearly the same as with Mixbox.
- Differences: ultramarine+yellow goes olive, and the harness sheet shows chroma 45 → 36 and a muddier mud stack.
- It needs a 3-day calibration against real paint charts (`evidence/sheet_mixbox_vs_km12.png`). The libmypaint WGM
  mixer was rejected as too dull.
- Mixbox becomes an optional `@oilpaint/mixbox` package (CC BY-NC).

New dependencies: serde, serde_json, schemars, optional `libm` and rayon (all MIT/Apache); `json-schema-to-typescript`
and mp4-muxer (MIT). p5 (LGPL) is a peer dependency, never bundled. No GPL.

Packages:
- `oilpaint`: API, hosts and CLI.
- `@oilpaint/core`: WASM.
- `@oilpaint/mixbox`: the optional Mixbox plug-in.

Delivery: ESM plus an IIFE build, `instantiateStreaming` with a single-file inlined build for CDNs. Tests: cargo
(golden digests); Node vitest; Playwright on Chromium and Firefox checking StrokeList and plane SHA-256 across hosts.
CI: GitHub Actions, plus a nightly Python harness run. Python stays the reference and golden generator; its planner
retires after level E, and the frozen C after L1.

## 8. Milestones

| L | Content | Days | Depends | Ends with |
|---|---|---|---|---|
| L0 | freeze strict C, golden corpus, StrokeList v2 + ScenePlan spec drafts, parity CI | 2 | – | parity table |
| L1 | Rust kernel, lighting, image ops, mixers, math, CLI, ctypes shim | 3 | L0 | harness report Rust vs C |
| L2 | ScenePlan v1 + JSON Schema + Rust spec compiler + TS DSL + validation | 4 | L1 | Storm Light guide sheets from TS; compile time measured |
| L3 | Rust planner port + composed `plan` + toolbox exports + incremental error planes | 5 | L2 | level E on Storm Light; identical StrokeList SHA in Node and Chromium |
| L4 | WASM host: Worker, Level-1 API, standalone and p5 adapters, frames, time-lapse | 4 | L3 | demo pages: brush playground, Storm Light painting live |
| L5 | agent tooling: headless API/CLI JSON, `oil-metrics`, partial re-render, catalogue, guide view, AGENTS.md, gallery, cookbook | 5 | L3 (L4 for docs) | agent runs the gallery from AGENTS.md alone |
| L6 | calibrated open mixer default, Mixbox plug-in | 3 | L1 | swatch comparison; **public alpha** |
| L7 | paint that settles (taper, displacement, fresh-paint levelling, spline outlines) | 3 | L1 | A/B/C strip (v1 evidence) |
| L8 | speed: threads native + WASM, per-pixel work, lighting worker | 4 | L4 | timing table, threads identical |
| L9 | planner fixes: cut-in edges, rock facets, halo glaze, tube palettes | 3 | L3, L11 for halo | tower/rock/halo crops |
| L10 | brush memory, flat/filbert/round, twist and speed | 4 | L1 | brush demo |
| L11 | wet/dry stages, optical glazes | 3 | L1 | glow/haze before/after |
| L12 | palette knife | 3 | L1 | knife demo |
| L13 | 1.0: docs site, CI hardening, release | 2 | all | published package |

Total **≈48 agent-days**, against 41 in v2. The public alpha lands after L6, at ~26 days.
- **Cheaper (−2):** no JS detmath, no JS-vs-Rust planner parity tests, no marshalling split between JS logic and WASM
  pixel ops, one numeric implementation.
- **More expensive (+9):** Rust port of the planner heuristics (+1 over TS), the Rust spec compiler with schema and TS
  DSL (+2), agent tooling (+5, new), and metrics in Rust (+1, inside L5).

**Browser expectations for Storm Light:**
- Single thread, on this 2.1 GHz VM: plan ~6-7 s (projected); paint 1200x1500 ~16 s and 2400x3000 ~60 s (measured).
- With threads (projected): 1.3-1.6x on 2 cores, and perhaps 2-3x on 8 cores for big strokes.

**Risks:**
- Planner heuristics that hide in numpy behaviour: level E catches them.
- Spec expressiveness vs. hooks: portability flag.
- Rust port verbosity: the toolbox stages double as test seams.
- The threaded WASM toolchain: the single-thread default is unaffected.
- iOS memory: compact mode.
- Open-mixer look: calibration plus the Mixbox plug-in.
- `detmath` cost ~10%: f32 polynomials.
- Agent JSON drifting from the harness: a shared metric crate, and the harness calls it.

## 9. Questions for Sean (my default in brackets)

1. **Old paintings:** must old stroke files keep repainting pixel-identical? [Yes, via the Rust legacy mode, already
   bit-identical to strict C.]
2. **Scene freedom:** how much scene-authoring freedom up front? Declarative only, or declarative plus JS functions?
   [Declarative plus pure-function hooks, sampled or in custom loops; hooks mark a plan non-portable.]
3. **Default colours:** is an open mixer that makes ultramarine+yellow olive until calibrated acceptable for the
   alpha? [Yes, with Mixbox as an opt-in plug-in.]
4. **Calibration data:** calibrate against real paint charts rather than Mixbox output? [Yes.]
5. **Browser size:** what is the largest size the browser must reach? [2400x3000 on desktop; bigger via the CLI.]
6. **Threads:** is the fast threaded mode only on pages that send two security headers acceptable? [Yes;
   single-thread works everywhere.]
7. **Name and order:** npm name `oilpaint`, alpha after L6, physics after? [Yes, if the name is free.]
