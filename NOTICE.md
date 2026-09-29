# Third-party notice

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
  (`spikes/mixers/LICENSE_spectral.js`). The plan's default open mixer is built on these tables.
