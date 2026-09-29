# LIBRARY_PLAN: oilpaint as a p5.brush-style library (Rust engine, JS planner)

`SP=/tmp/claude-0/-home-claude/cd4531f6-3314-512b-9953-4f74d55c5a55/scratchpad`. The previous engine plan is
`SP/plans/ENGINE_PLAN_v1.md`; its physics items are folded in below. Spikes: `SP/work_lib/` (Rust crate `oilcore/`,
planner spike `planspike/`, mixers `mixers/`). Every number is in `SP/work_lib/SPIKE_LOG.txt`, and the images are in
`SP/work_lib/evidence/`. The repo and `work_eval` were not changed. Oil only; watercolour, pencil and WebGPU come later.

## 1. What the library looks like

**Recommendation:** a TypeScript library with a Rust/WASM engine: kernel, canvas planes, lighting, mixing and every
full-image pixel operation, including the planner's. Scheduling, geometry and per-stroke planner logic stay in JS.
Measured reason: per-stroke logic in JS is 40-130x faster than today's Python, while full-image passes in plain JS are
no faster than numpy (section 4).

There are two API levels and two hosts. It works like p5.brush: set state, then draw. The standalone build is the
default, and the p5 build is an add-on.

**Level 1, immediate painting** (units: canvas pixels by default, `oil.units("cw")` for canvas widths):
```ts
import * as oil from "oilpaint";                        // standalone: HTMLCanvas / OffscreenCanvas, no p5
await oil.init({ canvas, width: 1200, height: 1500, ground: "#e9e1d6", seed: 1907 });
oil.set("flat", ["cobalt_blue", "lead_white", 0.4], 24);  // brush preset, colour (hex | tube | mix), width
oil.paint({ pickup: 0.12, load: 1, ridge: 0.5, marble: 0.3 });   // wet-paint behaviour, sticky like p5 state
oil.field("sea", waves({ angle: 0, amplitude: 9, wavelength: 0.18 }));
oil.flowLine(200, 900, 300);                            // follows the active field
oil.beginStroke(100, 400); oil.move(-10, 120, 0.8); oil.endStroke(-5, 0.3);   // p5.brush-style gesture
oil.fill(polygon, { brush: "filbert", spacing: 1.6, angle: "field" });       // a region filled with strokes
oil.mode("scumble"); oil.line(80, 300, 700, 280);
oil.dry(0.3);                                           // the layer dries before wet-over-dry work
await oil.render({ light: "painting" });                // flush the stroke queue (worker), relight, blit
```
**Level 2, the auto-painter (unique to us):**
```ts
import { plan, paint } from "oilpaint";
import stormLight from "./scenes/storm-light.js";        // export default function build(S: Scene) {...}
const strokes = await plan(stormLight, { planWidth: 600, seed: 1907, onProgress });  // StrokeList v2 (cw units)
const job = paint(strokes, { canvas, width: 2400, light: "painting" });
for await (const f of job.frames({ every: 100 })) status.textContent = `${f.layerName} ${f.progress}`;
await job.done; const png = await job.toBlob(); const mp4 = await job.timelapse({ size: [1200, 1500], fps: 24 });
```
Storm Light in TS is a mechanical port: the Python DSL becomes the same calls with option objects.
```ts
export default function build(S: Scene) {
  S.canvas({ aspect: [4, 5], ground: "#e9e1d6" });
  S.target.fill(gradientV([[0, "#232a58"], [0.24, "#3a3b78"], [0.40, "#66679e"], [0.51, "#b4a6c6"], [HZ, "#e8cbb8"]]));
  S.target.blob(0.22, HZ - 0.03, 0.40, 0.09, "#f4bd90", { softness: 0.7, strength: 0.85, seed: 31 });
  S.region("sky", above(HZ), { edge: 0.03 });
  S.flow("sky", sweep({ angle: -14, curl: 0.5, noise: 0.3, scale: 0.5 }));
  S.style("sky", { colors: [...SKY_MID, ...SKY_DARK.slice(0, 2)], width: [0.025, 0.04], length: [0.10, 0.26], marble: 0.4 });
  S.layers([layer("Sky long strokes", { regions: ["sky", "glow"], placement: "error", T: 12, fs: 0.5, fg: 1.2 }) /* ... */]);
}
```
Core types (abridged):
```ts
type Color = string | [string, string, number] | [number, number, number];
type Mode = "paint" | "scumble" | "smudge" | "glaze" | "knife";
interface Brush { shape: "flat" | "filbert" | "round"; width: number; opacity?: number; pickup?: number; load?: number;
  ridge?: number; levee?: number; stiff?: number; marble?: number; flow?: number; mode?: Mode; /* ...BrushParams */ }
interface StrokeList { version: 2; engine: string; aspect: number; strokes: Float32Array /* packed */; layers: LayerRange[] }
interface PaintJob { frames(o?: { every?: number }): AsyncIterable<Frame>; done: Promise<void>; toBlob(): Promise<Blob> }
```
- **p5 add-on:** `import "oilpaint/p5"` gives `oil.instance(p)` (p5 2.x instance mode, like p5.brush). It draws into the
  p5 canvas through `drawingContext` (2D `putImageData` of dirty rects, or a texture in WEBGL); p5 is a peer dependency
  and is not bundled.
- **Progress:** painting runs in a Web Worker, so the page never blocks. The worker posts frames every N strokes, and
  the time-lapse is WebCodecs + mp4-muxer in browsers, ffmpeg in Node.
- **Scenes:** the three Python scenes are ported by hand (storm_v3.py is ~250 lines of DSL calls) into ES modules
  exporting `build(S)`; a JSON scene spec comes later for tools.

## 2. Rust core

```
oilcore/ (cargo workspace, MIT)
  oil-kernel   canvas planes, bristle-lane brush (4 modes now), stroke file v2 codec, counter RNG, detmath, Mixer trait
  oil-mix      km (default, open), rgb; mixbox in a separate crate/package (CC BY-NC)
  oil-image    blur, resize, Sobel, distance transform, structure tensor, polygon fill, value noise, contours
  oil-light    relight, weave;  oil-plan-ops: incremental error plane, guide rasterisation
  oil-wasm     C-ABI exports + small hand-written TS glue (the spike needed no wasm-bindgen), worker protocol
  oil-native   rayon threads, CLI (plan/replay/relight), ctypes shim so the Python eval harness drives Rust
```
**Spike (SP/work_lib/oilcore, ~1,500 lines).** It ports all of `brush.c` (4 modes, lanes, height, pick-up), the
Mixbox polynomial, `light.relight` and the OpenCV-equivalent image ops. It builds native and to `wasm32-unknown-unknown`
(71-75 KB).
- Kernel parity on the eval swatch sheet (210 strokes) and on all 12,495 Storm Light strokes at 600 and 1200 px: Rust
  with the platform libm is **bit-identical to C built strict** (`-ffp-contract=off`, no fast-math) on every plane.
  Today's fast-math C build differs from strict C by up to 124/255 on 9.9% of pixels. That is the ns/RNG amplification
  found earlier, so the current binary itself is not a stable reference.
- With `detmath` (own exp/sin from + − × ÷ only), native, Node and Chromium (scalar and SIMD128) produce **identical
  SHA-256** at 600, 1200 and 2400 px. Against strict C the gap is h ≤ 2e-6 and rgb 0.000/255.
- Relight in Rust matches Python within 0.002/255 and is 2-6x faster (`evidence/relight_python_vs_rust.png`).

| Storm Light kernel, single thread | 600x750 | 1200x1500 | 2400x3000 |
|---|---|---|---|
| C fast-math (today) | 2.7 s | 9.9 s | ~36 s |
| Rust native (libm / detmath) | 3.1-3.8 / 4.0 s | 11.9 / 13.3 s | – / ~50 s |
| WASM in Node (detmath+SIMD) | 5.0 s | 15.7 s | 57 s, 647 MB heap |
| WASM in headless Chromium 141 | 4.2-4.8 s | 16.2 s | 60 s, 561 MB heap |

So WASM runs at 1.2-1.35x native Rust time and ~1.6x today's C. `detmath` costs ~10%, and SIMD128 auto-vectorisation
gains ≤8%.

**Threads.** Native uses rayon for fixed 16-row chunks inside each segment, plus a stroke dependency scheduler.
Earlier C measurement: 1.3-1.6x on big strokes with 2 cores; the painterly order caps stroke-level parallelism at
~1.5x. In the browser, the default is single-thread WASM inside a Worker, which works on any page. A threaded build
(atomics; stable now offers `wasm32-wasip1-threads`, and nightly `build-std` also installed here) loads only when
`crossOriginIsolated` (COOP/COEP headers; the spike page reached `crossOriginIsolated = true`). Fixed chunks plus int64
pick-up sums make threaded and single-thread results bit-identical.

**Memory.** Planes cost 56 B/px (403 MB at 2400x3000; ~580 MB with the settle/glaze planes). Chrome and Firefox allow 4 GB of wasm32 memory, so 2400x3000 fits on desktop.
iOS Safari tabs die around 1-1.5 GB. Exact tiling is impossible, because a stroke's pick-up reads its whole segment
across tile borders. The fallback is a "compact" mode (f16 latent, RGBA8 display, ~34 B/px, deterministic but not
identical to full mode), with size capped from `navigator.deviceMemory`. Prints above ~4800 px use the native CLI;
wasm64 comes later.

**M0, the contract both implementations satisfy (now L0).** Freeze `brush.c` as the v1 reference, built strict. The
golden corpus holds the swatch sheet at 400/1600, Storm Light strokes at 600/1200, and per-mode unit strokes: stroke
files + SHA-256 per plane + PNGs. Stroke file v2 (binary: header, packed points x,y,w,p[,angle,speed], per-stroke
params, layer table, engine version) carries a counter-based RNG (`hash(seed, lane, sample)`), stored arc length
(fixes the ns flip) and int64 reductions. Parity levels:

| Level | What must match | Status |
|---|---|---|
| A: Rust v2, all targets | bit-identical planes (native, Node, Chromium; Firefox/Safari in CI) | shown for native/Node/Chromium (v1 RNG + detmath) |
| B: Rust legacy mode vs frozen C strict (v1 files, Linux) | bit-identical | shown |
| C: Rust portable vs C reference | rgb ≤ 1/255, h ≤ 1e-4 on the corpus | shown (h 2e-6) |
| D: size consistency (v2) | 1800 vs 2400 at 600 px: ≤ 1% of pixels > 8/255 | today 10.3%; v2 RNG expected to fix (not yet shown) |
| E: JS planner vs Python | eval-harness metrics + visual (section 3) | to do |

## 3. Planner in JS

| Python call | Used for | Replacement |
|---|---|---|
| `cv2.GaussianBlur`, `light.blur` (resize INTER_AREA/LINEAR) | soft masks, references, edges | Rust `oil-image` (ported in spike, matches cv2 within 0.002/255) |
| `cv2.resize` NEAREST/AREA | region ids, error cells | Rust |
| `cv2.remap` INTER_CUBIC + `np.random.default_rng` lattice | value noise (`noise.fbm`) | Rust bicubic noise on a hashed lattice (deterministic; differs from numpy) |
| `cv2.distanceTransform` + `Sobel` | `contour` flow | Rust exact EDT (Felzenszwalb) + Sobel |
| `skimage.feature.structure_tensor` | fallback flow | Rust (Sobel + Gaussian) |
| `cv2.fillPoly`, `dilate`/`erode` | polygon masks, `band_around` | Rust scanline fill, min/max filter |
| `cv2.findContours` | `curve="boundary"` | TS marching squares on the mask (or analytic polygon edges) |
| `scipy.spatial.cKDTree` | palette snap | TS brute force over ≤3k Lab candidates (measured 3 µs/query) |
| `np.random` uniform/normal/integers/shuffle | all placement | TS PCG32 + Box-Muller on detmath |
| Lab, Mixbox LUT, `np.interp/cumsum` | colour, paths | TS (Rust for full images) |

**Spike (600 px, real Storm Light inputs: sky mask/flow, real proxy canvas, 729-colour palette, 13,795 paths from the
real stroke list):**

| | error map ×27 | palette snap ×12,500 | path tracing ×13,795 |
|---|---|---|---|
| Python (repo) | 1.52 s | 2.72 s | 1.30 s |
| JS, naive / sRGB lookup table | 5.3 / 1.5 s | 0.04 s | 0.03 s |
| Rust → WASM (Node) / native | 1.3 / 0.9 s | 0.02 / 0.015 s | 0.015-0.026 s |

- JS and Rust find the same 24,454 error cells. The proxy canvas lives in WASM, so the error plane should be maintained
  there incrementally (dirty rect per stroke) rather than recomputed.
- **Projected full plan in the browser**, single thread at 600 px: 6-8 s. That is the proxy painting 4.2-5.0 s
  (measured) + error maps 0.2-1.3 s + JS per-stroke work ≈0.3 s + guides 0.5-1 s (estimate), against Python's 32 s
  today (12 s with the v1 prototype fixes). The preview paints live while it plans.
- **Determinism:** same scene + seed + planner version gives identical strokes on every JS engine. This requires that
  the planner never calls `Math.sin/cos/exp/log/pow/cbrt/atan2` directly (they can differ across engines; a small
  detmath module wraps them), uses its own PCG32, and runs image ops in WASM. CI checks Node against Chromium
  (+ Firefox).
- **Acceptance** (JS will not reproduce Python's stroke list):
  - `eval run --scene` on Storm Light at 600 and 1600, every region metric within `eval/thresholds.json` of the
    Python-planned baseline (`hairline`, `bristle_L`, `edge_step_p99`, `C_mean`, `L_p50`, `coh_local`, `orient`, `acf_*`).
  - Per-layer stroke count within ±10%, width/length medians within ±5%, angle-histogram distance ≤0.1 (new
    `strokes.*` metrics).
  - Sean signs off a side-by-side. The harness gains a `--strokes` input so it can score JS-planned stroke files.

## 4. Licence and mixers

The library is MIT. Mixbox (CC BY-NC) cannot be the default. The `Mixer` trait is: `encode(rgb) → latent`, linear
`lerp`, `decode(latent) → rgb`, latent width (7-16 floats), and optional glaze optics. Tested in Python and in the Rust
kernel (`mixers/`, feature `mixer_km12`):
- **Kubelka-Munk on spectral.js 3.0 data (MIT)**, 12 bands (≈ the 38-band result). Latent = (Y·KS₁..₁₂, Y) + RGB
  residual, 16 floats; unmixed colours round-trip exactly. Cobalt + cadmium yellow → vivid green (L57 C63 h139;
  Mixbox L54 C60 h142), and tints stay clean. Differences: ultramarine + yellow goes olive (h89 vs Mixbox's green h137),
  orange/blue blends go browner, and a pale scumble over blue turns lavender. On the harness sheet: paint chroma
  45 → 36, mud-stack mud fraction 0.04 → 0.26, and 19 fails, most of them "off the Mixbox curve" by definition
  (`evidence/sheet_mixbox_vs_km12.png`, `mix_ramps.png`). The Storm Light preview is visually close: chroma 24 vs 25,
  mud 0.01 vs 0 (`storm_mixbox_vs_km12.png`).
- **libmypaint WGM (ISC)**, 10 bands: dull, dark greens (C29), and ultramarine + yellow goes grey (C5). Rejected.

**Recommendation:** the default is our own KM mixer (MIT, spectral.js attribution), followed by a calibration step: a
per-tube spectral reflectance and tinting strength for the ~20 named tubes, tuned so ultramarine + yellow is green and
glazes darken; about 3 agent-days. Mixbox ships as an optional `@oilpaint/mixbox` package (CC BY-NC notice, opt-in).
Calibrate against real paint charts, not Mixbox outputs (question 3).

**Licences of everything used:**
- Runtime and our code: Rust std only (MIT/Apache), our code MIT, spectral.js data (MIT), mp4-muxer (MIT), optional
  rayon (MIT/Apache).
- p5.js (LGPL-2.1) is a peer dependency only, never bundled. Optional Mixbox is CC BY-NC.
- Dev: TypeScript, Playwright/puppeteer-core (Apache-2.0), vitest/esbuild (MIT). Python reference: numpy/scipy/
  scikit-image/imageio (BSD), opencv (Apache-2.0), pymixbox (CC BY-NC). No GPL.

## 5. Packaging

- npm workspace:
  - `oilpaint`: the API plus the standalone and p5 adapters.
  - `@oilpaint/core`: WASM and glue.
  - `@oilpaint/planner`: the planner and scene DSL.
  - `@oilpaint/mixbox`: the optional Mixbox plug-in.
  - `@oilpaint/cli`: Node plan, replay and time-lapse.
- ESM first, plus an IIFE global build for `<script>` users (as p5.brush does).
- WASM loads via `instantiateStreaming(fetch(new URL("oilcore.wasm", import.meta.url)))`. A single-file CDN build
  inlines it as base64 (~100 KB).
- jsDelivr/unpkg serve `application/wasm`; workers use `new URL(..., import.meta.url)` with a blob fallback on CDNs.
  `.d.ts` files come from the TS source.
- Docs site: an API reference plus a gallery where every example paints live, including Storm Light.
- Versioning is semver. Stroke files carry the engine version, and a minor release may not change any golden digest.
- Tests: `cargo test` (units + golden digests); Node vitest (API, planner determinism); Playwright Chromium and Firefox
  (golden SHA-256, perf smoke, COOP/COEP threaded build).
- CI: GitHub Actions (linux/mac/windows native, Node, browsers), plus a nightly Python eval harness driving the Rust
  core through ctypes.
- The Python code stays the reference and the golden-corpus generator. The C kernel is frozen at L0 and retired after
  L1. The Python planner is retired once the JS planner passes level E. The eval harness stays as the Python measuring
  stick, driving Rust through ctypes.

## 6. Milestones

**Physics waits for the port.** The kernel transliteration took about one agent-hour and was bit-identical on the
first run. Rust is as fast as strict C, and the only faster C (fast-math) is not reproducible. Doing settle, brush
memory and glazes in C first would double the implementation and parity work. So: freeze C at L0, port at L1, and do
all new physics in Rust. The eval harness measures both along the way.

| L | Content | Days | Ends with |
|---|---|---|---|
| L0 | contract: frozen strict C, golden corpus, stroke file v2 spec, parity CI skeleton | 2 | parity table on the corpus |
| L1 | Rust core: kernel (legacy v1 + v2 RNG), lighting, image ops, KM + Mixbox mixers, CLI, ctypes shim | 3 | harness report Rust vs C; Storm Light replayed by Rust |
| L2 | WASM + worker runtime + Level-1 API + standalone/p5 adapters | 4 | demo page: paint with brushes and fields in the browser |
| L3 | TS planner + scene DSL + Storm Light in JS + progress/time-lapse | 6 | Storm Light planned and painted live in a browser; level E passed |
| L4 | calibrated open KM mixer (default), `@oilpaint/mixbox` plug-in | 3 | swatch sheet: KM vs Mixbox vs real-paint charts; public alpha (MIT-clean) |
| L5 | paint that settles (taper, wet displacement, fresh-paint levelling, spline outlines), default light restored | 3 | A/B/C strip on Storm Light (v1 `evidence/BEFORE_AFTER_settle_900.png`) |
| L6 | speed: threads native + WASM (COOP/COEP), incremental error plane, per-pixel work, lighting worker | 4 | timing table native/Node/Chromium, 1 vs N threads identical |
| L7 | planner fixes: edge cut-in, rock facets, halo glaze, tube palettes | 3 | tower base, rock, halo crops |
| L8 | brush memory, flat/filbert/round, twist and speed inputs | 4 | brush demo page; sky/sea crops |
| L9 | wet/dry stages, optical KM glazes | 3 | glow/haze before/after |
| L10 | palette knife | 3 | knife demo page; rock/foam crops |
| L11 | docs, gallery, CI hardening, 1.0 | 3 | published package + gallery |

Total **≈41 agent-days**: ~21 on the library itself (L0-L4, L11) and ~20 on physics and speed.

**Browser expectations for Storm Light.** Single-thread, measured on this 2.1 GHz VM:
- Plan: 6-8 s (projected).
- Paint: 1200x1500 in ~16 s, 2400x3000 in ~60 s (measured).
- Lighting: 0.4 s at 1200 px, 1.6 s at 2400 px.

With threads (projected from the C measurement): ~1.3-1.6x on 2 cores and perhaps 2-3x on 8 cores for big strokes, so
1200x1500 in ~7-12 s and 2400x3000 in ~25-45 s. L6's per-pixel work targets another 1.3x, and typical laptop cores are
faster than this VM.

**Risks:** threaded WASM toolchain (single-thread worker stays default); iOS memory (compact mode, size caps); open
mixer look (L4 calibration, Mixbox plug-in); numpy behaviours the JS planner misses (level E catches them); `detmath`
cost ~10% (f32 polynomials); API scope creep (freeze the Level-1 API at L2).

## 7. Questions for Sean (my default in brackets)

1. **Old paintings:** must old stroke files keep repainting pixel-identical? [Yes, through the Rust legacy mode; it is
   already bit-identical to strict C. The current fast-math binary itself is not a stable reference.]
2. **Default colours:** is an open mixer that paints ultramarine+yellow olive until calibrated acceptable for the first
   public alpha? [Yes. Ship KM as the default, calibrate in L4, and offer Mixbox as an opt-in non-commercial plug-in.]
3. **Calibration data:** may we tune the open mixer against real paint charts (photographed swatches of real tubes)
   rather than Mixbox's output? [Yes. It keeps us clear of Mixbox's licence.]
4. **Browser size:** what is the largest size the browser must reach? [2400x3000 on desktop; larger through the Node
   CLI; phones capped by memory.]
5. **Threads:** is it acceptable that the fast threaded mode only works on pages that send two security headers?
   [Yes. Single-thread works everywhere.]
6. **Name and API style:** npm name `oilpaint` and a p5.brush-like global `oil.*` API with a standalone default?
   [Yes, if the name is free; otherwise `p5.oil`.]
7. **Order:** library first (L0-L4), then physics (L5-L10)? [Yes. Physics is written once, in Rust.]
