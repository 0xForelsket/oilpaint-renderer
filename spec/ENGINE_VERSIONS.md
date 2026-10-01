# Engine versions

Which engine version (see `README.md`) shipped where. A StrokeList can be painted only by the engine version that
wrote it. This table tells you which package or git tag that is.

| Engine version | Package version | Git | Notes |
|---|---|---|---|
| (v1, not part of this scheme) | – | tag `v1-python-c` | Python planner, C kernel, Mixbox; `strokes.npz` files; reproduces Storm Light |
| `2.0.0-dev.1` | unreleased | main, from L1 (commit after 8b3ed90) | first Rust engine output: kernel port with counter RNG, fixed-order pick-up sums, material transport, pure-Rust maths, sRGB tables; mixers rgb and opt-in Mixbox (Ochrell cases are added when it is wired in; they change no existing output) |
| `2.0.0-dev.2` | unreleased | main, after L1 (Sean's transport decision) | v1's transport (alpha deposit + dirty-brush pick-up) with Ochrell as the default mixer; no `amount` plane (Ochrell 368 B/px). With rgb and Mixbox bit-identical to the L1 step-2 kernel |
| `2.0.0-dev.3` | unreleased | main, L3 | pick-up, approach and release rates per distance (half a brush width), not per path segment; the Rust planner (`oil-plan`, a cutover from v1's, Sean's decision); the scene compiler's flows evaluated at points, cropped 16-bit masks, image targets, presets. First golden: `golden/2.0.0-dev.3.json` (31 cases, 7 hosts) |
| `2.0.0-dev.4` | unreleased | brush iteration 2 | Sparse scumble contacts, independent bristle load life and pickup affinity. Author format/catalog 2 add shaped contact envelopes. dev.3 files require the matching engine at commit `a19d964`; no silent upgrade. |
| `2.0.0-dev.5` | unreleased | brush iteration 3 | Brush-scale scumble clusters with surface contact; smoother/local relief; pressure recruits paint contact and body; neighbouring lane bundles gather/spread with individual release. Author/catalog schema 2 retained. dev.4 requires commit `bc18d7b`. |
| `2.0.0-dev.6` | unreleased | isolated scumble refinement | Only the scumble contact mask changes: solid/medium/small contacts with ragged edges and short links. Other six presets and their paint planes match dev.5; author/catalog schema 2 unchanged. dev.5 is at `c4a8cd3`. |
