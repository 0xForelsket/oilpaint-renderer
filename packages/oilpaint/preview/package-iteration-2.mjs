// Package the revised review into a broad sheet: original-size pixels, fewer duplicated views.
import fs from "node:fs";
import { chromium } from "playwright";
const out = "docs/reports/brush-review-2";
const catalog = JSON.parse(fs.readFileSync("crates/oil-brush/catalog.json", "utf8"));
const panel = (id, title, note) =>
  `<figure><h2>${title}</h2><p>${note}</p><img src="data:image/png;base64,${fs.readFileSync(out + "/" + id + ".png").toString("base64")}"></figure>`;
const lighting = [
  panel("impasto-accent-contact-soft", "Impasto / Soft light", "Same deposited height, original lighting"),
  panel("impasto-accent-contact-raking", "Impasto / Raking light", "Same paint planes; no added height"),
  panel("pickup-forward-studio", "Wet mixing / Forward →", "Blue pickup follows the stroke toward the right"),
  panel("pickup-reverse-studio", "Wet mixing / Reverse ←", "Blue pickup follows the stroke toward the left"),
].join("");
const contacts =
  catalog.presets
    .map((p) =>
      panel(p.id + "-contact-studio", p.name + " / Candidate", "Curve + press/lift above; pressure ramp + taper below"),
    )
    .join("") +
  panel("dry-drag-runout-studio", "Dry drag / Depletion", "Long stroke with fixed finite load; the tail runs out");
const forms = catalog.presets
  .map((p) =>
    panel(p.id + "-forms-studio", p.name + " / Form study", "Same leaf, fruit and stone paths, colors and light"),
  )
  .join("");
const html = `<!doctype html><html><meta charset="utf-8"><style>*{box-sizing:border-box}body{margin:0;width:3980px;padding:40px;background:#202522;color:#ece8dd;font:23px Arial}h1{font-size:48px;margin:0 0 14px}p{margin:5px 0 16px;line-height:1.4;color:#ccd4c5}h2{font-size:26px;margin:0 0 5px}h3{font-size:30px;margin:28px 0 16px}.grid{display:grid;grid-template-columns:repeat(4,960px);gap:24px 20px}figure{margin:0}figure p{font-size:20px}img{display:block;width:960px}footer{margin-top:25px;font-size:22px;line-height:1.45}</style><h1>Brush review · Iteration 2 · Seven candidates</h1><p>Engine dev.4 / author 2 / catalog 2. All panels are original 960 px renderer output. No post-render paint effects. Presets are not artist-approved.</p><p>First row: controlled relief lighting and directional pickup. Middle rows: contact, pressure, release and depletion. Bottom rows: form-description probes.</p><h3>01 / Relief and pickup diagnostics</h3><div class="grid">${lighting}</div><h3>02 / How marks begin, change and end</h3><div class="grid">${contacts}</div><h3>03 / Same small subjects, all seven brushes</h3><div class="grid">${forms}</div><footer>Technical checks: 19 documents, paint planes and Studio images match native, Node, Chromium and Firefox. All five views match native/Node; all impasto views also match both browsers.<br>Limits remain visible: geometric bristle lanes, repeated parallel passes, low-pressure translucency and an embossed look under strong light. No cast shadows or physical filbert/fan/knife model.<br>Full-size before/after, unlit and height views plus source documents: docs/reports/brush-review-2/index.html.</footer></html>`;
fs.mkdirSync("out/brush-iteration-2", {recursive:true});
fs.writeFileSync("out/brush-iteration-2/compact.html", html);
const browser = await chromium.launch();
try {
  const page = await browser.newPage({ viewport: { width: 3980, height: 900 }, deviceScaleFactor: 1 });
  await page.setContent(html, { waitUntil: "load" });
  await page.screenshot({ path: out + "/review-compact.png", fullPage: true });
  console.log(
    JSON.stringify({
      pixels: await page.evaluate(() => [document.documentElement.scrollWidth, document.documentElement.scrollHeight]),
      bytes: fs.statSync(out + "/review-compact.png").size,
    }),
  );
} finally {
  await browser.close();
}
