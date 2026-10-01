// Native/WASM review, preserving earlier evidence. Run --native-only for a quick visual pass.
import fs from "node:fs";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import assert from "node:assert/strict";
import { chromium, firefox } from "playwright";
import { loadEngine } from "../src/engine.ts";
import { createAuthor, defaultView, rakingView } from "../src/author.ts";
import { encodePng } from "../src/png.ts";
import { reviewCases } from "./study-cases.js";
const out = "docs/reports/brush-review-3";
fs.mkdirSync(out, { recursive: true });
const author = createAuthor(await loadEngine());
const cases = reviewCases(author);
const previous = JSON.parse(
  fs.readFileSync("docs/reports/brush-review-2/impasto-accent-contact.oil-author.json"),
).catalog;
const views = [defaultView, { ...defaultView, mode: "unlit" }, { ...defaultView, mode: "height" }, rakingView];
const names = ["studio", "unlit", "height", "raking"];
const hash = (b) => createHash("sha256").update(b).digest("hex");
function native(d, before = false) {
  const requests = [
    { op: "compile", document: d },
    { op: "render", document: d, width: 960, mixer: "ochrell" },
    { op: "hashes" },
    ...views.map((view) => ({ op: "view", view })),
  ];
  const r = spawnSync(
    before ? "out/brush-iteration-3/baseline/author-dev4.exe" : "target/release/examples/author.exe",
    [],
    { input: requests.map((r) => JSON.stringify(r)).join("\n") + "\n", encoding: "utf8", maxBuffer: 90 * 1024 * 1024 },
  );
  if (r.status !== 0) throw Error(r.stderr);
  const result = r.stdout
    .trim()
    .split("\n")
    .map((s) => JSON.parse(s));
  for (const r of result) assert.ok(!r.result.error, r.result.error);
  return result;
}
const save = (id, r) =>
  fs.writeFileSync(`${out}/${id}.png`, encodePng(r.result.width, r.result.height, Uint8Array.from(r.pixels)));
const browsers = [];
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
  for (const c of cases) {
    fs.writeFileSync(`${out}/${c.id}.oil-author.json`, JSON.stringify(c.d, null, 2) + "\n");
    const n = native(c.d);
    const strokeHash = hash(Uint8Array.from(n[0].strokes));
    for (let i = 0; i < views.length; i++) save(c.id + "-" + names[i], n[i + 3]);
    assert.equal(hash(author.compile(c.d)), strokeHash);
    author.render(c.d, 960);
    assert.deepEqual(author.hashes(), n[2].result);
    for (let i = 0; i < views.length; i++)
      assert.equal(hash(author.view(views[i]).data), hash(Uint8Array.from(n[i + 3].pixels)));
    const hostTimings = {};
    for (const b of browsers) {
      const r = await b.page.evaluate(
        async ({ d, views }) => {
          const a = window.author;
          const digest = async (b) =>
            Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", b)))
              .map((v) => v.toString(16).padStart(2, "0"))
              .join("");
          const strokes = await digest(a.compile(d));
          const t = performance.now();
          a.render(d, 960);
          const ms = performance.now() - t;
          const planes = a.hashes();
          const images = [];
          for (const view of views) images.push(await digest(a.view(view).data));
          return { strokes, planes, images, ms };
        },
        { d: c.d, views: c.id === "impasto" ? views : [views[0]] },
      );
      assert.equal(r.strokes, strokeHash);
      assert.deepEqual(r.planes, n[2].result);
      r.images.forEach((im, i) => assert.equal(im, hash(Uint8Array.from(n[i + 3].pixels))));
      hostTimings[b.name] = r.ms;
    }
    if (c.before && fs.existsSync("out/brush-iteration-3/baseline/author-dev4.exe")) {
      const d = { ...structuredClone(c.d), engine: "2.0.0-dev.4", catalog: previous };
      const n = native(d, true);
      save(c.id + "-before", n[3]);
      save(c.id + "-before-raking", n[6]);
      fs.writeFileSync(`${out}/${c.id}-before.oil-author.json`, JSON.stringify(d, null, 2) + "\n");
    }
    evidence.push({ id: c.id, strokeHash, planes: n[2].result, hostTimings });
    console.log(c.id + " native/Node" + (browsers.length ? "/Chromium/Firefox" : "") + " pass");
  }
  const verification = process.argv.includes("--native-only")
    ? "native and Node only"
    : "native, Node, Chromium and Firefox";
  fs.writeFileSync(
    out + "/measurements.json",
    JSON.stringify({ engine: author.document().engine, verification, width: 960, evidence }, null, 2) + "\n",
  );
  const img = (id, label) => `<figure><img src="${id}.png"><figcaption>${label}</figcaption></figure>`;
  const cards = cases
    .map(
      (c) =>
        `<section><h2>${c.title}</h2><div class="pair">${c.before ? img(c.id + "-before", "Iteration 2 · identical authored paths and light") : ""}${img(c.id + "-studio", "Iteration 3 · Studio")}</div><details><summary>Raking, unlit and height</summary><div class="pair">${c.before ? img(c.id + "-before-raking", "Iteration 2 · Raking") : ""}${img(c.id + "-raking", "Iteration 3 · Raking")}${img(c.id + "-unlit", "Unlit")}${img(c.id + "-height", "Height: fixed 0–3")}</div></details></section>`,
    )
    .join("");
  fs.writeFileSync(
    out + "/index.html",
    `<!doctype html><html lang="en"><meta charset="utf-8"><title>Brush combinations · iteration 3</title><style>body{margin:0;padding:28px;background:#202522;color:#ece7de;font:20px system-ui}.pair{display:grid;grid-template-columns:1fr 1fr;gap:18px}figure{margin:0}img{width:100%}figcaption{padding:8px 0}section{border-top:1px solid #5d6d58;margin-top:22px;padding-top:14px}p{line-height:1.5}summary{padding:15px;cursor:pointer}a{color:#c4d5b0}</style><h1>Natural combinations · iteration 3</h1><p>Seven existing candidate presets; engine dev.5, author/catalog format 2. Ten 960 px diagnostic documents. Current verification: ${verification}. Presets remain candidates, not artist-approved.</p><p>Scumble: brush-scale patches and dragged connections respond to fixed canvas tooth and actual relief. Impasto: smoother body between selected ridges; hgain stays 1.5. Paint pressure recruits contact and body rather than simply scaling opacity. Neighbouring lane bundles gather, spread and release pickup at different rates.</p><p>The same-path comparisons isolate renderer/default changes. The new form study deliberately changes the painting strategy: directional leaf marks, curved fruit turns and a few broad stone planes, using all seven brushes for appropriate roles. It is a separate artistic probe, not a numerical A/B.</p><a href="review.png">Single review PNG</a>${cards}<p>Limitations: clustered contacts and tooth are approximations, not a physical fibre model. Lane grouping is not bristle collision mechanics. The short-turn sample bends the path; there is no axial brush-roll state. No cast shadows or layered film. Review contours and combinations, not only texture.</p></html>`,
  );
} finally {
  for (const b of browsers) await b.browser.close();
}
