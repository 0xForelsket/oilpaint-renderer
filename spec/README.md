# oilpaint specs

These specs define the two seams of the new engine:

| Spec | What it is | Status |
|---|---|---|
| [`SCENEPLAN_V1.md`](SCENEPLAN_V1.md) | **ScenePlan v1**, the JSON input that describes a painting: canvas, target, regions, flows, light, styles, layers. Schema: [`sceneplan-1.schema.json`](sceneplan-1.schema.json) (generated from the Rust types); example: [`examples/`](examples/) | frozen in L2 |
| [`STROKELIST_V2.md`](STROKELIST_V2.md) | **StrokeList v2**, the binary output of the planner and input of the painter; carries the engine version | draft (L0); frozen with the codec in L1 |
| [`ERRORS.md`](ERRORS.md) | The structured error format and the error codes shared by the Rust core, the CLI and the TS API | kept current; scene codes implemented in L2 |
| [`ENGINE_VERSIONS.md`](ENGINE_VERSIONS.md) | Which engine version shipped in which package version or git tag | kept current from L1 |

## The engine version

The engine version is one string, for example `2.0.0-dev.1`. It is defined once in Rust
(`oil_kernel::ENGINE_VERSION`) and names everything that can change an output bit:

- the kernel and the canvas planes;
- the planner and the scene compiler;
- the mixers, including the pinned Ochrell model;
- the maths library, the RNG streams and the codecs.

**Rules:**

1. **Same version, same output.** For one engine version, the same inputs give byte-identical outputs on every
   supported host: native (Windows x64, Linux x64, macOS arm64), Node ≥ 22, Chromium and Firefox. This is the
   engine's only reproducibility promise, and CI checks it by SHA-256 (`../ci/xhost`).
2. **Bump on any output change.** Any change that can move an output bit bumps the version. Changes that
   cannot (docs, error messages, faster code with identical bits) keep it. The golden hashes
   (`../golden/<version>.json`) enforce this: an output change without a bump fails CI.
3. **The version gate.** A StrokeList records the version that wrote it. Painting it with any other version fails
   with `ENGINE_VERSION_MISMATCH`. There are no converters and no legacy modes.
4. **Scenes carry forward.** A ScenePlan may record the version it was authored with. A different engine
   *warns* (`ENGINE_VERSION_DIFFERS`) and plans anyway, because scenes are source code carried forward.
   `strictEngine: true` makes the warning an error.
5. **Package versions are separate.** The npm and crate versions follow semver on their own. A release that
   changes no output keeps the engine version. `ENGINE_VERSIONS.md` maps between them.

**v1 (the Python/C renderer)** is not an engine version of this scheme. Its stroke files (`strokes.npz`) are
refused with `V1_STROKE_FILE`, and the only way to paint them is the git tag `v1-python-c`.

## Conventions shared by both specs

- **Units.** Geometry is in canvas-width units (cw). x runs over [0, 1], and y runs down over [0, aspectH/aspectW]
  (a 4:5 portrait has y in [0, 1.25]). Widths and lengths are fractions of the canvas width, so a scene or a stroke
  list means the same thing at any resolution.
- **Angles** are degrees: 0 points along +x, 90 along +y (down).
- **Colours** are authored in sRGB, as `#rrggbb`, a tube name, an `[r, g, b]` triple in [0, 1], or a two-colour
  mix `["a", "b", t]`. Mixes are evaluated by the active mixer. Mixer states (latents) never appear in either
  format.
- **Seeds** are unsigned 32-bit integers.
- **Numbers** must be finite: NaN and infinity are rejected at every boundary.
