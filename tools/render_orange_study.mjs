import fs from "node:fs";
import { createHash } from "node:crypto";
import { loadEngine } from "../packages/oilpaint/src/engine.ts";
import { createAuthor } from "../packages/oilpaint/src/author.ts";
import { encodePng } from "../packages/oilpaint/src/png.ts";
import { orangeStudy, studyLight } from "../studies/orange-study.ts";
const engine = await loadEngine();
const a = createAuthor(engine);
const d = orangeStudy(engine.version, a.catalog());
const out = process.argv[2] ?? "out/orange-study/draft-01";
fs.mkdirSync(out, { recursive: true });
fs.writeFileSync(out + "/painting.oil-author.json", JSON.stringify(d, null, 2) + "\n");
fs.writeFileSync(out + "/painting.oilstrokes", a.compile(d));
const times = [];
for (const width of [384, 768]) {
  const t = performance.now();
  a.render(d, width);
  const paintMs = performance.now() - t;
  for (const mode of ["lit", "unlit", "height"]) {
    const i = a.view({ ...studyLight, mode });
    fs.writeFileSync(`${out}/${mode}-${width}.png`, encodePng(i.width, i.height, i.data));
  }
  times.push({ width, paintMs, planes: a.hashes() });
}
const frozen = Object.fromEntries(
  ["crates/oil-kernel/src/brush.rs", "crates/oil-brush/catalog.json"].map((p) => [
    p,
    createHash("sha256").update(fs.readFileSync(p)).digest("hex"),
  ]),
);
fs.writeFileSync(
  out + "/report.json",
  JSON.stringify(
    { engine: engine.version, strokes: d.groups.reduce((n, g) => n + g.strokes.length, 0), studyLight, frozen, times },
    null,
    2,
  ) + "\n",
);
console.log(out);
