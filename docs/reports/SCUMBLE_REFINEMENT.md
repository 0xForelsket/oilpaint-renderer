# Scumble refinement with the other six brushes held stable

2026-10-02. Implements the user's request to keep the six other brushes and address scumble separately. The orange study is a preservation control, not evidence that scumble has been resolved.

## Change and scope

Only `scumble_contact` changes rendered behavior. The surrounding paint kernel, other modes, planner, author compiler, relighting and all seven catalog definitions remain unchanged. The contact mask now combines:

- larger solid contacts with roughened borders and occasional interruptions;
- medium patches recruited within broader contact regions;
- smaller catches near those regions, rather than uniform fine spray;
- short trailing connections of 0.065 reference brush widths, reduced from 0.18;
- the existing canvas-fixed tooth and actual deposited relief, which influence contact thresholds.

Coherent domain distortion and fine contact variation break up the edges of the larger patches. This modifies pigment deposition and deposited height; no final-image texture, blur or other substitute is used. It remains a heuristic contact model, not a physical fibre simulation. The artist still needs to judge the result.

## Focused review

[Before/after PNG](scumble-refinement/review.png) · [Review gallery](scumble-refinement/index.html)

The comparison uses the same stroke geometry, seed, pigment, pressure and light at 384 and 768 px. The broad/narrow widths are 0.10 and 0.044 cw. It covers flat ground, irregular dry painted underlay, and a short crossing pass. The underlay uses differently directed and loaded marks rather than repeating blue ridges. Its full paint-plane hashes match before scumbling, so substrate changes do not confound the comparison.

Lit, unlit and fixed-scale height images, author documents and [runtime/stability evidence](scumble-refinement/verification.json) are saved with the review. Earlier studies and the accepted orange are untouched.

## Stability boundary

For each retained preset, dev.5 and dev.6 native paint planes match exactly on the same authored control in **RGB and Ochrell**; the current Node/WASM output matches those same planes. The accepted orange's planes also match exactly. These checks compare all five persistent planes, not only the final color image. A separate comparison confirms that the irregular underpainting is unchanged.

The retained six now have a persistent regression fixture in `spec/examples/retained-brushes.json`, checked by `crates/oil-author/tests/retained_brushes.rs`. It also rejects accidental changes to their catalog definitions. The reference hashes were first compared against the saved dev.5 executable. Test fixtures are explicitly stamped for the running engine; public loaders still enforce strict version matching.

A later paint stroke over **changed scumble** may of course pick up different paint. Stability means the six brushes on the same starting surface have unchanged behavior, not that physically dependent downstream pixels ignore an earlier edit.

## Version and reproduction

Engine **2.0.0-dev.6** is required because scumble pixels change. Author/catalog schema **2** is unchanged. Earlier saved files are preserved and require their matching engine; dev.5 is available at commit `c4a8cd3`. No compatibility mode or saved-file migration was added.

With the preview server running on port 4173 and the current native author example/WASM built:

```powershell
npm run review:scumble
npm run review:scumble:package
```

The comparative verifier requires the captured dev.5 author executable at `out/scumble-refinement/baseline/author-dev5.exe`. On another checkout, obtain it by building the author example at the matching dev.5 commit. The checked-in before images and verification data remain available without that executable. `--native-only` makes a faster native/Node visual pass but does not replace the complete browser verification.

## Verification interpretation

The contact-mask regression samples four seeds at 64 samples per reference brush width. It requires small, medium and larger connected components, limits isolated one/two-sample dots, and requires substantial edge/partial-contact content. These mechanical checks guard the requested size variety and prevent a return to dominant spray or only large islands; they are not artist approval.

Measured on the tested four-seed contact grid: 197 small (3–30 samples), 45 medium (31–180), and 32 larger components; isolated one/two-sample components account for 172 of 52,845 solid-contact samples. There are 14,409 boundary samples and 23,078 partial-contact samples. These are internal mask statistics, not image-quality scores or claims about real paint.


## Final checks and cost

- Workspace release tests and Clippy passed. The added retained-six integration test passed in RGB/Ochrell, and its Clippy check passed.
- TypeScript typecheck and all 16 tests passed.
- All three focused inputs at both 384 and 768 px matched native Rust, Node, Chromium and Firefox for paint planes and lit output. The review includes unlit and height images as well.
- Standard cross-host G1 passed all 33 cases on native Windows, Node, Chromium, Firefox and WebKit. `golden/2.0.0-dev.6.json` records the new engine baseline; prior goldens remain unchanged.
- All seven catalog definitions are byte-identical after line-ending normalization. The six retained brushes have twelve matching old/new controls (six presets times two mixers). The accepted orange and the irregular substrate also match dev.5 paint planes exactly.

A six-seed warmed Node/WASM comparison, alternating old/new order at 384 × 192 with two scumble strokes, measured author.render medians of **9.42 ms before / 11.03 ms after**. Ranges were 8.67–21.98 ms and 10.54–22.03 ms respectively. This includes compilation/replay and excludes relighting; processor affinity was 0xF with other verification active. It is a small observational workload, not a universal performance guarantee. [Raw paired timings](scumble-refinement/timings.json). Reproduce with `node tools/benchmark_scumble.mjs` using the saved dev.5 WASM.

The focused review remains a candidate for artistic judgment. The tests establish stable behavior and a measurable range of contact sizes; they do not decide whether the paint character is settled.
