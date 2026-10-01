# Brush review iteration 2

2026-10-01. This iteration implements the supplied review priorities, keeps the same seven candidate presets, and preserves the first gallery and all existing paintings.

## What changed

1. **Relief presentation:** Soft, Studio and Raking controls use the same paint planes. The default Studio direction is lower and more directional; Raking is an explicit diagnostic. Impasto's `hgain` remains 1.5. The change exposes deposited height rather than uniformly raising it.
2. **Scumble:** deposition now uses sparse, locally substantial contacts instead of forcing thin lane coverage everywhere. Two coherent spatial scales select contacts, with substrate height relative to blurred height and pressure influencing the threshold. The pattern is evaluated in brush-width units on the canvas, before paint deposition; it is not a texture overlay. An initial coarse pattern was rejected during visual inspection and refined into smaller contacts.
3. **Contact and release:** smooth width/pressure transitions replace the linear, long-held envelope. Round contact uses a curved, off-center footprint. Seeded contact variation changes gradually along the mark. Fine detail uses minimal variation, lower raggedness and no splay. The preview adds pressure ramp, long taper, press/lift, depletion and reverse-crossing samples.
4. **Independent bristle behavior:** lanes deplete on different lengthwise schedules and have persistent pickup affinities. Dry bundles can break and reconnect at different positions; some wet bundles carry substrate color while adjacent ones retain more clean paint. These mechanisms alter deposition and carried material, not the final image.

## Review material

[Large review gallery](brush-review-2/index.html) · [Single PNG](brush-review-2/review-compact.png)

Source images are 960 px wide. Each candidate has a same-light before/after contact panel, separately inspectable Studio/Raking/unlit/height views, and the same leaf/fruit/stone paths and colors. The form studies are controlled probes, not finished paintings: parallel passes and faceted silhouettes remain conspicuous. Fine detail is capped at its own width range.

The diagnostic section compares unchanged impasto paint under Soft and Raking light, Wet mixing in both directions, a dried-underpaint control and a stroke that runs out of paint. The before images were rendered by the preserved dev.3 native executable at commit `a19d964` using separately generated version-1 source documents. Existing saved documents were not upgraded or overwritten.

## Verification

- New tests require scumble to leave open gaps and substantial local contacts; require raking illumination to reveal more contrast without mutating albedo or height; and require reversing a wet stroke to reverse the direction of blue pickup.
- The existing incremental/full replay, unchanged-group, serialization, range and deterministic width/resolution checks remain active in both mixers. Ten author tests pass.
- TypeScript typecheck and all 16 tests pass. Browser QA covers 53 timed control/sample interactions, plus drawing, file roundtrips, invalid input, local preservation, references and narrow layout.
- The final review generator compares 19 documents at 960 px: compiled StrokeList bytes, all five persistent paint planes and Studio images across native Rust, Node, Chromium and Firefox. Native/Node compare all five views for every case; both browsers also compare all five impasto views. See [raw evidence](brush-review-2/measurements.json).
- Cross-host engine regression cases and final workspace tests are recorded with the committed dev.4 golden and final verification summary below.

## Version boundary

Engine **2.0.0-dev.4**, author format **2**, catalog **2**. Kernel changes affect pixels and contact changes affect compiled geometry, so these are explicit version breaks. Earlier author documents and StrokeLists fail with version-mismatch errors. To reproduce dev.3 / author 1, use commit `a19d964`. The old schema, golden, gallery and artwork files remain intact; there is no silent compatibility path.

The catalog adds `contact` (`flat`, `round`, `point`) and `variation` (0–0.25). Embedded definitions still prevent future catalog tuning from changing a saved document. The revised authoring API and controls are documented in [BRUSH_AUTHORING.md](../BRUSH_AUTHORING.md).

## Remaining artistic and engine limits

Raking illumination deliberately exposes both useful relief and the existing model's embossed appearance. There are still no cast shadows, a wetness-dependent gloss model or a layered film. The rounded profile is geometric contact, not a physical filbert simulation. Scumble uses a deterministic surface-contact approximation rather than resolved physical canvas tooth. Dry and wet paths remain bristle lanes: the bundles have more independent paint histories, but their trajectories still follow the authored stroke. Low pressure remains translucent, and very small marks can disappear at low resolution.

The larger form studies show that controlling contour, spacing, direction and overlap remains essential. Presets alone do not solve repeated-ribbon placement. All seven remain candidates awaiting further visual review; passing numerical checks is not artist approval.


## Final checks and measured latency

Final workspace tests and Clippy pass; all 16 TypeScript tests pass. Cross-host G1 passes all 33 cases on native Windows, Node, Chromium, Firefox and WebKit. The resulting engine-specific regression hashes are in `golden/2.0.0-dev.4.json`; the dev.3 golden is unchanged. The 19-document review generation completed with exact agreement. Browser QA passed 53 timed interactions; see `ui-measurements.json`.

Ten warmed Chromium repetitions per operation, one loaded-flat curve, performance-core affinity mask 0xF. Other verification processes were still active, so these numbers are observations under concurrent load, not isolated performance comparisons:

| Width | Operation | Worker paint median | View median | Input-to-canvas median / range |
|---|---|---:|---:|---:|
| 256 | Change load | 5.8 ms | 11.0 ms | 98.7 ms / 93.2–109.7 ms |
| 256 | Relight | 0 ms | 11.1 ms | 28.3 ms / 27.7–30.5 ms |
| 384 | Change load | 11.6 ms | 29.2 ms | 123.8 ms / 116.4–139.0 ms |
| 384 | Relight | 0 ms | 30.5 ms | 51.7 ms / 48.4–88.1 ms |

[Raw latency](brush-review-2/latency.json) includes all repeats. Input-to-canvas includes the existing 75 ms paint / 16 ms light debounce. The shareable compact PNG is 3980 × 3112, approximately 3.3 MB, retaining every panel's original 960 px width. Generate it with `node packages/oilpaint/preview/package-iteration-2.mjs`.
