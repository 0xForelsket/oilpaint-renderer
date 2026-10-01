import fs from "node:fs";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { chromium, firefox } from "playwright";
import { loadEngine } from "../packages/oilpaint/src/engine.ts";
import { createAuthor, defaultView } from "../packages/oilpaint/src/author.ts";
import {
  editRegion,
  replaceRegion,
} from "../packages/oilpaint/src/composition.ts";
import { catalogStillLife } from "../scenes/catalog_still_life.ts";
import { encodePng } from "../packages/oilpaint/src/png.ts";
const out = "docs/reports/planner-integration";
fs.mkdirSync(out, { recursive: true });
const e = await loadEngine(),
  a = createAuthor(e),
  scene = catalogStillLife(a.catalog());
scene.spec.engine = e.version;
const hash = (b) => createHash("sha256").update(b).digest("hex");
const run = (exe, input) => {
  const r = spawnSync(exe, [], {
    input,
    encoding: "utf8",
    maxBuffer: 80 * 1024 * 1024,
  });
  assert.equal(r.status, 0, r.stderr);
  return r.stdout;
};
const start = performance.now(),
  p = e.plan(scene, { width: 256, seed: 1907 }),
  nodePlanMs = performance.now() - start;
const native = JSON.parse(
  run("target/release/examples/composition.exe", JSON.stringify(scene.spec)),
);
assert.deepEqual(native.document, p.document);
assert.equal(native.strokesHash, hash(p.strokes));
for (const r of p.report.regions)
  if (r.region !== "shadows")
    assert.ok(r.covered >= 0.95, `${r.region} coverage ${r.covered}`);
const edited = editRegion(p.document, "leaf", (g) => {
  for (const s of g.strokes)
    for (const k of ["color", "color2"]) {
      const c = s[k];
      s[k] = [Math.min(1, c[0] * 1.18 + 0.04), c[1] * 0.94, c[2] * 0.7];
    }
});
const revised = structuredClone(scene.spec);
revised.styles.leaf.width = [0.02, 0.035];
revised.styles.leaf.colors = ["#8d913e", "#526e31"];
revised.styles.leaf.snap = 0.6;
const candidate = e.plan(revised, { width: 256, seed: 1907 });
const replanned = replaceRegion(p.document, candidate.document, "leaf");
for (const d of [edited, replanned])
  for (const g of p.document.groups)
    if (g.region !== "leaf")
      assert.deepEqual(
        d.groups.find((v) => v.id === g.id),
        g,
      );
const light = { ...defaultView, bump: 0.65, contrast: 0.25, specular: 0.05 };
const variants = [
  ["planned", p.document],
  ["leaf-edit", edited],
  ["leaf-replan", replanned],
];
const checks = [];
for (const [name, d] of variants) {
  const rows = [
    { op: "render", document: d, width: 384, mixer: "ochrell" },
    { op: "hashes" },
    { op: "view", view: light },
  ];
  const n = run(
    "target/release/examples/author.exe",
    rows.map((r) => JSON.stringify(r)).join("\n") + "\n",
  )
    .trim()
    .split("\n")
    .map((x) => JSON.parse(x));
  for (const r of n) assert.ok(!r.result.error, r.result.error);
  a.render(d, 384);
  const planes = a.hashes(),
    image = a.view(light);
  assert.deepEqual(planes, n[1].result);
  assert.equal(hash(image.data), hash(Uint8Array.from(n[2].pixels)));
  fs.writeFileSync(
    `${out}/${name}.composition.json`,
    JSON.stringify(d, null, 2) + "\n",
  );
  fs.writeFileSync(
    `${out}/${name}-384.png`,
    encodePng(image.width, image.height, image.data),
  );
  checks.push({ name, planes, image: hash(image.data) });
}
// Fresh replay must agree with incremental editing, including downstream pickup.
const fresh = createAuthor(await loadEngine());
fresh.render(replanned, 384);
assert.deepEqual(a.hashes(), fresh.hashes());
// A later-group change must actually reuse a prefix at a size supporting multiple checkpoints.
a.render(p.document, 128);
const late = structuredClone(p.document);
late.groups.at(-1).visible = false;
assert.ok(a.render(late, 128).reusedGroups > 0);
fresh.render(late, 128);
assert.deepEqual(a.hashes(), fresh.hashes());
const crop = (i, x0, y0, x1, y1) => {
  const bytes = [];
  for (let y = Math.floor(y0 * i.width); y < Math.ceil(y1 * i.width); y++)
    for (let x = Math.floor(x0 * i.width); x < Math.ceil(x1 * i.width); x++)
      bytes.push(
        ...i.data.subarray((y * i.width + x) * 3, (y * i.width + x) * 3 + 3),
      );
  return hash(Uint8Array.from(bytes));
};
a.render(p.document, 768);
const originalImage = a.view(light);
a.render(edited, 768);
const editImage = a.view(light);
for (const box of [
  [0.345, 0.15, 0.67, 0.49],
  [0.69, 0.24, 0.935, 0.48],
])
  assert.equal(crop(originalImage, ...box), crop(editImage, ...box));
const browsers = [];
for (const [name, runtime] of [
  ["Chromium", chromium],
  ["Firefox", firefox],
]) {
  const browser = await runtime.launch();
  try {
    const page = await browser.newPage();
    await page.goto(
      "http://127.0.0.1:4173/packages/oilpaint/preview/blank.html",
    );
    const result = await page.evaluate(
      async ({ scene, variants, light }) => {
        const { loadEngine } = await import("/packages/oilpaint/src/engine.ts");
        const { createAuthor } =
          await import("/packages/oilpaint/src/author.ts");
        const e = await loadEngine(),
          a = createAuthor(e);
        const sha = async (b) =>
          Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", b)))
            .map((v) => v.toString(16).padStart(2, "0"))
            .join("");
        const t = performance.now();
        const p = e.plan(scene, { width: 256, seed: 1907 });
        const ms = performance.now() - t;
        const frames = [];
        for (const [name, d] of variants) {
          a.render(d, 384);
          frames.push({
            name,
            planes: a.hashes(),
            image: await sha(a.view(light).data),
          });
        }
        return {
          strokesHash: await sha(p.strokes),
          document: p.document,
          frames,
          planMs: ms,
        };
      },
      { scene: scene.spec, variants, light },
    );
    assert.equal(result.strokesHash, native.strokesHash);
    assert.deepEqual(result.document, p.document);
    assert.deepEqual(result.frames, checks);
    browsers.push({ name, planMs: result.planMs });
  } finally {
    await browser.close();
  }
}
fs.writeFileSync(
  out + "/scene.sceneplan.json",
  JSON.stringify(scene.spec, null, 2) + "\n",
);
fs.writeFileSync(
  out + "/leaf-replan.sceneplan.json",
  JSON.stringify(revised, null, 2) + "\n",
);
fs.writeFileSync(
  out + "/verification.json",
  JSON.stringify(
    {
      engine: e.version,
      strokes: p.report.strokes,
      groups: p.document.groups.length,
      coverage: p.report.regions,
      strokesHash: native.strokesHash,
      planningMs: { native: native.planMs, Node: nodePlanMs, browsers },
      checks,
      nonLeafGroupsUnchanged: true,
      orangeAndStonePixelCropsUnchanged: true,
      incrementalEqualsFull: true,
      checkpointPrefixReuse: true,
    },
    null,
    2,
  ) + "\n",
);
console.log(
  "PASS: native/Node/Chromium/Firefox planning + edited replay; preserved groups and pixel crops; full/incremental equality",
);
