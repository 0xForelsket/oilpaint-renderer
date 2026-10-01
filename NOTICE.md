# Third-party notice

## Ochrell integration (optional)

`--mixer ochrell` uses the independent sibling Ochrell library through a local
Rust path dependency. Ochrell original code is MIT OR Apache-2.0; its attributed
CIE data and derived numerical assets are CC BY-SA 4.0. Retain Ochrell's LICENSE,
LICENSE-MIT, LICENSE-APACHE, data/README.md, data/COLOUR-LICENSE and coefficient
provenance when distributing this integration. This does not relicense the
existing renderer. Its unresolved original-code license noted below still
applies to release planning.

No Mixbox artifacts or Spectral.js coefficients are acquired by this integration.
The bridge compiles the existing bristle geometry with its old Mixbox decoder
excluded. The old backend code and its original notices remain in this repo.
`ochrell-roundtrip` and `ochrell-srgb` are diagnostic controls, not new physical
paint models. Display RGB tube names do not identify measured commercial paints.

Colour mixing uses **Mixbox** (c) 2022 Secret Weapons (Sochorova & Jamriska, "Practical Pigment Mixing for Digital
Painting", SIGGRAPH Asia 2021), installed from PyPI as `pymixbox`, licensed **CC BY-NC 4.0**
(https://scrtwpns.com/mixbox). It is free for personal / non-commercial use only. Paintings rendered with the
default `--mixer mixbox` are therefore for non-commercial use; every `run.json` records this attribution.
`--mixer rgb` (plain RGB interpolation) does not use Mixbox and has no such restriction, but looks much flatter.

The renderer's own code was written for this project with Claude (Anthropic). No licence has been chosen for it yet:
add a LICENSE file before sharing the repo.

## Spikes (`spikes/`, evidence only)

- `spikes/mixers/helpers.c`, `helpers.h`: taken from **libmypaint** (c) Martin Renold and contributors, ISC licence
  (`spikes/mixers/COPYING_libmypaint`). Used only to test its spectral mixer against Mixbox; the plan does not ship it.
- `spikes/mixers/spectral_data.json`: data tables from **spectral.js 3.0.0**, MIT licence
  (`spikes/mixers/LICENSE_spectral.js`). Used by the KM-12 mixer spike only. The v3 plan's open mixer was built on
  these tables; the current plan (v4) uses Ochrell instead and does not ship them.

## The new engine (`crates/`, library plan v4)

Nothing here is published yet; packaging follows `docs/plans/LIBRARY_PLAN.md`, section 10. The engine's own code is
written for this project (MIT is the planned licence; no LICENSE file yet).

- **Ochrell** (default mixer, `crates/oil-mix`), a Cargo git dependency pinned by commit: code MIT OR Apache-2.0.
  Its generated optical tables (`src/optical_generated.rs`) are CC BY-SA 4.0, derived from CIE 2019 data, although
  Cargo's metadata lists only the code licence. Any build with the default mixer is "MIT AND CC-BY-SA-4.0" and must
  carry the attribution in `../ochrell/data/README.md`. Sean chose to ship it as is (plan section 10, option A):
  the package licence will be `(MIT AND CC-BY-SA-4.0)`, with the CIE attribution and licence text alongside.
- **Mixbox** (`mixbox` crate 2.0.0, CC BY-NC 4.0, non-commercial): only in the opt-in `oil-mix-mixbox` crate, the
  CLI's `mixbox` feature, the Python-harness shim (`crates/oil-shim`) and the cross-host test build
  (`crates/oil-xhost`); never in a default build. Later only in the `@oilpaint/mixbox` package.
- **Other crates** (checked with `cargo metadata` in L1 and L2): serde, serde_json, sha2 and its RustCrypto
  helpers, png, flate2, miniz_oxide, crc32fast, fdeflate, simd-adler32, zlib-rs, adler2, memchr, itoa, libm (via
  mixbox), bitflags, generic-array, typenum, libc, cfg-if, cpufeatures, version_check, proc-macro2, quote, syn,
  unicode-ident, zmij; from L2 (ScenePlan schema and errors) schemars and schemars_derive (MIT),
  serde_path_to_error, serde_derive_internals, dyn-clone, ref-cast and ref-cast-impl (MIT OR Apache-2.0). All MIT,
  Apache-2.0, Zlib, Unicode-3.0, Unlicense or 0BSD (as alternatives or combinations). No GPL.
- **The TS package** (`packages/oilpaint`) has no runtime dependencies. Its SHA-256 and PNG encoder are written for
  this project.
- **npm dev dependencies** (never shipped): Playwright 1.58.2 (Apache-2.0), for the cross-host check;
  TypeScript 7.0.2 (Apache-2.0), for type checks; json-schema-to-typescript 16.0.0 (MIT), which generates
  `src/sceneplan.ts`, with its MIT dependencies (prettier, lodash, js-yaml, @apidevtools/json-schema-ref-parser and
  others) and argparse 2.0.1 (Python-2.0, a permissive licence); @types/node (MIT).

## Orange painting-study reference

The photograph docs/reports/orange-study/reference.jpg is Single Orange (Fruit) by Augustus Binu, CC BY-SA 3.0. Source: https://commons.wikimedia.org/wiki/File:Single_Orange_%28Fruit%29.jpg . License: https://creativecommons.org/licenses/by-sa/3.0/ . It is unchanged; the review displays it smaller. The painted interpretation and comparison in that study folder carry the same license. This asset-specific notice does not change the renderer/source-code license status.
