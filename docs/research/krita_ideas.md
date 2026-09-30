# What oilpaint can learn from Krita and libmypaint (ideas only)

Research note, 2026-09-30. Read-only study; no repo file other than this report was written. Krita is GPL and is
treated as *reading material*: everything below is described in my own words as behaviour, parameters and maths. No
code from any GPL file is reproduced. libmypaint is ISC and is described the same way.

Evidence tags used throughout:

- **[doc]** verified in Krita's manual (docs.krita.org) or another official page I fetched.
- **[src]** verified by reading source on invent.kde.org (Krita) or GitHub (libmypaint). The files were read through a
  summarising fetch tool, not line by line, so exact constants I quote from Krita source are marked *(as reported)*.
- **[forum]** stated by a developer or artist in a merge request, forum thread or blog.
- **[inf]** my inference or design proposal. Nothing tagged [inf] was tested; no oilpaint number in this note is measured.

Costs: **S** = up to 1 agent-day, **M** = 1-3, **L** = more than 3 (the unit LIBRARY_PLAN uses).

## 0. Licence verification

| Project | Verified licence | How verified | Consequence |
|---|---|---|---|
| Krita | GPL. README: "Krita as a whole is licensed under the GNU Public License, Version 3." Files I read (colour smudge strategies, smudge-length option data, hairy brush) carry `GPL-2.0-or-later` in their headers. | krita README (raw GitHub mirror) and file headers on invent.kde.org | No code may enter oilpaint. Reading is fine; describe behaviour only. Same rule as CLAUDE.md rule 5. |
| libmypaint | ISC. README: "License: ISC, see COPYING". COPYING is the ISC text, "Copyright (C) 2008-2011 Martin Renold and contributors". | github.com/mypaint/libmypaint README and COPYING (master) | Permissive, so copying with the notice would be legal, but the plan already says ideas only. Nothing here needs it. |
| mypaint-brushes (presets) | Public domain / CC0 for the presets; installer scripts GPL-2+. | its README | Not used. Noted only because "MyPaint brushes" is sometimes assumed to share libmypaint's licence. |
| MyPaint app | GPL-2+ | mypaint-brushes README says so | Ideas only. |

The Krita manual text and forum posts are quoted only in very short fragments or paraphrased.

## 1. Where oilpaint stands today (from brush.rs, oil-light, the plans)

Read for this comparison: `crates/oil-kernel/src/brush.rs`, `planes.rs`, `crates/oil-light/src/lib.rs`,
`docs/plans/LIBRARY_PLAN.md` (v4) and `ENGINE_PLAN_v1.md`.

Facts that matter for the comparisons below (all read directly from the code):

- **Deposit:** per pixel, `alpha = coverage * lane weight * pressure * opacity * loadf`, where `loadf = clamp(load / vdry)`.
  The canvas state moves toward the brush paint: `state += alpha * (paint - state)`. Pressure only scales alpha.
- **Pick-up:** each pixel is assigned to its dominant lane. Per segment, each lane averages the canvas state and wetness
  under it. The lane's tip colour moves toward that average at a rate of `0.6 * wetness`, and "dirt" moves toward
  `pickup * wetness`. Both updates happen **per segment, not per unit length**. Load depletion, by contrast, is
  per length (`deplete * len / wref`).
- **Height (paint mode):** `h += a*flatten*(local_mean + new_relief - h) + a*(1-flatten)*0.5*new_relief`. The relief is
  built from lane ridges, edge levees, a centre furrow, a start blob and stiff-paint noise.
- **Scumble:** alpha is multiplied by a smoothstep of (height minus blurred height) against `dry_thresh`. It has no
  effect on flat, bare canvas.
- **Smudge:** deposits the lane-weighted tip colours; height is only *flattened* toward the local mean, never moved.
- **Canvas tooth:** the weave exists only in `oil-light` (`weave`, attenuated by `exp(-h/h0)` so thick paint hides it). The
  kernel's `grain` is a per-pixel hash seeded by the stroke seed, so it is not locked to the canvas. Note that
  `weave()` band-limits to fine noise when the thread pitch is below 2.2 px, which with 1290 threads across is true for
  any canvas narrower than about 2840 px. [inf, from the code]
- **Planes:** state (Ochrell 85 floats), rgb, height, wet, cover, blurred height: 372 B/px with Ochrell.
- **Planned (LIBRARY_PLAN):** F0 layered film spike, L7 settling paint, L11 wet/tacky/dry stages and KM glaze film,
  L10 brush memory plus flat/filbert/round plus twist and speed, L12 palette knife.

## 2. Feature study

### 2.1 The Color Smudge engine

**How it works.**

- **Two rates. [doc]** Since Krita 5.0 (the "new algorithm") the engine separates *Smudge Length* (how strongly the
  previous dab's content is carried) from *Colour Rate* (how much of the foreground colour is added to the mix). Before
  5.0 a low smudge length distorted the colour rate, and old behaviour was roughly "final = x% foreground + (100-x)%
  canvas" with opacity tied to smudge length. [forum: developer post "New Colorsmudge Engine for Krita 5.0" and the
  older "Smudge Brush Question(s)" thread]
- **Order per dab. [src]** First the smudged content is composited at the smudge rate, then the paint colour is layered
  on top at the colour rate. Reported constants: colour-rate opacity is the colour rate squared times the overall opacity;
  dulling opacity is 0.8 times smudge rate times opacity *(as reported)*. Both are then modulated by the brush mask, so
  the tip shape and any texture modulate how hard each pixel is smudged. [inf for the texture consequence]
- **Smearing mode. [doc][src]** Each dab reads the canvas from where the *previous dab* was (source rectangle = destination
  rectangle shifted by the previous-to-current dab vector) and blends it in at the smudge rate. The first dab of a stroke
  has nothing to read, so nothing is smudged. The net effect is that the canvas *content itself* is dragged along the
  stroke by one dab spacing per dab, with its structure intact: an edge is pulled into a streak, brush texture shapes
  the streak. The manual calls it "very impasto oil feel" and advises low spacing. A "Smear Alpha" switch decides whether
  transparency is copied or composited over.
- **Dulling mode. [doc][src]** The engine averages the canvas colour over a neighbourhood (Smudge Radius, a percentage of
  brush size, up to 300%), fills the dab with that single colour, then applies paint colour. One averaged colour has no
  structure, so it "dulls strong colours" and smooths; the manual advises large spacing for it.
- **Overlay. [doc]** Sample the merged image instead of just the current layer. Off by default, incompatible with Paint
  Thickness.
- **Precision/speed. [forum]** The new engine works in 16-bit integer and was optimised with SIMD; an earlier bug divided
  smudge radius by 100.
- **What I could not confirm.** How "smudge length" turns into a rate and whether spacing is compensated. The manual only
  says smudging is "greatly affected by Spacing and Opacity".

**Visual effect.** Smearing gives dragged, streaky, structure-preserving wet paint. Dulling gives soft, neutralised
blending.

**Krita artists' view of what is missing. [forum]** The most requested feature is a "dirty brush": picked-up colour that
persists between strokes and slowly drains back to the loaded colour. Krita does not have it; artists approximate it
with a low colour rate.

**Texture interaction. [doc]** Texture is listed as a standard option of the engine but the manual gives no smudge-specific
behaviour. Community bundles reduce texture strength (about 60%) for "a less dense brush". [forum]

**oilpaint equivalent.**

- *Dulling* is roughly what lane pick-up already does: each bristle averages the canvas under itself.
- oilpaint's per-lane tip colours and "dirt" are **richer than Krita's smudge**: lane-wise streaks, carry and release, and
  wetness weighting.
- The planned brush memory (L10, reservoirs) is Krita's most-requested missing feature.
- *Smearing* has no equivalent. oilpaint smudge moves colour through a per-lane average, not the canvas content, and
  it never moves height (`h` is only flattened toward the local mean).
- Colour rate as an independent knob: oilpaint has `opacity` (alpha) and `pickup`; the deposit is a lerp toward
  `zload + dirt*(ztip - zload)`, which is close to a colour-rate/smudge-rate split already.
- Krita and libmypaint both have spacing-dependent smudge rates. **oilpaint has the same flaw** in its per-segment pick-up
  (see idea 5).

**Ideas.** Idea 1 (smear transport), idea 3 (load life cycle), idea 5 (distance-based rates). Minor: a *wide-radius*
neutralising pick-up (Krita's dulling radius) would need a low-resolution mean-state plane; M, low gain, not ranked.

---

### 2.2 Paint Thickness and the lightness-map tip (Krita's height-based smudging)

**How it works.**

- **Tip as relief. [doc][src]** In *Lightness Map* tip mode (4.3+), the tip image keeps its transparency and takes the
  foreground colour, and its lightness is preserved: the deviation from mid-grey is read as height (dark = dip, light =
  bump). Three sliders shape it: Neutral Point (which grey counts as flat), Brightness, Contrast.
- **Height store. [doc][src]** In the colour smudge engine (5.0+), the relief is not painted into the colour. It goes
  into a **separate 8-bit RGB height layer** ("interstroke data") that exists only until the brush, layer or tool changes.
  The colour is kept in its own plane so that smudging never drags highlights and shadows around (the manual says why:
  otherwise everything smudges toward white or black).
- **Display. [src]** The visible pixel is the colour with its lightness modulated by the height layer (neutral height = no
  change; higher is brighter, lower darker). That is a **baked lightness offset in a fixed direction**, not a relight.
  Paint Thickness scales the height contrast.
- **Two modes. [doc][src]** *Overwrite* (the height layer takes the stroke's relief at brush opacity, so a low thickness
  smooths old paint) and *Paint over existing* (height opacity is interpolated between the smudge rate and full by the
  thickness, so new relief blends with old).
- **Texture Height mode (5.0). [doc][forum]** The pattern is treated as a heightfield and Strength as a fill level. Unlike
  subtract, one stroke can reach full coverage. At depth 0 the dabs fade away; at 100% you get the full texture.
  *Linear Height* multiplies in to soften transitions. Photoshop's Height mode is undocumented, and Krita's version is a
  custom variant. [forum]

**Visual effect.** Stroke ends and edges that look thick, streaks with light and shadow, all lit from one fixed direction.
The community RGBA impasto bundles do the same with animated tip slices rendered from a Blender sculpt, so the light is
baked in and the effect breaks if the canvas is mirrored. [forum: Memileo brushes]

**oilpaint equivalent.** oilpaint is **ahead**: a persistent f32 height plane, a real relight, colour and relief kept
separate by construction. Krita's 8-bit height also invites banding (users see it in normal-map painting; [forum]). The
mapping between the two:

| Krita | oilpaint |
|---|---|
| Overwrite vs paint-over mixing of old and new relief | `flatten` (0.6 default): part replaces toward local mean + new relief, part adds |
| Paint Thickness / lightness strength | `hgain` times `thick` (load) |
| Tip lightness map (bristle imprint) | lane ridges, levees, furrow, blob, `stiff` noise: all procedural |
| Height texture mode | `scumble` with `dry_thresh`: relief only, no canvas tooth |

**Ideas.** Idea 2 (canvas-locked tooth, from the Height mode), idea 8 (tip relief atlas, from the lightness-map
tips). Krita's smudge does not move height either, which idea 1 addresses.

---

### 2.3 The Bristle ("Hairy") engine

**How it works. [doc][src]**

- Every pixel of the brush tip with non-zero alpha becomes one bristle; the pixel's alpha becomes the bristle's **length**.
  The tip therefore sets bristle placement, and a Density setting drops bristles at random.
- Per dab, each bristle position is transformed: rotated to the stroke angle, scaled (Scale option), randomly offset
  (Random Offset), and sheared in proportion to pressure (Shear). The bristle then draws an anti-aliased line from its own
  previous position to the new one.
- **Ink depletion:** each bristle has an ink amount that falls along a user curve indexed by how much it has been used.
  Depletion can lower **opacity** or **saturation** (the colour is desaturated toward grey). Weights can include
  pressure, bristle length and the ink amount ("Use weights").
- **Threshold:** bristles shorter than a pressure-derived threshold do not paint: light pressure uses only the long
  bristles, heavy pressure recruits the rest. The heightmap-tip tutorial says the darker areas of the tip are drawn first and
  the lightest only at very high pressure. **Mouse Pressure** maps the Scale to drawing speed when there is no tablet.
- **Soak Ink:** bristle colours are sampled from the canvas under the brush when painting begins, but only from the
  unscaled centre area, only one layer, and transparent sources give black; it cannot soak *and* paint at once.
- **Connect Hairs** links neighbouring bristle ends; **Composite Bristles** off means "darken only" per pixel.

**Result and shortcomings. [doc][forum]** Ribbon-like, ink-like strokes that suit expressive work. Krita's own blog calls
the results expressive rather than realistic, and users report jagged edges, slow drawing at high densities, and the
soak-or-paint limit. There is no relief and no continuous pick-up along the stroke.

**oilpaint equivalent.** Ahead on nearly everything: per-lane Gaussians, wobble, ragged ends, dropouts, splay hairs,
streak colours, marbled loads, continuous per-lane pick-up, and relief. What Krita has that oilpaint lacks: **bristle length
set by a tip map so that pressure recruits bristles**, and **speed as a proxy for pressure/spread**. In oilpaint,
`pres` only multiplies alpha, so a light stroke is uniformly transparent instead of narrower and streakier.

**Ideas.** Idea 4 (reach map: pressure recruits lanes; also gives round/filbert/flat cross-profiles). Depletion in
"saturation mode" is a crude stand-in for what a thin film does optically; see idea 6.

---

### 2.4 Tip modes, gradient map and texture options

**How they work. [doc]**

- **Lightness Map** (tip): see 2.2. **Gradient Map** (tip, 4.4+): the tip's lightness indexes a gradient (dark = left end,
  white = right end), which gives coloured stamps with changeable colours.
- **Texture (Pattern) option:** pattern, scale, X/Y offset, random offset *per stroke*, invert, neutral point,
  brightness/contrast, and a **cutoff** (policy: disabled / pattern / brush; range of grey values that are allowed to
  matter). Modes: *multiply* (alpha multiplication; "soft feel"), *subtract* ("harsher"), *height* and *linear height*
  (5.0), lightness map and gradient map (these act on colour: Strength decides how much texture versus paint colour comes
  through), and the Photoshop-like darken/overlay/dodge/burn/hard-mix set. 5.3 adds a "soft texturing" checkbox and an
  automatic invert for the eraser.
- **Locked to the canvas or not:** the pattern is positioned in image space with a fixed offset unless the per-stroke
  random offset is on. I did not find a Photoshop-style "texture each dab" switch in Krita's manual. [doc, partly inf]
- **Opacity vs flow, painting mode [doc]:** *Build-up* applies opacity per dab; *Wash* applies opacity to the stroke as a
  whole.

**Visual effect.** Pressure-dependent breakup of the stroke over a canvas-locked grain: dry-brush skipping, with the same
tooth appearing in every pass.

**oilpaint equivalent.** `grain` is a small random alpha jitter, not canvas-locked. `scumble` breaks up only where there is
paint relief. Wash vs build-up is solved by geometry: each pixel of a stroke is owned by one segment, so a stroke never
builds up on itself along a straight line (crossings do build up, as in life). Libmypaint's "opacity linearisation" (below)
is not needed for the same reason.

**Ideas.** Idea 2. Gradient-map tips (colour from thickness) are what the F0/L11 film will do properly; not a separate idea.

---

### 2.5 Height and lighting in Krita

**What exists.**

- **Phong Bumpmap filter. [doc][src]** Height in, lit image out. Normals come from neighbouring height differences with a fixed
  vertical component (reported as 8) *(as reported)*, then normalised. Lighting is ambient + Lambert diffuse + Phong
  specular, with several selectable lights (azimuth and inclination), summed additively and clamped. A "Normal Map"
  checkbox uses the input as a normal map instead.
- **Height to Normal Map filter. [doc]** Simple, Prewitt or Sobel kernels (Prewitt stronger, Sobel subtler), radius scaling
  the strength, and an axis swizzle for MikkT vs OpenGL conventions.
- **Tangent Normal brush engine. [doc][forum]** Paints normal-map colours from stylus tilt, drawing direction or rotation.
  The manual does not mention impasto. Users report banding at 8 bits and trouble blending.
- **Blend modes. [doc][forum]** 5.2 adds a Lambert shading blend mode; a "fake PBR" draft (paint grey around 50% neutral)
  was submitted in 2022. Both are baked lightness offsets.
- **Artists' impasto tricks. [forum]** (a) A *clone layer* over the paint with a Phong Bumpmap filter mask, height taken from
  the paint layer's **alpha**, blended over the paint. Limits: height is locked to alpha, flat colour has no height, works
  best with textured brushes. (b) RGBA tip bundles (light baked in). (c) The *satin* layer style to get a "liquid" look
  on thick paint. (d) An unsharp-mask filter layer so textures "pop". (e) Combine Normal Map / Normalize layers for
  authoring normal maps.

**oilpaint equivalent.** `oil-light` is more advanced: two-scale normals, weave height attenuated by the paint's own height,
a capped cavity term, a satin highlight faded on thin paint, and a relief tint. Krita's single fixed-direction lightness is
strictly weaker (and light cannot be changed after painting).

**Ideas.** Idea 7 (gloss from paint state: the "satin" trick done physically), plus a small optional second light (Krita's
multi-light filter): low expected gain, S, listed under minor ideas.

---

### 2.6 libmypaint (ISC)

**Dab model. [src]**

- Round dab with a radial hardness profile of two linear segments in squared radius; optional ellipse ratio and angle
  with a direction filter; extra anti-aliasing when the radius is under about 3 px.
- Dabs are placed by summing three spacing rates (per basic radius, per actual radius, per second) and dropping a dab
  each time the total passes 1; a partial dab carries over to the next input event.
- **Opacity linearisation:** the stroke's alpha is converted to a per-dab alpha so that the accumulated alpha over the
  expected overlap is the requested one (alpha_dab = 1 - (1 - alpha)^(1/dabs_per_pixel)).
- Offsets by random noise (Gaussian, scaled by radius) and by speed; snap to pixel; lock alpha; colorize; posterize (reduce
  to N levels).

**Smudge. [src][forum]**

- A smudge **bucket** holds an RGBA colour plus a "recentness". Each dab: the update factor is max(0.01, smudge length).
  Recentness decays by that factor and is compared with a threshold (0.5^smudge_length_log); when it falls below, the
  bucket resamples the canvas at the dab within a radius of dab radius times e^(smudge_radius_log), clamped to 0.2-1000
  px, and updates as old times f plus sample times (1-f), alpha-weighted. Smudge length of 1 never resamples.
- The brush colour is then mixed with the bucket colour by the smudge amount (legacy RGB mix, or the spectral mix).
- **Buckets:** 11 of them (1.5+; the settings file says 256), selectable by any input, so a rake brush can give **each
  bristle its own smudge state**. [forum: "Smudge Buckets (more of them!)"]
- The rate is **per dab**, so it depends on spacing. The developers say "relative to dab count". [forum]

**Paint mode (Pigment). [src][forum]** A spectral-upsampled *weighted geometric mean*: each colour becomes a spectrum (the
code I read uses 10-band "small" tables; the PR text says 30 channels, and I could not reconcile that), the mix is the
product of each spectrum raised to its weight, and a 0..1 slider blends it with plain linear RGB. Everything is in linear
RGB. It gives more natural yellow-plus-blue, but pure saturated primaries still do not go clean green, as the developer
notes. A cheaper "subtractive WGM in linear RGB at gamma about 2.4" variant was found to look much like the spectral one.

**Krita's use. [doc]** Krita's MyPaint engine hosts libmypaint brushes, but presets are stored as `.kpp`, not `.myb`. Krita's
own spectral-mixing attempts are draft merge requests: a port of the MyPaint mix and a port of spectral.js. The
spectral.js one reported hue drift toward green when smudging repeatedly because mixing errors accumulated in the
brush pipeline, and reviewers wanted it at layer level only. [forum] The Krita 5.3 manual's Mix blending modes list no
spectral or KM mode. [doc]

**oilpaint equivalent.** oilpaint's per-lane tip state *is* a per-bristle smudge bucket, and Ochrell's 41-band K/S mixer
replaces the WGM. The Krita drift bug is a caution that oilpaint already avoids: the canvas keeps the mixer *state* (K and S),
and only the display colour is decoded, so there is no encode/decode round trip per dab.

**Not adopted:** WGM spectral mixing (Ochrell is stronger: absorption and scattering are separate), posterize, opacity
linearisation, dab-count spacing.

**Ideas.** Idea 5 (distance-based rates, to avoid libmypaint's spacing flaw). From libmypaint's inputs list: separate *fine* and
*gross* speed (two filter constants) could feed L10's speed input; a smoothed speed is better than the instantaneous one
for dry-brush breakup. [inf]

---

### 2.7 Other realism tricks in Krita, and what Krita cannot do for oil

**Tricks in use. [doc][forum]**

- **Canvas texture:** a pattern layer at low opacity in overlay or multiply, a Phong Bumpmap filter mask over a canvas
  pattern, or the brush Texture option (Height/Linear Height at about 20% strength).
- **Blending brushes:** the "Wet painting" preset family uses the colour smudge engine (smearing, colour rate). The
  Basic_Wet preset plus a fade curve gives the "paints, then turns into a smudger as the paint runs out" stroke of the
  sculpt-paint tutorial. Knife-like blenders use dulling with colour rate 0.
- **Dry painting** presets do not interact with existing paint, "like tempera or acrylics". [doc]
- **Wet edges:** there is no wet-edge (watercolour-style dark rim) option; the feature has been requested. [forum]
- **Post:** unsharp mask, oil-paint or edge filters over the result.

**What Krita still cannot do. [forum, doc]** Users list: no pick-up of existing paint like real wet media (the "dirty
brush" request), no reaction of the brush to the paint's structure, no displacement, no physics-based simulation. In addition
(from the manual): height exists only inside one smudge brush session; there is no drying or wet/tacky/dry state; no
native pigment mixing in the released manual; lighting of relief is baked into colour in a single direction.

**Reading for oilpaint:** its differentiators (persistent relief, relight, per-bristle carry, KM mixing, planned drying
stages) are exactly the gaps Krita users complain about. The useful takeaways are therefore narrower: a few *transport*
and *coverage* mechanisms Krita or libmypaint has that oilpaint does not.

---

## 3. Ideas for oilpaint, in detail

### Idea 1. Smear (advection) transport for smudge, including relief  -  cost **M**  -  kernel, then knife L12

- **What:** a new transport for `MODE_SMUDGE` (and a `drag` weight for paint strokes): each pixel takes the canvas *content*
  (mixer state, height, wetness) from a point behind it along the stroke tangent, instead of only the lane-average tip
  colour. Krita's smearing mode is the model: content dragged by about one dab spacing per dab, structure kept. [doc][src]
- **Maths [inf]:** per segment, take a read-only snapshot of state, height and wetness over that segment's bounding box (a
  strip about 1.45 widths by 0.5 widths, small). For each covered pixel with deposit alpha `a`, source = snapshot at
  `p - d * t` where `t` is the segment tangent and `d = k * segment length` (k near 1, per-brush `drag_len`). Then
  `state += a * drag * (source_state - state)` and the same for height. Reading only from the snapshot makes it
  order-independent, so it stays deterministic and thread-safe. Use integer-pixel offsets with a carried sub-pixel
  remainder to avoid repeated bilinear blur.
- **Buys:** wet-in-wet blends where an existing edge is pulled into a streak; drag of ridges (today smudge only flattens
  height); sub-lane detail that a per-lane average destroys; the basis for the knife (L12).
- **Status in oilpaint:** none. Nearest: smudge mode's `ztip` and height flatten.
- **Cost drivers:** the snapshot for the 85-float state is bandwidth-heavy but the strip is small (a 60 px brush with
  0.5-width segments is about 2,600 px, about 0.9 MB per segment at 340 B/px). Needs a new appended BrushParams field
  (engine-version bump), a swatch for streak length versus `drag_len`, and a G2/threads check.
- **Risk:** blur or smearing to mush if `drag` is high; mixing of tip colour (`dirt`) and drag must be tuned so that
  smudge does not get muddy.

### Idea 2. Canvas-locked tooth in the deposit test (Krita's Height texture mode)  -  cost **S-M**  -  kernel + oil-light

- **What:** paint reaches a pixel only if the effective contact (pressure times load factor) exceeds the local surface
  height: canvas tooth plus existing paint relief, with a soft edge. Heavy pressure or a full load reaches everywhere; a dry
  or light stroke catches only the peaks. This generalises `scumble` (relief-only, constant `dry_thresh`, no pressure or
  load coupling, useless on bare canvas) and follows Krita's Height mode (pattern = heightfield, strength = fill level). [doc]
- **Why it matters:** the same tooth appears in every pass. Ground colour shows through at the same places in successive dry
  strokes, which is how real dry-brush and scumbling look. Today each stroke's `grain` breakup is independent (seed-based).
- **How [inf]:** a shared tooth field in canvas-width units (hashed value noise at a resolvable scale plus the slub noise;
  thread gratings only when the pitch is at least about 3 px). Put it in `oil-image` and use it in both the kernel and
  `oil-light`, so the paint and the light agree. Store as a u8 plane (about 0.3% of 372 B/px) or compute on the fly.
  Gate it with a per-stroke `tooth` parameter (0 for paint, 0.3-0.8 for scumble/dry-brush styles).
- **Caveat:** real thread pitch is about 0.5-2 px at 600-2400 px width, so a literal weave is pixel noise at those sizes;
  use the tooth at a scale that resolves, and check with the resolution-consistency test (G3).
- **Status:** the weave exists only in lighting; scumble uses paint relief only.

### Idea 3. A load life cycle: deposit, then thin, then drag  -  cost **S** (better with idea 1)  -  kernel

- **What:** as `loadf` falls, shift the brush from depositing to *dragging what is already there*. Krita's sculpt-paint
  recipe fades opacity along the stroke while keeping colour and smudge, so the stroke turns from a painter into a smudger.
  [doc: tutorial]
- **Today:** alpha is multiplied by `loadf`, so the tail just goes transparent, and dropouts add ragged streaks.
  `dirt` cannot exceed `pickup * wetness`.
- **How [inf]:** `dirt_target = max(pickup*wetness, (1 - loadf) * tail_mix)`, and add an alpha floor that follows
  `tail_mix` so the drag does not vanish; with idea 1, weight the drag by `(1 - loadf)`. A new `tail_mix` in BrushParams.
- **Buys:** stroke ends that blend into the underlying paint, pulled out and feathered, not faded.

### Idea 4. Pressure-recruited bristle reach map (from the Bristle engine's threshold)  -  cost **S-M**  -  L10 brush types

- **What:** give each lane a reach `r` in [0,1] (its "length"); a lane touches the canvas only when effective pressure
  exceeds `1 - r`, with a soft edge. Krita's Bristle engine skips bristles shorter than a pressure-derived threshold, and its
  tip-heightmap tutorial makes dark tip areas draw first. [doc][src]
- **Profiles [inf]:** *flat* has reach near 1 across the width with slightly shorter edge lanes; *filbert* an elliptical
  profile; *round* a peaked profile so width grows with pressure (this is the plan's "width proportional to p^gamma" without any new
  geometry). Random per-lane jitter of `r` gives natural irregularity.
- **Where:** multiply the per-lane gate into `g.gf`/`g.gr` in `prep_lanes`, which already carries per-lane gains; pressure is
  already a per-point column. `pres` would stop being a pure alpha multiplier (keep a small part).
- **Buys:** light strokes that are narrower and streaky, not merely transparent; the planned shapes with one mechanism;
  a hook for `speed`/`load` to lower effective pressure (dry-brush).
- **Status:** none (lane dropouts are random and independent of pressure).

### Idea 5. Distance-based pick-up rates  -  cost **S**  -  kernel (hygiene, engine-version bump)

- **Problem:** both libmypaint (rates per dab, so spacing-dependent) and Krita (docs: smudging "greatly affected by
  Spacing") have this. oilpaint's tip update (`0.6 * wetness`), `dirt` rise (0.6), `release` and smudge `pickup` are
  applied **per segment**, while `deplete` is already per length. A stroke resampled at another point density
  therefore smears differently. [inf, from the code]
- **Fix [inf]:** `rate_eff = 1 - (1 - rate)^(len / (wref * L0))` with `L0` equal to the planner's step (0.5 width), so
  existing planner strokes change very little and hand-authored dense strokes stop over-smearing. Uses the engine's
  own `pow`, which keeps determinism.
- **Acceptance:** replay one stroke at 1x and 2x point density; the mean colour difference should fall to near zero (today
  it is nonzero by construction).

### Idea 6. Depletion as optical film thinning (needs the F0 film)  -  cost **S** once F0 exists  -  film/L11

- **What:** Krita's ink depletion offers *opacity* and *saturation* modes; the saturation one is a crude fake of paint
  thinning. In the layered KM film, lower `loadf` should reduce the **deposited film thickness** (the layer's S and K scale),
  and the underlayer should show through *optically*, not as stippled alpha. [doc for Krita's modes; inf for the rest]
- **Buys:** dry-brush tails and thin scumbles whose colour is a physically plausible tint of the ground, with less need for
  dropout noise.

### Idea 7. A gloss plane fed by paint state (the "satin" trick done physically)  -  cost **S-M**  -  oil-light, after L11

- **What:** `oil-light` today derives gloss only from height (`gloss_h0..h1`). Fresh oil is shinier, and dry paint sinks and
  goes matte in patches. Feed `relight` an optional gloss plane written from wetness/age/thickness. Krita artists reach for
  a "satin" layer style to get a liquid look on thick paint, so the wish is real. [forum]
- **Where:** an extra u8 plane (`LightParams` reads it); the kernel or L11's stage bookkeeping writes it.
- **Buys:** wet-vs-dry sheen differences and glossy tops on fresh impasto, which the thickness-only gloss cannot separate.

### Idea 8. A tip-relief atlas (Krita's lightness-map tips)  -  cost **M-L**  -  kernel, own assets

- **What:** sample a small atlas of authored or measured bristle-imprint height profiles in brush space (across the
  stroke, along the stroke), locked to canvas-width units, as the source of the ridge and roughness terms in place of (or mixed
  with) `vnoise`. The community's impasto bundles are built from exactly such tip slices, and the manual has a
  "heightmap bristle brush tip" tutorial. [doc][forum]
- **Risks:** visible tiling; asset licence (make our own); any atlas change is an engine-version bump. Determinism is
  fine (fixed data, bilinear sampling).

### Minor ideas (not ranked)

- **Second (fill) light** option in `LightParams`, like Krita's multi-light Phong filter. S; low gain. [doc]
- **Along-stroke sensor curves** (fade/distance) for `pickup`, `hgain` and `load`, as Krita's brush options allow. S.
- **Wide-radius pick-up** (Krita's dulling radius) via a low-resolution mean-state plane. M; low gain.
- **Smoothed "gross" speed** for the L10 speed input (libmypaint has fine and gross speed). S.

---

## 4. Top 8 ideas, ranked by expected realism gain [inf: my judgement; nothing here has been tested]

| Rank | Idea | Source of the idea | Where | Cost | Why this rank |
|---|---|---|---|---|---|
| 1 | Smear (advection) transport of state and relief | Krita smearing mode | kernel; feeds L12 knife | M | Wet-in-wet blending and pulled edges are the hallmark oil look and the largest gap; smudge today moves no relief and only lane averages |
| 2 | Canvas-locked tooth in the deposit test | Krita Height texture mode | kernel + oil-light | S-M | Consistent broken colour across passes, ground peeking through; one mechanism for scumble, dry-brush, glaze; resolution caveat noted |
| 3 | Load life cycle: deposit, thin, drag | Krita sculpt-paint recipe, colour rate vs smudge rate | kernel | S | Cheap, makes every stroke end better; best after idea 1 |
| 4 | Pressure-recruited reach map | Krita Bristle threshold and heightmap tips | L10 brush types | S-M | Light strokes become streaky and narrow; gives flat/filbert/round with one mechanism |
| 5 | Distance-based pick-up rates | libmypaint/Krita spacing flaw | kernel | S | Correctness: same look at any point density; small direct visual gain |
| 6 | Depletion as optical film thinning | Krita ink depletion (opacity/saturation) | F0/L11 film | S once F0 exists | Physically plausible dry-brush tails; only if F0 goes |
| 7 | Gloss plane from paint state | Krita satin-layer trick | oil-light, after L11 | S-M | Sheen differences between fresh and dry paint; subtle |
| 8 | Tip-relief atlas | Krita lightness-map tips | kernel, own assets | M-L | More organic relief than procedural ridges, but tiling and asset risks |

Suggested order of work: 5 first (pure hygiene, do it in the next engine-version bump), then 2 and 4 with L7/L10, then 1
and 3 together, then 6 and 7 with the film and stage work.

## 5. Considered and rejected

- **Krita's baked lightness and 8-bit height store:** oilpaint has a real height plane and a relight; going the other way
  would lose the ability to change the light and would band.
- **Animated RGBA tips with baked light** (Memileo-style): the light direction is fixed in the tip. Same reason.
- **libmypaint weighted-geometric-mean spectral mixing:** Ochrell's K/S mixing is more physical.
- **Posterize:** a stylised look, not oil realism; palette snapping in the planner already covers the useful part.
- **Opacity linearisation, Wash vs Build-up:** the segment ownership rule already avoids self-overlap along a stroke.
- **Tangent Normal engine and Phong filter's fixed vertical normal component:** authoring tools for normal maps, not
  a painting model.
- **Krita's bristle line drawing:** oilpaint's lane brush already exceeds it.

## 6. Verification log and open questions

Verified from official docs [doc]: colour smudge option list and mode descriptions; Paint Thickness (two modes, separate
height map, discarded on switching); brush-tip modes (lightness map 4.3+, gradient map 4.4+); texture modes, cutoff, strength;
bristle engine options; height-to-normal filter; Phong Bumpmap "Normal Map" checkbox; Tangent Normal options; Opacity/Flow and
painting modes; the Wet/Dry preset families; MyPaint engine page; Mix blending-mode list.

Read in source [src]: Krita colour smudge strategy files (base, lightness, overlay), smudge-length option data, interstroke data,
Phong pixel processor and hairy brush (all `GPL-2.0-or-later` headers; constants marked "as reported" came through a summariser);
libmypaint `mypaint-brush.c`, `brushsettings.json`, `helpers.c`, `mypaint-tiled-surface.c`, README and COPYING.

Not verified: how Krita maps smudge length to a rate and whether it compensates spacing; whether Krita has a Photoshop-style
"texture each dab" switch (I found none in the manual); the number of spectral bands libmypaint uses in paint mode (10-band "small"
tables in the code, "30 channels" in the PR text); whether any Krita spectral-mixing merge request has been merged (the Mix blending
modes page for 5.3 lists none); licences of third-party Krita brush bundles (not used).

Nothing in oilpaint was run or measured for this note. All costs and rankings are estimates.

## 7. Sources

Krita manual and release notes
- https://docs.krita.org/en/reference_manual/brushes/brush_engines/color_smudge_engine.html
- https://docs.krita.org/en/reference_manual/brushes/brush_engines/bristle_engine.html
- https://docs.krita.org/en/reference_manual/brushes/brush_settings/brush_tips.html
- https://docs.krita.org/en/reference_manual/brushes/brush_settings/texture.html
- https://docs.krita.org/en/reference_manual/brushes/brush_settings/options.html
- https://docs.krita.org/en/reference_manual/brushes/brush_settings/opacity_and_flow.html
- https://docs.krita.org/en/reference_manual/brushes/brush_engines/tangen_normal_brush_engine.html
- https://docs.krita.org/en/reference_manual/brushes/brush_engines/mypaint_engine.html
- https://docs.krita.org/en/reference_manual/filters/map.html
- https://docs.krita.org/en/reference_manual/filters/edge_detection.html
- https://docs.krita.org/en/reference_manual/blending_modes/mix.html
- https://docs.krita.org/en/reference_manual/krita_4_preset_bundle.html
- https://docs.krita.org/en/tutorials/krita-brush-tips/sculpt-paint-brush.html
- https://docs.krita.org/en/tutorials/krita-brush-tips/bristle_from_heightmap_brush_tip.html
- https://krita.org/en/release-notes/krita-5-0-release-notes/
- https://krita.org/en/release-notes/krita-5-2-release-notes/
- https://krita.org/en/release-notes/krita-5-3-release-notes/
- https://krita.org/en/posts/2013/hairy-brushes/

Krita source, merge requests and forum (invent.kde.org and krita-artists.org)
- https://raw.githubusercontent.com/KDE/krita/master/README.md
- https://invent.kde.org/graphics/krita/-/raw/master/plugins/paintops/colorsmudge/KisColorSmudgeStrategyBase.cpp
- https://invent.kde.org/graphics/krita/-/raw/master/plugins/paintops/colorsmudge/KisColorSmudgeStrategyLightness.cpp
- https://invent.kde.org/graphics/krita/-/raw/master/plugins/paintops/colorsmudge/KisColorSmudgeStrategyWithOverlay.cpp
- https://invent.kde.org/graphics/krita/-/raw/master/plugins/paintops/colorsmudge/KisColorSmudgeInterstrokeData.cpp
- https://invent.kde.org/graphics/krita/-/raw/master/plugins/paintops/colorsmudge/KisSmudgeLengthOptionData.cpp
- https://invent.kde.org/graphics/krita/-/raw/master/plugins/paintops/colorsmudge/kis_colorsmudgeop.cpp
- https://invent.kde.org/graphics/krita/-/raw/master/plugins/paintops/hairy/hairy_brush.cpp
- https://invent.kde.org/graphics/krita/-/raw/master/plugins/paintops/hairy/bristle.h
- https://invent.kde.org/graphics/krita/-/raw/master/plugins/filters/phongbumpmap/phong_pixel_processor.cpp
- https://invent.kde.org/graphics/krita/-/merge_requests/756 (new colorsmudge engine)
- https://invent.kde.org/graphics/krita/-/merge_requests/806 (new texture blending modes)
- https://invent.kde.org/graphics/krita/-/merge_requests/1249 (spectral blending of mypaint)
- https://invent.kde.org/graphics/krita/-/merge_requests/1566 (fake PBR blending mode)
- https://invent.kde.org/graphics/krita/-/merge_requests/1997 (spectral blend as in spectral.js, draft)
- https://krita-artists.org/t/testing-needed-new-colorsmudge-engine-for-krita-5-0/22398
- https://krita-artists.org/t/smudge-brush-question-s-for-artists/6068
- https://krita-artists.org/t/new-brush-s-texture-blending-modes-height/3050
- https://krita-artists.org/t/realtime-impasto-effects-using-layers/5653
- https://krita-artists.org/t/memileo-impasto-brushes/92952
- https://krita-artists.org/t/how-to-create-thicker-paint-strokes/27331
- https://krita-artists.org/t/tangent-normal-engine/21619
- https://krita-artists.org/t/any-good-bristle-engine-brushes-and-how-use-them/53326
- https://krita-artists.org/t/paint-like-color-mixing-kubelka-munk/78156

libmypaint and MyPaint
- https://github.com/mypaint/libmypaint (README) and https://github.com/mypaint/libmypaint/blob/master/COPYING
- https://raw.githubusercontent.com/mypaint/libmypaint/master/mypaint-brush.c
- https://raw.githubusercontent.com/mypaint/libmypaint/master/brushsettings.json
- https://raw.githubusercontent.com/mypaint/libmypaint/master/helpers.c
- https://raw.githubusercontent.com/mypaint/libmypaint/master/mypaint-tiled-surface.c
- https://github.com/mypaint/libmypaint/releases
- https://github.com/mypaint/mypaint/pull/957 (linear and spectral pigment features)
- https://github.com/mypaint/mypaint-brushes
- https://community.mypaint.app/t/smudge-tweaks-testers-needed/1202
- https://community.mypaint.app/t/smudge-buckets-more-of-them/978
- https://community.mypaint.app/t/questions-about-smudge-colour-vs-smudge-length-and-eliptical-dab-angle-vs-ratio/1436

oilpaint files read
- crates/oil-kernel/src/brush.rs, crates/oil-kernel/src/planes.rs, crates/oil-light/src/lib.rs
- docs/plans/LIBRARY_PLAN.md, docs/plans/ENGINE_PLAN_v1.md, docs/STYLE_REFERENCE.md, docs/reports/L1.md (skimmed)
