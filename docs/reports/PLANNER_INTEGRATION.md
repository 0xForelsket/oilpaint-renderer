# Retained brushes in the planner — 2026-10-02

The four authorized follow-up priorities are implemented: freeze all seven accepted brushes, connect them to region/pass planning, improve placement and edge handling, and demonstrate an automatically planned still life with precise local edits.

[Compact review gallery](planner-integration/index.html) · [One PNG for review](planner-integration/review.png) · [API and exact launch commands](../PLANNER_AUTHORING.md)

## What changed

`oil-brush` now owns the one shared catalog and contact compiler. The catalog moved without changing its bytes, and all seven presets' RGB/Ochrell paint-plane hashes match the approved dev.6 baseline. The brush kernel is unchanged. Acceptance is recorded in `spec/brush-acceptance.json`; imported modified presets remain candidates.

The planner accepts a brush per region or pass, embeds catalog definitions in scene source, and applies calibrated contact/pressure/loading profiles. Named brushes use finer flow-following paths, coherent wobble, in-region color sampling and mask-supported edge contacts. Scene controls permit quieter joins, thin shadow deposits and sparse thick highlights. These are placement/recipe controls, not new paint physics.

Every planned pass/region pair exports a stable named group, including empty groups. `oil-composition` format 1 stores exact resolved points, brush parameters and seeds; it does not apply profiles twice. Named RNG scopes replace positional indices. `editRegion` and `replaceRegion` preserve every unselected group. Full downstream replay preserves correct wet pickup and deposition. Pass blur/drying boundaries are retained.

The browser composition workbench runs planning/painting in a worker, with 40 ms coalesced repaint scheduling and separate view updates. It supports source planning, selection, tint, visibility, translation, selected-region brush replanning, reference capture, save/open, StrokeList export and lit/unlit/height views at 384/768 px. BrushLab retains the full brush/material/lighting controls and pointer drawing.

## Demonstration and visual review

`scenes/catalog_still_life.ts` authors a synthetic target, masks, flows and passes; all **804 strokes in 13 groups** are planned automatically. There are no manual path corrections or composited paint effects. The target is a generated guide, not a photo. Saved output includes normal-size 384 px and larger 768 px renders.

Soft-mask coverage (proxy cover > .05): leaf **96.3%**, orange **95.4%**, stone **96.6%**. The deliberately sparse/translucent shadow region reaches **58.4%**. Coverage is a technical measure, not a quality score.

The leaf-only recolor and candidate replan preserve all other authored groups exactly. The recolor also preserves orange and stone image crops bit-for-bit. These pixel checks use separated subjects; overlap and wet transport may legitimately change downstream pixels even when their authored strokes remain unchanged.

The image is still stylized. Shadows are too weak, some stroke joins remain conspicuous, and contours can be angular. The demonstration makes those planning problems visible without redesigning the accepted brushes. It does not establish finished-painting realism or close L3's broader visual sign-off.

## Measured performance

Single local runs on Windows x64, Node 24 and installed Playwright browsers; warm-up and host load can affect timings. These are measurements, not latency guarantees.

| Operation | Measured time |
|---|---:|
| 256 px planning, native | 160 ms |
| 256 px planning, Node/WASM | 346 ms |
| 256 px planning, Chromium/WASM | 319 ms |
| 256 px planning, Firefox/WASM | 1,904 ms |
| Chromium composition tint, 384 px: paint / view / update | 336 / 25 / 457 ms |
| Other 384 px local edits: paint / complete update | 294–390 / 415–502 ms |
| Selected-region candidate plan + paint + display | 781 ms |
| Lit view only: paint / view / update | 0 / 26 / 27 ms |
| Unlit view only: paint / view / update | 0 / 1.4 / 2.8 ms |

Update timing runs from scheduling the operation (including validation/debounce/worker transport where applicable) through canvas update. It excludes pre-scheduling edit construction and the browser's next physical display refresh. Raw [planning/parity measurements](planner-integration/verification.json) and [UI measurements](planner-integration/ui-performance.json) are saved. UI entries are tint, hide, show, translate, replan, unlit, lit in order. Large compositions are visibly slower than single-stroke previews; the worker keeps computation off the UI thread but does not make editing 60 fps.

## Verification

- `cargo clippy --workspace --release --all-targets -- -D warnings` and `cargo test --workspace --release`: pass.
- `npm run build:wasm`, `npm run typecheck`, `npm run test:ts`: pass (17 TS tests).
- `npm run test:preview`: all 57 BrushLab checks pass.
- `npm run test:composition-ui`: selection, local edit preservation, visibility, translation, brush replan, export, relight and narrow viewport pass.
- `npm run verify:still-life`: native/Node/Chromium/Firefox plan bytes and resolved documents agree; all three baseline/edit/replan render variants agree in five paint planes and lit image. Incremental/full replay and nonzero checkpoint reuse pass.
- Full cross-runtime harness: **33 cases on five hosts**, with all required identical cases agreeing and two expected libm negative controls. [Report](planner-integration/xhost.md); new `golden/2.0.0-dev.7.json`. No fresh Linux/macOS run is claimed.
- Both RGB and Ochrell test exact planner-proxy/StrokeList/composition replay; serialization and invalid values; brush selection precedence/ranges; adding an unrelated region without rerolling existing scopes; unchanged groups; earlier edits and later checkpoint reuse.

## Version and scope limits

Engine dev.7 is required because planner output changed. Author 2 and catalog 2 retain their formats; ScenePlan 1 additions are optional fields. Existing authored brush outputs are identical to dev.6 under the regression fixtures, but saved files still require their exact engine. Dev.6 remains at `858a22a`; previous paintings and review galleries were preserved. Historical source-hash review scripts should run at their matching revisions.

Selected-region replanning computes a whole candidate before accepting only that region. It rejects canvas/engine changes, new selected-region pass groups and different blur/dry boundaries. Independent RNG does not remove physical/error-feedback dependencies from a full plan. Exact local preservation uses the explicit acceptance helper.

The workbench's captured reference is session-only; BrushLab's reference persists in browser storage. There is no universal scene inference from a photograph, automatic artistic edge classification, new paint film simulation, tiling, renderer retirement or publication in this milestone.
