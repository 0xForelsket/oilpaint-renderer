# Brush priorities — implementation and review

2026-10-01. Scoped delivery: catalog, stable groups/local edits, interactive preview. No changes to the study scripts, paintings or saved painting files; no publishing or push.

## Delivered

- Seven shared candidate profiles in the Rust catalog, served to TypeScript through WASM. Width, pressure and depletion behavior are compiled in Rust. Definitions are embedded in versioned source documents.
- Named groups with permanent IDs, permanent stroke IDs, independent random scopes, exact path/color/preset edits, visibility and order. Groups compile to existing layers. Stored authored values outside an edited group remain unchanged.
- Full-plane, bounded checkpoint replay. Earlier edits invalidate dependent suffixes; subsequent pickup and drying are replayed. Native tests compare every plane, including blurred height, with full replay in both mixers.
- A real-renderer worker preview with pointer input, repeatable samples, wet underpaint, all requested material and lighting controls, fixed reference comparison, imports/exports, reset and measured status.

[Launch and API](../BRUSH_AUTHORING.md) · [review gallery](brush-review/index.html) · [preview screenshot](brush-review/preview.png)

## Verification

| Check | Result |
|---|---|
| `cargo clippy --workspace --release --all-targets -- -D warnings` | Passed |
| `cargo test --workspace --release` | Passed; subsequent author tests also pass (7 author tests) |
| `npm run build:wasm` | Passed |
| `npm run typecheck` / `npm run test:ts` | Passed; 16 TypeScript tests |
| `npm run xhost` | Passed: 33 cases on native Windows, Node, Chromium, Firefox, WebKit; all regression hashes unchanged |
| `npm run review:brushes` | Passed: 14 candidate/size cases on native, Node, Chromium and Firefox; compiled bytes, five paint planes and three image views agree |
| `npm run test:preview` | Passed: 46 timed interaction checks plus pointer, preservation, invalid-input, persistence, narrow-layout checks and worker-image/full-replay comparison |

The gallery has seven presets at 256 and 512 px, three widths per image (0.008, 0.028, 0.065 cw, clamped for fine detail), and straight/curve/dab samples crossing wet blue underpaint. Native and browser reference images are checked in for lit, unlit and fixed-scale height views. [Hashes and gallery timings](brush-review/measurements.json) record every case. Gallery timings are single unpinned observations, not controlled benchmarks.

Additional calibration assertions confirm that loaded paint deposits substantially more coverage than dry drag, impasto adds more height than loaded flat, and wet-mixing color changes when the substrate is dried. Preset validation tests include unsupported fields/controls, nonfinite values, duplicate identities, serialization and version gates. Replay tests cover first/last edits, order, visibility, drying, deletion, geometry, preset, ground and resolution changes.

## Measured interactive latency

Core Ultra 7 258V laptop, Chromium 145, 10 warmed repetitions per operation and size, one loaded-flat curved sample, Ochrell. Browser processes inherited PowerShell processor affinity `0xF` (performance cores). Input-to-display includes 75 ms material debounce or 16 ms lighting debounce, worker transfer and canvas upload. These are small-brush-preview measurements, not large-painting estimates.

| Operation | Worker paint median | View median | Input → canvas median | Input → canvas range |
|---|---:|---:|---:|---:|
| 256 px, change load | 4.6 ms | 9.2 ms | 96.5 ms | 91.1–104.9 ms |
| 256 px, relight | 0 ms | 8.4 ms | 25.8 ms | 24.8–26.5 ms |
| 384 px, change load | 8.3 ms | 18.7 ms | 103.1 ms | 102.2–114.2 ms |
| 384 px, relight | 0 ms | 19.5 ms | 43.5 ms | 36.2–47.3 ms |

[Raw repeated latency](brush-review/latency.json) and [broader UI run](brush-review/ui-measurements.json). Reproduce with the preview server running:

```powershell
(Get-Process -Id $PID).ProcessorAffinity = 15
node packages/oilpaint/preview/benchmark.mjs
```

The initial unpinned 384 px cold smoke measurement was 147 ms paint, 61 ms view, 312 ms input-to-display. Warmup and CPU scheduling matter. Pointer strokes stream through a coalescing queue, but a running WASM call cannot be interrupted, and the preview does not promise 60 fps on large documents.

## Visual observations and remaining limits

The rendered gallery shows separated lanes and depleted tails in Dry drag, narrower landings/tails in Rounded dab, relief-sensitive broken Scumble, raised Impasto accents, and blue pickup along Wet mixing crossings. Lit/unlit/height references use exactly the same paint planes; no screen-space paint substitute is used.

The underlying primitive still yields some flat/faceted landings and ribbon-like long strokes. At 256 px, narrow dry tracks may collapse to a few pixels; minimum fine-detail widths can be subpixel. The gallery makes this visible rather than claiming width/resolution invariance. Relief remains the existing two-scale model: high height and bump can look embossed. These observations motivate further artist tuning, not an implementation of the future brush/film/knife roadmap.

All seven names remain **candidate presets**, with no artist sign-off inferred from tests. Physical filbert/fan/knife behavior is not implemented. The browser exposes Ochrell and the RGB baseline; the existing optional Mixbox build is not part of this preview. Existing binary painting files replay unchanged with their matching engine but are not converted into editable author documents. Keep the new JSON source alongside flattened StrokeList exports.
