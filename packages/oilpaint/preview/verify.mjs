// User-visible QA: presets/samples/controls, local edits, pointer, references, serialization and latency.
import { chromium } from "playwright";
import assert from "node:assert/strict";
import fs from "node:fs/promises";
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1500, height: 1000 } });
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
const metrics = [];
const wait = async (old) => {
  await page.waitForFunction((id) => window.previewMeasurement?.id > id, old, { timeout: 30000 });
  assert.equal(await page.locator("#error").textContent(), "");
  return page.evaluate(() => window.previewMeasurement);
};
const change = async (fn, label) => {
  const old = await page.evaluate(() => window.previewMeasurement?.id ?? 0);
  await fn();
  const m = await wait(old);
  metrics.push({ label, ...m });
  return m;
};
const range = async (id, value) =>
  page.locator("#" + id).evaluate((e, v) => {
    e.value = String(v);
    e.dispatchEvent(new Event("input", { bubbles: true }));
  }, value);
const download = async (id) => {
  const [d] = await Promise.all([page.waitForEvent("download", { timeout: 60000 }), page.locator("#" + id).click()]);
  return fs.readFile(await d.path());
};
try {
  await page.goto("http://127.0.0.1:4173");
  await wait(0);
  for (const p of ["loaded-flat", "rounded-dab", "dry-drag", "scumble", "impasto-accent", "fine-detail", "wet-mixing"])
    await change(() => page.selectOption("#preset", p), "preset " + p);
  for (const kind of ["straight", "curve", "dab", "pressure", "taper", "runout", "reverse", "twist"])
    await change(() => page.locator(`[data-sample="${kind}"]`).click(), "sample " + kind);
  for (const light of ["soft", "studio", "raking"]) {
    const m = await change(() => page.locator(`[data-light="${light}"]`).click(), "lighting preset " + light);
    assert.equal(m.paintMs, 0);
  }
  await change(() => page.locator("#underpaint").click(), "wet crossing");
  for (const [id, v] of [
    ["pressure", 0.7],
    ["load", 0.9],
    ["pickup", 0.5],
    ["hgain", 1.1],
    ["dry", 0.4],
    ["body", 0.8],
    ["opacity", 0.9],
    ["vdry", 0.4],
    ["width", 0.03],
  ])
    await change(() => range(id, v), id);
  for (const id of ["unlit", "height", "lit"]) {
    const m = await change(() => page.selectOption("#view", id), "view " + id);
    assert.equal(m.paintMs, 0);
  }
  for (const [id, v] of [
    ["lx", 0.3],
    ["ly", -0.2],
    ["lz", 0.8],
    ["bump", 1.2],
    ["contrast", 0.25],
    ["specular", 0.07],
  ]) {
    const m = await change(() => range(id, v), "light " + id);
    assert.equal(m.paintMs, 0);
  }
  await change(() => page.selectOption("#mixer", "rgb"), "RGB");
  await change(() => page.selectOption("#mixer", "ochrell"), "Ochrell");
  await change(() => range("ground", "#c5bba7"), "ground");
  await change(() => range("paint", "#c1618a"), "paint");
  await page.locator("#capture").click();
  assert.ok((await page.locator("#referenceLabel").textContent()).includes("Wet mixing"));
  await page.locator("summary").filter({ hasText: "Save & exchange" }).click();
  const baseline = JSON.parse((await download("docExport")).toString());
  const catalog = JSON.parse((await download("presetExport")).toString());
  assert.equal(catalog.version, 2);
  await page.locator("#add").click();
  await page.locator("#name").fill("Lilies");
  await page.locator("#name").press("Tab");
  await change(() => page.locator('[data-sample="dab"]').click(), "new lilies");
  const edited = JSON.parse((await download("docExport")).toString());
  assert.deepEqual(edited.groups.slice(0, 2), baseline.groups);
  await change(() => page.locator("#visible").uncheck(), "hide");
  await change(() => page.locator("#visible").check(), "show");
  await change(() => page.locator("#up").click(), "order earlier");
  await change(() => page.locator("#down").click(), "order later");
  await page.locator("summary").filter({ hasText: "Precise group geometry" }).click();
  await page.locator("#dx").fill("0.02");
  await change(() => page.locator("#transform").click(), "translate");
  await page.locator("#path").fill("[[0.3,0.2,0.8],[0.7,0.3,1]]");
  await change(() => page.locator("#pathApply").click(), "precise path");
  await page.locator("#path").fill("[[0,0,-1],[1,1,1]]");
  await page.locator("#pathApply").click();
  assert.match(await page.locator("#error").textContent(), /INVALID_STROKE/);
  await change(() => page.locator("#reset").click(), "reset");
  const before = await page.evaluate(() => window.previewMeasurement.id);
  await page.locator("#live").scrollIntoViewIfNeeded();
  const box = await page.locator("#live").boundingBox();
  await page.mouse.move(box.x + box.width * 0.2, box.y + box.height * 0.6);
  await page.mouse.down();
  for (let i = 0; i < 15; i++) {
    await page.mouse.move(box.x + box.width * (0.2 + i * 0.03), box.y + box.height * (0.6 - i * 0.01));
    await page.waitForTimeout(20);
  }
  await page.mouse.up();
  await wait(before);
  await page.waitForTimeout(300);
  const drawn = JSON.parse((await download("docExport")).toString());
  assert.ok(drawn.groups.at(-1).strokes.length >= 2);
  // Paint and light events in one burst must retain the pending document repaint.
  await change(async () => {
    await range("pickup", 0.1);
    await range("bump", 0.5);
  }, "rapid paint + light");
  const saved = await download("docExport");
  await page.evaluate(async (d) => {
    const { loadEngine } = await import("/packages/oilpaint/src/engine.ts");
    const { createAuthor } = await import("/packages/oilpaint/src/author.ts");
    const a = createAuthor(await loadEngine());
    const $ = (id) => document.getElementById(id);
    a.render(d, +$("resolution").value, $("mixer").value);
    const img = a.view({
      mode: $("view").value,
      direction: [+$("lx").value, +$("ly").value, +$("lz").value],
      bump: +$("bump").value,
      contrast: +$("contrast").value,
      specular: +$("specular").value,
    });
    const shown = $("live").getContext("2d").getImageData(0, 0, img.width, img.height).data;
    for (let i = 0; i < img.data.length; i++) {
      if (img.data[i] !== shown[Math.floor(i / 3) * 4 + (i % 3)]) throw Error("worker image differs from full replay");
    }
  }, JSON.parse(saved.toString()));
  await change(() => page.locator("#clear").click(), "clear");
  await page.locator("#docImport").click();
  await change(
    () => page.locator("#file").setInputFiles({ name: "saved.json", mimeType: "application/json", buffer: saved }),
    "document roundtrip",
  );
  assert.deepEqual(JSON.parse((await download("docExport")).toString()), JSON.parse(saved.toString()));
  await page.locator("#presetImport").click();
  await page.locator("#file").setInputFiles({
    name: "bad.json",
    mimeType: "application/json",
    buffer: Buffer.from('{"version":99,"presets":[]}'),
  });
  await page.waitForFunction(() => document.querySelector("#error").textContent.includes("VERSION"));
  assert.match(await page.locator("#error").textContent(), /CATALOG_VERSION/);
  await page.locator("#presetImport").click();
  await change(
    () =>
      page.locator("#file").setInputFiles({
        name: "presets.json",
        mimeType: "application/json",
        buffer: Buffer.from(JSON.stringify(catalog)),
      }),
    "catalog roundtrip",
  );
  assert.ok((await download("strokesExport")).length > 100);
  await change(() => page.locator("#denseDry").click(), "dense dry controls");
  await change(() => page.locator("#formStudy").click(), "form-following study");
  await change(() => page.locator("#pickupTail").click(), "long pickup study");
  await change(() => page.selectOption("#resolution", "256"), "256 px");
  await page.locator("#capture").click();
  await page.screenshot({ path: "out/brush-review/preview.png", fullPage: true });
  await page.reload();
  await wait(0);
  assert.ok((await page.locator("#referenceLabel").textContent()).includes("retained"));
  await page.locator("#dropReference").click();
  assert.equal(await page.locator("#referenceLabel").textContent(), "No saved reference");
  await page.setViewportSize({ width: 390, height: 844 });
  await page.screenshot({ path: "out/brush-review/mobile.png", fullPage: true });
  assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
  assert.deepEqual(errors, []);
  await fs.writeFile("out/brush-review/ui-measurements.json", JSON.stringify({ metrics, errors }, null, 2));
  console.log(JSON.stringify({ checks: metrics.length, errors, metrics }, null, 2));
} catch (e) {
  console.error({
    error: await page.locator("#error").textContent(),
    status: await page.locator("#status").textContent(),
    errors,
  });
  throw e;
} finally {
  await browser.close();
}
