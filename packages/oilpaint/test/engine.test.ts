// Needs the WASM: npm run build:wasm.
import assert from "node:assert/strict";
import { test } from "node:test";
import { loadEngine, OilError } from "../src/engine.ts";
import { all, above, constant, fieldShape, gradientV, scene } from "../src/scene.ts";

const engine = await loadEngine();

const small = () =>
  scene((S) => {
    S.canvas({ aspect: [4, 5], ground: "#e9e1d6" });
    S.target.fill("#336699").glow([0.5, 0.4], 0.2, "#ffe9a8");
    S.region("all", all(), { flow: constant(10) });
    S.region("sky", above(0.5));
    S.style("sky", { width: [0.02, 0.03] });
    S.layers([{ name: "one", regions: ["all", "sky"] }]);
  });

test("the engine reports its version and schema", () => {
  assert.match(engine.version, /^2\.0\.0/);
  assert.equal((engine.schema() as { title: string }).title, "ScenePlan");
});

test("validation reports structured errors with fixes", () => {
  assert.deepEqual(engine.validate(small()), { valid: true, errors: [], warnings: [] });
  const spec = small().spec;
  (spec.styles.sky as Record<string, unknown>).widht = [0.02, 0.03];
  const v = engine.validate(spec);
  assert.equal(v.valid, false);
  assert.equal(v.errors[0].code, "SCHEMA");
  assert.equal(v.errors[0].path, "/styles/sky/widht");
  assert.equal(v.errors[0].fix, 'did you mean "width"?');
  const units = small().spec;
  units.styles.sky.width = [12, 20];
  assert.equal(engine.validate(units).errors[0].code, "UNITS");
});

test("inputs larger than the current memory (the memory grows during the call)", async () => {
  const fresh = await loadEngine();
  const bytes = 64 << 20; // more than the module starts with
  const v = fresh.validate(" ".repeat(bytes) + JSON.stringify(small().spec));
  assert.equal(v.valid, true);
});

test("guides: sizes, planes and previews", () => {
  const g = engine.guides(small(), { width: 80 });
  assert.deepEqual(g.size, [80, 100]);
  assert.equal(g.target().length, 80 * 100 * 3);
  assert.equal(g.regionId().length, 80 * 100);
  assert.equal(g.masks().length, 2 * 80 * 100);
  assert.equal(g.flow().length, 80 * 100 * 2);
  assert.ok(g.regionFlow("all"));
  assert.equal(g.regionFlow("sky"), null);
  const ids = g.regionId();
  assert.equal(ids[0], 1); // top-left is sky (declared later, so it wins)
  assert.equal(ids[ids.length - 1], 0);
  const sheet = g.preview("sheet");
  assert.equal(sheet.data.length, sheet.width * sheet.height * 3);
  engine.guides(small(), { width: 64 });
  assert.throws(() => g.target(), (e: unknown) => e instanceof OilError && e.code === "STALE_GUIDES");
});

test("guides are deterministic and follow the mixer only through mixes", () => {
  const a = engine.guides(small(), { width: 64 }).sha256;
  const b = engine.guides(small(), { width: 64 }).sha256;
  assert.deepEqual(a, b);
  assert.deepEqual(engine.guides(small(), { width: 64, mixer: "rgb" }).sha256, a);
});

test("sampled fields: hashed, resampled, and checked", () => {
  const s = scene((S) => {
    S.canvas({ aspect: [1, 1], ground: "#ffffff" });
    S.target.fill("#000000");
    S.field("left", "mask", (x) => (x < 0.5 ? 1 : 0), 32);
    S.region("all", all());
    S.region("left", fieldShape("left"));
    S.layers([]);
  });
  const g = engine.guides(s, { width: 64 });
  const ids = g.regionId();
  assert.equal(ids[10], 1);
  assert.equal(ids[60], 0);
  s.fields.get("left")![0] = 0.5;
  assert.throws(() => engine.guides(s, { width: 64 }), (e: unknown) => e instanceof OilError && e.code === "FIELD_HASH_MISMATCH");
});

test("invalid plans throw OilError with every issue", () => {
  const spec = small().spec;
  spec.regions.push({ name: "sky", shape: { above: 0.2 } });
  spec.layers[0].regions = ["all", "skye"];
  try {
    engine.guides(spec, { width: 64 });
    assert.fail("expected an error");
  } catch (e) {
    assert.ok(e instanceof OilError);
    const codes = e.issues.map((i) => i.code);
    assert.ok(codes.includes("DUPLICATE_NAME") && codes.includes("UNKNOWN_REGION"), codes.join());
    assert.equal(e.issues.find((i) => i.code === "UNKNOWN_REGION")?.fix, 'did you mean "sky"?');
  }
});

test("a picture as the target (image op), fitted to the canvas", () => {
  const w = 8, h = 10;
  const px = new Uint8Array(w * h * 4);
  for (let j = 0; j < h; j++) for (let i = 0; i < w; i++) px.set(i < w / 2 ? [255, 0, 0, 255] : [0, 0, 255, 255], (j * w + i) * 4);
  const s = scene((S) => {
    S.canvas({ aspect: [4, 5], ground: "#ffffff" });
    S.picture("photo", { width: w, height: h, data: px });
    S.target.fill("#00ff00").image("photo", { fit: "cover" });
    S.region("all", all());
    S.layers([]);
  });
  const g = engine.guides(s, { width: 64 });
  const t = g.target();
  const at = (i: number, j: number) => Array.from(t.subarray((j * 64 + i) * 3, (j * 64 + i) * 3 + 3));
  assert.deepEqual(at(5, 40).map(Math.round), [1, 0, 0]);
  assert.deepEqual(at(58, 40).map(Math.round), [0, 0, 1]);
});

test("an unknown preset is reported with a suggestion", () => {
  const spec = small().spec;
  spec.preset = "impresionist";
  const v = engine.validate(spec);
  assert.equal(v.errors[0].code, "UNKNOWN_PRESET");
  assert.equal(v.errors[0].fix, 'did you mean "impressionist"?');
});

test("plan: a StrokeList, repeatable, with a report", () => {
  const s = scene((S) => {
    S.canvas({ aspect: [4, 5], ground: "#e9e1d6" });
    S.target.fill(gradientV([[0, "#2c3266"], [1.25, "#e8cbb8"]]));
    S.region("all", all(), { flow: constant(10, { noise: 0.2 }) });
    S.style("all", { width: [0.05, 0.08], length: [0.1, 0.2] });
    S.layers([{ name: "block in", regions: "all", errorThreshold: 10 }]);
  });
  const a = engine.plan(s, { width: 96, seed: 3, mixer: "rgb" });
  const b = engine.plan(s, { width: 96, seed: 3, mixer: "rgb" });
  assert.ok(a.report.strokes > 5, `${a.report.strokes} strokes`);
  assert.deepEqual([...a.strokes.subarray(0, 8)], [0x89, 0x4f, 0x49, 0x4c, 0x0d, 0x0a, 0x1a, 0x0a]);
  assert.deepEqual(a.strokes, b.strokes);
  assert.equal(a.report.regions[0].region, "all");
  assert.notDeepEqual(engine.plan(s, { width: 96, seed: 4, mixer: "rgb" }).strokes, a.strokes);
});
