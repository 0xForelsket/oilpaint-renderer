# Spikes

Throw-away experiments that back the plans in `docs/plans/`. They are evidence, not product code: nothing in `oilpaint/`
imports from here.

| Folder | What it is | Build / run |
|---|---|---|
| `oilcore/` | Rust port of the brush kernel (paint, scumble, smudge, glaze), relighting, image ops and mixers. Features: `detmath` (own exp/sin, bit-identical on every target), `mixer_km12` (open Kubelka-Munk mixer instead of Mixbox). About 1,500 lines. | `cargo build --release` (native), add `--target wasm32-unknown-unknown` for WASM. `tools/` has the parity scripts (C vs Rust vs WASM, Node and Chromium). |
| `planspike/` | Planner hot loops (error map, palette snap, path tracing) in JS and in Rust/WASM, and the JS/WASM boundary-cost test. | `python dump_inputs.py` (writes `data/`, not kept), then `node spike_js.mjs` / `spike_wasm.mjs` / `spike_granularity.mjs`. `rs/` is the Rust side. |
| `mixers/` | Open colour mixers compared with Mixbox: libmypaint's spectral WGM (ISC) and Kubelka-Munk on spectral.js 3.0 data (MIT). | `python open_mixers.py`, `python mix_test.py`. |
| `engine-prototypes/` | Prototype kernels and tests from the engine plan: multithreaded kernel, dab and knife tests, wet-paint levelling, glaze. | Scripts import the renderer from a scratch path; adjust the paths at the top before running. |

Scripts here were written in a scratch workspace and still contain absolute paths to it (for example
`planspike/dump_inputs.py`). Change them before running. Results are in `docs/plans/SPIKE_LOG.txt`.
