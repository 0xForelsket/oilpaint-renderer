import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { inflateSync } from "node:zlib";
import { encodePng } from "../src/png.ts";
import { above, all, constant, ellipse, scene, SceneError, sweep } from "../src/scene.ts";
import { sha256Hex } from "../src/sha256.ts";

test("sha256 matches node:crypto at block boundaries", () => {
  for (const n of [0, 1, 55, 56, 63, 64, 65, 119, 120, 1000, 4099]) {
    const data = Uint8Array.from({ length: n }, (_, i) => (i * 31 + 7) & 255);
    assert.equal(sha256Hex(data), createHash("sha256").update(data).digest("hex"), `length ${n}`);
  }
});

test("the DSL emits plain JSON with only the keys the author set", () => {
  const s = scene((S) => {
    S.canvas({ aspect: [4, 5], ground: "#e9e1d6" });
    S.target.fill("lead_white").blob(0.5, 0.5, 0.2, 0.1, "cobalt_blue", { strength: 0.5 });
    S.region("all", all());
    S.region("sky", above(0.6), { edge: 0.03 });
    S.flow("sky", sweep({ angle: -14 }));
    S.style("sky", { width: [0.02, 0.03], curvature: undefined });
    S.layers([{ name: "one", regions: ["sky"] }]);
  });
  assert.deepEqual(JSON.parse(JSON.stringify(s.spec)), s.spec);
  assert.deepEqual(s.spec.regions[1], { name: "sky", shape: { above: 0.6 }, edge: 0.03, flow: { sweep: { angle: -14 } } });
  assert.deepEqual(s.spec.target[1], { blob: { c: [0.5, 0.5], r: [0.2, 0.1], color: "cobalt_blue", strength: 0.5 } });
  assert.deepEqual(s.spec.styles.sky, { width: [0.02, 0.03] });
  assert.deepEqual(ellipse(0.1, 0.2, 0.3, 0.4), { ellipse: { c: [0.1, 0.2], r: [0.3, 0.4] } });
});

test("a flow for an undeclared region is an error", () => {
  assert.throws(
    () => scene((S) => { S.region("a", all()); S.flow("b", constant(0)); }),
    (e: unknown) => e instanceof SceneError && e.code === "UNKNOWN_REGION",
  );
});

test("Storm Light's TS port matches the committed example ScenePlan", async () => {
  const storm = (await import("../../../scenes/storm_v3.ts")).default;
  const committed = JSON.parse(readFileSync(new URL("../../../spec/examples/storm_v3.sceneplan.json", import.meta.url), "utf8"));
  assert.deepEqual(JSON.parse(JSON.stringify(storm.spec)), committed);
  assert.equal(storm.spec.regions.length, 20);
  assert.equal(storm.spec.layers.length, 15);
});

test("PNG encoding round-trips", () => {
  const rgb = Uint8Array.from({ length: 5 * 3 * 3 }, (_, i) => i * 5);
  const png = encodePng(5, 3, rgb);
  assert.deepEqual([...png.subarray(0, 8)], [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  const dv = new DataView(png.buffer);
  assert.equal(dv.getUint32(16), 5);
  assert.equal(dv.getUint32(20), 3);
  const idatLen = dv.getUint32(33);
  const raw = inflateSync(png.subarray(41, 41 + idatLen));
  assert.equal(raw.length, 3 * (1 + 15));
  assert.deepEqual([...raw.subarray(1, 16)], [...rgb.subarray(0, 15)]);
});
