import { compile } from "json-schema-to-typescript";
import { readFileSync, writeFileSync } from "node:fs";
const schema = JSON.parse(
  readFileSync("spec/composition-1.schema.json", "utf8").replace(/^\uFEFF/, ""),
);
writeFileSync(
  "packages/oilpaint/src/composition-types.ts",
  await compile(schema, "Composition", {
    bannerComment:
      "// Generated from oil-author::Composition / spec/composition-1.schema.json. Do not edit.",
    additionalProperties: false,
    unreachableDefinitions: true,
  }),
);
