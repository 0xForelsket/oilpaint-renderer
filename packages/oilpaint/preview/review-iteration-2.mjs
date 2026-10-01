// Iteration-two review: larger controlled marks, unchanged-height lighting, directionality and form studies.
import fs from "node:fs";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import assert from "node:assert/strict";
import { chromium, firefox } from "playwright";
import { loadEngine } from "../src/engine.ts";
import { createAuthor, sampleMark, defaultView, softView, rakingView } from "../src/author.ts";
import { encodePng } from "../src/png.ts";
const out = "docs/reports/brush-review-2";
fs.mkdirSync(out, { recursive: true });
const author = createAuthor(await loadEngine());
const baselineCatalog = JSON.parse(
  fs.readFileSync("docs/reports/brush-review/loaded-flat.oil-author.json", "utf8"),
).catalog;
const hash = (b) => createHash("sha256").update(b).digest("hex");
const group = (id, strokes, dryAfter = 1) => ({ id, name: id, visible: true, dryAfter, strokes });
function document() {
  const d = author.document(817);
  d.aspect = [2, 1];
  return d;
}
function mark(id, preset, width, path, color = [0.87, 0.42, 0.2], controls = {}) {
  return { id, preset, width, path, color, controls };
}
function marks(p) {
  const d = document();
  const w = Math.min(p.width[2], p.id === "fine-detail" ? 0.018 : 0.095);
  const upper = sampleMark("curve", p.id, w, "curve", 0.09);
  upper.path = upper.path.map(([x, y, v]) => [0.06 + (x - 0.12) * 0.77, y * 0.78, v]);
  const lower = sampleMark("pressure", p.id, w, "pressure", 0.36);
  lower.path = lower.path.map(([x, y, v]) => [0.06 + (x - 0.12) * 0.77, y, v]);
  const dab = sampleMark("press-lift", p.id, w, "dab", 0.17);
  dab.path = dab.path.map(([x, y, v]) => [x + 0.3, y, v]);
  const taper = sampleMark("taper", p.id, w, "taper", 0.36);
  taper.path = taper.path.map(([x, y, v]) => [0.73 + (x - 0.12) * 0.28, y, v]);
  d.groups = [group("marks", [upper, lower, dab, taper])];
  return d;
}
function forms(p) {
  const d = document();
  d.aspect = [3, 1];
  const strokes = [];
  const max = Math.min(p.width[2], 0.04);
  const colors = [
    [0.19, 0.4, 0.2],
    [0.78, 0.34, 0.13],
    [0.37, 0.4, 0.43],
  ];
  for (let subject = 0; subject < 3; subject++) {
    const cx = 0.17 + subject * 0.33;
    for (let i = 0; i < 15; i++) {
      const t = i / 14;
      const y = 0.065 + t * 0.205;
      const half =
        subject === 2
          ? 0.1 * (t < 0.25 ? 0.4 + 2.2 * t : t < 0.7 ? 0.95 - 0.25 * (t - 0.25) : 0.84 - 1.7 * (t - 0.7))
          : (subject === 0 ? 0.085 : 0.105) * Math.sqrt(Math.max(0, 1 - (t * 2 - 1) ** 2));
      const shade = 0.7 + 0.45 * (1 - t);
      const c = colors[subject].map((v) => Math.min(1, v * shade));
      const pts = [];
      for (let j = 0; j <= 16; j++) {
        const u = j / 16;
        pts.push([cx - half + 2 * half * u, y + 0.012 * 4 * u * (1 - u), 0.7 + 0.25 * (1 - u)]);
      }
      strokes.push(
        mark(
          `${subject}-${i}`,
          p.id,
          Math.max(p.width[0], max * (subject === 2 ? 0.64 : 0.8) * (0.3 + 0.7 * Math.min(1, half / 0.035))),
          pts,
          c,
        ),
      );
    }
    const vein =
      subject === 0
        ? [
            [cx - 0.04, 0.25, 1],
            [cx + 0.04, 0.08, 0.2],
          ]
        : subject === 2
          ? [
              [cx - 0.065, 0.105, 1],
              [cx + 0.06, 0.14, 0.35],
            ]
          : [
              [cx - 0.04, 0.07, 1],
              [cx + 0.01, 0.045, 0.3],
            ];
    strokes.push(
      mark(
        `accent-${subject}`,
        p.id,
        Math.min(p.width[2], 0.009),
        vein,
        subject === 0 ? [0.6, 0.65, 0.3] : subject === 2 ? [0.57, 0.58, 0.54] : [0.48, 0.41, 0.23],
      ),
    );
  }
  d.groups = [group("leaf-fruit-stone", strokes)];
  return d;
}
function pickup(reverse = false, dry = 1) {
  const d = document();
  const s = sampleMark("pickup", "wet-mixing", 0.09, reverse ? "reverse" : "straight", 0.24);
  s.controls = { deplete: 0.04 };
  d.groups = [
    group(
      "underpaint",
      [
        mark(
          "blue",
          "loaded-flat",
          0.095,
          [
            [0.48, 0.08, 1],
            [0.48, 0.42, 1],
          ],
          [0.12, 0.3, 0.7],
        ),
      ],
      dry,
    ),
    group("pickup", [s]),
  ];
  return d;
}
function depletion(p) {
  const d = document();
  const s = sampleMark("runout", p, 0.065, "runout", 0.24);
  d.groups = [group("runout", [s])];
  return d;
}
const cases = author.catalog().presets.flatMap((p) => [
  { id: p.id + "-contact", title: p.name + " — contact", d: marks(p), baseline: true },
  { id: p.id + "-forms", title: p.name + " — leaf / fruit / rough stone", d: forms(p) },
]);
cases.push(
  { id: "pickup-forward", d: pickup(), title: "Wet mixing — left to right" },
  { id: "pickup-reverse", d: pickup(true), title: "Wet mixing — right to left" },
  { id: "pickup-dry", d: pickup(false, 0), title: "Wet mixing — dry substrate control" },
  { id: "dry-drag-runout", d: depletion("dry-drag"), title: "Dry drag — long depletion" },
  { id: "loaded-flat-runout", d: depletion("loaded-flat"), title: "Loaded flat — long depletion" },
);
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
function native(d, views, old = false) {
  const width = 960;
  const requests = [
    { op: "compile", document: d },
    { op: "render", document: d, width, mixer: "ochrell" },
    { op: "hashes" },
    ...views.map((view) => ({ op: "view", view })),
  ];
  const exe = old ? "out/brush-iteration-2/baseline/author-dev3.exe" : "target/release/examples/author.exe";
  const r = spawnSync(exe, [], {
    input: requests.map((v) => JSON.stringify(v)).join("\n") + "\n",
    encoding: "utf8",
    maxBuffer: 90 * 1024 * 1024,
  });
  if (r.status !== 0) throw Error(r.stderr);
  const results = r.stdout
    .trim()
    .split("\n")
    .map((s) => JSON.parse(s));
  for (const r of results) assert.ok(!r.result.error, r.result.error);
  return results;
}
const evidence = [];
const image = (file, r) =>
  fs.writeFileSync(out + "/" + file, encodePng(r.result.width, r.result.height, Uint8Array.from(r.pixels)));
try {
  for (const c of cases) {
    fs.writeFileSync(`${out}/${c.id}.oil-author.json`, JSON.stringify(c.d, null, 2) + "\n");
    const views = [
      defaultView,
      { ...defaultView, mode: "unlit" },
      { ...defaultView, mode: "height" },
      softView,
      rakingView,
    ];
    const n = native(c.d, views);
    const strokeHash = hash(Uint8Array.from(n[0].strokes));
    const planes = n[2].result;
    const names = ["studio", "unlit", "height", "soft", "raking"];
    for (let i = 0; i < views.length; i++) image(c.id + "-" + names[i] + ".png", n[i + 3]);
    assert.equal(hash(author.compile(c.d)), strokeHash);
    author.render(c.d, 960);
    assert.deepEqual(author.hashes(), planes);
    for (let i = 0; i < views.length; i++)
      assert.equal(hash(author.view(views[i]).data), hash(Uint8Array.from(n[i + 3].pixels)));
    const times = {};
    for (const b of browsers) {
      const result = await b.page.evaluate(
        async ({ d, views }) => {
          const a = window.author;
          const digest = async (b) =>
            Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", b)))
              .map((v) => v.toString(16).padStart(2, "0"))
              .join("");
          const bytes = await digest(a.compile(d));
          const t = performance.now();
          a.render(d, 960);
          const paintMs = performance.now() - t;
          const hashes = a.hashes();
          const images = [];
          for (const v of views) images.push(await digest(a.view(v).data));
          return { bytes, paintMs, hashes, images };
        },
        { d: c.d, views: c.id === "impasto-accent-contact" ? views : [views[0]] },
      );
      assert.equal(result.bytes, strokeHash);
      assert.deepEqual(result.hashes, planes);
      result.images.forEach((im, i) => assert.equal(im, hash(Uint8Array.from(n[i + 3].pixels))));
      times[b.name] = result.paintMs;
    }
    if (c.baseline && fs.existsSync("out/brush-iteration-2/baseline/author-dev3.exe")) {
      const old = structuredClone(c.d);
      old.version = 1;
      old.engine = "2.0.0-dev.3";
      old.catalog = baselineCatalog;
      const o = native(old, [softView], true);
      image(c.id + "-before.png", o[3]);
      fs.writeFileSync(`${out}/${c.id}-before.oil-author.json`, JSON.stringify(old, null, 2) + "\n");
    }
    evidence.push({
      id: c.id,
      strokeHash,
      planes,
      times,
      browserViews: c.id === "impasto-accent-contact" ? names : ["studio"],
    });
    console.log(c.id + " passes native/Node/Chromium/Firefox");
  }
  fs.writeFileSync(
    out + "/measurements.json",
    JSON.stringify(
      { engine: "2.0.0-dev.4", width: 960, views: ["studio", "unlit", "height", "soft", "raking"], evidence },
      null,
      2,
    ) + "\n",
  );
  const im = (id, label) => `<figure><img src="${id}.png"><figcaption>${label}</figcaption></figure>`;
  const cards = author
    .catalog()
    .presets.map(
      (p) =>
        `<section><h2>${p.name} · candidate</h2><p>Contact panel: curved loaded stroke and press/lift above; pressure ramp and long taper below. Width ${p.id === "fine-detail" ? ".018" : Math.min(p.width[2], 0.095)} cw. Each source image is 960 px wide.</p><div class="pair">${im(p.id + "-contact-before", "Iteration 1 · soft light")}${im(p.id + "-contact-soft", "Iteration 2 · identical soft light")}</div><details><summary>Studio / raking / unlit / height</summary><div class="pair">${im(p.id + "-contact-studio", "Studio")}${im(p.id + "-contact-raking", "Raking")}${im(p.id + "-contact-unlit", "Unlit")}${im(p.id + "-contact-height", "Height: fixed 0–3")}</div></details><div class="pair">${im(p.id + "-forms-studio", "Same leaf, fruit and stone paths · studio")}${im(p.id + "-forms-raking", "Same paint planes · raking")}</div></section>`,
    )
    .join("");
  fs.writeFileSync(
    out + "/index.html",
    `<!doctype html><html lang="en"><meta charset="utf-8"><title>Brush contact review — iteration 2</title><style>body{font:20px system-ui;margin:0;padding:30px;background:#202522;color:#eee9df}h1{font-size:38px}.pair{display:grid;grid-template-columns:1fr 1fr;gap:18px}img{width:100%}figure{margin:0}figcaption{padding:8px 0;color:#c1ccb9}section{border-top:1px solid #657260;margin-top:24px;padding-top:16px}a{color:#c1d8a9}p{line-height:1.5}summary{cursor:pointer;padding:16px}details img{width:100%}</style><h1>Readable contact and relief · iteration 2</h1><p>Seven candidate brushes. Engine dev.4 / author 2 / catalog 2. Real paint geometry and deposition; no added image texture. All 19 documents, paint planes and studio images match native, Node, Chromium and Firefox. All five views match native/Node; the impasto lighting comparison also checks all five in both browsers. The previous sources/renders remain intact.</p><p>Review: beginnings, middle contact and release; substantial sparse scumble; independent lane exhaustion and color pickup. Form studies reuse the same paths and palettes across all seven, subject to each width range. They are controlled form-description probes, not finished paintings.</p><h2>Impasto lighting: one unchanged paint surface</h2><div class="pair">${im("impasto-accent-contact-soft", "Soft · original light settings")}${im("impasto-accent-contact-raking", "Raking · same height, stronger directional illumination")}</div><p>Impasto hgain stays 1.5. Raking light reveals the existing relief; this renderer still uses local normals, not cast shadows.</p><h2>Directional wet pickup and a dry control</h2><div class="pair">${im("pickup-forward-studio", "Forward: left → right")}${im("pickup-reverse-studio", "Reverse: right → left")}${im("pickup-dry-studio", "Forward over dried blue")}${im("dry-drag-runout-studio", "Dry drag: finite load runs out")}</div>${cards}<p>Limits: this remains a bristle-lane model. Scumble patches are a deterministic contact approximation, not physical canvas microgeometry. Rounded contact is an asymmetric geometric footprint, not a simulated filbert. No artist approval inferred.</p></html>`,
  );
  const page = browsers[0].page;
  await page.setViewportSize({ width: 1980, height: 900 });
  await page.goto("http://127.0.0.1:4173/" + out + "/index.html");
  await page.screenshot({ path: out + "/review.png", fullPage: true });
} finally {
  for (const b of browsers) await b.browser.close();
}
