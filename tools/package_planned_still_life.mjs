import fs from "node:fs/promises";
import { chromium } from "playwright";
const out = "docs/reports/planner-integration";
const v = JSON.parse(await fs.readFile(`${out}/verification.json`, "utf8"));
const html = `<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Retained brushes · planned still life</title>
<style>*{box-sizing:border-box}body{margin:0;background:#eeeae2;color:#282d2a;font:16px system-ui,sans-serif}main{max-width:1240px;margin:auto;padding:32px}h1{font-size:30px;margin:8px 0}p{line-height:1.5}small{color:#555}header{margin-bottom:24px}.grid{display:grid;grid-template-columns:repeat(3,1fr);gap:20px}figure{margin:0;background:#faf8f3;border:1px solid #d4d0c5;padding:12px}img{display:block;width:100%;height:auto}h2{font-size:18px;margin:8px 0}figcaption{font-size:14px;line-height:1.45;min-height:68px}.proof{padding:18px 22px;background:#dedfd4;margin-top:24px;border-left:4px solid #587356}a{color:#325c43}.details{display:grid;grid-template-columns:1fr 1fr;gap:24px}footer{font-size:13px;line-height:1.5;border-top:1px solid #ccc6b9;margin-top:24px;padding-top:16px}@media(max-width:700px){.grid,.details{grid-template-columns:1fr}main{padding:16px}}</style>
<main><header><small>ENGINE ${v.engine} · SEVEN RETAINED BRUSHES · PLANNER INTEGRATION</small><h1>One plan, independently editable subjects</h1><p>Orange, leaf and stone generated from region masks, flow fields and a shared brush catalog.<br>${v.strokes} planned strokes · ${v.groups} editable pass/region groups · no hand-painted touch-up.</p></header>
<div class="grid"><figure><img src="target.png" alt="Analytic target"><h2>1 · Source target</h2><figcaption>Generated color and form guide, not a photograph. Region flows and the pass schedule are authored.</figcaption></figure>
<figure><a href="painting-768.png"><img src="planned-384.png" alt="Planned painting"></a><h2>2 · Automatic painting</h2><figcaption>Shared brush contacts, form-following strokes, mask-supported edges, thin shadows and selective relief.</figcaption></figure>
<figure><a href="leaf-edit.png"><img src="leaf-edit-384.png" alt="Leaf recolored"></a><h2>3 · Change only the leaf</h2><figcaption>Warmer olive paint. All other groups retain identical authored marks; orange and stone crop pixels also match.</figcaption></figure></div>
<div class="proof"><strong>Verified in native Windows, Node, Chromium and Firefox.</strong><br>Identical planned StrokeList bytes and paint planes. Edited incremental replay equals full replay. All seven brush baselines remain unchanged.</div>
<div class="details"><section><h2>Coverage and timing</h2><p>Leaf ${(100 * v.coverage.find((x) => x.region === "leaf").covered).toFixed(1)}% · orange ${(100 * v.coverage.find((x) => x.region === "orange").covered).toFixed(1)}% · stone ${(100 * v.coverage.find((x) => x.region === "stone").covered).toFixed(1)}% soft-mask coverage.</p><p>256 px planning: native ${v.planningMs.native.toFixed(0)} ms; Node ${v.planningMs.Node.toFixed(0)} ms; ${v.planningMs.browsers.map((x) => `${x.name} ${x.planMs.toFixed(0)} ms`).join("; ")}. Single measured runs on this machine.</p></section><section><h2>Review boundary</h2><p>This remains a stylized planner study. The cast shadows are weak, and some joins and contours remain repetitive or angular. Technical preservation is verified; finished-painting realism and broader planner visual acceptance are not claimed.</p></section></div>
<footer><a href="/packages/oilpaint/preview/composition.html">Open workbench</a> · <a href="/docs/PLANNER_AUTHORING.md">API and launch instructions</a> · <a href="verification.json">Verification data</a> · <a href="scene.sceneplan.json">Scene source</a> · <a href="planned.composition.json">Editable painting</a> · <a href="leaf-edit.composition.json">Leaf edit</a> · <a href="leaf-replan-384.png">Selected-region replan</a><br>Candidate replanning computes a full plan, then accepts only the selected region. The seven accepted brush definitions and brush renderer were held fixed.</footer></main></html>`;
await fs.writeFile(`${out}/index.html`, html);
const browser = await chromium.launch();
try {
  const page = await browser.newPage({
    viewport: { width: 1280, height: 1000 },
    deviceScaleFactor: 1,
  });
  await page.goto("http://127.0.0.1:4173/" + out + "/index.html");
  await page
    .locator("img")
    .evaluateAll((images) => Promise.all(images.map((i) => i.decode())));
  await page.screenshot({ path: `${out}/review.png`, fullPage: true });
} finally {
  await browser.close();
}
console.log(`${out}/review.png`);
