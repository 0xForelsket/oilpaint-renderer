import test from "node:test";
import assert from "node:assert/strict";
import { loadEngine } from "../src/engine.ts";
import { createAuthor, defaultView } from "../src/author.ts";
import { editRegion, replaceRegion } from "../src/composition.ts";
import { catalogStillLife } from "../../../scenes/catalog_still_life.ts";

test("planned composition saves, locally edits and selectively accepts a candidate replan", async () => {
  const e = await loadEngine(),
    a = createAuthor(e),
    s = catalogStillLife(a.catalog());
  const p = e.plan(s, { width: 128, seed: 1907 });
  const d = p.document;
  assert.equal(d.format, "oil-composition");
  assert.ok(d.groups.some((g) => g.region === "leaf"));
  assert.deepEqual(a.parse(JSON.stringify(d)), d);
  const changed = editRegion(d, "leaf", (g) => {
    for (const s of g.strokes) s.color = [0.5, 0.55, 0.2];
  });
  for (const g of d.groups)
    if (g.region !== "leaf")
      assert.deepEqual(
        changed.groups.find((x) => x.id === g.id),
        g,
      );
  a.render(d, 128);
  a.render(changed, 128);
  const actual = a.hashes();
  const fresh = createAuthor(await loadEngine());
  fresh.render(changed, 128);
  assert.deepEqual(actual, fresh.hashes());
  a.view(defaultView);
  assert.deepEqual(actual, a.hashes());
  const candidate = structuredClone(d);
  for (const g of candidate.groups) {
    for (const s of g.strokes) s.points[0][0] += 0.005;
  }
  const accepted = replaceRegion(d, candidate, "leaf");
  for (const g of d.groups)
    if (g.region !== "leaf")
      assert.deepEqual(
        accepted.groups.find((x) => x.id === g.id),
        g,
      );
  assert.notDeepEqual(
    accepted.groups.filter((g) => g.region === "leaf"),
    d.groups.filter((g) => g.region === "leaf"),
  );
  const mismatch = structuredClone(candidate);
  mismatch.ground[0] = 0;
  assert.throws(() => replaceRegion(d, mismatch, "leaf"), /SCOPE_CHANGED/);
  const bad = structuredClone(d);
  bad.version = 99;
  assert.throws(() => a.compile(bad), /VERSION/);
  const duplicate = structuredClone(d);
  duplicate.groups[1].id = duplicate.groups[0].id;
  assert.throws(() => a.compile(duplicate), /GROUP_ID/);
  const invalid = structuredClone(d);
  const stroke = invalid.groups.flatMap((g) => g.strokes)[0];
  stroke.points[0][3] = 3;
  assert.throws(() => a.compile(invalid), /INVALID_COMPOSITION/);
});
