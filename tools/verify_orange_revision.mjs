import fs from "node:fs";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
const before = JSON.parse(fs.readFileSync("docs/reports/orange-study/painting.oil-author.json"));
const after = JSON.parse(fs.readFileSync("docs/reports/orange-study-2/painting.oil-author.json"));
assert.deepEqual(
  JSON.parse(fs.readFileSync("docs/reports/orange-study/report.json")).studyLight,
  JSON.parse(fs.readFileSync("docs/reports/orange-study-2/report.json")).studyLight,
);
assert.equal(before.engine, after.engine);
assert.deepEqual(before.catalog, after.catalog);
assert.deepEqual(before.aspect, after.aspect);
assert.deepEqual(before.ground, after.ground);
const geometry = (d) => d.groups.find((g) => g.id === "orange-ground").strokes.map(({ color, ...s }) => s);
assert.deepEqual(geometry(after), geometry(before));
assert.deepEqual(
  after.groups.find((g) => g.id === "accents").strokes.find((s) => s.id === "crown-hollow"),
  before.groups.find((g) => g.id === "accents").strokes.find((s) => s.id === "crown-hollow"),
);
const sha = (b) => createHash("sha256").update(b).digest("hex");
const protectedFiles = fs
  .readdirSync("docs/reports/orange-study")
  .map((n) => "docs/reports/orange-study/" + n)
  .concat("studies/orange-study.ts");
for (const p of protectedFiles) {
  const r = spawnSync("git", ["show", "a908b2e:" + p], { maxBuffer: 8 * 1024 * 1024 });
  assert.equal(r.status, 0, p);
  const normalize = (b) => (/\.(md|ts|json|html)$/.test(p) ? Buffer.from(b.toString().replace(/\r\n/g, "\n")) : b);
  assert.equal(sha(normalize(r.stdout)), sha(normalize(fs.readFileSync(p))), p + " changed");
}
fs.writeFileSync(
  "docs/reports/orange-study-2/preservation.json",
  JSON.stringify(
    {
      base: "a908b2e",
      originalFilesUnchanged: protectedFiles,
      underpaintingGeometryUnchanged: true,
      catalogUnchanged: true,
      lightingUnchanged: true,
      changes: "shadow strokes; underpainting colors; form and highlight strokes",
    },
    null,
    2,
  ) + "\n",
);
console.log("PASS: original study and catalog preserved; approved underpainting geometry unchanged");
