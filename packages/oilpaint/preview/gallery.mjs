// Reproducible review gallery and cross-runtime checks for every candidate.
import fs from "node:fs";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import assert from "node:assert/strict";
import { chromium, firefox } from "playwright";
import { loadEngine } from "../src/engine.ts";
import { createAuthor, sampleMark, defaultView } from "../src/author.ts";
import { encodePng } from "../src/png.ts";
const out = "out/brush-review-current";
fs.mkdirSync(out, { recursive: true });
const a = createAuthor(await loadEngine());
const rows = [];
const hash = (b) => createHash("sha256").update(b).digest("hex");
const docs = a.catalog().presets.map((p) => {
  const d = a.document(3107);
  d.aspect = [4, 3];
  const under = [];
  const marks = [];
  for (let row = 0; row < 3; row++) {
    const width = Math.min(p.width[2], Math.max(p.width[0], [0.008, 0.028, 0.065][row]));
    for (let col = 0; col < 3; col++) {
      const id = `${row}-${col}`;
      const y = 0.13 + row * 0.24;
      const u = sampleMark("under-" + id, "loaded-flat", 0.04);
      u.path = [
        [0.17 + col * 0.32, y - 0.06, 1],
        [0.17 + col * 0.32, y + 0.09, 1],
      ];
      u.color = [0.13, 0.34, 0.62];
      under.push(u);
      const s = sampleMark("sample-" + id, p.id, width, col === 0 ? "straight" : col === 1 ? "curve" : "dab", y);
      s.path = s.path.map(([x, yy, pressure]) => [
        col === 2 ? 0.79 + (x - 0.49) * 1.8 : col * 0.32 + 0.035 + (x - 0.12) * 0.33,
        y + (yy - y) * 0.45,
        pressure,
      ]);
      marks.push(s);
    }
  }
  d.groups = [
    { id: "underpaint", name: "Wet blue crossings", visible: true, dryAfter: 1, strokes: under },
    { id: "swatches", name: p.name, visible: true, dryAfter: 1, strokes: marks },
  ];
  return { p, d };
});
const browsers = [];
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
try {
  for (const { p, d } of docs) {
    fs.writeFileSync(`${out}/${p.id}.oil-author.json`, JSON.stringify(d, null, 2) + "\n");
    for (const width of [256, 512]) {
      const key = p.id + "-" + width;
      const requests = [
        { op: "compile", document: d },
        { op: "render", document: d, width, mixer: "ochrell" },
        { op: "hashes" },
        ...["lit", "unlit", "height"].map((mode) => ({ op: "view", view: { ...defaultView, mode } })),
      ];
      const native = spawnSync("target/release/examples/author.exe", [], {
        input: requests.map((r) => JSON.stringify(r)).join("\n") + "\n",
        encoding: "utf8",
        maxBuffer: 60 * 1024 * 1024,
      });
      if (native.status !== 0) throw Error(native.stderr);
      const result = native.stdout
        .trim()
        .split("\n")
        .map((line) => JSON.parse(line));
      for (const r of result) assert.ok(!r.result.error, r.result.error);
      const expected = hash(Uint8Array.from(result[0].strokes));
      const planes = result[2].result;
      assert.equal(hash(a.compile(d)), expected);
      let start = performance.now();
      a.render(d, width);
      const nodeMs = performance.now() - start;
      assert.deepEqual(a.hashes(), planes);
      const row = { preset: p.id, width, strokes: expected, planes, nodePaintMs: nodeMs, hosts: {} };
      for (const [i, mode] of ["lit", "unlit", "height"].entries()) {
        const r = result[3 + i];
        fs.writeFileSync(
          `${out}/${key}-native-${mode}.png`,
          encodePng(r.result.width, r.result.height, Uint8Array.from(r.pixels)),
        );
        assert.equal(hash(a.view({ ...defaultView, mode }).data), hash(Uint8Array.from(r.pixels)));
      }
      for (const { name, page } of browsers) {
        const rendered = await page.evaluate(
          ({ d, width, view }) => {
            const a = window.author;
            const strokeBytes = Array.from(a.compile(d));
            const t = performance.now();
            a.render(d, width);
            const paintMs = performance.now() - t;
            const planes = a.hashes();
            const images = {};
            const viewMs = {};
            for (const mode of ["lit", "unlit", "height"]) {
              const start = performance.now();
              const image = a.view({ ...view, mode });
              viewMs[mode] = performance.now() - start;
              images[mode] = { ...image, data: Array.from(image.data) };
            }
            return { strokeBytes, planes, paintMs, viewMs, images };
          },
          { d, width, view: defaultView },
        );
        assert.equal(hash(Uint8Array.from(rendered.strokeBytes)), expected);
        assert.deepEqual(rendered.planes, planes);
        for (const [i, mode] of ["lit", "unlit", "height"].entries()) {
          const img = rendered.images[mode];
          assert.equal(hash(Uint8Array.from(img.data)), hash(Uint8Array.from(result[3 + i].pixels)));
          if (name === "chromium")
            fs.writeFileSync(
              `${out}/${key}-browser-${mode}.png`,
              encodePng(img.width, img.height, Uint8Array.from(img.data)),
            );
        }
        row.hosts[name] = { paintMs: rendered.paintMs, viewMs: rendered.viewMs };
      }
      rows.push(row);
      console.log(key + " native/Node/Chromium/Firefox identical");
    }
  }
  fs.writeFileSync(
    `${out}/measurements.json`,
    JSON.stringify(
      {
        engine: (await loadEngine()).version,
        note: "Single passes, unpinned; includes WASM warmup. Timing is observational, not a benchmark guarantee.",
        cases: rows,
      },
      null,
      2,
    ) + "\n",
  );
  const cards = docs
    .map(
      ({ p }) =>
        `<article><h2>${p.name} <small>candidate</small></h2><p>Rows: 0.008 / 0.028 / 0.065 cw, clamped to preset range. Columns: straight / curve / dab, crossing wet blue paint.</p><div class="pair"><figure><img src="${p.id}-512-native-lit.png"><figcaption>Native · 512 px</figcaption></figure><figure><img src="${p.id}-512-browser-lit.png"><figcaption>Chromium WASM · 512 px (identical bytes)</figcaption></figure></div><p>${[256, 512].map((w) => ["native", "browser"].map((host) => ["lit", "unlit", "height"].map((mode) => `<a href="${p.id}-${w}-${host}-${mode}.png">${w} ${host} ${mode}</a>`).join(" · ")).join("<br>")).join("<br>")}</p><a href="${p.id}.oil-author.json">Reopen document in preview</a></article>`,
    )
    .join("");
  fs.writeFileSync(
    `${out}/index.html`,
    `<!doctype html><html lang="en"><meta charset="utf-8"><title>Candidate brush review</title><style>body{font:16px system-ui;background:#202522;color:#e5e3d8;max-width:1200px;margin:auto;padding:30px}h1{font-size:32px}small{color:#c4ad7a;font-size:14px}.pair{display:grid;grid-template-columns:1fr 1fr;gap:20px}figure{margin:0}img{width:100%}a{color:#bcd4aa}article{padding:25px 0;border-top:1px solid #566353}p{line-height:1.7}@media(max-width:650px){.pair{grid-template-columns:1fr}}</style><h1>Seven candidate brushes</h1><p>Engine 2.0.0-dev.4 · author format 2 · catalog 2. Real renderer output, consistent relief lighting. Automated calibration and cross-runtime evidence; awaiting artist review.</p><p>Inspect loaded landings, edge breakup, tails, wet-blue pickup and relief. Height maps use a fixed 0–3 scale. Fine-detail marks and dry bristles can disappear below one pixel; compare both resolutions. No physical filbert, fan or knife model is claimed.</p><a href="measurements.json">Plane hashes and measured timings</a>${cards}</html>`,
  );
} finally {
  for (const b of browsers) await b.browser.close();
}
