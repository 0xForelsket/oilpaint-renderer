# Brush authoring and preview

This is the bounded implementation of the three requested priorities, independent of the remaining LIBRARY_PLAN milestones. The seven brush presets are **candidates awaiting artist review**. `impressionist` remains a scene/style preset; brush presets are separate data.

## Launch

From PowerShell:

```powershell
Set-Location C:\Users\sdhui\projects\oilpaint-renderer
npm run build:wasm
npm run preview
```

Open http://127.0.0.1:4173. The server binds only to loopback. Node 24 strips the library's TypeScript for browser module loading; no new bundler or runtime dependency is required. The preview runs the actual Rust kernel in a dedicated WASM worker. The main-thread engine is used only for catalog validation and document/StrokeList export.

Choose a candidate and draw, or replace the selected group's marks with a repeatable straight, curved or dab sample. Samples retain a stable ID and seed. Add wet blue underpaint to examine pickup. Select **Wet underpaint** and change its drying to compare wet versus dry pickup. Ground, paint, Ochrell/RGB mixer, pressure, width, load, thickness, body, opacity and dry-breakup threshold are adjustable. Lit/unlit/height views and lighting do not repaint. Height is shown on a fixed 0–3 scale.

Select groups by stable ID in the menu, rename, hide, reorder or recolor them. Geometry controls translate/scale stored paths, or edit one stroke's exact `[x,y,pressure]` array. New pointer strokes enter the selected group. Color edits preserve geometry and pressure; preset changes intentionally select that preset's default width and controls. The geometry scale is about the canvas origin and affects positions, not widths. Width is a separate control.

Save reference captures the current rendered image and view/mixer metadata to browser local storage; it persists on reload. It remains fixed when live controls change. Save/open documents preserves the source and embedded catalog. Export/import catalog exchanges **candidate** definitions. Reset restores the selected catalog definition's controls, including imported definitions. Export StrokeList produces a normal version-gated `.oilstrokes` file for the existing native `oil paint` CLI.

## Source format and API

Rust `oil-author` owns the catalog, validation, path sampling, brush compilation and replay. `crates/oil-author/catalog.json` is the one built-in catalog. TypeScript gets this data from WASM, never a duplicated set of constants. `spec/author-2.schema.json` and `src/author-types.ts` are generated from Rust. Regenerate with:

```powershell
cargo run --release -q -p oil-author --example schema | Set-Content spec/author-1.schema.json
npm run gen:author
```

```ts
import { loadEngine, createAuthor, editGroup, moveGroup, sampleMark,
         scopedRandom, defaultView } from './packages/oilpaint/src/index.ts';
const author = createAuthor(await loadEngine());
let document = author.document(1907);
document.groups = [
  { id: 'water', name: 'Water', visible: true, dryAfter: 0.8,
    strokes: [sampleMark('water-1', 'loaded-flat', 0.025)] },
  { id: 'lilies', name: 'Lilies', visible: true, dryAfter: 1,
    strokes: [sampleMark('lily-1', 'rounded-dab', 0.03, 'dab')] },
];
document = editGroup(document, 'lilies', group => {
  group.strokes[0].color = [0.9, 0.5, 0.6];
  group.strokes[0].path[0][0] += 0.01;
});
author.render(document, 384, 'ochrell');
const image = author.view(defaultView); // RGB8; another view() only relights
const bytes = author.compile(document); // existing StrokeList v2
const restored = author.parse(JSON.stringify(document));
const independentDraw = scopedRandom(document.seed, ['lilies', 'lily-1', 'placement'], 0);
```

Document format is explicitly `oil-author`, version **2**. It requires the exact running engine string and an embedded catalog version **2**. Unknown fields, versions, presets, controls, duplicate IDs, invalid ranges and nonfinite numbers fail validation. A group ID is unique in a document; stroke IDs are globally unique. Group and stroke array order is paint order. Renaming display names does not change seeds. Seed scopes use length-delimited UTF-8 FNV-1a; random draws should name the permanent group, subject, purpose and counter. Adding draws in one scope cannot consume another scope's stream.

The current iteration uses engine `2.0.0-dev.6`, author format **2** and catalog **2**. The dev.6 change is isolated to scumble contact. The six retained presets and their same-input paint output match dev.5 exactly; the catalog is unchanged. Earlier-engine documents and StrokeLists are still explicitly refused by the version gate. Dev.5 is preserved at commit `c4a8cd3`. Author/catalog schema 2 remains unchanged; dev.4 / schema 2 is preserved at commit `bc18d7b`. Use commit `a19d964` for dev.3 / author 1; its reference gallery and saved source files remain intact. No silent migration or legacy rendering switch is provided. The author compiler and catalog have their own version gates. Definitions remain embedded, so changing a built-in preset cannot reinterpret a saved document. Keep the JSON source for IDs/editing alongside flattened StrokeList exports.

Each group compiles to an existing layer with pre-group height blur (0.002 cw) and post-group wetness multiplier. Paths are resampled at roughly 0.2 brush widths (16–2048 segments) using arc length in Rust. Stored pointer pressure multiplies the preset pressure envelope. Width/pressure envelopes use smooth interpolation into the landing and release after 58% of the path. Catalog 2 adds `contact` (`flat`, `round`, `point`) and `variation` (0–0.25). Round contact uses a curved, off-centre footprint; coherent variation is tied to stable stroke seeds. Fine detail uses very low variation. Depletion defaults to `preset.depletion / max(1, 2 * pathLength / width)`; a per-stroke `deplete` override is an advanced escape hatch.

## Candidate calibration and useful ranges

Width is in canvas-width units (cw); pressure is 0–1. In paint mode it recruits contact width and deposited thickness; high-body loaded paint reaches substantial opacity earlier in the pressure range. Zero pressure deposits nothing. Other modes retain pressure-weighted alpha. The catalog's width triplets are `[minimum, default, maximum]`. Minimum widths are authoring limits, not promises that a subpixel mark remains visible.

| Candidate | Width min / default / max | Main behavior |
|---|---|---|
| Loaded flat | .002 / .024 / .12 | Full body, restrained relief, loaded landing with a modest tail taper |
| Rounded dab | .002 / .030 / .12 | Narrow landing and tail, fuller middle; use short paths for dabs |
| Dry drag | .002 / .026 / .12 | Low body/load, stronger depletion and pressure lift; separated bristle tracks |
| Scumble | .003 / .032 / .12 | Engine scumble mode, broken coverage responding to height relative to its blur |
| Impasto accent | .002 / .018 / .08 | Higher deposited height and ridge, reduced flattening; use sparingly |
| Fine detail | .0005 / .004 / .025 | Fewer lanes, low splay, narrow lifting tail |
| Wet mixing | .002 / .025 / .12 | Paint mode with strong pickup/release, lower load and relief |

Core controls: `load` 0–1.5, `pickup` 0–1, `hgain` (thickness) 0–3, `body` 0–1, `opacity` 0–1, `vdry` .05–1. More load sustains coverage; more body fills between lanes; opacity scales deposition rather than acting as an independent layer transparency. More pickup carries wet substrate color down the path. Higher `vdry` exposes bristles earlier as paint depletes. More thickness adds actual height, but large values plus strong relief lighting can still look embossed. The preview offers Soft (the prior settings), Studio (bump 1.05, contrast .45, specular .12) and Raking (same bump, contrast .6, specular .25). These light changes reuse the exact same paint planes. Impasto hgain stays at 1.5.

Advanced validated controls: `flatten`, `hardness`, `dropout`, `release`, `streakMix`, `marble` 0–1; `nb` integer 2–68; `splay` 0–3; `deplete`, `streak`, `grain`, `ragged`, `ridge`, `levee`, `furrow`, `blob`, `stiff` 0–3. `mode` is 0 paint / 1 scumble / 2 smudge / 3 glaze in a preset. Current author marks have a single paint color, so `marble` has no second-color contrast. No filbert, fan or palette-knife mechanics are claimed.

The starting profiles draw on `study-03/paint_study.py` and `study-04/refine_study.py`: high body for loaded paint, lower levees, controlled height, narrower landings/tails and path-relative depletion. Those scripts and every saved painting were read only.

## Replay and responsiveness

The replay cache compares the actual compiled ordered layer prefix, including geometry, colors, seeds, parameters, blur and drying. It restores the latest available unchanged checkpoint and replays **all** downstream groups. A change to ground, aspect, size or mixer starts a new canvas. Renaming a group alone does not change paint. It does not attempt unsafe rectangular dirty-region rendering: later pickup may carry an early color edit elsewhere.

Checkpoints contain all six planes (material, RGB, height, wetness, cover, blurred height), are limited to two, and share a 64 MiB budget. The author preview canvas budget is 192 MiB; cache misses fall back to full replay. This is not the large-canvas/tiled roadmap. Native exports can use the existing `oil paint` command for larger renders.

A worker keeps painting off the UI thread; parameter edits debounce for 75 ms, light changes for 16 ms, and pointer drawing schedules throttled previews. There is at most one worker request in flight and one coalesced pending request. Paint changes survive a following light-only event. The status measures worker paint, view and input-to-display latency with `performance.now()`. Heavy documents remain costly, and rendering cannot be interrupted halfway through a kernel call. Pointer preview is not a guaranteed 60 fps brush engine.

## Verification and review

`docs/reports/brush-review/index.html` preserves the iteration-one review gallery: seven candidates, three widths, straight/curved/dab samples crossing wet blue paint, 256/512 px, fixed lighting, native and Chromium images, plus unlit/height images. The generator also checks Node and Firefox: compiled StrokeList bytes, all five persistent paint-plane hashes, and all three view images must match exactly. A mismatch fails generation. These are technical regression checks, not artist validation.

With the preview server running:

```powershell
cargo build --release -p oil-author --example author
npm run review:brushes
npm run test:preview
```

Normal checks: `cargo clippy --workspace --release --all-targets -- -D warnings`, `cargo test --workspace --release`, `npm run build:wasm`, `npm run typecheck`, `npm run test:ts`, `npm run xhost`.

The UI verification covers every preset/sample, material/light/view controls, two mixers, group preservation/order/visibility, translation, exact path edits, pointer drawing, invalid input, rapid paint/light changes, import/export/reset, persistent reference and narrow viewport layout. Rust checks full versus incremental replay in both mixers, width/resolution sampling, roundtrip/version rejection and untouched compiled strokes.

Remaining limits: the existing bristle primitive still makes geometric, sometimes faceted landings and long ribbons; these presets do not implement new brush physics. Subpixel detail and individual dry lanes may disappear at low resolution. Scumble is relief-sensitive, not a full physical canvas-tooth simulation. Drying multiplies wetness; it is not a chemical cure model. Wet mixing uses existing brush pickup, not a layered film or palette knife. Height view clips above 3 by design. See the measured review report for timings and observations.


## Iteration-two review

See [larger contact/form review](reports/brush-review-2/index.html) and [report](reports/BRUSH_ITERATION_2.md). The new pressure ramp, long taper, press-and-lift, runout and reverse-crossing samples are available in the preview. `npm run review:iteration2` regenerates current native/Node/Chromium/Firefox evidence; it requires a running preview server and `cargo build --release -p oil-author --example author`. Before images use a dev.3 executable captured at `out/brush-iteration-2/baseline/author-dev3.exe`; without it the checked-in before images are retained. Regenerate those only with the matching `a19d964` engine. The earlier `review:brushes` command now writes `out/brush-review-current`, preserving the historical gallery.

## Iteration-three studies

The preview adds Short turn (a bent path, not simulated axial brush roll), Dense dry setting (existing Dry drag controls), Form study (all seven used for appropriate roles), and Long pickup test. The dense dry recipe uses body .48, load .85, deplete .04, dropout .055 and vdry .34. It is not an eighth preset. The shared authored studies are in packages/oilpaint/preview/study-cases.js.

Run npm run review:iteration3 with the preview server running to verify ten 960 px documents and generate the gallery; npm run review:package3 produces the compact PNG. Before images use the captured dev.4 native executable in out/brush-iteration-3/baseline/author-dev4.exe. Without it, existing before images remain in place. The historical reviews are preserved.
