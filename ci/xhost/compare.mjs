// Compares the per-host reports of the cross-host check (G1 in docs/plans/LIBRARY_PLAN.md, section 5).
//   node ci/xhost/compare.mjs [--dir out/xhost] [--golden-dir golden] [--write-golden] [--min-hosts 2]
// Fails (exit 1) when hosts ran different engine versions, a case is missing on a host, an "identical" case has
// more than one digest, or a digest differs from golden/<engine>.json. "may_differ" cases are negative controls:
// reported, never failing. --write-golden records the agreed digests of a fully agreeing run; it may add cases
// but never changes an existing digest (that needs an engine-version bump).
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { args, OUT_DIR } from "./paths.mjs";

const opt = args(process.argv.slice(2), { dir: OUT_DIR, goldenDir: "golden", writeGolden: false, minHosts: "2" });
const reports = readdirSync(opt.dir)
  .filter((f) => f.endsWith(".json") && f !== "report.json")
  .map((f) => JSON.parse(readFileSync(join(opt.dir, f), "utf8")))
  .sort((a, b) => a.host.localeCompare(b.host));
const problems = [];
if (reports.length < Number(opt.minHosts)) problems.push(`only ${reports.length} host report(s) in ${opt.dir}`);
const engines = [...new Set(reports.map((r) => r.engine))];
if (engines.length > 1) problems.push(`hosts ran different engine versions: ${engines.join(", ")}`);
const engine = engines[0];

const names = [...new Set(reports.flatMap((r) => Object.keys(r.cases)))].sort();
const goldenPath = join(opt.goldenDir, `${engine}.json`);
const golden = existsSync(goldenPath) ? JSON.parse(readFileSync(goldenPath, "utf8")) : null;
const rows = [];
const agreed = {};
for (const name of names) {
  const got = reports.map((r) => r.cases[name]);
  const expect = got.find(Boolean).expect;
  const missing = reports.filter((r, i) => !got[i]).map((r) => r.host);
  const digests = [...new Set(got.filter(Boolean).map((c) => c.sha256))];
  let verdict;
  if (missing.length) {
    verdict = `FAIL: missing on ${missing.join(", ")}`;
    problems.push(`${name}: ${verdict}`);
  } else if (expect === "may_differ") {
    verdict = digests.length > 1 ? `differs (negative control, ${digests.length} digests)` : "agrees (negative control)";
  } else if (digests.length > 1) {
    verdict = `FAIL: ${digests.length} different digests`;
    problems.push(`${name}: ${verdict}`);
  } else {
    agreed[name] = digests[0];
    const g = golden?.cases?.[name];
    if (golden && g === undefined) verdict = "identical (not in golden)";
    else if (golden && g !== digests[0]) {
      verdict = "FAIL: differs from golden";
      problems.push(`${name}: identical on every host but differs from ${goldenPath}`);
    } else verdict = golden ? "identical, matches golden" : "identical";
  }
  rows.push({ name, expect, digests: got.map((c) => (c ? c.sha256.slice(0, 12) : "-")), verdict });
}

const table = [
  `Cross-host check, engine ${engine}, ${reports.length} hosts` + (golden ? `, golden ${goldenPath}` : ", no golden for this engine version"),
  "",
  `| case | expect | ${reports.map((r) => r.host).join(" | ")} | verdict |`,
  `|---|---|${reports.map(() => "---|").join("")}---|`,
  ...rows.map((r) => `| ${r.name} | ${r.expect} | ${r.digests.join(" | ")} | ${r.verdict} |`),
  "",
  problems.length ? `FAIL (${problems.length}):\n${problems.map((p) => `- ${p}`).join("\n")}` : "PASS",
  "",
].join("\n");
mkdirSync(opt.dir, { recursive: true });
writeFileSync(join(opt.dir, "report.md"), table);
writeFileSync(join(opt.dir, "report.json"), JSON.stringify({ engine, hosts: reports.map((r) => r.host), rows, problems }, null, 1) + "\n");
console.log(table);

if (opt.writeGolden) {
  if (problems.length) {
    console.error("not writing golden: the run has failures");
    process.exit(1);
  }
  const cases = { ...(golden?.cases ?? {}) };
  for (const [name, digest] of Object.entries(agreed)) {
    if (cases[name] !== undefined && cases[name] !== digest) {
      console.error(`not writing golden: ${name} would change within engine ${engine}; bump the engine version`);
      process.exit(1);
    }
    cases[name] = digest;
  }
  const sorted = Object.fromEntries(Object.keys(cases).sort().map((k) => [k, cases[k]]));
  mkdirSync(opt.goldenDir, { recursive: true });
  writeFileSync(goldenPath, JSON.stringify({ engine, hosts: reports.map((r) => r.host), cases: sorted }, null, 1) + "\n");
  console.error(`wrote ${goldenPath}`);
}
process.exit(problems.length ? 1 : 0);
