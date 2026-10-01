# LIBRARY_PLAN v4: Rust engine and planner, Ochrell mixer, agent-first TypeScript

Written 2026-09-30, and it supersedes v3 (`LIBRARY_PLAN_v3_cparity.md`). Oil painting only.

**2026-10-01 scoped authoring work:** the separately authorized brush catalog, named groups/local editing and
interactive preview are implemented as an additive `oil-author` module. See [API and format](../BRUSH_AUTHORING.md)
and [verification report](../reports/BRUSH_PRIORITIES.md). This does not claim L3 visual acceptance, retirement,
the full L4 host API, or the later brush-physics milestones. Existing engine/StrokeList outputs remain unchanged.

**Changed from v3:**
- **Hard break with v1.** There is no legacy mode, no frozen strict-C kernel, and no parity level defined relative
  to C. The v1 renderer lives on as git tag `v1-python-c`, which is the only way to reproduce Storm Light.
- **Ochrell is the default mixer.** It replaces the Kubelka-Munk mixer on spectral.js data and its 3-day
  calibration. Mixbox stays an opt-in, non-commercial plug-in for comparison.
- **Memory, timings and the browser size limit** are reworked around Ochrell's measured cost: 373 B/px and about
  4x the paint time of Mixbox.
- **Physics moves before the hosts.** It goes straight into the Rust kernel after the planner port.

**Where the numbers come from.** Every number is labelled *measured* or *projected*.
- *Measured* numbers come from `docs/plans/SPIKE_LOG.txt`, on a 2 vCPU Xeon VM at 2.1 GHz under Linux with
  rustc 1.95 (called "the VM" below).
- They also come from `../ochrell/docs/integration-results.md`, on this Windows laptop (Core Ultra 7 258V,
  rustc 1.91.1 MSVC; called "the laptop").
- Projections name the numbers they scale from.

Spike code paths: `spikes/oilcore` (Rust kernel port), `native/ochrell-brush` (Ochrell bridge),
`spikes/mixers`, `spikes/engine-prototypes`. Physics details: `ENGINE_PLAN_v1.md`.

## 1. Architecture and layering

| Rust `oilcore` (WASM + native) | TypeScript `oilpaint` |
|---|---|
| scene-spec compiler: target, regions, flows and light map from primitives | scene authoring DSL that emits the spec; validation with fix hints |
| planner: incremental error planes, site proposal, path tracing, palette snap, ordering, layering | orchestration: the default pipeline call, or a user-written loop over planner stages |
| kernel (4 modes, later the knife), mixers behind a trait, lighting, image ops, metrics | Worker/canvas/OffscreenCanvas host, p5 add-on, progressive frames, time-lapse |
| StrokeList v2 codec with the engine-version gate, counter RNG, one pure-Rust maths library | headless Node API, CLI, JSON feedback, agent docs and tooling |

**Canonical interface.**
- **Input:** `ScenePlan` JSON (versioned, `spec/sceneplan-1.schema.json`). Optional raw guide maps (`Float32Array`
  target, masks, flows, light) can be referenced by name as an escape hatch.
- **Output:** `StrokeList v2` (binary: header, packed points, per-stroke params, layer table, **engine version**,
  input hashes). The spec is in `spec/STROKELIST_V2.md`.
- Everything between the two runs in Rust. A spec plus a seed is the whole reproducible description of a
  painting, for one engine version.

**Declarative primitives** (all seeded, in canvas-width units, and unchanged from v3):
- shapes: `above/below/ellipse/disc/polygon/wedge/bandAround/union/intersect/noisy`
- target ops: `fill` (solid, `gradientV`), `blob`, `polygon`, `bands`, `glow`, `beam`
- flows: `constant/sweep/waves/swirlAround/radialFrom/upward/contour`, `facets`, and a structure-tensor fallback
- light: `glow/beam/lamp`
- palettes (tubes, hex and mixes), styles, layers and brush presets

These cover every call in the three existing scenes.

**Escape hatches** (decided: declarative primitives plus pure-function hooks):
1. **Sampled fields.** A JS function `(x, y) => [dx, dy] | mask | rgb` is sampled by TS at guide resolution into
   a `Float32Array`, and the array is hashed into the StrokeList header.
2. **Pure-function hooks** in a user-written TS loop (section 3).

A plan that used either carries `portable: false`, because JS `Math.*` may differ between engines. Authors may
assert `portable: true` for arithmetic-only hooks. Painting a StrokeList is always portable.

## 2. API (unchanged from v3)

**Level 1** is p5.brush-like immediate painting: `set`, `line`, `flowLine`, `beginStroke/move/endStroke`, `fill`,
`field`, `mode`, `dry`, `render`.

For agents:
- `createPainter({ width, height, units: "px" | "cw", seed })` returns an explicit object. Units and seed are
  required.
- There is no hidden state: `p.state()` returns JSON.
- Every call returns a stroke id, and `p.strokes()` returns a StrokeList that replays at any size.
- Rendering is explicit, and errors are structured (section 8).

```ts
const p = await oil.createPainter({ width: 1200, height: 1500, units: "px", seed: 7, ground: "#e9e1d6" });
p.set("flat", ["cobalt_blue", "lead_white", 0.4], 24).paint({ pickup: 0.12, marble: 0.3 });
p.field("sea", { kind: "waves", angle: 0, amplitude: 9, wavelength: 0.18 }).flowLine(200, 900, 300);
await p.render({ light: "painting" });
```

**Level 2** is the auto-painter. A TS DSL builds a ScenePlan whose `.spec` is plain JSON; `oil.plan(build,
{ planWidth, seed })` is one Rust call, and `oil.paint(strokes, { canvas, width, light })` runs in a Worker. The
three Python scenes are hand-ported to TS modules. p5 is a peer dependency (never bundled), and the time-lapse
uses WebCodecs with mp4-muxer.

## 3. Planner as a toolbox (unchanged from v3)

`oil.plan(spec)` is one composed Rust call. The same stages are exported through WASM, with a TS wrapper in
`oilpaint/plan`, so users can write their own loop: `compile`, `canvas`, `rng`, `errorPlane`, `sites`, `order`,
`tracePath`, `sampleRef`, `snapColor`, `makeStroke`, `paintStroke`, `planLayer`, `plan`, `strokeList`.

- **Stable in 1.0:** the list above.
- **Experimental:** gap fill, palette piles, brush-memory state, knife placement.
- **Internal:** lane evaluation, incremental error updates.

Handles and typed arrays cross the boundary, never JSON per call. The call cost was *measured* at ≤0.5 µs per WASM
call (Node, the VM): 13,795 path traces took 16.5-20.9 ms as single calls and 14-23 ms batched.

## 4. Rust core

```
Cargo.toml (workspace)   crates/                        (L1 built the first ten; L2 added oil-errors, oil-scene, oil-wasm)
  oil-math        exp, sin/cos, ln, pow, cbrt, atan2, sRGB transfer: pure Rust from IEEE basic ops; the only maths used
  oil-mix         Mixer trait; ochrell (default, pinned by commit), rgb; sRGB transfer tables for per-pixel work
  oil-mix-mixbox  opt-in Mixbox mixer (CC BY-NC), never a default dependency
  oil-kernel      canvas planes (borrowed Planes view), bristle-lane brush, 4 modes, counter RNG, chunked pick-up sums
  oil-strokes     StrokeList v2 codec, engine-version gate, validation
  oil-image       blur, resize, Sobel; hashed value noise, polygon fill, EDT, min/max (L2); marching squares in L3
  oil-light       relight, weave
  oil-paint       StrokeList + mixer + width -> canvas; the procedural test sheet
  oil-cli         `oil`: version, info, testsheet, paint (PNGs + JSON report)
  oil-shim        ctypes shim for the Python harness (bridge ABI); oil-xhost: cross-host cases (native + WASM)
  oil-errors      structured errors {code, path, got, expected, fix} and did-you-mean                       L2
  oil-scene       ScenePlan types (serde + schemars -> JSON Schema -> TS types), validation, compiler, previews L2
  oil-plan        planner stages and the composed plan(); incremental error planes                           L3
  oil-wasm        C-ABI exports and hand-written TS glue (minimal in L2: validate, schema, guides)            L4
  oil-metrics     agent feedback metrics (port of the eval_metrics.py subset)                               L5
```

The workspace depends on `../ochrell` as a sibling checkout pinned by commit. CI checks out the pinned revision.
The golden hashes (section 5) catch any output change in Ochrell.

**Mixer trait.** It lets a compact mixer coexist with Ochrell.
- **Contract.**
  - `ID` is a stable string, recorded in outputs.
  - `LAT` is the floats per state (Ochrell 85, Mixbox 7, rgb 3).
  - `encode(srgb) -> state` and `decode_linear(state) -> [f64; 3]`.
  - `validate(state)`, plus the streak and interpolation rules.
- **Generic kernel.** The kernel is generic over `M: Mixer` and monomorphised, so there is no dynamic dispatch
  per pixel.
- **Packaging.** One WASM build carries Ochrell and rgb. The Mixbox plug-in is a separate WASM build of the same
  kernel, which keeps the licences apart.
- **Colours in StrokeLists.** They are stored as authored sRGB, not latents. A StrokeList is mixer-independent
  and small, and the engine encodes each colour once per replay.

**Transport** (how paint moves between brush and canvas; decided after L1): v1's.
- A deposit moves the pixel's mixer state toward the brush paint by the deposit alpha.
- Mixing with the paint underneath comes from the brush: each bristle tip picks up wet paint ("dirt") and carries
  it along.
- The material transport of the Ochrell integration (paint amounts, amount-weighted mixing with all wet paint) was
  tried in L1 and dropped because it washed out broken colour.
- The film spike (F0, section 11) tests a layered film before L7.

**Kernel, from the spike.** `spikes/oilcore`, about 1,500 lines, ports all of `brush.c`, `relight` and the image
ops. It builds native and to WASM (71-75 KB). *Measured on the VM:*
- Rust with platform libm is bit-identical to strict-built C on the swatch sheet and on all 12,495 Storm Light
  strokes.
- With pure-Rust maths (`detmath`), native, Node and Chromium (scalar and SIMD128) give identical SHA-256 at 600,
  1200 and 2400 px.
- Relight matches Python within 0.002/255 and is 2-6x faster.

The Ochrell bridge `native/ochrell-brush` already compiles this kernel with an 85-float state. Its batched and
single-stroke replays are exact, and so are snapshot-and-resume (*measured*, laptop).

**Threads (optional).**
- **Native:** rayon, with fixed 16-row chunks and a stroke-dependency scheduler (1.3-1.6x *measured* in C on 2
  cores).
- **Browser:** single-thread WASM in a Worker by default. A threaded build loads only when `crossOriginIsolated`.
- Results are identical with or without threads (section 5).

## 5. Determinism: the engine's own guarantee

**Guarantee.** For one engine version, the same inputs give byte-identical outputs on every supported host:
- Same ScenePlan, seed and plan options give the same StrokeList.
- Same StrokeList and paint options (mixer, size, light) give the same canvas planes and PNGs.

The supported hosts are native (Windows x64 MSVC, Linux x64, macOS arm64), Node ≥ 22, Chromium and Firefox; WebKit
is best-effort. Nothing is promised across engine versions. Outputs are compared by SHA-256.

**Engine version.**
- A single string (`oil_kernel::ENGINE_VERSION`, for example `2.0.0-dev.1`), bumped by any change that can move an
  output bit: kernel, planner, compiler, mixer (including the pinned Ochrell model), maths, RNG or codec.
- It is written into every StrokeList and every JSON report.
- Loading a StrokeList written by another engine version fails with `ENGINE_VERSION_MISMATCH`. The message names
  both versions and says how to get the matching one (npm version, or git tag).
- A v1 `strokes.npz` fails with `V1_STROKE_FILE` and points to the tag `v1-python-c`.
- There is no loader or converter for v1 files.

**Rules that make the guarantee hold:**
- **One maths implementation.** Every transcendental that can reach a pixel or a stroke goes through `oil-math`:
  exp, sin/cos, atan2, pow, cbrt and the sRGB transfer, all built from IEEE basic operations. Basic ops and `sqrt`
  are correctly rounded on x86-64, aarch64 and wasm32.
  - Platform libm is banned by clippy `disallowed-methods` (`f32/f64::exp/ln/powf/powi/sin/cos/atan2/cbrt/tanh`),
    and so is `mul_add`.
  - *Measured on the VM:* glibc libm natively vs Rust's own libm in WASM already differed on 0.23% of pixels.
- **Ochrell's maths.** Code inspection found platform `powf` on the kernel path, in the sRGB transfer:
  - `Color::linear` (encode);
  - `Color::from_linear_gamut_mapped` (every `decode`);
  - `conversion::srgb_to_linear` / `linear_to_srgb`, called by the bridge's per-pixel coverage composite
    (`material::composite`).

  `cbrt` (OKLab) and `powi` appear only in diagnostics, and the K/S optics use only `+ - * / sqrt`.

  *Measured* (L0 cross-host negative control, laptop): that exact sRGB transfer with platform `powf` differs between
  native Windows (the C runtime's pow) and WASM (Rust's libm) on 9.7% of 400,000 f64 results, by up to 4 ulp.
  Platform `exp` differs on 9.5%.
  - After rounding to f32, as Ochrell's decode does, none of that grid flipped.
  - The encode residual is a difference of f64 values, though, and a 2400 px painting runs about 10⁹ per-pixel
    composites. So flips are expected (*projected*), and pick-up then propagates them.
  - The `oil-math` cases were identical on native, Node, Chromium, Firefox and WebKit.

  **Fix:** the engine never calls those functions. It converts sRGB to and from linear with `oil-math`, calls
  Ochrell's existing `decode_linear`, and needs one additive Ochrell API, `encode_linear(&self, [f64; 3])`.
  `encode(c)` becomes `encode_linear(c.linear())`, bit-identical for existing callers. There are no dependencies
  and no `unsafe`, and Ochrell's tests stay green. **Done in L1:** Ochrell commit `ffd6ee9`, after its optimisation
  rounds were committed; the engine pins that commit (`docs/reports/L1.md`).
- **No FMA, fast-math or relaxed SIMD.** SIMD128 is allowed only for lane-wise IEEE ops with a fixed reduction
  order that the scalar path shares.
- **Order-free state.** RNG is by counter (a hash of seed, stroke, lane, sample and purpose), so the sample count
  moves only the array end, never the pattern. Pick-up sums accumulate per fixed 16-row chunk (by absolute row)
  and are added in chunk order, so any split of rows over threads gives the same bits. (L1 dropped v3's int64
  fixed-point sums: Ochrell's K times paint-amount sums overflow or lose precision in a 32.32 format.) Collections
  that feed outputs are `Vec`/`BTreeMap` only.
- **No NaN in state.** WASM NaN payloads are not deterministic. Validation happens at every boundary, with debug
  assertions in the kernel.

**Checks** (continuous from L1):

| Check | Requirement | Status |
|---|---|---|
| G1 cross-host | identical SHA-256 of StrokeList bytes and of every canvas plane on native, Node, Chromium and Firefox (WebKit reported) | **L1: kernel shown** on native Windows, Node, Chromium, Firefox, WebKit, Edge and Chrome, for the test sheet (all planes, three mixers) and Storm Light at 600 px (*measured*). **L2:** Storm Light's guide planes and the validation codes on the same 7 hosts. **L3:** the planner's StrokeLists (Storm Light, still life) on the same 7 hosts |
| G2 invariance | identical output across 1 vs N threads, batched vs single-stroke replay, snapshot and resume vs straight through, and every memory mode (contiguous, tiled, cold) | batch/single and snapshot/resume exact in the Ochrell bridge (*measured*); threads L8; tiles L8 |
| G3 size consistency | replay at 1800 and 2400, downsampled to 600: ≤ 1% of pixels > 8/255 | **L1: 0.03%** against v1's 10.93% on the same strokes (*measured*, laptop) |
| G4 version gate | wrong-version StrokeList gives `ENGINE_VERSION_MISMATCH`; v1 npz gives `V1_STROKE_FILE` | **L1: done** (codec tests; a real `dev.0` file and your v1 `strokes.npz` are both refused with clear messages) |
| G5 golden regression | `golden/<engine-version>.json` holds hashes of fixed corpora; any change without a version bump fails CI | **L1:** `golden/2.0.0-dev.1.json`, 25 cases; regression only, generated by the new engine. **L2:** `golden/2.0.0-dev.2.json`, 29 cases (7 scene-compiler cases added, no output changed). **L3:** `golden/2.0.0-dev.3.json`, 31 cases |

**One-off port checks.** These are sanity checks, run once and then retired, not guarantees.

| Check | Requirement | When |
|---|---|---|
| P1 kernel port | step 1: bit-exact against the spike kernel (same maths feature, Mixbox, same latents); step 2, after the engine changes (counter RNG, chunked sums, `oil-math`): the new kernel against the C kernel through the eval harness, within the warn tolerances of `eval/thresholds.json`. There is no C compiler on the laptop, so step 2 uses the spike Rust kernel (`OILPAINT_KERNEL=rust`, shown bit-identical to strict C) | **L1: passed.** Step 1 bit-identical; step 2 within one standard error over 8 seeds; the spike matches the C baseline on every sheet metric |
| P2 Ochrell port | new kernel with Ochrell against the `native/ochrell-brush` bridge: the 17 gates of `tools/check_ochrell.py`, plus harness deltas reported | **L1: passed.** 17/17 gates; within one standard error over 8 seeds |
| ~~E planner port~~ | dropped: the planner port is a cutover (below) | – |

**The planner port is a cutover** (Sean, 2026-09-30). v3's level E (the Rust planner within harness thresholds of the
Python planner) is dropped, and Storm Light is not reproduced. The new planner starts from v1's mechanisms but
changes them from the first commit:
- neutral defaults and style presets;
- flows evaluated at stroke points;
- stroke variety;
- pick-up per distance.

L3 acceptance instead:
- The Rust planner plans Storm Light (`scenes/storm_v3.ts`, Impressionist preset) and a second scene end to end from
  TS, natively and in WASM.
- **Sanity gates:**
  - every region is painted: coverage at least 95% of its soft-mask area;
  - no bare canvas where the target has paint: at most 0.5% of pixels at the ground colour;
  - stroke sizes stay within each style's ranges.
- **Reported for information only, not a gate:** the harness's region metrics and per-layer stroke statistics
  against v1's plan of the same scene.
- Identical StrokeList SHA-256 on native, Node, Chromium and Firefox.
- Planning time, measured.
- Sean signs off a side-by-side of the new planner against v1.

Then one dedicated commit removes the C kernel, the Python kernel wrapper and the Python planner (the tag keeps
them). The Python eval harness stays until the Rust metrics replace it (L5). From that commit it measures the
engine's plans and renders, through the `oil` CLI and the shim.

**Python calls and their replacements** (all in Rust):

**Python calls and their replacements** (all in Rust):

| Python call | Replacement |
|---|---|
| `cv2.GaussianBlur`, `light.blur` | `oil-image` Gaussian, downsampled for sigma ≥ 8: the 20 Storm Light mask blurs took Python 0.18 s and Rust 0.10-0.13 s (*measured*, VM) |
| `cv2.resize` (area, linear, nearest) | ported in the spike (matches cv2 within 0.002/255 in relight) |
| `cv2.remap` cubic + `np.random` lattice (`noise.fbm`) | hashed-lattice bicubic noise, per shape bbox: 0.065 s for a full 600x750 4-octave field (*measured*, VM) |
| `cv2.distanceTransform` + `Sobel`, `skimage structure_tensor` | exact EDT + Sobel + Gaussian |
| `cv2.fillPoly`, `dilate/erode`, `findContours` | scanline fill, min/max filter, marching squares |
| `scipy cKDTree` | brute force over ≤ 3k Lab candidates (0.015 s for 12,500 snaps, *measured*, VM) |
| `np.random` | PCG32 + Box-Muller on `oil-math` |
| Mixbox LUT, Lab, `np.interp/cumsum` | `oil-mix`, `oil-plan` |

## 6. The mixer: Ochrell by default

**Ochrell 0.2** (`../ochrell`, code MIT OR Apache-2.0) does spectral Kubelka-Munk mixing on synthetic pigments.
- **State:** 41 bands of K and S plus an RGB residual, 85 f32 = 340 B.
- **What persists:** the material history in each pixel. An RGB round trip loses it: *measured* mean/max ΔE2000
  of 1.02/12.8 on four-colour mixes.
- **Limits:** no real-paint calibration, no physical glazing. Height does not affect reflectance.
- **Why it replaces the v3 plan:** it replaces "our KM mixer on spectral.js data + 3-day calibration". The
  calibration step is dropped; Ochrell's own evaluation is the evidence (`../ochrell/docs/revision-report.md`).

**Cost, measured** (laptop, single thread; `integration-results.md`):

| | Mixbox (v1 renderer) | Mixbox, 85-float control | Ochrell |
|---|---:|---:|---:|
| state interpolation / decode / cached mix + decode, ns | 4.70 / 18.76 / 39.64 | – | 36.22 / 183.07 / 222.65 |
| Storm Light replay at 600 px (13,988 strokes, median of 3), s | 4.55 | 14.77 | 18.03 |
| canvas bytes per pixel | 57 | 373 | 373 |
| peak working set at 600x690, paint + light, MB | 147.6 | 277.9 | 273.7 |

Reading the table:
- Most of the slowdown (4.5 → 14.8 s) comes from moving an 85-float state per painted pixel. Decode accounts for
  the smaller remaining part (14.8 → 18.0 s). L8 therefore targets state traffic first.
- Naive f16 storage was *measured* and rejected: 0.0001 pick-ups vanish (mean/max ΔE2000 10.9/41.8 on
  `tiny_pickup_4096`). Keeping the residual in f32 does not rescue it.

**Mixbox** (CC BY-NC 4.0) is an opt-in plug-in: crate `oil-mix-mixbox` and package `@oilpaint/mixbox`, a separate
WASM build. It is used for comparisons and for the P1 and E port checks, which compare like with like. The
default builds never contain it.

**Compact mixer slot.** The trait keeps room for a cheaper state (rgb today; later, for example, a reduced-band
Ochrell variant or a recipe-weight state). Any compact mixer is a different mixer ID, so outputs differ by
definition. It must be measured against Ochrell f32 on the ochrell storage-error cases before it is offered.

## 7. Memory and size limits

**Canvas planes per pixel:**
- latent: 340 B
- display RGB: 12 B
- height, wet, coverage, amount and height-blur: 20 B
- region mask: 1 B

That is 373 B/px, *measured* and verified in the ochrell report. The engine (L1) has no region plane, so it uses
**368 B/px** with Ochrell (56 with Mixbox, 40 with rgb; *measured*; v1's transport needs no `amount` plane). L7 and L11 add about 26 B/px (edge and fresh
planes, age, glaze film), so about 400 B/px (*projected*). These totals exclude planner proxies, lighting
temporaries and snapshots. *Measured* peak memory of the native CLI (L1): 171 MB, 712 MB and 3.0 GB at 600, 1200
and 2400 px.

| Canvas | Mpx | Ochrell planes | Mixbox v1 (57 B/px) |
|---|---:|---:|---:|
| 600x750 | 0.45 | 168 MB (*measured*) | 26 MB |
| 800x1000 | 0.80 | 298 MB | 46 MB |
| 1200x1500 | 1.80 | 671 MB | 103 MB |
| 1600x2000 | 3.20 | 1.19 GB | 182 MB |
| 2400x3000 | 7.20 | 2.69 GB (*measured*) | 410 MB |

Rows without a label are arithmetic from the *measured* 373 B/px.

**Browser size limit** (decided: it follows the memory finding):
- wasm32 memory tops out at 4 GiB in desktop Chromium and Firefox. iOS Safari kills tabs at about 1-1.5 GB.
  These are reported limits, *not measured* here.
- **A single allocation cannot exceed 2 GiB on wasm32** (*measured*, L1: Storm Light at 2400 px aborts in Node and
  Chromium). The Ochrell latent plane crosses 2 GiB at about 6.3 Mpx (about 2240x2800), so beyond that a browser
  needs L8's tiled storage whatever its memory budget.
- The host checks `width x height x bytesPerPixel(mixer)` against a budget before allocating, and refuses with
  `CANVAS_TOO_LARGE`. The error carries the largest allowed size at the requested aspect, and the CLI command.
- The defaults below are *projected*; L4 measures real peak memory in Chromium and Firefox and adjusts them.

| Host | Default budget for canvas planes | Largest 4:5 canvas | Notes |
|---|---|---|---|
| desktop browser | 1.2 GB | 1600x2000 | a `maxCanvasBytes` option raises it at the author's risk |
| mobile browser (touch UA) | 0.3 GB | 800x1000 | |
| Node / native CLI | machine RAM | 2400x3000 (2.69 GB) and above | |

**Memory modes.** v3's compact f16 mode is dropped because it was measured and rejected.
1. **Contiguous** (default) keeps full f32 planes.
2. **Tiled lossless storage** (L8), which Ochrell names as its next step.
   - Canvas planes live in 64x64 tiles.
   - A tile is *unallocated* until first touched (it reads as ground), *hot* (f32), or *cold* (compressed
     losslessly: XOR-delta against the left neighbour, byte-plane shuffle, then an LZ-class coder under a
     permissive licence).
   - Before a segment is painted, every tile its bbox and pick-up can reach is made hot. An LRU keeps the hot set
     under budget.
   - Output is bit-identical to contiguous by construction (G2).
   - Sparse allocation helps only while regions stay unpainted, and a finished scene touches everything. The win
     therefore depends on the compression ratio of painted K/S data, which is **unknown**. L8 starts with a
     half-day probe on the final 2400x3000 Storm Light canvas.
   - If the ratio is at least 2.5x, desktop browsers can reach 2400x3000 in about 1.1 GB cold plus the hot set
     (*projected*). Otherwise the browser limit stays as above, and big prints use the native CLI.
3. **Compact mixer** (section 6). It changes the output, so it is a different mixer, not a memory mode.

## 8. Timings

| Storm Light, single thread | 600x750 | 1200x1500 | 2400x3000 |
|---|---|---|---|
| **L1 engine, Ochrell, native CLI** (*measured*, laptop) | **4.6–5.9 s** | **16.1–18.9 s** | **65.5–71.3 s** |
| **L1 engine, Ochrell, WASM Chromium / Node** (*measured*) | **9.1–9.4 s** | **33–34 s** | impossible (2 GiB allocation) |
| **L1 engine, Ochrell, WASM WebKit** (*measured*) | 15.3 s | 46.9 s | – |
| L1 engine, Mixbox / rgb, native CLI (*measured*) | 3.5 s / 3.0–3.2 s | – | – |
| paint, Mixbox, laptop (Python dispatch + native) | 4.5 s (*measured*) | – | – |
| paint, Ochrell bridge, laptop (Python dispatch + native) | 18.0 s (*measured*) | ~59 s (v4 projection) | ~225 s (v4 projection) |
| v4 projection, Ochrell, WASM (Node / Chromium) | ~22 s | ~74 s | – |
| L8 target: threads and state traffic | ≤ 3 s native, ≤ 5 s WASM | ≤ 10 / ≤ 17 s | ≤ 40 s native |
| relight, Rust | 0.05 s (*measured*, VM) | 0.21 s (*measured*, VM) | 1.55 s WASM (*measured*, VM) |

L1 measured Ochrell in the engine at about 1.6x Mixbox, not 4x: no Python dispatch, and the display decode goes
through sRGB tables instead of `powf`. The sRGB tables (planned for L8) landed in L1. Firefox's WASM speed is not
measured: the only Firefox here is Playwright's build, which runs WASM on its baseline tier (78–84 s at 600 px,
bit-identical output); L4 measures a stock Firefox.

How the v4 projections were made:
- Size scaling uses the spike kernel's ratios on the VM: 600 → 1200 was 3.3x, and 600 → 2400 was 12.5x.
- WASM/native is 1.25x (Node det+SIMD 5.02 s against native detmath 4.04 s, VM).
- The VM and the laptop are different machines. Ratios transfer; absolute numbers only roughly.

**Planning at 600 px (WASM, single thread).**
- v3 *projected* 5.5-7 s with a Mixbox proxy canvas: proxy painting 4.2-5.0 s, compile about 1 s, error planes
  0.2-1.3 s, logic about 0.1 s.
- With an Ochrell proxy, proxy painting is about 4x, so planning is about 18-22 s (*projected* in v4).
- **Updated after L1:** Ochrell paints at about 1.6x Mixbox in the engine and WASM at 9.1-9.4 s per 600 px replay,
  so planning at 600 px is about 11-13 s in WASM (*projected*), about 5-6 s at plan width 320.
- **Measured in L3:** planning Storm Light at 600 px takes 7.6 s native and 15.2–15.6 s in WASM (Chromium,
  Node), against the *projected* 11–13 s; the still life takes 2.5 s and 4.6–4.8 s. v1's Python planner takes
  22–25 s on the same kernel.
- **Updated after L2:** the guide compile is *measured* on Storm Light at 600 px: 0.69 s native, 1.16 s Chromium,
  1.39 s Node (medians, one interleaved pinned round; v1 took 4.1 s). Flows are half of it.
- **Decided after L2 (Sean): the planner evaluates authored flows at stroke points** instead of rasterising 20
  whole-canvas fields (160 B/px, 72 MB at 600 px). *Measured:* v1's planner makes 145,971 flow lookups on Storm Light
  (12,433 strokes), under 2% of the 9M raster values; an exact point evaluation of the sky's sweep costs 124 ns, so
  all lookups take about 18 ms. The structure-tensor fallback, contour distance fields (over the polygon's area) and
  sampled fields (at their own size) stay rasters. Soft masks are stored cropped to their non-zero box and 16-bit
  (34 MB → 9.6 MB cropped, *measured*). Guides drop from 265 to about 36 B/px (*projected*).

**Agent loop** (*projected*, updated after L1):
- A draft is 300 px paint plus plan width 300, about 5 s in WASM.
- A 600 px preview is about 20-23 s in WASM (plan about 11-13 s, paint 9.1-9.4 s *measured*), about 8-10 s native,
  and less after L8.
- `--draft` may also select the rgb mixer, for layout-only iterations, and says so in its JSON.
- v3's "600 px in about 6 s" is reachable natively for the paint alone (4.6-5.9 s, *measured*), not end to end.

## 9. Agent-first deliverables (unchanged from v3 except for the timings)

1. **Headless Node API and CLI** (same WASM, no browser): `oilpaint render scene.ts --preview --json`. It writes
   PNGs and one JSON document an agent can read without looking at pixels:
   - size, seed, **engine version**, mixer ID and portable flag;
   - timing per stage;
   - per-layer strokes, coverage and metrics (`hairline`, `bristle_L`, `edge_step_p99`, `C_mean`, `L_p50`);
   - per-region coverage;
   - warnings with codes (`REGION_NO_FLOW`, `REGION_UNPAINTED`, `LAYER_OVERDRAW`, `BLACK_PIXELS`, `CLIPPING`,
     `CANVAS_TOO_LARGE`);
   - paths to the contact sheets.
2. **Short loops:** draft and preview (section 8); `--only-layer N` / `--only-region R`; `--compare A.json B.json`.
3. **Typed API and schema.**
   - Rust types generate the JSON Schema, which generates the TS types.
   - A data catalogue of brush presets, tubes, flows and style keys (name, units, range, "raise it → / lower it →").
   - Actionable validation errors, for example `{code, path, got, expected, fix}`.
4. **Guide debug view:** `oilpaint guides scene.ts` renders the sheets plus JSON stats.
5. **Docs for agents:** `AGENTS.md` and a skill file, a gallery of about 12 small scenes plus Storm Light, and a
   recipe cookbook.
6. **No Rust toolchain needed by authors:** prebuilt WASM in npm, with optional prebuilt native CLI binaries.

## 10. Licence and packaging

**Code.** MIT throughout. New dependencies are serde, serde_json and schemars, plus optional rayon (all MIT or
Apache), `json-schema-to-typescript` and mp4-muxer (MIT). p5 (LGPL) is a peer dependency, never bundled. There is
no GPL code; libmypaint and Krita are for ideas only.

**Ochrell's data is CC BY-SA 4.0.** This is my reading, not legal advice.
- **What is covered.** The runtime needs Ochrell's generated tables (`src/optical_generated.rs`: the basis
  spectra and the CIE-derived quadrature weights). Ochrell licenses these, conservatively, as CC BY-SA 4.0 derived
  data, because they derive from CIE 2019 datasets under that licence (`../ochrell/data/README.md`). Any build
  with the default mixer, WASM or native, therefore contains CC BY-SA material.
- **Attribution.** Anyone who shares it, including every app that bundles oilpaint, must:
  - credit CIE (and the Colour transport notice), with the DOIs;
  - name the licence, with a link;
  - keep the disclaimer;
  - say that the data was modified.

  A licences file in the package, plus a runtime `oil.notices()`, satisfies "reasonable to the medium".
- **ShareAlike.** It binds the *adapted data*. Changed tables must be shared under BY-SA 4.0, or GPLv3 as the
  compatible licence. It does not, on the usual reading and CC's own FAQ, turn the engine code that uses the data
  into BY-SA.
  - The combined artefact is **"MIT AND CC-BY-SA-4.0"**, the position Ochrell's README already takes for itself.
  - Commercial use is allowed (unlike Mixbox).
  - No terms or DRM may be added on top of the data.
  - Paintings rendered with it are computed results, not adaptations of the tables.
- **Consequence for "an MIT library".** It is not a pure-MIT artefact. Some corporate licence policies flag
  BY-SA, even though the obligations are mostly attribution.
- **Options:**
  - **(A) Ship as is. Chosen by Sean on 2026-09-30.** The package licence is `(MIT AND CC-BY-SA-4.0)`, the
    tables stay isolated inside the Ochrell crate, and the notices are exposed (licence file, NOTICE, and
    `oil.notices()` at runtime). L6 packages it.
  - **(B)** Move the tables to a separately shipped data file. The code artefacts become pure MIT; the
    obligations for the data do not change, and Ochrell needs an API change.
  - **(C)** Regenerate the tables from permissive sources, for example an analytic CMF fit. That is a
    model-changing Ochrell project with its own evaluation.
  - **(D)** Ship Ochrell as `@oilpaint/ochrell`, separate from an MIT core whose default is rgb. That contradicts
    "Ochrell is the default".
- **Publishing** follows option A, with clear licence and notices. Until L6 nothing is published at all.
  `../ochrell` stays a path dependency, so nothing is vendored into this repo.

**Packages:**
- `oilpaint`: API, hosts and CLI. The name was free on npm on 2026-09-30.
- `@oilpaint/core`: the WASM.
- `@oilpaint/mixbox`: the non-commercial plug-in. The `@oilpaint` scope needs an npm org and is unverified.

**Delivery.** ESM plus an IIFE build, and `instantiateStreaming` with a single-file inlined build for CDNs.
- Tests: cargo (goldens), Node vitest, and Playwright on Chromium and Firefox checking the G1 hashes.
- CI: GitHub Actions matrix (Windows, Linux, macOS native, plus Node and the browsers). While the harness lives, a
  nightly Python harness run.

## 11. Milestones (re-cut)

Milestone IDs are kept from v3 so that references stay valid; the **Order** column is the execution order. The
physics milestones run straight after the planner port and its retirement commit, so that:
- the Python planner never has to drive new physics;
- the plane layout (memory design) is final before the hosts;
- the brush-parameter catalogue and the gallery are written once.

### Realism roadmap (decided after L2)

**The goal** (Sean, 2026-09-30):
- oilpaint is a general oil-painting engine, specialised for oil paint but not for one style.
- "Realistic" means the output reads as a photograph of real oil paint on canvas, in whatever style the scene asks
  for.
- Storm Light is one scene, in an Impressionist (Monet-ward) style.

**Evidence:**
- `docs/research/`: a market and science study, and a study of Krita's engines (ideas only).
- 1:1 crops of Storm Light at 2400 px. The strokes read as wax or plastic: relief is uniform, and stroke ends are
  capsules.
- The relief light changes 12.6% of pixels by more than 20/255, with no cast shadows (*measured*).
- Every product and paper that moved to physically based relief lighting cites flat "embossed" impasto as the main
  failure.

The work falls into three layers:

1. **Engine: real oil paint, style-neutral.**
   - Relief lighting: shadows cast by the relief and occlusion (exact horizon sweeps on the height field,
     deterministic), plus sharper microfacet highlights (L7).
   - Surface state: gloss that follows the paint (fresh and thick is glossy, thin or sunk-in is matte), and an
     optional varnish layer (L7).
   - Canvas tooth in the deposit: paint lands where pressure and load beat the canvas texture plus existing relief,
     so a dry brush catches on ridges and the ground shows through (L7, with a tooth scale that resolves: real
     threads are 0.63-0.75 mm).
   - Smear transport: smudge drags colour and relief along the stroke instead of only flattening (L7; the knife in
     L12 builds on it).
   - Drying stages and glaze optics (L11). Glazes stay: old-master styles need them.
   - Brush shapes from pressure-recruited bristles, a load life cycle where a dry brush drags paint instead of
     fading, and stroke-end shapes (L10).
   - Wet-into-wet as a paint film (F0).
2. **Planner: mechanisms, not a style.**
   - Stroke variety in size, shape and direction (L3).
   - A photo as the target, via a new target op on a sampled rgb field (L3).
   - "Thick lights, thin darks" as an option (L3).
   - Pick-up rates per distance, not per path segment (L3, with its version bump).
   - A focus/detail map, a value-first pass, and control of lost and found edges (L9).
3. **Style presets: data, from L3.**
   - Named bundles of style, layer, surface and lighting settings. Working names: Impressionist, old-master glazing,
     alla prima realist, heavy impasto, knife. The final names are Sean's.
   - v1's defaults carry Storm Light's taste: for example `L_floor` 20, "Monet: no real darks".
   - L3 gives the engine neutral defaults and moves those values into the Impressionist preset. Storm Light is not
     reproduced: the planner port is a cutover (section 5).

**Measurement.** L5's gallery holds one scene per preset. A realism test set compares renders with photos of real
paintings in each style (open-access museum images). No published study provides one.

**Not adopted:**
- Neural refinement: it is hard to keep deterministic, and the code is restrictively licensed (FRIDA is GPL-3.0;
  Stylized Neural Painting is non-commercial).
- Krita's baked-in lighting and its spectral mixer: Ochrell is stronger.

| Order | L | Content | Days | Depends | Ends with |
|---|---|---|---|---|---|
| 1 | L0 | v1 tag; StrokeList v2 and ScenePlan v1 spec drafts (engine version, mixer ID, error codes); golden layout; cross-host determinism CI skeleton; `.gitattributes` | 1.5 | – | specs, CI skeleton runnable locally |
| 2 | L1 | Rust workspace: `oil-math` (pure-Rust maths, sRGB), `oil-mix` (Mixer trait, Ochrell default via `decode_linear`/`encode_linear`, rgb, Mixbox opt-in), kernel port (borrowed planes, counter RNG, chunked sums), image ops, lighting, StrokeList v2 codec with version gate, CLI, ctypes shim, first goldens | 4 | L0 | **done** (`docs/reports/L1.md`): P1 and P2 passed; G1 on 7 hosts; G3 0.03%; G4; goldens. Transport decided after L1: v1's, with Ochrell (engine `2.0.0-dev.2`) |
| 3 | L2 | ScenePlan v1 + JSON Schema + Rust spec compiler + TS DSL + validation | 4 | L1 | **done** (`docs/reports/L2.md`): Storm Light guide sheets from TS; compile 0.69 s native, 1.16 s Chromium, 1.39 s Node (v1 4.1 s); guides identical on 7 hosts |
| 4 | L3 | Rust planner port + composed `plan` + toolbox exports + incremental error planes; flows evaluated at stroke points; neutral defaults and style presets as data; stroke variety; photo target op; pick-up per distance; then the retirement commit | 6.5 | L2 | cutover acceptance (section 5): Storm Light and a second scene planned end to end, sanity gates, v1 metrics reported, Sean's side-by-side; identical StrokeList SHA on native, Node, Chromium and Firefox. **Measured** (`docs/reports/L3.md`): all gates pass; identical StrokeLists on 7 hosts; Storm Light plans in 7.6 s native, 15 s WASM (v1 22–25 s). Waiting for Sean's sign-off, then the retirement commit |
| 5 | F0 | **film spike** (throwaway prototype, measured): a two-layer paint film per pixel (top layer + body), mixing only at the interface, driven by pressure, drag, wetness and paint stiffness; Kubelka-Munk layer optics from Ochrell's K/S; colour computed when viewed, not per dab | 1 | L3 | go/no-go: 8-seed harness mix metrics, a Storm Light side-by-side against v1's transport, memory and speed. If go, L7 and L11 are rebuilt around the film |
| 6 | L7 | **the realism milestone:** paint that settles (height taper, wet displacement, fresh-paint levelling, spline outlines) straight in the kernel, no v1 flags; relief lighting (cast shadows, occlusion, microfacet highlights) as the new default light; surface state (gloss from paint state, optional varnish); canvas tooth in the deposit; smear transport | 7 | L3 | A/B/C strip; harness strata, facet and hairline acceptance (ENGINE_PLAN 3.2); before/after crops for each preset |
| 7 | L11 | wet/tacky/dry stages (dry brush over dried relief) and the optical KM glaze film (+16 B/px; the old-master glazing preset) | 2.5 | L7 | glow/haze before and after; glaze swatch acceptance |
| 8 | L4 | WASM host: Worker, Level-1 API, standalone and p5 adapters, frames, time-lapse; memory budget and size limits, measured | 4 | L11 | demo pages; peak-memory table in Chromium and Firefox |
| 9 | L5 | agent tooling: headless API/CLI JSON, `oil-metrics`, partial re-render, catalogue, guide view, AGENTS.md, gallery (one scene per style preset), cookbook; realism test set (renders against photos of real paintings per style); Python harness retired once `oil-metrics` matches it | 6 | L4 | agent runs the gallery from AGENTS.md alone; realism test set scored |
| 10 | L6 | mixers for release: Mixbox plug-in package, Ochrell-vs-Mixbox swatch sheet, harness mix metrics relative to the active mixer, licence packaging per Sean's choice (section 10) | 1.5 | L5 | swatch comparison; **public alpha** |
| 11 | L8 | memory and speed: tile store with lossless cold tiles (probe first); Ochrell pixel path (state traffic, lane-order SIMD128; the sRGB tables already landed in L1); threads native + WASM; lighting worker; tiles also lift wasm32's 2 GiB-per-allocation limit | 7 | L6 | memory and timing tables; G2 across threads and memory modes |
| 12 | L9 | planner fixes: cut-in edges, rock facets, halo glaze, tube palettes; focus/detail map, value-first pass, lost-and-found edge control | 5 | L3, L11 | tower, rock and halo crops; a realist-preset scene with and without the focus map |
| 13 | L10 | brush memory, flat/filbert/round from pressure-recruited bristles, twist and speed; load life cycle (a dry brush drags paint); stroke-end shapes | 4 | L1 | brush demo |
| 14 | L12 | palette knife (on L7's smear transport) | 3 | L7 | knife demo |
| 15 | L13 | 1.0: docs site, CI hardening, release | 2 | all | published package |

Total **≈ 59 agent-days** (v3: 48; 49.5 before the realism roadmap). The public alpha lands after L6, at about
38 days. It includes the settling-paint, relief-lighting, surface and glaze work, and the style presets. v3 placed
the physics after its alpha, at 26 days.
- **Cheaper (−3.5):**
  - L0: no strict-C freeze or C-derived golden corpus (−0.5);
  - L6: no 3-day calibration, and Ochrell is already integrated (−1.5);
  - L7, L10, L11: no v1-default flags or v1 digests (−1.5).
- **More expensive (+5):**
  - L1: the Mixer trait for an 85-float state, pure sRGB/pow with `encode_linear`, the version gate (+1);
  - F0: the film spike, added after L1's transport decision (+1);
  - L8: tiled lossless storage and the Ochrell pixel path (+3);
  - the realism roadmap after L2 (+9.5): L3 +1.5, L7 +4.5, L9 +2, L10 +0.5, L5 +1.

**Risks:**
- **Ochrell cost:** about 1.5x Mixbox's paint time in the engine (L1 final kernel, *measured*; 4x through the Python
  bridge) and 6.6x its memory. It is handled by L8 and the browser limits. A speed-parity claim with Mixbox would be unsupported.
- **Ochrell development continues** (its optimisation rounds). It is pinned by commit (`ffd6ee9` since L1), its
  model version goes into the engine version, and goldens catch drift.
- **Tile compression ratio unknown.** It is measured before building; if it is poor, the browser limit stays.
- **No parity gate for the planner** (a cutover). Regressions are caught by the sanity gates, the harness metrics
  reported against v1, and the side-by-side.
- The threaded WASM toolchain does not affect the single-thread default.
- `oil-math` costs about 10% (*measured* for detmath on the VM).
- **Patents** (`docs/research/`):
  - Adobe's US 8,462,173 and US 8,599,213 are active until 2031.
  - They claim a brush that deposits from a reservoir buffer and a pick-up buffer. That resembles our lane load plus
    "dirt", inherited from v1.
  - Only automated summaries of the claims have been read, and older research (Baxter's dAb) may be prior art.
  - A professional review is needed before the public alpha (L6); nothing is at risk while the code stays private.
- Agent JSON drifting from the harness is prevented by a shared metric crate.

## 12. Decisions and open questions

**Decided (Sean, 2026-09-30):**
1. **Hard break.** Old stroke files are not repainted; tag `v1-python-c` reproduces Storm Light.
2. **Ochrell is the default mixer.** Mixbox is an opt-in plug-in, and v3's calibration step is dropped.
3. **Scene freedom:** declarative primitives plus pure-function hooks, with hooks marked non-portable.
4. **Threads are optional.** Single-thread works everywhere; the threaded build needs `crossOriginIsolated`.
5. **npm name `oilpaint`.**
6. **The browser size limit follows the memory finding** (section 7).
7. **Ochrell data licence: option A** (section 10). Ship as is, `(MIT AND CC-BY-SA-4.0)`, with a clear licence
   and notices.
8. **The engine waits for the Ochrell optimisation agent to finish** before it uses Ochrell.
   - L1 builds everything else first: the kernel, the mixer trait with rgb and Mixbox, maths, codec, lighting,
     CLI, shim and port check P1.
   - L1 then plugs Ochrell in (P2) against a clean, pinned Ochrell commit, together with the one additive API
     (`encode_linear`).
9. **Paint transport: v1's, with Ochrell** (after L1). A deposit moves the pixel toward the brush paint by alpha,
   and mixing comes from the brush's dirt pick-up. The material transport washed out Storm Light's broken colour
   (`docs/reports/L1/storm_transport_choice.png`). Engine `2.0.0-dev.2`.
10. **A film spike (F0) runs before L7.** It tests a layered paint film with interface mixing and optical layering
    as the paradigm for L7/L11 (section 11).
11. **L3 evaluates authored flows at stroke points** instead of storing whole-canvas rasters (section 8).
12. **A general oil-painting engine, not one style** (after L2). The engine is style-neutral real oil paint; styles
    are presets; Storm Light uses the Impressionist preset. The realism roadmap is in section 11.
13. **The planner port is a cutover** (after L2). There is no parity gate with v1's planner (level E is dropped), and
    Storm Light is not reproduced. The L3 acceptance is in section 5.

**Open for Sean:**
1. **Visual sign-offs:** the L3 side-by-side (new planner against v1), and the sky fluidity setting (L7, three
   settings shown).
2. **Carried from ENGINE_PLAN** (decide by L8): halve the block-in strokes (a 44% kernel-time layer), and
   1200-px layer images by default.
3. **Patent review before the public alpha (L6):** US 8,462,173 and US 8,599,213 (section 11, Risks).
4. **Preset names** (working names in section 11).

**2026-10-01 brush review iteration 2:** Engine dev.4 and author/catalog 2 implement the requested contact, scumble, directional-pickup and lighting refinements. Earlier gallery evidence is retained. See [iteration report](../reports/BRUSH_ITERATION_2.md); no extra presets or broader roadmap milestones are claimed.

**2026-10-02 brush review iteration 3:** dev.5 refines clustered scumble, local relief, pressure contact and lane grouping, and tests form-following placement. Seven presets retained; author/catalog schema 2 unchanged. See [report](../reports/BRUSH_ITERATION_3.md).

**2026-10-02 isolated scumble refinement:** dev.6 changes only scumble contact; six retained brushes are regression-locked to dev.5. No catalogue retuning or other renderer work. See [focused report](../reports/SCUMBLE_REFINEMENT.md).
