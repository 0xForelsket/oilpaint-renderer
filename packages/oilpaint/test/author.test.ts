import test from "node:test";
import assert from "node:assert/strict";
import { loadEngine } from "../src/engine.ts";
import { createAuthor, editGroup, moveGroup, sampleMark, scopedRandom, defaultView } from "../src/author.ts";
test("catalog, local edits, serialization, independent scopes and replay", async () => {
  const a = createAuthor(await loadEngine());
  const d = a.document();
  d.groups = ["water", "lilies", "reeds"].map((id, i) => ({
    id,
    name: id,
    visible: true,
    dryAfter: 1,
    strokes: [sampleMark(id + "-1", i === 2 ? "wet-mixing" : "loaded-flat", 0.03, "straight", 0.3 + i * 0.008)],
  }));
  const bytes = a.compile(d);
  assert.deepEqual(a.compile(a.parse(JSON.stringify(d))), bytes);
  const e = editGroup(d, "lilies", (g) => {
    g.strokes[0].color = [0.1, 0.3, 0.9];
  });
  assert.deepEqual(d.groups[0], e.groups[0]);
  assert.deepEqual(d.groups[2], e.groups[2]);
  assert.notDeepEqual(e.groups[1], d.groups[1]);
  assert.equal(scopedRandom(7, ["water", "1"]), scopedRandom(7, ["water", "1"]));
  assert.notEqual(scopedRandom(7, ["water", "1"]), scopedRandom(7, ["lilies", "1"]));
  a.render(d, 96);
  a.render(e, 96);
  const h = a.hashes();
  const full = createAuthor(await loadEngine());
  full.render(e, 96);
  assert.deepEqual(h, full.hashes());
  a.view(defaultView);
  assert.deepEqual(h, a.hashes());
  const moved = moveGroup(e, "lilies", 0);
  assert.equal(moved.groups[0].id, "lilies");
  a.render(moved, 96);
  full.render(moved, 96);
  assert.deepEqual(a.hashes(), full.hashes());
  assert.throws(() => a.parse(JSON.stringify({ ...d, version: 1 })), /VERSION/);
  assert.throws(() => a.parse(JSON.stringify({ ...d, engine: "old" })), /ENGINE_VERSION/);
  const bad = a.catalog();
  bad.presets[0].paint.pickup = 2;
  assert.throws(() => a.validateCatalog(bad), /INVALID_CONTROL/);
  assert.throws(() => a.render(d, 4096), /CANVAS_TOO_LARGE/);
});
