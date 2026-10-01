import fs from "node:fs";
import { loadEngine } from "../packages/oilpaint/src/engine.ts";
import { createAuthor, defaultView } from "../packages/oilpaint/src/author.ts";
import { editRegion } from "../packages/oilpaint/src/composition.ts";
import { encodePng } from "../packages/oilpaint/src/png.ts";
import { catalogStillLife } from "../scenes/catalog_still_life.ts";
const e = await loadEngine(),
  a = createAuthor(e),
  scene = catalogStillLife(a.catalog());
scene.spec.engine = e.version;
const out = process.argv[2] ?? "out/planner-integration/draft-01";
fs.mkdirSync(out, { recursive: true });
fs.writeFileSync(out + "/scene.sceneplan.json", JSON.stringify(scene.spec, null, 2) + "\n");
const t = performance.now();
const p = e.plan(scene, { width: 256, seed: 1907 });
console.log({ planMs: performance.now() - t, strokes: p.report.strokes, regions: p.report.regions });
fs.writeFileSync(out + "/planned.composition.json", JSON.stringify(p.document, null, 2) + "\n");
fs.writeFileSync(out + "/painting.oilstrokes", p.strokes);
fs.writeFileSync(out + "/plan-report.json", JSON.stringify(p.report, null, 2) + "\n");
for (const width of [384, 768]) {
  a.render(p.document, width);
  const i = a.view({ ...defaultView, bump: 0.65, contrast: 0.25, specular: 0.05 });
  fs.writeFileSync(`${out}/painting-${width}.png`, encodePng(i.width, i.height, i.data));
}
const edited = editRegion(p.document, "leaf", (g) => {
  for (const s of g.strokes) {
    for (const key of ["color", "color2"]) {
      const c = s[key];
      s[key] = [Math.min(1, c[0] * 1.18 + 0.04), c[1] * 0.94, c[2] * 0.7];
    }
  }
});
fs.writeFileSync(out + "/leaf-edit.composition.json", JSON.stringify(edited, null, 2) + "\n");
a.render(edited, 768);
const i = a.view({ ...defaultView, bump: 0.65, contrast: 0.25, specular: 0.05 });
fs.writeFileSync(out + "/leaf-edit.png", encodePng(i.width, i.height, i.data));
const g = e.guides(scene, { width: 384 });
fs.writeFileSync(out + "/target.png", encodePng(g.size[0], g.size[1], g.preview("target").data));
