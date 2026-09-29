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

## Library plan (v4): what the new engine will ship

Nothing below is published yet. See `docs/plans/LIBRARY_PLAN.md`, section 10.
- Ochrell (default mixer): code MIT OR Apache-2.0. Its generated optical tables are CC BY-SA 4.0 (derived from
  CIE 2019 data), so any build with the default mixer is "MIT AND CC-BY-SA-4.0" and must carry the attribution
  listed in `../ochrell/data/README.md`. The publishing choice is Sean's (plan section 10, options A-D).
- Mixbox: only in the opt-in `oil-mix-mixbox` crate and `@oilpaint/mixbox` package (CC BY-NC 4.0, non-commercial).
