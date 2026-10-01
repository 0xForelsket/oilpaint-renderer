# One orange — frozen-brush painting test

2026-10-02. A painting/authoring test in response to the fourth review, not another brush redesign or diagnostic sheet.

## View first

- [Normal-size reference comparison](comparison.png) — photo and actual 384 px painting.
- [Painting at normal size](lit-384.png).
- [Larger painting](lit-768.png); [unlit](unlit-768.png); [height](height-768.png) are secondary inspection views.

The study uses **15 authored strokes**: two shadow marks, three underpainting marks, seven form/transition marks and three accents. Two underpainting marks are continuous circling passes of one and a half turns each, with a short center fill. The count therefore describes authored brush gestures, not fifteen straight segments. A single dominant direction above/front-left guides the painted values and relief light. Only the small highlight carries substantial height.

## Reference and attribution

Reference: **Single Orange (Fruit)**, photographed by **Augustus Binu**, 26 December 2018.

- File page: https://commons.wikimedia.org/wiki/File:Single_Orange_%28Fruit%29.jpg
- Original: https://upload.wikimedia.org/wikipedia/commons/9/9e/Single_Orange_%28Fruit%29.jpg
- License: **CC BY-SA 3.0**, https://creativecommons.org/licenses/by-sa/3.0/
- `reference.jpg` is the unchanged original; the comparison displays it smaller.
- The painted interpretation and comparison are offered under CC BY-SA 3.0. The renderer and authoring code retain the repository's existing license status; this does not relicense the renderer.

No endorsement by the photographer is implied. The reference informed silhouette, a unified orange mass, the light area, crown depression and contact shadow. Its peel texture was not copied into the paint.

## What was frozen

Engine **2.0.0-dev.5**, author/catalog schema **2**, all seven catalog definitions and the renderer were unchanged from `fb118dc`. Existing paintings and prior reviews were untouched. The study varies ordinary per-stroke settings: width, pressure, load, opacity, pickup and thickness, with restrained color streaking for the underpainting. These are saved in the study document; no preset defaults were edited.

The final image comes entirely from the actual Rust paint renderer and its lighting. There is no photographic underlay, raster color fill, CSS paint simulation, image blur, sharpening, texture overlay or image-generation substitution in the painted result.

## Painting decisions and findings

The first draft left small uncovered joins and visibly separate vertical masses. Those problems were already present in the unlit image. The final underpainting closes the silhouette with overlapping continuous round passes; closer neighbouring colors and low-height wet mixing reduce internal contrast. The outline, contact shadow and one highlight carry the strongest accents.

At 384 px, my visual assessment is that the result reads as one orange rather than several overlapping lobes. It is still flatter and more graphic than the photograph. The right-hand transition, crown/light patch and cast-shadow edge remain too explicit; faint circular handling is visible in the larger view. This is an observation for review, not an artist sign-off or a claim of finished realism.

The edge investigation stayed within the frozen engine. Current paint pressure recruits width and body; high-body paint does not provide an independent soft opacity envelope. `hardness` primarily changes bristle coverage and its threshold, while each mark has one load color and one opacity. Lower contrast, thinner paint and pickup helped the joins, but pressure alone could not soften them without also changing contact. That is the focused issue to revisit if this normal-size study still feels assembled. No speculative global preset changes were made.

## Reproduce and edit

Source: `studies/orange-study.ts`. The saved `painting.oil-author.json` can be opened in the existing preview's **Save & exchange → Open document**. `painting.oilstrokes` is the flattened native replay artifact.

From the repository root, with the existing WASM built and the preview server available on port 4173:

```powershell
node tools/render_orange_study.mjs docs/reports/orange-study
node tools/verify_orange_study.mjs
node tools/package_orange_study.mjs
```

The renderer script defaults to scratch output under `out/orange-study` when no destination is given. Five local draft renders are retained there; the delivered folder contains only the final painting, its reference and its verification material.

## Verification

- The study source typechecks under the repository's TypeScript toolchain.
- JSON serialization roundtrips, and the StrokeList compiles deterministically.
- Native Rust, Node, Chromium and Firefox agree exactly at 384 px on StrokeList bytes, all five paint planes and the lit image. See [verification.json](verification.json).
- The final study was rendered at 384 and 768 px. It was judged at 384 first; 768 was used to inspect joins rather than add texture.
- Hash checks compare the kernel, catalog, author compiler, painter and lighting source with `fb118dc` and confirm they are unchanged. No engine/version bump or new golden is needed.
- `report.json` records the saved lighting, stroke count, raw paint timings and plane hashes. Timings are single observational runs, not a new performance benchmark.
