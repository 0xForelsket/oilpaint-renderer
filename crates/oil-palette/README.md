# Native painting with palette recipes

The opt-in native API supports 1-16 paints per palette. A recipe retains one
component per paint through brush pickup, deposit, streaks and saved replay.
The existing default renderer and RGB StrokeList format are unchanged.

For eight paints, load a complete Ochrell optical package and construct a direct
mixer. Other counts use the same API with a different N (up to 16):

```rust
use oil_palette::{PaletteMixerN, PaletteJobN, RecipeLoadN};
# fn example(bytes: &[u8], geometry: oil_strokes::StrokeList)
# -> Result<(), Box<dyn std::error::Error>> {
let mixer = PaletteMixerN::<8>::from_palette_bytes(bytes)?;
let ground = mixer.recipe([0., 0., 0., 0., 0., 0., 0., 1.])?;
let paint = mixer.recipe([1., 2., 0., 0., 1., 0., 0., 4.])?;
let loads = vec![RecipeLoadN::solid(paint); geometry.strokes.len()];
let job = PaletteJobN::new(mixer, geometry, ground, loads)?;
let (canvas, _) = job.paint_final(512)?;
let saved = job.to_bytes()?;
let restored = PaletteJobN::<8>::from_bytes(&saved)?;
# let _ = (canvas, restored);
# Ok(()) }
```

Slot order comes from the package; the example does not assume the last paint
actually is white. `paint(name)` also returns a pure recipe. To import an RGB
StrokeList, use `PaletteJobN::from_rgb`: matching happens during authoring and
reports achieved colors/errors, then painting consumes only saved recipes.

`PaletteMixer` and `PaletteJob` remain the original prepared four-paint aliases.
They still use OPL1 acceleration and write byte-compatible OPJ1. Direct palettes
use `PaletteMixerN<N>` / `PaletteJobN<N>` (including direct N=4 if desired).
There is no multidimensional LUT for larger palettes, no reduced four-paint
subset and no implicit RGB correction. Direct decoding has a different cost
from the prepared four-paint path; no equal-throughput claim is made.

## Saved jobs

OPJ1 retains its original layout. OPJ2 contains:

1. Magic `OPJ2`, u32 paint count, u32 decoder kind (0 = direct optical package).
2. Three u32-length-prefixed sections: optical palette, empty table section,
   and version-gated StrokeList geometry.
3. N f32 ground proportions, u32 load count, then 3*N f32 per stroke (main,
   secondary and signed zero-sum streak direction).
4. SHA-256 of all preceding bytes. All numbers are little-endian.

Wrong paint counts, decoder modes, palette identities, checksums, geometry
versions, nonfinite/negative weights and mismatched lengths fail explicitly.
No automatic palette conversion occurs. Recipe planes cost `4*N` bytes/pixel;
the current complete canvas accounting is `28 + 4*N` bytes/pixel (60 for eight,
68 for ten and 92 for sixteen), before transient buffers and allocator overhead.

Run `cargo test --release --offline -p oil-mix -p oil-palette`. Checks cover
legacy four-paint behavior, all-plane bit-identical replay with 8/10/16 paints,
high-index recipe corruption, RGB-independent explicit painting, bounded streaks
and sixteen-paint target import. These test coefficients are synthetic.

## Local Old Holland Eight packages

The sibling Ochrell study now exports plain K-M and empirical-pair OPP3 packages
on the source's native 31-band 400-700 nm grid. Load either using
`PaletteMixerN::<8,false,31>::from_palette_bytes` and `PaletteJobN<8,false,31>`.
The B=31 parameter is the spectral sample count; it does not change the eight
stored material fractions. OPP3 carries the grid, display projection, model kind
and all empirical controls, and OPJ2 embeds that whole definition. A plain K-M
package and an empirical package have different identities. An incompatible grid
is rejected instead of resampled. Four-paint prepared APIs remain unchanged.

```text
cargo run --release --offline -p oil-palette --example old_holland_eight -- <palette.opp> <output-directory>
```

The example expects the Old Holland paint names. It renders 94 strokes with
matched colors and explicit eight-component recipes, writes PNG and OPJ2 files,
and checks all canvas planes for exact same-host replay. The RGB preview covers
only 400-700 nm; it is not a full-visible color measurement. The empirical path's
cross-platform logarithm/exponential bit parity has not been established.
Local models and fitted accuracy results live under the sibling project's
`target/measured-oils/unified-eight/` and `experiments/oil_unified_eight/`.
The default renderer and public distribution remain unchanged.

The subsequent balanced full-data package is the preferred local experimental
measured choice for recipe painting. It lives at the sibling Ochrell path
`target/measured-oils/balanced-eight/old-holland-eight-balanced-empirical.opp`.
The previous package remains available. The decision and measured tradeoffs are
in Ochrell's `experiments/oil_balanced_package/REPORT.md`; this is not a global
default switch or a claim that arbitrary RGB targets become reachable.

Compare two local packages with identical recipes, RGB targets and stroke
geometry, including exact saved-job and future-mixture replay:

```text
cargo run --release --offline -p oil-palette --example compare_old_holland -- <previous.opp> <balanced.opp> <output-directory>
```

The example writes six PNG/OPJ2 pairs, per-recipe and per-target CSVs, all-plane
hashes and seven alternating warmed timing observations. At 384x480 on the study
host, the balanced 94-stroke fixture took a median 224 ms; direct display decode
was about 1.02 microseconds per recipe. Target matching is an authoring cost,
separate from these paint timings. There is no eight-paint LUT in this package;
all eight material proportions and the exact direct decoder are retained.

## Exact target-match cache

Each `PaletteMixerN` now retains up to 1024 exact authoring results by default.
Repeated calls to `match_target` or `encode`, including RGB import's streak
variants, reuse those results. The key is the three original finite f32 RGB bit
patterns. Nearby colors are not rounded into the same entry. Entries belong to
one immutable palette/decoder instance; loading another package starts empty.

`mixer.target_cache_stats()` reports hits, misses, evictions, entries and capacity.
`mixer.with_target_cache_capacity(n)` configures a bounded FIFO cache and resets
it; zero disables retention. `mixer.clear_target_cache()` requires mutable access
and clears entries/counters while retaining capacity. A cache hit returns the
entire original `TargetMatchN`, including its original solver evaluation count;
use miss counts to measure actual new searches.

The cache is synchronized for shared mixers, but the inverse solver runs outside
the lock. Concurrent misses for the same target may both solve. No cache state
is serialized, and neither painting nor forward decoding accesses it. Saved
recipes, achieved colors, errors, optical definitions and OPJ bytes are unchanged.
The cold path for unique targets still runs the full solver. A warm-cache timing
assumes those exact colors were already matched; filling the cache has a cost.

Run the profiling and equivalence benchmark with a local measured package:

```text
cargo run --release --offline -p oil-palette --example authoring_cache -- <palette.opp> <output-directory> [pre-change-fixture.opj]
```

The optional pre-change job checks exact compatibility against an archived
uncached run. The benchmark checks the fixture and repeated/unique target sets
under disabled, cold and warm modes, reporting seven rotated observations plus
warm-cache priming time. Numerical results are in the sibling Ochrell report
`experiments/palette_authoring_cache/REPORT.md`. This accelerates authoring;
it is not a forward LUT or a change to the paint model.

## Final-image rendering

Use `job.paint_final(width)` when only the completed image is needed. It runs
the same brush/material simulation, then evaluates the existing decoder once
for each pixel that received a deposit. Pixels with no deposit keep the exact
ground color. All material proportions, height, wetness, coverage, blurred
height, statistics and final RGB match `job.paint(width, after_layer)` exactly
on the verified cases. No approximation, LUT or new canvas plane is introduced.

Keep `paint(width, after_layer)` for intermediate layer previews. Its callbacks
still see current RGB after every layer. `paint_final` has no preview callback
and does not modify the saved OPJ format; the same job can be rendered with
either method after reload. Both methods enforce the same dimension/memory limits.
The optimization is opt-in, and saved model identities and engine version stay
unchanged because final output bits are preserved.

The native scaling benchmark runs the saved balanced Old Holland Eight scenes
at widths 512/1024/2048 with a 4:5 aspect. Its diagnostic RGB-bypass mode isolates
removable decoding work and is never a display option. See the sibling Ochrell
report `experiments/palette_canvas_scaling/REPORT.md` for timings, exact checks,
canvas allocation accounting and observed benchmark-process working sets.

```text
cargo run --release --offline -p oil-palette --example canvas_scaling -- <saved-job-directory> <output-directory> 1024
```

That directory must contain `fixed-recipes.opj`, `matched-targets.opj` and
`renderer-fixture.opj` from the existing package comparison. It loads already
authored recipes; matching is outside every paint timing. Three measured rounds
follow one warmup, with mode order rotated. The measured speedup depends on
how often pixels are revisited; no equal-throughput claim is made for arbitrary
scenes, other paint counts, browsers or other hosts.
