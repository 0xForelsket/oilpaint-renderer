# Plans for the next phase (oil painting first)

Written 2026-09-29, before any of it was built. Nothing here has been implemented; the three decisions at the end of
`LIBRARY_PLAN.md` (section 9) are still open.

| File | What it is |
|---|---|
| `LIBRARY_PLAN.md` | **Current plan (v3).** oilpaint as a p5.brush-style library: Rust/WASM engine *and planner*, TypeScript scene authoring, agent-first tooling. About 48 agent-days in milestones L0-L13; public alpha after L6 (about 26 days). |
| `LIBRARY_PLAN_v2_jsplanner.md` | The previous version (planner in JS, about 41 agent-days). Kept for the reasoning behind the change. |
| `ENGINE_PLAN_v1.md` | The physics/kernel plan (settling paint, glazes, palette knife, speed). Its items are folded into the library plan's milestones L7-L12. |
| `SPIKE_LOG.txt` | Every measurement behind the library plan (Rust vs C, WASM, planner hot loops, mixers, maths-library parity). |
| `evidence/engine/` | Images and logs behind `ENGINE_PLAN_v1.md` (before/after, flaw crops, glaze and knife prototypes, profiles). |
| `evidence/library/` | Images behind the library plan (mixer ramps, Mixbox vs open mixer, Python vs Rust relight). |

## Where the plans' `SP/...` paths went

The plans were written in a scratch workspace and refer to it as `SP/`. In this repo:

| In the plans | Here |
|---|---|
| `SP/work_eval/` (measuring stick) | this repo, `eval/`, `oilpaint/eval*.py`, `tests/t8_eval.py` |
| `SP/work_lib/oilcore/` (Rust kernel spike) | `spikes/oilcore/` |
| `SP/work_lib/planspike/` | `spikes/planspike/` |
| `SP/work_lib/mixers/` | `spikes/mixers/` (sources only) |
| `SP/work_lib/evidence/`, `SPIKE_LOG.txt` | `docs/plans/evidence/library/`, `docs/plans/SPIKE_LOG.txt` |
| `SP/work_engine/evidence/`, `logs/` | `docs/plans/evidence/engine/`, `docs/plans/evidence/engine/logs/` |
| `SP/work_engine/exp/` | `spikes/engine-prototypes/` (sources and small logs only) |

Not kept: compiled binaries (`.so`, `.wasm`, Rust `target/` folders), the multi-hundred-MB replay dumps (`.npy`) and the
large per-scene renders. The spike scripts still contain some absolute paths from the scratch workspace; see
`spikes/README.md`.
