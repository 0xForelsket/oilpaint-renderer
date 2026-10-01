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

1. Magic `OPJ2`, u32 paint count, u32 decoder kind (0 = direct K-M).
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
and sixteen-paint target import. Test coefficients are synthetic. A measured
Old Holland Eight package and empirical optical-model integration remain separate
work; supporting eight recipe slots does not supply those coefficients.
