import { chromium } from "playwright";
import fs from "node:fs/promises";
import assert from "node:assert/strict";
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1450, height: 1000 } });
const errors = [];
const measurements = [];
page.on("pageerror", (e) => errors.push(String(e)));
const changed = async (f) => {
  const id = await page.evaluate(() => window.compositionMeasurement?.id ?? 0);
  await f();
  await page.waitForFunction(
    (id) => window.compositionMeasurement?.id > id,
    id,
    { timeout: 30000 },
  );
  assert.equal(await page.locator("#error").textContent(), "");
  measurements.push(await page.evaluate(() => window.compositionMeasurement));
};
const download = async () => {
  const [d] = await Promise.all([
    page.waitForEvent("download"),
    page.locator("#save").click(),
  ]);
  return JSON.parse(await fs.readFile(await d.path(), "utf8"));
};
try {
  await page.goto(
    "http://127.0.0.1:4173/packages/oilpaint/preview/composition.html",
  );
  await page.waitForFunction(() => window.compositionMeasurement, {
    timeout: 30000,
  });
  assert.equal(await page.locator("#error").textContent(), "");
  await page.selectOption("#region", "leaf");
  await page.locator("#reference").click();
  const original = await download();
  await changed(() => page.locator("#tint").click());
  const edited = await download();
  for (const g of original.groups)
    if (g.region !== "leaf")
      assert.deepEqual(
        edited.groups.find((x) => x.id === g.id),
        g,
      );
  assert.notDeepEqual(
    edited.groups.filter((g) => g.region === "leaf"),
    original.groups.filter((g) => g.region === "leaf"),
  );
  await changed(() => page.locator("#visible").uncheck());
  await changed(() => page.locator("#visible").check());
  await page.locator("#dx").fill(".005");
  await changed(() => page.locator("#translate").click());
  await page.selectOption("#preset", "rounded-dab");
  await changed(() => page.locator("#replan").click());
  const replanned = await download();
  for (const g of original.groups)
    if (g.region !== "leaf")
      assert.deepEqual(
        replanned.groups.find((x) => x.id === g.id),
        g,
      );
  await changed(() => page.selectOption("#view", "unlit"));
  assert.equal(
    (await page.evaluate(() => window.compositionMeasurement)).paintMs,
    0,
  );
  await changed(() => page.selectOption("#view", "lit"));
  const [dl] = await Promise.all([
    page.waitForEvent("download"),
    page.locator("#strokes").click(),
  ]);
  assert.ok((await fs.stat(await dl.path())).size > 100);
  await page.screenshot({
    path: "out/planner-integration/workbench.png",
    fullPage: true,
  });
  await page.setViewportSize({ width: 390, height: 844 });
  assert.ok(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  );
  assert.deepEqual(errors, []);
  await fs.writeFile(
    "out/planner-integration/ui-performance.json",
    JSON.stringify(measurements, null, 2),
  );
  console.log(
    "PASS: composition UI, region-only tint, visibility, translation, preset replan, export, relight and narrow viewport",
  );
} catch (e) {
  console.error({
    errors,
    error: await page.locator("#error").textContent(),
    status: await page.locator("#status").textContent(),
  });
  throw e;
} finally {
  await browser.close();
}
