# Same orange — contact, turning surface and integrated light

2026-10-02. A revision of the form-coherence study accepted in the user's review. Renderer dev.5 and all seven brush presets remain frozen.

[Before/after at normal size](comparison.png) · [Revised 384 px painting](lit-384.png) · [768 px painting](lit-768.png)

## Painting changes

- **Contact shadow:** ten very thin, progressively smaller washes build the outward falloff, followed by a near-contact transition and a compact dark contact. The ends of the darkest mark tuck under the fruit. The two former pale shapes are replaced by a value progression; all softness comes from actual overlapping pigment deposits, not an image blur.
- **Turning surface:** the existing underpainting's color is biased toward the shadow half-tone, so a bright untouched outer strip does not surround the dark turn. Broader, low-contrast passages carry the light into the right-hand half-tone. A small underside turn connects the fruit to its contact shadow. The underpainting paths, widths, pressures and controls are unchanged.
- **Highlight:** the separate bright bed was removed. Overlapping, quieter light-field marks establish the illuminated area before a smaller, lower-relief accent is placed in it. The crown depression is unchanged.

The result has **28 authored strokes**: 12 for the shadow/falloff, 3 for the preserved underpainting, 11 for form and transitions, and 2 accents. Most of the additional marks are low-opacity shadow glazes. The same dominant light direction and exactly the same renderer lighting parameters are retained.

At normal viewing size my assessment is that the fruit has more weight, the right-side shading is less rim-like, and the small accent belongs more clearly to the surrounding light. It remains a stylized painting: some light-plane boundaries and circular handling are still visible, and the shadow is not a physical soft-shadow simulation. These are review observations, not an artist sign-off.

## Preservation and verification

- The complete original `docs/reports/orange-study` folder and `studies/orange-study.ts` match commit `a908b2e`. Nothing in the accepted study was overwritten.
- Engine, palette/catalog, authoring version, canvas, background and lighting are unchanged. The three underpainting marks match their former paths, widths, pressures and control settings exactly; only their colors changed. The crown mark matches exactly.
- TypeScript checking passed for the revised source.
- Native Rust, Node, Chromium and Firefox agree exactly at 384 px on compiled StrokeList bytes, all five paint planes and the lit image. JSON roundtrip and the frozen kernel/catalog checks also pass. See [verification](verification.json) and [preservation](preservation.json).
- 384 and 768 px images were rendered from the same saved source. [Report](report.json) includes the actual settings, raw timing observations and hashes.

Source: `studies/orange-study-revision.ts`. It derives a fresh document from the original study and changes only this painting's named groups. The saved `painting.oil-author.json` opens in the existing preview through **Save & exchange → Open document**. `painting.oilstrokes` is the flattened replay file.

## Reproduce

From the repository root, with the existing WASM and native author example built, and the preview server running on port 4173:

```powershell
node tools/render_orange_revision.mjs docs/reports/orange-study-2
node tools/verify_orange_study.mjs docs/reports/orange-study-2
node tools/verify_orange_revision.mjs
node tools/package_orange_revision.mjs
```

The render script otherwise writes to `out/orange-revision/draft-01`. Local drafts are retained under `out/orange-revision`; this folder contains only the delivered revision.

## Reference and license

The reference remains [Single Orange (Fruit)](https://commons.wikimedia.org/wiki/File:Single_Orange_%28Fruit%29.jpg) by Augustus Binu, CC BY-SA 3.0. The unchanged photograph is stored in the original study folder. These painting revisions and comparisons use [CC BY-SA 3.0](https://creativecommons.org/licenses/by-sa/3.0/) as the preceding study does. No endorsement by the photographer is implied. Source-code licensing is unchanged.
