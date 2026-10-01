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
let (canvas, _) = job.paint(512, |_, _| {})?;
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
