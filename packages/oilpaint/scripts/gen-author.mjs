import { compile } from "json-schema-to-typescript";
import { readFileSync, writeFileSync } from "node:fs";
const schema = JSON.parse(readFileSync("spec/author-1.schema.json", "utf8").replace(/^\uFEFF/, ""));
writeFileSync(
  "packages/oilpaint/src/author-types.ts",
  await compile(schema, "Document", {
    bannerComment: "// Generated from Rust oil-author via spec/author-1.schema.json. Do not edit.",
    additionalProperties: false,
    unreachableDefinitions: true,
  }),
);
