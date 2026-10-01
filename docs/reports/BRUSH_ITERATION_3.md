# Brush combinations — iteration 3

2026-10-02. Implements the supplied third review: clustered surface-catching scumble, less uniformly embossed impasto, less regular bundles, and form-following overlap. The existing seven presets are retained.

## Changes

- Scumble now combines brush-scale contact clusters, a short trailing connection, a canvas-fixed tooth field and actual substrate height relative to blurred height. It no longer distributes independent fine speckles uniformly over the footprint. Pressure and relief recruit contact; sparse deposits remain locally substantial. This is a deterministic contact approximation, not physical canvas fibres.
- Paint relief has smoother body between selected ridges. Coherent body variation controls where ridge strength is expressed; high-frequency everywhere-present height noise was removed. Impasto's height gain stays **1.5**; its stiffness, levee and blob defaults were reduced. Strongly outlined edges can still occur under raking light.
- Neighbouring bristle bundles gather and part gradually over distance, varying their widths and spacing. Their pickup release rates differ while their material histories remain persistent. Clean and contaminated paint are still carried by lanes; there is no particle or collision simulation.
- In paint mode, pressure recruits contact width and deposited thickness. High-body loaded paint reaches substantial coverage earlier in its pressure range, while zero pressure deposits nothing. Very light contact can still be translucent. Opacity remains an independent deposit control, and scumble/smudge/glaze retain pressure-weighted alpha.

## Authoring and review

[Compact review PNG](brush-review-3/review.png) · [Detailed gallery](brush-review-3/index.html)

The ten 960 px documents include same-path engine comparisons, scumble over flat and raised dry paint, impasto under unchanged light settings, loaded pressure over blue, sparse and denser dry paint over underlayers, rounded turns, practical fine-detail marks, a long pickup tail, varied overlapping strokes, and a leaf/fruit/stone study.

The form study changes the painting strategy deliberately. An overlapping thin foundation closes the silhouette; directional leaf marks, curved fruit turns and a few stone planes replace repeated identical horizontal rows. All seven brushes have specific roles. It remains visibly constructed from marks and is not presented as a finished realistic painting. The former identical-path study remains in the earlier gallery for controlled comparison.

The preview adds **Short turn**, **Dense dry setting**, **Form study**, and **Long pickup test**. Short turn bends the path with changing pressure; it does not simulate axial brush rotation. Dense dry is an existing-preset recipe (body .48, load .85, deplete .04, dropout .055, vdry .34), not another preset. Shared fixtures live in `packages/oilpaint/preview/study-cases.js`, used by both the preview and review generator.

## Version boundary and preservation

Engine **2.0.0-dev.5** changes rendered pixels. Author/catalog schema **2** is unchanged: no new document or binary fields are needed. Saved files from earlier engines fail the existing engine-version gate. Use commit `bc18d7b` for dev.4 and `a19d964` for dev.3. Previous schemas, goldens, galleries, painting scripts and saved artwork are unchanged.

The before panels use separately generated source documents and the matching captured dev.4 native executable, with the same raw paths, colors and lights as the after panels. They are not silently converted saved artworks. The current catalog remains embedded in every new source document.

## Verification

Tests specifically check clustered contacts rather than fine spray, stronger recruitment on raised versus recessed surface, changing bundle spacing/widths, substantial low-pressure loaded coverage with a narrower/thinner footprint, and zero-pressure non-deposition. The existing directional pickup, unchanged-group preservation, version/serialization gates, resolution checks and incremental/full replay tests remain active.

The review compares compiled strokes, all five paint planes and Studio images across native Rust, Node, Chromium and Firefox. Native/Node compare all four views for every document; the impasto case also compares all four views in both browsers. Final check results and timings are recorded below after verification.

## Remaining limits

The clustered scumble can still show brush-shaped islands and directional fragments. It is a heuristic surface-contact model. Bristle bundles still follow a common stroke trajectory, so coordinated tracks have been reduced rather than eliminated. Impasto no longer has uniform fine bump noise, but large steps between overlapping marks can still look embossed. There are no cast shadows, a physical brush-roll state, wetness-driven gloss or a layered film. The form studies make placement more varied but still need artistic judgment of silhouette, color and overlap. All seven presets remain candidates.


## Final verification and performance

- `cargo clippy --workspace --release --all-targets -- -D warnings`: passed.
- `cargo test --workspace --release`: passed, including 11 author tests and the new kernel contact/bundle tests.
- TypeScript typecheck and all 16 tests: passed.
- Browser workflow: 57 timed checks passed, including the new studies, plus file roundtrips, pointer drawing, preserved groups, reference capture and small-screen layout. One earlier concurrent run timed out waiting for a download; the download helper now awaits the click/event together, and the complete rerun passed.
- Review: all ten 960 px documents matched native, Node, Chromium and Firefox for compiled strokes, paint planes and Studio images. Native/Node matched every view; all four impasto views also matched in both browsers.
- Standard G1: all 33 cases passed on native Windows, Node, Chromium, Firefox and WebKit. The new engine-specific regression baseline is `golden/2.0.0-dev.5.json`; prior goldens are unchanged.

Ten warmed Chromium repetitions with inherited performance-core affinity mask 0xF, one loaded-flat curve and Ochrell. Cross-host verification was still active in the background, so these are observations under concurrent load, not an isolated comparison. Input-to-display includes 75 ms paint / 16 ms light debounce.

| Width | Change | Paint median | View median | Input-to-display median / range |
|---|---|---:|---:|---:|
| 256 | Load | 5.2 ms | 11.0 ms | 96.3 ms / 92.2–101.6 ms |
| 256 | Light | 0 ms | 10.0 ms | 27.2 ms / 26.9–28.8 ms |
| 384 | Load | 9.0 ms | 22.0 ms | 109.0 ms / 106.5–121.8 ms |
| 384 | Light | 0 ms | 21.5 ms | 39.0 ms / 38.5–39.5 ms |

Raw data: [latency](brush-review-3/latency.json), [browser workflow](brush-review-3/ui-measurements.json), [review hashes](brush-review-3/measurements.json), [G1 report](brush-review-3/xhost-report.json). The compact review PNG is 3000 × 3248 pixels, approximately 3.0 MB. Generate it with `npm run review:package3`.
