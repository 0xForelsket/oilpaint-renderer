# CLAUDE.md: oilpaint

## Project brief

oilpaint is Sean's offline oil-painting renderer. It made the painting Storm Light
(`../storm-light-painting`). It is being rebuilt as a p5.brush-style library, for oil painting only:

- **Rust/WASM core:** the paint engine (kernel, mixers, lighting, image ops), the planner and the scene
  compiler.
- **TypeScript:** scene authoring, hosting (Worker, canvas, p5 add-on, Node) and agent tooling.
- **Agent-first:** headless, deterministic, JSON feedback.

The plan is `docs/plans/LIBRARY_PLAN.md`, the only current plan. It is built in milestones L0 to L13. After each
milestone, stop and report.

State of the repo:

- **v1 renderer (tag `v1-python-c`):** Python planner, C kernel `oilpaint/csrc/brush.c`, Mixbox. That tag is
  the only way to reproduce Storm Light. Later versions do not load v1 stroke files, and that is deliberate
  (a hard break).
- **Still on main until the L3 retirement commit:** the Python renderer, including `--mixer ochrell` via
  `native/ochrell-brush`. The Rust spike kernel is `spikes/oilcore`.
- **Ochrell** (`../ochrell`, MIT OR Apache-2.0 code, CC BY-SA 4.0 data): the default mixer of the new engine.
  Its integration evidence is in `docs/OCHRELL.md` and `../ochrell/docs/integration-results.md`.
- **The new engine:** the Rust workspace in `crates/`, the TypeScript packages in `packages/` (from L2), and the
  specs in `spec/`. Engine version `2.0.0-dev.4` (L3 in progress; `dev.2` after L1: v1's paint transport with
  Ochrell, Sean's decision);
  Ochrell is pinned by commit (Cargo git dependency in `crates/oil-mix/Cargo.toml`). Milestone reports:
  `docs/reports/`. A film spike (F0) runs before L7.

## Rules

1. **Hard break.** No legacy mode, no compatibility layer, no v1 stroke loader, and no bit-parity with the C
   kernel as a requirement.
   - Stroke files carry an engine version. Loading a file from another engine version fails with a clear
     message.
   - The guarantee is: same engine version and same inputs give identical output on every host (native, Node,
     Chromium, Firefox).
   - All transcendental maths goes through the engine's pure-Rust maths. Never call platform libm
     (`f32::exp`, `powf`, `ln`, `sin`, `cbrt` ...) in anything that affects pixels or strokes. No FMA, no
     fast-math, no relaxed SIMD, no hash-map iteration order in outputs.
2. **C kernel.** It is only a one-off sanity check while porting (Rust vs C within tolerance, via the eval
   harness). There is no C compiler on this machine: compare against the Rust spike kernel
   (`OILPAINT_KERNEL=rust`) instead, and do not install a compiler.
3. **Retirement.** The planner port (L3) is a cutover: there is no parity gate with v1's planner, and Storm Light
   is not reproduced (Sean's decision; acceptance in the plan, section 5). At the end of L3, one dedicated commit
   removes the C kernel, the Python kernel wrapper and the Python planner. The Python eval harness stays until the
   Rust metrics replace it (L5).
4. **Measured acceptance.** Use the eval harness, parity tables and timings. Reports keep measured and
   projected numbers apart and say which is which.
5. **Licences.**
   - No GPL code. libmypaint and Krita are for ideas only. Keep `NOTICE.md` accurate.
   - Mixbox (CC BY-NC) is only an opt-in plug-in.
   - Ochrell's CIE-derived data is CC BY-SA 4.0. Sean chose option A: ship it as is, with the package licence
     `(MIT AND CC-BY-SA-4.0)` and clear licence and notice files (plan section 10). Nothing is published before
     L6.
6. **Ochrell edits.** Keep its invariants: no `unsafe`, no dependencies, and its own tests green. It has its own
   git repo. Another agent may be working there (for example the optimisation rounds in
   `docs/optimization-*.md`), so check `git status` there first and never touch files you did not write.
   - The engine pins an Ochrell commit and Cargo builds a clean checkout of it, never the live folder. To move the
     pin, wait for a clean Ochrell tree with its rounds committed, then update `rev` and re-run the goldens.
   - An additive change there (like `encode_linear`, `ffd6ee9`) keeps existing results bit-identical, runs its
     tests and regenerates `MANIFEST.sha256` exactly as `tools/package_release.py` does, without writing the zip.
7. **Git.**
   - Small commits with clear messages, and no attribution trailers.
   - Never rewrite history, never force-push, never delete Sean's outputs (`out/`, renders, stroke files).
   - Never overwrite, reformat or revert work you did not do.
8. **Decisions.** Decide for yourself, unless the decision cannot be undone or needs Sean's eye: visual
   sign-offs, licences, naming, anything published.

## Running on Windows (native, PowerShell; not WSL)

Toolchain:

- Windows x64, rustc 1.91.1 (MSVC), Python 3.12 via uv, Node 24.
- No C compiler on PATH.
- Playwright browser builds are cached in `%LOCALAPPDATA%\ms-playwright`; Edge and Chrome are installed.
- The Rust target `wasm32-unknown-unknown` is needed for WASM builds (`rustup target add wasm32-unknown-unknown`).

Python (v1 renderer and eval harness):

```powershell
uv venv .venv
uv pip install --python .venv\Scripts\python.exe -r requirements.txt
$env:OILPAINT_KERNEL = "rust"          # no C compiler here: use the Rust spike kernel (spikes/oilcore)
$env:PYTHONIOENCODING = "utf-8"
.venv\Scripts\python.exe -m oilpaint test all
.venv\Scripts\python.exe -m oilpaint eval run --name mychange
.venv\Scripts\python.exe -m oilpaint eval compare eval\baseline_v1.json out\eval\mychange\eval.json
```

Ochrell integration checks (see `docs/OCHRELL.md`):

```powershell
.venv\Scripts\python.exe -m unittest discover -s tests -p test_ochrell.py -v
.venv\Scripts\python.exe tools\check_ochrell.py
```

New engine (Rust workspace `crates/`, TypeScript and CI scripts at the root):

```powershell
cargo clippy --workspace --release --all-targets -- -D warnings   # clippy.toml bans platform libm (determinism)
cargo test --workspace --release
$env:PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD = "1"; npm install         # Playwright 1.58.2 matches the cached browsers
npm run xhost          # cross-host check: native + Node + Chromium + Firefox + WebKit, then compare (ci/xhost)
```

The `oil` CLI (build with `--features mixbox` for the opt-in Mixbox mixer):

```powershell
cargo build --release -p oil-cli --features mixbox
.\target\release\oil.exe testsheet --out out\ts.oilstrokes
.\target\release\oil.exe paint out\ts.oilstrokes --width 800 --mixer ochrell --light painting --out out\ts
.venv\Scripts\python.exe tools\plan_to_strokelist.py scenes\storm_v3.py out\storm.oilstrokes   # Python planner -> StrokeList v2
node ci/xhost/scene.mjs --strokes out\storm.oilstrokes --width 600 --hosts native,node,chromium   # timing + full-scene determinism
```

ScenePlan, guides, the planner and the TS package (L2, L3):

```powershell
.\target\release\oil.exe plan spec\examples\storm_v3.sceneplan.json --out out\storm.oilstrokes --report out\storm_plan.json   # the Rust planner
node packages/oilpaint/src/cli.ts plan scenes\storm_v3.ts --out out\storm.oilstrokes                                   # the same, TS -> WASM
.\target\release\oil.exe paint out\storm.oilstrokes --width 1200 --light painting --out out\storm --npy
.venv\Scripts\python.exe tools\compare_plans.py --v1-scene scenes/storm_v3.py --spec spec/examples/storm_v3.sceneplan.json --out out\cmp   # v1 vs engine planner (until the L3 retirement)
npm run build:wasm            # crates/oil-wasm -> packages/oilpaint/wasm/oil.wasm (git-ignored; the TS package needs it)
npm run typecheck; npm run test:ts
node packages/oilpaint/src/cli.ts guides scenes\storm_v3.ts --width 600 --out out\guides     # TS scene -> guide sheet (WASM)
.\target\release\oil.exe guides spec\examples\storm_v3.sceneplan.json --out out\guides --npy   # the same, native
.venv\Scripts\python.exe tools\compare_guides.py --rust out\guides --out out\guides\compare      # against v1's guides
```

The ScenePlan types are generated, never edited by hand: change `crates/oil-scene/src/spec.rs`, then regenerate the
schema (`$env:OIL_WRITE_SCHEMA = "1"; cargo test --release -p oil-scene`) and the TS types (`npm run gen:types`).

The Python harness can drive the new kernel through `crates/oil-shim`: set `OILPAINT_KERNEL=oil` and use an 85-float
mixer (`--mixer ochrell`, or `mixbox-material` for Mixbox). Judge engine changes over several seeds (8 in L1): the
harness's single-seed thresholds are tighter than the seed-to-seed spread of the mixing and outline metrics.

**Timing on this laptop:** the Core Ultra 7 258V has 4 performance cores (CPUs 0-3) and 4 low-power efficiency
cores (4-7), and single-thread runs on an efficiency core are about 1.4x slower. Windows moves threads between them,
so unpinned timings swing by 2x. Pin benchmarks to a performance core (`Process.ProcessorAffinity = 1 -shl 2` in
PowerShell), or pin the PowerShell process itself to all four (`0xF`) so child processes and browsers inherit it,
and report ranges over repeats.

## Conventions

- **Units.** Geometry is in canvas-width units (cw), as in v1.
- **Line endings.** Text files use LF, enforced by `.gitattributes`. Golden and binary files are stored
  byte-for-byte.
- **Golden hashes.** They come from the new engine and are regression checks only, keyed by engine version.
  Output that changes means an engine-version bump, and the goldens are regenerated with it.
- **Scratch output** goes to `out/` (ignored).
- **TypeScript** is run directly by Node 24 (type stripping). Use erasable syntax only (no enums, namespaces or
  parameter properties) and `.ts` extensions in relative imports; `tsc` is only for checks.
