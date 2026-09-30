# Research notes

Background research for the roadmap, written by Sonnet research agents on 2026-09-30 at Sean's request after L2.
They are inputs, not decisions: the plan (`docs/plans/LIBRARY_PLAN.md`) records what is adopted.

| File | What |
|---|---|
| [`realism_market_science.md`](realism_market_science.md) | What makes a render read as a photo of a real oil painting: products (Rebelle, Painter, ArtRage, Fresco, ...), papers (relief lighting, gloss, glazing, canvas, learned painters, perception), painters' practice, the photographic stage, licences and patents |
| [`krita_ideas.md`](krita_ideas.md) | Krita's and libmypaint's paint engines, described in our own words (Krita is GPL: ideas only, no code), compared with our kernel |

**How far to trust them.**
- Every claim is tagged: verified from the source ([V], [doc], [src]), seen only in a search snippet ([S]), forum ([forum]) or inference ([I]).
- Checked after the fact:
  - The Krita report's two concrete claims about `crates/oil-kernel/src/brush.rs` (smudge moves no relief; pick-up rates are per segment, depletion per length): correct.
  - The market report's Storm Light darks (darkest 0.1% at L* 19.3): confirmed at 19.5-20.6 on a 2400 px render.
  - Its "about 5% of pixels shift > 20/255 under the light" is 12.6% at 2400 px.
- **Patents.** Two Adobe patents cited (US 8,462,173 and US 8,599,213, both active until 2031) claim a brush with a
  reservoir buffer and a pick-up buffer that deposit onto the canvas. That resembles our lane load plus "dirt". Only
  automated summaries of the claims have been read. Have the full claims reviewed by a patent professional before
  anything is published (L6).
