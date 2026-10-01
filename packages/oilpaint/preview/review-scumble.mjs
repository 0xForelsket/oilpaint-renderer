// Native quick pass, or native/Node/browser review with a saved dev.5 comparison.
import fs from "node:fs";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { chromium, firefox } from "playwright";
import { loadEngine } from "../src/engine.ts";
import { createAuthor, defaultView } from "../src/author.ts";
import { encodePng } from "../src/png.ts";
import { scumbleCases, stableBrushCases } from "./scumble-cases.js";
const out = "docs/reports/scumble-refinement";
fs.mkdirSync(out, { recursive: true });
const engine = await loadEngine();
const author = createAuthor(engine);
const hash = (b) => createHash("sha256").update(b).digest("hex");
const normalize = (b) => Buffer.from(b.toString().replace(/\r\n/g, "\n"));
const oldCatalog = spawnSync("git", ["show", "c4a8cd3:crates/oil-author/catalog.json"]);
assert.equal(oldCatalog.status, 0);
assert.equal(
  hash(normalize(oldCatalog.stdout)),
  hash(normalize(fs.readFileSync("crates/oil-brush/catalog.json"))),
  "catalog definitions changed",
);
function native(document, width, before = false, mixer = "ochrell", view = true) {
  const d = { ...document, engine: before ? "2.0.0-dev.5" : engine.version };
  const req = [
    { op: "render", document: d, width, mixer },
    { op: "hashes" },
    ...(view
      ? [
          { op: "view", view: defaultView },
          { op: "view", view: { ...defaultView, mode: "unlit" } },
          { op: "view", view: { ...defaultView, mode: "height" } },
        ]
      : []),
  ];
  const r = spawnSync(
    before ? "out/scumble-refinement/baseline/author-dev5.exe" : "target/release/examples/author.exe",
    [],
    { input: req.map((v) => JSON.stringify(v)).join("\n") + "\n", encoding: "utf8", maxBuffer: 60 * 1024 * 1024 },
  );
  assert.equal(r.status, 0, r.stderr);
  const results = r.stdout
    .trim()
    .split("\n")
    .map((v) => JSON.parse(v));
  for (const x of results) assert.ok(!x.result.error, x.result.error);
  return results;
}
const save = (name, r) =>
  fs.writeFileSync(`${out}/${name}.png`, encodePng(r.result.width, r.result.height, Uint8Array.from(r.pixels)));
const cases = scumbleCases(author);
const browsers = [];
const stable = [];
for (const c of stableBrushCases(author))
  for (const mixer of ["rgb", "ochrell"]) {
    const a = native(c.document, 384, true, mixer, false),
      b = native(c.document, 384, false, mixer, false);
    assert.deepEqual(a[1].result, b[1].result, c.id + " changed on " + mixer);
    author.render(c.document, 384, mixer);
    assert.deepEqual(author.hashes(), b[1].result, c.id + " differs in Node");
    stable.push({ preset: c.id, mixer, planes: b[1].result });
  }
// The accepted orange is a non-scumble integration control, including multiple brush types and drying.
const orange = JSON.parse(fs.readFileSync("docs/reports/orange-study-2/painting.oil-author.json"));
assert.deepEqual(
  native(orange, 384, true, "ochrell", false)[1].result,
  native(orange, 384, false, "ochrell", false)[1].result,
  "accepted orange changed",
);
if (!process.argv.includes("--native-only"))
  for (const [name, runtime] of [
    ["chromium", chromium],
    ["firefox", firefox],
  ]) {
    const browser = await runtime.launch();
    const page = await browser.newPage();
    await page.goto("http://127.0.0.1:4173/packages/oilpaint/preview/blank.html");
    await page.evaluate(async () => {
      const { loadEngine } = await import("/packages/oilpaint/src/engine.ts");
      const { createAuthor } = await import("/packages/oilpaint/src/author.ts");
      window.author = createAuthor(await loadEngine());
    });
    browsers.push({ name, browser, page });
  }
const evidence = [];
try {
  const substrate = { ...cases[1].document, groups: cases[1].document.groups.slice(0, 1) };
  const oldSub = native(substrate, 768, true),
    newSub = native(substrate, 768);
  assert.deepEqual(oldSub[1].result, newSub[1].result, "comparison substrate changed");
  save("substrate", newSub[2]);
  for (const c of cases) {
    fs.writeFileSync(`${out}/${c.id}.oil-author.json`, JSON.stringify(c.document, null, 2) + "\n");
    for (const width of [384, 768]) {
      const old = native(c.document, width, true),
        n = native(c.document, width);
      for (const [i, mode] of ["studio", "unlit", "height"].entries()) {
        save(`${c.id}-${width}-before-${mode}`, old[i + 2]);
        save(`${c.id}-${width}-after-${mode}`, n[i + 2]);
      }
      author.render(c.document, width);
      assert.deepEqual(author.hashes(), n[1].result);
      const expected = hash(Uint8Array.from(n[2].pixels));
      assert.equal(hash(author.view(defaultView).data), expected);
      const times = {};
      for (const b of browsers) {
        const r = await b.page.evaluate(
          async ({ d, width, view }) => {
            const a = window.author;
            const t = performance.now();
            a.render(d, width);
            const ms = performance.now() - t;
            const h = a.hashes();
            const image = a.view(view).data;
            const sha = Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", image)))
              .map((v) => v.toString(16).padStart(2, "0"))
              .join("");
            return { planes: h, image: sha, paintMs: ms };
          },
          { d: c.document, width, view: defaultView },
        );
        assert.deepEqual(r.planes, n[1].result);
        assert.equal(r.image, expected);
        times[b.name] = r.paintMs;
      }
      evidence.push({ id: c.id, width, planes: n[1].result, image: expected, times });
      console.log(c.id + " " + width + " passes");
    }
  }
  fs.writeFileSync(
    `${out}/verification.json`,
    JSON.stringify(
      {
        engine: engine.version,
        hosts: ["native", "Node", ...browsers.map((b) => b.name)],
        stableBrushes: stable,
        acceptedOrangeUnchanged: true,
        substrateUnchanged: true,
        substrate: newSub[1].result,
        evidence,
      },
      null,
      2,
    ) + "\n",
  );
} finally {
  for (const b of browsers) await b.browser.close();
}
