import fs from "node:fs";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { chromium, firefox } from "playwright";
import { loadEngine } from "../packages/oilpaint/src/engine.ts";
import { createAuthor } from "../packages/oilpaint/src/author.ts";
import { studyLight } from "../studies/orange-study.ts";
const out = "docs/reports/orange-study";
const d = JSON.parse(fs.readFileSync(out + "/painting.oil-author.json"));
const sha = (b) => createHash("sha256").update(b).digest("hex");
const requests = [
  { op: "compile", document: d },
  { op: "render", document: d, width: 384, mixer: "ochrell" },
  { op: "hashes" },
  { op: "view", view: studyLight },
];
const result = spawnSync("target/release/examples/author.exe", [], {
  input: requests.map((r) => JSON.stringify(r)).join("\n") + "\n",
  encoding: "utf8",
  maxBuffer: 30 * 1024 * 1024,
});
assert.equal(result.status, 0, result.stderr);
const n = result.stdout
  .trim()
  .split("\n")
  .map((x) => JSON.parse(x));
for (const r of n) assert.ok(!r.result.error, r.result.error);
const strokeHash = sha(Uint8Array.from(n[0].strokes)),
  imageHash = sha(Uint8Array.from(n[3].pixels));
const a = createAuthor(await loadEngine());
assert.deepEqual(a.parse(JSON.stringify(d)), d);
assert.equal(sha(a.compile(d)), strokeHash);
a.render(d, 384);
assert.deepEqual(a.hashes(), n[2].result);
assert.equal(sha(a.view(studyLight).data), imageHash);
const hosts = ["native", "Node"];
for (const [name, runtime] of [
  ["Chromium", chromium],
  ["Firefox", firefox],
]) {
  const browser = await runtime.launch();
  try {
    const p = await browser.newPage();
    await p.goto("http://127.0.0.1:4173/packages/oilpaint/preview/blank.html");
    const r = await p.evaluate(
      async ({ d, light }) => {
        const { loadEngine } = await import("/packages/oilpaint/src/engine.ts");
        const { createAuthor } = await import("/packages/oilpaint/src/author.ts");
        const a = createAuthor(await loadEngine());
        const hash = async (b) =>
          Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", b)))
            .map((v) => v.toString(16).padStart(2, "0"))
            .join("");
        const strokes = await hash(a.compile(d));
        a.render(d, 384);
        return { strokes, planes: a.hashes(), image: await hash(a.view(light).data) };
      },
      { d, light: studyLight },
    );
    assert.equal(r.strokes, strokeHash);
    assert.equal(r.image, imageHash);
    assert.deepEqual(r.planes, n[2].result);
    hosts.push(name);
  } finally {
    await browser.close();
  }
}
const frozenFiles = [
  "crates/oil-kernel/src/brush.rs",
  "crates/oil-kernel/src/lib.rs",
  "crates/oil-author/catalog.json",
  "crates/oil-author/src/lib.rs",
  "crates/oil-paint/src/lib.rs",
  "crates/oil-light/src/lib.rs",
];
const frozen = {};
for (const f of frozenFiles) {
  const old = spawnSync("git", ["show", `fb118dc:${f}`], { maxBuffer: 4 * 1024 * 1024 });
  assert.equal(old.status, 0);
  const normalize = (b) => Buffer.from(b.toString().replace(/\r\n/g, "\n"));
  assert.equal(sha(normalize(old.stdout)), sha(normalize(fs.readFileSync(f))), f + " changed");
  frozen[f] = sha(normalize(old.stdout));
}
fs.writeFileSync(
  out + "/verification.json",
  JSON.stringify(
    {
      engine: d.engine,
      strokes: d.groups.reduce((n, g) => n + g.strokes.length, 0),
      width: 384,
      hosts,
      strokeHash,
      imageHash,
      planes: n[2].result,
      frozenBase: "fb118dc",
      frozen,
    },
    null,
    2,
  ) + "\n",
);
console.log("PASS: native / Node / Chromium / Firefox; source roundtrip; kernel and catalog unchanged");
