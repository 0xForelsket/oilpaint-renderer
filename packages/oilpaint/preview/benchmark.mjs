import { chromium } from "playwright";
import fs from "node:fs";
const browser = await chromium.launch();
const page = await browser.newPage();
const rows = [];
async function change(fn) {
  const old = await page.evaluate(() => window.previewMeasurement?.id ?? 0);
  await fn();
  await page.waitForFunction((id) => window.previewMeasurement?.id > id, old);
  return page.evaluate(() => window.previewMeasurement);
}
async function range(id, value) {
  await page.locator("#" + id).evaluate((e, v) => {
    e.value = v;
    e.dispatchEvent(new Event("input", { bubbles: true }));
  }, String(value));
}
try {
  await page.goto("http://127.0.0.1:4173");
  await page.waitForFunction(() => window.previewMeasurement);
  for (const width of [256, 384]) {
    await change(() => page.selectOption("#resolution", String(width)));
    await change(() => page.locator('[data-sample="curve"]').click());
    for (let i = 0; i < 10; i++)
      rows.push({ width, kind: "paint", ...(await change(() => range("load", 0.8 + i * 0.015))) });
    for (let i = 0; i < 10; i++)
      rows.push({ width, kind: "relight", ...(await change(() => range("bump", 0.5 + i * 0.03))) });
  }
  fs.writeFileSync(
    "docs/reports/brush-review-2/latency.json",
    JSON.stringify(
      {
        conditions:
          "Chromium; process affinity inherited from PowerShell mask 0xF; 10 warmed repeats per operation and size; one loaded-flat curve; includes 75ms paint or 16ms lighting debounce.",
        rows,
      },
      null,
      2,
    ) + "\n",
  );
  const summary = [];
  for (const width of [256, 384])
    for (const kind of ["paint", "relight"]) {
      const r = rows.filter((r) => r.width === width && r.kind === kind);
      const stats = (k) => {
        const a = r.map((v) => v[k]).sort((a, b) => a - b);
        return { min: a[0], median: (a[4] + a[5]) / 2, max: a.at(-1) };
      };
      summary.push({
        width,
        kind,
        paintMs: stats("paintMs"),
        viewMs: stats("viewMs"),
        inputToCanvasMs: stats("latency"),
      });
    }
  console.log(JSON.stringify(summary, null, 2));
} finally {
  await browser.close();
}
