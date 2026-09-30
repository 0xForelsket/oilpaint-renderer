// Generate src/sceneplan.ts from spec/sceneplan-1.schema.json (itself generated from the Rust types in
// crates/oil-scene). `--check` fails if the committed file is stale.
import { compile } from "json-schema-to-typescript";
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../../", import.meta.url));
const schema = JSON.parse(readFileSync(`${root}spec/sceneplan-1.schema.json`, "utf8"));
// json-schema-to-typescript intersects a referenced type with a type inferred from its default (NumOrMap & number
// would reject per-region maps), so defaults on references are dropped for type generation only.
(function strip(node) {
  if (Array.isArray(node)) return node.forEach(strip);
  if (node && typeof node === "object") {
    if ("default" in node && (node.allOf || node.$ref || node.anyOf || node.oneOf)) delete node.default;
    Object.values(node).forEach(strip);
  }
})(schema);
const out = fileURLToPath(new URL("../src/sceneplan.ts", import.meta.url));
const banner = "// Generated from spec/sceneplan-1.schema.json by scripts/gen-types.mjs: do not edit.\n// The schema is generated from the Rust types in crates/oil-scene/src/spec.rs.";
let ts = await compile(schema, "ScenePlan", { bannerComment: banner, additionalProperties: false, unreachableDefinitions: true, style: { printWidth: 120 } });
ts = ts.replace(/\r\n/g, "\n");
if (process.argv.includes("--check")) {
  const now = readFileSync(out, "utf8");
  if (now !== ts) {
    console.error("src/sceneplan.ts is stale: run npm run gen:types");
    process.exit(1);
  }
  console.log("src/sceneplan.ts is up to date");
} else {
  writeFileSync(out, ts);
  console.log(`wrote ${out} (${ts.length} bytes)`);
}
