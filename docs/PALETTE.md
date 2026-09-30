# Palette recipe painting (opt-in native API)

`oil-mix::palette::PaletteMixer` and `oil-palette::PaletteJob` connect Ochrell's
prepared palette decoder to the existing Rust brush kernel. Four f32 paint
proportions are retained per pixel. Targets are matched once during authoring;
painting, mixing, pick-up and replay use recipes directly.

The current `OchrellMixer` remains the default. This is a native Rust API and
example, not a new CLI mixer flag or TypeScript/ScenePlan option. WASM bindings,
browser execution evidence and default promotion are subsequent work. No existing
stroke format is reinterpreted and no engine-version bump is needed for an opt-in
new container: OPJ1 embeds the current version-gated StrokeList geometry.

## Use from code

```rust
use oil_mix::palette::PaletteMixer;
use oil_palette::{PaletteJob, RecipeLoad};

let mixer = PaletteMixer::from_bytes(&std::fs::read("palette.opp")?,
                                     &std::fs::read("palette.opl")?)?;
let found = mixer.match_target([0.31, 0.67, 0.47])?;
println!("achieved {:?}, error {}", found.achieved_srgb, found.error_ok100);
let ground = mixer.paint("white")?;
let green = mixer.recipe([1.0, 0.0, 1.0, 0.15])?;

// Any validated StrokeList supplies geometry, brush parameters and layer actions.
let geometry = oil_paint::testsheet::testsheet();
let loads = vec![RecipeLoad::solid(green); geometry.strokes.len()];
let painting = PaletteJob::new(mixer, geometry, ground, loads)?;
let (canvas, stats) = painting.paint(512, |layer, canvas| {
    // Optional progress or intermediate display.
})?;
std::fs::write("painting.opj", painting.to_bytes()?)?;
let restored = PaletteJob::from_bytes(&std::fs::read("painting.opj")?)?;
let replay = restored.paint(512, |_, _| {})?;
```

Amounts are normalized into the palette's four paint slots in order. The recipe
has no RGB residual. For a whole RGB-authored stroke list,
`PaletteJob::from_rgb(mixer, geometry)` returns the painting and a `MatchReport`:
ground plus main/secondary load reports in stroke order. Streak variants are also
matched once and stored as recipe directions. Explicit loads supply `main`,
`secondary`, and a finite zero-sum `dz` in [-1,1], already scaled by the desired
streak amount. Explicit painting ignores the geometry's RGB and streak-amount
fields; they are retained as authoring provenance. The brush's mode, marble,
strength, transport and other parameters still apply.

The matcher reports both the direct spectral search error and the prepared
decoder's actual error after f32 storage. `achieved_srgb` uses the engine's
portable transfer. Error is OKLab distance times 100, not a statement about real
paint accuracy. This synthetic palette cannot make deep black; a large error is
an expected useful result. Targets are not guaranteed to have a unique recipe or
a certified global optimum.

Palette files contain full optical data and attribution. Loading checks their
hashes, conventions and palette/LUT identity. See Ochrell's
[package and target documentation](../../ochrell/docs/palette-workflow.md).
User-defined coefficients must already be in consistent K/S units and the fixed
81-band grid; naming an artist or supplying RGB swatches does not supply optical
measurements. Measured Old Holland data remains unbundled pending redistribution
rights. More than four paints is not supported yet.

## Save and replay

OPJ1 contains, in order:

1. Four magic bytes `OPJ1`.
2. Three u32-length-prefixed byte sections: OPP1 optical palette, OPL1 prepared
   decoder, and the existing engine-version-gated StrokeList.
3. Four little-endian f32 ground proportions; u32 load count; twelve f32 values
   per stroke (main, secondary, streak delta).
4. SHA-256 of every preceding byte.

The limit is 128 MiB per bundle. Framing, count, finite normalized recipes,
zero-sum deltas, inner checksums, identities and engine version are checked.
Serialization preserves each stored f32 bit; loading does not rematch RGB or
rebuild the table. Checksums detect corruption, not source authenticity.

Painting supports dimensions up to 16,384 per side with at most 16,777,216 pixels.
The canvas keeps the renderer's other planes and brush behavior. Material storage
falls from 85 to 4 floats (340 to 16 bytes); the full canvas budget including the
optional blur plane is 368 versus 44 bytes/pixel, plus a shared LUT (about 3.3 MB
at resolution 65) and small per-palette authoring state. This memory calculation
does not predict whole-render speed.

## Reproduce the native example

From the Ochrell repository, generate two independently loadable synthetic inputs:

```powershell
cargo run --release --offline --example palette_workflow
```

From oilpaint-renderer:

```powershell
cargo test --release -p oil-mix -p oil-palette
cargo clippy --release -p oil-mix -p oil-palette --all-targets -- -D warnings
cargo run --release -p oil-palette --example palette_paint -- ../ochrell/target/palette-workflow/synthetic-four.opp ../ochrell/target/palette-workflow/synthetic-four.opl out/palette-workflow/synthetic-four
cargo run --release -p oil-palette --example palette_paint -- ../ochrell/target/palette-workflow/strong-white.opp ../ochrell/target/palette-workflow/strong-white.opl out/palette-workflow/strong-white
```

Each run makes target-matched and explicit-recipe PNGs, OPJ1 bundles and plane
hashes. It reloads from disk and asserts exact replay of all five saved canvas
planes. The strong-white palette doubles white's K and S, retaining its pure
color while changing tinting strength. The same fixed recipe loads produce a
different result, while target matching can compensate with different recipes.

See the [native integration report](reports/PALETTE_NATIVE.md) for verification
and the remaining integration boundary.
