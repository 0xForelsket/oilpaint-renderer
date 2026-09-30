# Palette native integration, 2026-09-30

Ochrell is pinned to clean commit `1d63f90b8f0e6d16d335df27a4ec28035169f8e3`.
It contains the prepared decoder, target matcher, optical packages and their
evidence. The core passed 57 tests including docs on Rust 1.91.1 and 1.75.0,
WASM compilation and 1,418 independent reference probes. Fresh target matching
tested 2,048 colors across two synthetic palettes, maximum error <0.000002
OKLab ×100. That is agreement with a synthetic model, not measured paint accuracy.

The renderer change adds a four-proportion mixer and a separate `oil-palette`
crate. No brush, planner, scene, TypeScript or WASM binding implementation changes
are required. Existing default RGB/Ochrell behavior remains intact. Concurrent
renderer development continued independently; this report concerns palette mode.

## Native checks

- All five `oil-mix` tests and four `oil-palette` integration tests passed.
- Scoped release Clippy with warnings denied passed.
- `cargo check --release -p oil-palette --target wasm32-unknown-unknown` passed;
  this is a compile check, not browser execution.
- The testsheet exercises the existing brush modes, layer blur/drying, secondary
  loads, wet mixing and streaks. Canvas states remained finite, nonnegative and
  normalized throughout the checked results.
- Explicit recipes ignore RGB inputs: changing every authoring RGB and streak
  amount while retaining explicit loads gives identical canvas bits.
- Changing optical data changes display colors while retaining the same recipe
  plane and brush transport. Same-name palettes with different optical contents
  reject one another's prepared tables.
- Both in-memory tests and real file readback replay exactly. Corrupt framing,
  counts, checksums, invalid recipes and wrong engine versions are rejected.

Two palette files were loaded into the native example. Each produced a
94-stroke, 512×640 testsheet twice (target-matched and directly authored). All
OPJ1 reloads reproduced the canvas planes bit for bit. Example bundle sizes:
3,350,689 bytes for Synthetic Four; 3,350,696 for the strong-white variant.
PNGs were visually inspected. These unlit brush fixtures demonstrate integration,
not aesthetic or physical paint validation.

Single local runs took 161–166 ms to author all target/streak recipes and
65–69 ms to paint a testsheet after authoring. These unpinned illustrative timings
include no comparative speed claim and are not steady-state benchmarks.
Maximum main target error was 17.584 OKLab ×100: some authored test colors lie
outside this palette's gamut. The importer reports this error instead of adding
a color residual.

Outputs are reproducible under `out/palette-workflow` using [the API guide](../PALETTE.md).
No generated painting or synthetic package is required to build the crate.

The default mixer was also compared directly across the old `ffd6ee9` and new
`1d63f90` Ochrell pins. With seed 9371, 10,000 color pairs produced 30,000
bit-identical encoded/mixed optical states and linear decodes. The small isolated
[pin-check source](palette-native/pin-check/src/main.rs) and manifest are retained:

```powershell
cargo run --release --offline --manifest-path docs/reports/palette-native/pin-check/Cargo.toml --target-dir out/palette-workflow/pin-check-build
```

All 31 existing native xhost cases ran successfully; their hashes are retained in
`palette-native/native-existing-cases.json`. Engine `2.0.0-dev.3` has no frozen
golden file at this checkpoint, so a one-host comparison is not golden regression
proof or cross-host evidence. No new golden was manufactured for this change.

The full workspace test attempt encountered errors in the concurrently developed
`oil-plan/src/style.rs` (nested macro repetition and missing overlay functions).
Those files are outside this change and were not modified by this work.
`cargo test --workspace --exclude oil-plan --release` then passed all 44 tests
in the remaining workspace, including the new palette workflow tests.

## Remaining scope

The renderer-facing Rust API is usable from code now. Browser/Node bindings,
ScenePlan and command-line palette selection, cross-host palette replay evidence,
measured palettes and a default change are not part of this native checkpoint.
The adapter already uses portable gamma/cube-root math; portability of this new
path still requires runtime evidence across hosts. The original mixer is retained.
