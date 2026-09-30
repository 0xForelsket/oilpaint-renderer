# Engine versions

Which engine version (see `README.md`) shipped where. A StrokeList can be painted only by the engine version that
wrote it. This table tells you which package or git tag that is.

| Engine version | Package version | Git | Notes |
|---|---|---|---|
| (v1, not part of this scheme) | – | tag `v1-python-c` | Python planner, C kernel, Mixbox; `strokes.npz` files; reproduces Storm Light |
| `2.0.0-dev.1` | unreleased | main, from L1 (commit after 8b3ed90) | first Rust engine output: kernel port with counter RNG, fixed-order pick-up sums, material transport, pure-Rust maths, sRGB tables; mixers rgb and opt-in Mixbox (Ochrell cases are added when it is wired in; they change no existing output) |
| `2.0.0-dev.2` | unreleased | main, after L1 (Sean's transport decision) | v1's transport (alpha deposit + dirty-brush pick-up) with Ochrell as the default mixer; no `amount` plane (Ochrell 368 B/px). With rgb and Mixbox bit-identical to the L1 step-2 kernel |
