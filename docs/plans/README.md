# Plans for the next phase (oil painting first)

The v3 plan, `ENGINE_PLAN_v1.md` and the spikes were written on 2026-09-29. On 2026-09-30 Sean decided the hard
break with v1 (tag `v1-python-c`), made Ochrell the default mixer and settled the scene-freedom question. The plan
was re-cut as v4 before any library code was written.

| File | What it is |
|---|---|
| `LIBRARY_PLAN.md` | **Current plan (v4).** oilpaint as a p5.brush-style library: Rust/WASM engine *and planner*, Ochrell mixer, TypeScript scene authoring, agent-first tooling. About 48.5 agent-days in milestones L0-L13; public alpha after L6 (about 30 days, with the physics milestones L7 and L11 moved ahead). |
| `LIBRARY_PLAN_v3_cparity.md` | The previous version: a legacy v1 mode, frozen strict C, parity levels relative to C, and an open KM mixer to calibrate. Kept for the reasoning. |
| `LIBRARY_PLAN_v2_jsplanner.md` | The version before that (planner in JS, about 41 agent-days). |
| `ENGINE_PLAN_v1.md` | The physics/kernel plan (settling paint, glazes, palette knife, speed). Its items are folded into the library plan's milestones L7-L12; its v1-compatibility items (legacy path, v1 defaults, v1 digests) no longer apply. |
| `SPIKE_LOG.txt` | Every measurement behind the library plan (Rust vs C, WASM, planner hot loops, mixers, maths-library parity), taken on a 2 vCPU Linux VM. |
| `evidence/engine/` | Images and logs behind `ENGINE_PLAN_v1.md` (before/after, flaw crops, glaze and knife prototypes, profiles). |
| `evidence/library/` | Images behind the library plan (mixer ramps, Mixbox vs open mixer, Python vs Rust relight). |

Milestone reports (measured results against this plan) are in `docs/reports/` (L1: `docs/reports/L1.md`).

The Ochrell measurements that v4 relies on live in the sibling repo, in `../ochrell/docs/integration-results.md`
(taken on the Windows laptop). `docs/OCHRELL.md` shows how to run the integration here.

## Where the older plans' `SP/...` paths went

The v2 and v3 plans and `ENGINE_PLAN_v1.md` were written in a scratch workspace and refer to it as `SP/`. In this
repo:

| In the plans | Here |
|---|---|
| `SP/work_eval/` (measuring stick) | this repo, `eval/`, `oilpaint/eval*.py`, `tests/t8_eval.py` |
| `SP/work_lib/oilcore/` (Rust kernel spike) | `spikes/oilcore/` |
| `SP/work_lib/planspike/` | `spikes/planspike/` |
| `SP/work_lib/mixers/` | `spikes/mixers/` (sources only) |
| `SP/work_lib/evidence/`, `SPIKE_LOG.txt` | `docs/plans/evidence/library/`, `docs/plans/SPIKE_LOG.txt` |
| `SP/work_engine/evidence/`, `logs/` | `docs/plans/evidence/engine/`, `docs/plans/evidence/engine/logs/` |
| `SP/work_engine/exp/` | `spikes/engine-prototypes/` (sources and small logs only) |

Not kept: compiled binaries (`.so`, `.wasm`, Rust `target/` folders), the multi-hundred-MB replay dumps (`.npy`) and
the large per-scene renders. The spike scripts still contain some absolute paths from the scratch workspace; see
`spikes/README.md`.
