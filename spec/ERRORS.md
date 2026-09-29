# Errors and warnings (draft)

Every error the engine reports, from Rust, the CLI JSON or the TypeScript API, has the same shape:

```json
{
  "code": "ENGINE_VERSION_MISMATCH",
  "message": "This stroke list was written by engine 2.0.0-dev.3; this is engine 2.0.0-dev.5.",
  "path": "/header/engineVersion",
  "got": "2.0.0-dev.3",
  "expected": "2.0.0-dev.5",
  "fix": "Re-plan from the ScenePlan with this engine, or paint it with a package that ships engine 2.0.0-dev.3 (see spec/ENGINE_VERSIONS.md)."
}
```

| Field | Meaning |
|---|---|
| `code` | stable identifier, UPPER_SNAKE; agents branch on it |
| `message` | one sentence for a human |
| `path` | JSON Pointer into the input (a ScenePlan or a StrokeList's logical structure) when one exists, otherwise omitted |
| `got`, `expected` | the offending value and what is allowed (a value, a range such as `"[0, 1]"`, or a list), when they apply |
| `fix` | a concrete next step, when one exists |

A validation pass reports **all** problems it finds, as a list, not just the first. Warnings use the same shape and
never stop a run. They go to the `warnings` array of the JSON report.

## Codes

### Stroke lists (`STROKELIST_V2.md`)

| Code | When |
|---|---|
| `V1_STROKE_FILE` | the input is a Python-renderer stroke file (`.npz`, zip magic `PK\x03\x04`). The fix names git tag `v1-python-c`. |
| `NOT_A_STROKELIST` | the first 8 bytes are not the StrokeList magic |
| `UNSUPPORTED_GENERATION` | the magic matches but the generation field is not 2 |
| `ENGINE_VERSION_MISMATCH` | the file's engine version differs from the running engine's |
| `CORRUPT_STROKELIST` | truncated file, bad section order or length, or the SHA-256 in `END ` does not match |
| `INVALID_STROKELIST` | the structure is intact but a value is out of range: non-finite, a colour outside [0, 1], fewer than 2 points, non-monotonic offsets, a layer range gap, an unknown mode, `nb` outside 3..72, and so on |

### Scenes (`SCENEPLAN_V1.md`)

| Code | When |
|---|---|
| `SCHEMA` | JSON does not match the ScenePlan v1 schema (unknown key, wrong type, missing required key) |
| `UNSUPPORTED_SCENEPLAN` | `sceneplan` is not 1 |
| `UNITS` | a size or position is outside its cw range; the fix converts from pixels |
| `RANGE` | a value is outside its documented range |
| `UNKNOWN_COLOR` | a colour string is neither `#rrggbb` nor a known tube |
| `UNKNOWN_REGION` | a style, layer or curve names a region that is not declared |
| `UNKNOWN_FIELD` | a shape or flow refers to a sampled field that was not supplied |
| `FIELD_HASH_MISMATCH` | a supplied sampled field does not match the hash recorded in the spec |
| `DUPLICATE_NAME` | two regions or two layers share a name |
| `ENGINE_VERSION_DIFFERS` | *warning*: `engine` in the spec differs from the running engine (an error with `strictEngine: true`) |

### Painting and hosts

| Code | When |
|---|---|
| `CANVAS_TOO_LARGE` | `width x height x bytesPerPixel(mixer)` exceeds the host's memory budget; `expected` gives the largest allowed size at the requested aspect, and `fix` gives the CLI command |
| `UNKNOWN_MIXER` | the requested mixer ID is not in this build (for example `mixbox` without the plug-in) |
| `PLAN_MIXER_DIFFERS` | *warning*: the stroke list was planned with a different mixer than the one painting it |
| `NON_PORTABLE_PLAN` | *warning*: the plan used hooks or sampled fields, so other JS engines may plan it differently (painting is unaffected) |

### Planning feedback (warnings, from L5)

`REGION_NO_FLOW`, `REGION_UNPAINTED`, `LAYER_OVERDRAW`, `BLACK_PIXELS`, `CLIPPING`: see `docs/plans/LIBRARY_PLAN.md`,
section 9.
