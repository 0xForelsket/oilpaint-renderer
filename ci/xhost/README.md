# Cross-host determinism check (xhost)

This checks the engine's guarantee (check G1 in `docs/plans/LIBRARY_PLAN.md`, section 5): **for one engine
version, the same inputs give byte-identical outputs on every host.**

```powershell
npm install                 # once; Playwright 1.58.2 uses the browsers cached in %LOCALAPPDATA%\ms-playwright
npm run xhost               # native + Node + Chromium + Firefox + WebKit on this machine, then compare
npm run xhost -- --skip-browsers
```

How it works:

1. `crates/oil-xhost` defines the **cases**: deterministic Rust functions that return bytes. The same code is built
   natively (the `xhost` binary) and to `wasm32-unknown-unknown` (C-ABI exports, no bindgen, no imports).
2. Each host runs every case and hashes the bytes with its **own** SHA-256:
   - native: the `sha2` crate;
   - Node: `node:crypto`;
   - browsers: WebCrypto, with the page served from `127.0.0.1` and the WASM loaded with `instantiateStreaming`.

   Each host writes `out/xhost/<host>.json`.
3. `compare.mjs` fails when:
   - an `identical` case has more than one digest;
   - a host misses a case;
   - hosts ran different engine versions;
   - a digest differs from `golden/<engine-version>.json`.
4. `may_differ` cases are **negative controls**: code the engine must never use (the platform libm). They show
   that the harness sees a real difference and are reported, never failing.

Diagnosing a difference: `xhost --dump-dir DIR` and `run-node.mjs --dump-dir DIR` write every case's bytes, so two
hosts can be diffed value by value.

Cases by milestone:

| Milestone | Cases |
|---|---|
| L0 | `oil-math` exp, sin/cos and expf over dense grids, plus the negative controls (platform `exp`; the sRGB transfer with platform `powf`, as Ochrell 0.2 computes it) |
| L1 | the procedural test sheet (`oil-paint`): its StrokeList bytes, and all six canvas planes (lat, rgb, h, wet, cover, amount) plus the lit image at 400 px, with the rgb and Mixbox mixers (Ochrell to follow) |
| L3 | StrokeList bytes planned from ScenePlans, and the planes they paint |

CI: `.github/workflows/determinism.yml` runs the native side on Windows, Linux and macOS and the WASM side in
Node and three browsers, then compares all seven reports.
