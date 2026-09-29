# StrokeList v2

The planner's output and the painter's input: an ordered list of brush strokes in canvas-width units. It
replays at any resolution and with any mixer.

- **Status:** implemented in L1 by the Rust codec `crates/oil-strokes` (reader, writer, validation) and a Python
  writer for the port checks, `oilpaint/strokelist.py`. The layout below changes only together with an
  engine-version bump.
- **Changes from the L0 draft:** the per-stroke `arclen` field was dropped. The kernel measures the stroke length
  in brush widths from the cw points itself, which makes the bristle sample count size-independent without
  storing anything.
- **File extension:** `.oilstrokes`. **Media type:** `application/x-oilpaint-strokes`.
- **Replaces:** v1's `strokes.npz`. That format is refused (`V1_STROKE_FILE`); there is no converter.

## Design rules

1. **Deterministic bytes.** The same stroke list always serialises to the same bytes, on every host. The file
   therefore contains no timestamps, host names, timings or paths. The SHA-256 of the whole file is the stroke
   list's identity; it is what the cross-host check compares.
2. **The version gate comes first.** The first 64 bytes have a layout that is frozen for every future generation.
   A reader checks the magic, the generation and the engine version before it parses anything else, so a file
   from another version is refused with a precise message and never misread.
3. **Mixer-independent.** Colours are stored as authored sRGB. The painter encodes them with the active mixer
   (Ochrell by default) using the engine's pure-Rust maths. Stroke lists are small and can be painted with
   any mixer; the mixer used for planning is recorded in `META`.
4. **Little-endian, 4-byte aligned, uncompressed.** The file can be memory-mapped, hashed and handed to WASM
   without a decoder. Compress it for transport (gzip, brotli); compression is outside the format.
5. **Exact match only.** A reader accepts only files written by its own engine version, so no field is optional
   and unknown sections are errors.

## Layout

```
offset  size  field
0       8     magic            89 4F 49 4C 0D 0A 1A 0A   ("\x89OIL\r\n\x1a\n"; catches text-mode and line-ending damage)
8       2     generation       u16 = 2
10      2     reserved         u16 = 0
12      1     versionLength    u8, 1..51
13      51    engineVersion    ASCII [0-9A-Za-z.+-], versionLength bytes, then zero bytes
64      ...   sections, in this order, each exactly once:
              META  CANV  LAYR  OFFS  PNTS  STRK  "END "
```

Bytes 0..64 are frozen for all generations. Every section is:

```
tag      4 bytes ASCII
length   u32, payload bytes (excluding padding)
payload  length bytes
padding  0-3 zero bytes to the next multiple of 4
```

### `META`: provenance (UTF-8 JSON)

Canonical JSON: the key order below, no whitespace, and numbers in shortest round-trip form. It never contains
anything host- or time-dependent.

```json
{"generator":"oilpaint-engine 2.0.0-dev.1",
 "title":"Storm Light",
 "plan":{"mixer":"ochrell-0.2","planWidth":600,"seed":1907,"portable":true},
 "inputs":{"sceneplan":"sha256:9f2c…","fields":{"sky_flow":"sha256:…"},"hooks":["filter:keep-long@1"]},
 "layers":["Toned ground","Ebauche dark masses"],
 "regions":["ground","rain","sky"]}
```

- `plan` is `null` for stroke lists made directly with the Level-1 painter API.
- `inputs.sceneplan` is the SHA-256 of the ScenePlan's canonical JSON.
- `inputs.fields` hashes each sampled field (the raw little-endian f32 array plus its width and height).
- `inputs.hooks` lists hook identifiers.
- `plan.portable` is false when hooks or sampled fields were used.
- `layers[i]` and `regions[i]` name the ids used in `LAYR` and `STRK`.

### `CANV`: canvas (24 bytes)

| Field | Type | Meaning |
|---|---|---|
| aspectW, aspectH | u32, u32 | aspect ratio as integers, for example 4, 5. At width W the height is `H = (2*W*aspectH + aspectW) / (2*aspectW)` in integer arithmetic (round half up). |
| groundR, groundG, groundB | f32 x 3 | canvas colour, sRGB [0, 1], encoded by the painting mixer |
| reserved | u32 | 0 |

### `LAYR`: layer table (24 bytes per layer)

| Field | Type | Meaning |
|---|---|---|
| start, end | u32, u32 | stroke index range `[start, end)`. Ranges are contiguous, increasing and cover every stroke. |
| flags | u32 | bit 0: blur the height plane before this layer (needed by scumble); bit 1: `dryAfter` is present |
| hblurSigma | f32 | height-blur sigma in cw (bit 0) |
| dryAfter | f32 | wetness factor in [0, 1] applied after the layer (bit 1) |
| reserved | u32 | 0 |

L11 (wet/tacky/dry stages) replaces `dryAfter` with per-style open times. That is an engine-version change like
any other.

### `OFFS`: point offsets (`(n + 1) x u32`)

Stroke i has points `[offs[i], offs[i+1])`. `offs[0] = 0`, offsets are non-decreasing, `offs[n]` is the point
count, and every stroke has at least 2 points.

### `PNTS`: points (16 bytes per point)

`x, y, w, p` as f32: position in cw, full brush width in cw (> 0), pressure in [0, 1]. L10 appends twist and speed
columns.

### `STRK`: stroke records (144 bytes per stroke)

All fields are 4 bytes. `u32` fields are marked; the rest are f32.

| # | Field | Range | Meaning |
|---:|---|---|---|
| 0 | layer (u32) | < layer count | layer id; the stroke must lie inside that layer's range |
| 1 | region (u32) | < region count, or `0xFFFFFFFF` | region id; none for Level-1 strokes |
| 2 | seed (u32) | any | per-stroke seed of the counter RNG |
| 3 | mode (u32) | 0-3 | 0 paint, 1 scumble, 2 smudge, 3 glaze |
| 4-6 | color | [0, 1] | main load, sRGB |
| 7-9 | color2 | [0, 1] | second load for `marble` (equals `color` when there is none) |
| 10 | streakAmount | ≥ 0 | scales the per-bristle colour variation. The painter derives the streak direction from `color` with the painting mixer. |
| 11 | opacity | [0, 1] | |
| 12 | pickup | [0, 1] | share of wet paint picked up from the canvas |
| 13 | load | ≥ 0 | paint on the brush at the start |
| 14 | deplete | ≥ 0 | load lost per unit length |
| 15 | vdry | > 0 | how fast the tail dries out |
| 16 | hgain | ≥ 0 | paint thickness |
| 17 | flatten | [0, 1] | share of the surface the stroke replaces |
| 18 | streak | ≥ 0 | streak strength |
| 19 | hardness | [0, 1] | edge crispness |
| 20 | grain | ≥ 0 | canvas-grain catch |
| 21 | dryThresh | ≥ 0 | load below which the brush skips |
| 22 | dryWidth | > 0 | width of the dry transition |
| 23 | nb (u32) | 2-68 | bristle lanes |
| 24 | dropout | [0, 1] | |
| 25 | ragged | ≥ 0 | outline raggedness |
| 26 | body | [0, 1] | paint body between lanes |
| 27 | release | [0, 1] | |
| 28 | streakMix | [0, 1] | |
| 29 | ridge | ≥ 0 | lane ridge relief |
| 30 | levee | ≥ 0 | edge levees |
| 31 | furrow | ≥ 0 | centre trough |
| 32 | blob | ≥ 0 | start blob |
| 33 | stiff | ≥ 0 | multi-scale roughness |
| 34 | marble | [0, 1] | two-colour load share |
| 35 | splay | [0, 3] | stray hairs |

The bristle pattern depends only on `seed`, the lane and the sample index along the stroke (a counter-based hash).
The sample count comes from the stroke's length in brush widths, measured from the cw points, so a stroke draws the
same bristles at every canvas size.
v1's `allow_mask` and `override_p` were never used by the kernel and are dropped, together with the canvas region
plane. L7, L10, L11 and L12 append fields: `edgeTaper`, `displace`, `flow`, `smoothPath`, `brushId`, `shape`,
blade parameters, and so on.

### `END `: integrity (32 bytes)

The SHA-256 of every byte before the `END ` tag. A mismatch is `CORRUPT_STROKELIST`.

## Reading

In order:

1. Fewer than 64 bytes: `CORRUPT_STROKELIST`.
2. The file starts with `PK\x03\x04`: `V1_STROKE_FILE`, with fix "paint it with git tag v1-python-c".
3. The magic differs: `NOT_A_STROKELIST`.
4. The generation is not 2: `UNSUPPORTED_GENERATION`.
5. The engine version differs from the reader's: `ENGINE_VERSION_MISMATCH`, carrying both versions and the fix
   (see `ERRORS.md`).
6. The section order, lengths and final hash are checked: `CORRUPT_STROKELIST`.
7. Values are checked against the ranges above; the reader reports every violation with its path, for example
   `/strokes/812/opacity`: `INVALID_STROKELIST`.

## Painting semantics (summary; the kernel defines the details)

For each layer in order:

1. If flag bit 0 is set, blur the height plane with `hblurSigma x W` pixels.
2. Paint strokes `start..end` in order.
3. If flag bit 1 is set, multiply wetness by `dryAfter`.

Each stroke encodes `color`, `color2` and its streak variants once with the painting mixer, then runs the brush
along the points scaled by W.

## Size

Storm Light has about 14,000 strokes and 150,000 points (*projected* from the v1 list), so the file is about 2.4 MB of
points, 2.0 MB of records and 1 KB of metadata. gzip typically halves it.
