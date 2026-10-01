# Planning with the retained brushes

Engine `2.0.0-dev.7` connects the seven retained catalog-2 brushes to ScenePlan regions and passes. The shared Rust crate `oil-brush` owns catalog validation and contact/path compilation. Its catalog is byte-identical to dev.6. The author and planner both consume it; `impressionist` remains a separate scene/style preset.

## Run the workbench

```powershell
Set-Location C:\Users\sdhui\projects\oilpaint-renderer
npm run build:wasm
npm run preview
```

Open http://127.0.0.1:4173/packages/oilpaint/preview/composition.html. It plans the orange, leaf and stone in a worker. Select a region to tint, translate, hide or replan it with a brush. Capture a reference before editing. Save/open preserves resolved marks; Export StrokeList produces a version-gated binary file. The source editor accepts a complete ScenePlan; its full-plan action replaces the composition. BrushLab remains at the root URL for individual brush controls and pointer drawing.

## Scene API

ScenePlan 1 gains optional `brushCatalog` at the root and `brushPreset`, `pressure`, `edgeFade`, and `edgeInset` in region/layer styles. `SceneBuilder.brushCatalog(catalog)` embeds definitions; omitting it selects the engine's built-ins. Embed it when saving scene source to pin definitions.

```ts
const author = createAuthor(engine);
S.brushCatalog(author.catalog());
S.style("leaf", {
  brushPreset: "loaded-flat",
  width: [.02, .04],
  pressure: [.65, 1],
  edgeFade: .25,
  edgeInset: .6,
  hgain: .12,
});
```

Preset selection: layer reference overrides region reference, which overrides the scene/style preset reference. Material values resolve neutral defaults → scene/style → selected brush → explicit region values → explicit layer values. Explicit overrides remain useful recipes, rather than creating more presets. Catalog width limits are validated; perspective-adjusted widths are clamped to them. Catalog contact and pressure profiles are applied once. Unless explicitly overridden, depletion is calibrated to path length using the same compiler as authored marks.

| Control | Range / interpretation |
|---|---|
| `width` | Two canvas-width values, inside the selected brush's range |
| `pressure` | Two values in 0–1, sampled per stroke; default `[1,1]` |
| `edgeFade` | 0–1; reduce deposition against the region mask |
| `edgeInset` | 0–2 half-widths; probe mask support along the footprint and reduce edge contact |
| `spill` | Existing style control; named brushes default to zero |
| `hgain` / `opacity` | Existing explicit style overrides for thin shadows and selective thick accents |

Edge controls are mask-based contact adjustments, not physical brush rotation or a full lost-and-found-edge solver. Named brushes sample target color inside their region, use smoother path wobble and finer path steps. Regions still need useful flow fields and a pass schedule. See `scenes/catalog_still_life.ts`: elongated leaf flow, off-center curved orange flow, restrained stone planes, thin shadows and sparse highlights.

## Resolved composition and local edits

```ts
import { loadEngine, createAuthor, editRegion, replaceRegion } from "oilpaint";
const engine = await loadEngine();
const author = createAuthor(engine);
const planned = engine.plan(scene, { width: 256, seed: 1907 });
const edited = editRegion(planned.document, "leaf", group => {
  for (const stroke of group.strokes) {
    stroke.color[0] *= .9;
    stroke.color2[0] *= .9;
  }
});
author.render(edited, 384);
const candidate = engine.plan(changedScene, { width: 256, seed: 1907 });
const accepted = replaceRegion(edited, candidate.document, "leaf");
author.render(accepted, 384);
```

`plan()` returns `{ report, strokes, document }`. The document is `oil-composition`, version **1**, with an exact engine gate. It stores resolved four-component points `[x,y,width,pressure]`, colors, all material parameters and seeds. It never reapplies a contact profile. `preset` is provenance; changing that label alone does not retune already-resolved marks. Use a candidate plan to change the brush recipe.

Each pass/region pair becomes a named group with a length-delimited stable ID; stroke IDs are group-local ordinals. Array order is paint order. Pass blur and drying operations remain at their original boundaries, including empty groups. Hiding paint does not remove those shared boundary operations. Display names can change independently of IDs. Planner random streams use seed, pass name, pass seed offset and region name rather than global indices. Renaming a planning scope intentionally changes its stream.

`editRegion` clones the composition and touches only matching groups. `replaceRegion` copies only the selected region's candidate strokes. Both clear `sourceHash`, since the result no longer exactly represents the original ScenePlan. Other groups retain geometry, colors, parameters, IDs and order. Replanning currently computes a **complete candidate** before accepting only one region; it is not a partial-planner speed optimization. Canvas/engine changes, new selected-region groups or changed pass boundary operations are rejected and require an explicit full composition replacement.

Independent random streams prevent random-draw coupling. A complete replan can still change later decisions through canvas coverage/error and paint interaction. Use `replaceRegion` when unrelated authored marks must stay fixed. Their final pixels may still change if downstream wet pickup carries edited paint into them; correct replay accounts for that.

The existing replay cache compares the compiled prefix and restores full paint/material state, then replays downstream groups. It does not use approximate rectangular updates. The same `createAuthor` session accepts `oil-author` 2 or `oil-composition` 1; `parse`, `compile`, `render`, and `view` validate both. Save the JSON alongside flattened StrokeLists to retain IDs.

## Reproduction and boundaries

```powershell
npm run demo:still-life
cargo build --release -p oil-plan --example composition -p oil-author --example author
npm run verify:still-life
npm run test:composition-ui
```

The verification and UI commands need the preview server. `demo:still-life` writes to `out/planner-integration/draft-01`; pass another output directory directly to `tools/plan_still_life.mjs` to retain variants. The verification command regenerates this milestone's review artifacts only. It does not touch the previous painting studies.

This is a bounded planner/authoring integration, not L3 visual acceptance or retirement of the Python renderer. Older StrokeLists and documents require their original engine. ScenePlan additions are optional source fields, and new outputs carry dev.7; there is no silent saved-file migration. Historical review scripts that pin source hashes must run at their matching commits.
