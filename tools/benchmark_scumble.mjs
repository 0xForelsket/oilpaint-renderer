import fs from "node:fs";
import { loadEngine } from "../packages/oilpaint/src/engine.ts";
import { createAuthor } from "../packages/oilpaint/src/author.ts";
import { scumbleCases } from "../packages/oilpaint/preview/scumble-cases.js";
const old = await loadEngine(new URL("../out/scumble-refinement/baseline/oil-dev5.wasm", import.meta.url));
const current = await loadEngine();
const a = createAuthor(old),
  b = createAuthor(current);
const input = scumbleCases(b)[0].document;
const samples = [];
for (let i = 0; i < 7; i++) {
  const pair = { seed: input.seed + i };
  for (const [name, author, engine] of i % 2
    ? [
        ["current", b, current],
        ["before", a, old],
      ]
    : [
        ["before", a, old],
        ["current", b, current],
      ]) {
    const doc = { ...input, seed: input.seed + i, engine: engine.version };
    const t = performance.now();
    author.render(doc, 384);
    pair[name] = performance.now() - t;
  }
  if (i > 0) samples.push(pair);
}
const summary = {};
for (const name of ["before", "current"]) {
  const v = samples.map((s) => s[name]).sort((a, b) => a - b);
  summary[name] = { min: v[0], median: (v[2] + v[3]) / 2, max: v.at(-1) };
}
const report = {
  reference: old.version,
  current: current.version,
  method:
    "Node WASM, 384x192, two scumble strokes on flat ground, six warmed paired seeds, alternating order; author.render including compile/replay, excluding relighting. Process affinity 0xF, other verification active.",
  samples,
  summary,
};
fs.writeFileSync("docs/reports/scumble-refinement/timings.json", JSON.stringify(report, null, 2) + "\n");
console.log(JSON.stringify(summary, null, 2));
