// One command for the whole cross-host check on this machine (npm run xhost):
// build native + WASM, run the cases natively, in Node and in the browsers, then compare.
//   node ci/xhost/xhost.mjs [--browsers chromium,firefox,webkit] [--skip-browsers] [--write-golden]
import { spawnSync } from "node:child_process";
import { rmSync } from "node:fs";
import { args, OUT_DIR } from "./paths.mjs";

const opt = args(process.argv.slice(2), { browsers: "chromium,firefox,webkit", skipBrowsers: false, writeGolden: false });
const run = (cmd, argv) => {
  console.error(`> ${cmd} ${argv.join(" ")}`);
  const r = spawnSync(cmd, argv, { stdio: "inherit", shell: false });
  if (r.error) throw r.error;
  return r.status;
};
const must = (cmd, argv) => {
  if (run(cmd, argv) !== 0) process.exit(1);
};

rmSync(OUT_DIR, { recursive: true, force: true });
must("cargo", ["build", "--release", "-p", "oil-xhost", "--lib", "--target", "wasm32-unknown-unknown"]);
must("cargo", ["run", "--release", "-q", "-p", "oil-xhost", "--bin", "xhost", "--", "--out", `${OUT_DIR}/native-${process.platform}-${process.arch}.json`]);
must(process.execPath, ["ci/xhost/run-node.mjs"]);
let browsersOk = true;
if (!opt.skipBrowsers) browsersOk = run(process.execPath, ["ci/xhost/run-browsers.mjs", "--browsers", opt.browsers]) === 0;
const compare = ["ci/xhost/compare.mjs", "--dir", OUT_DIR];
if (opt.writeGolden) compare.push("--write-golden");
const ok = run(process.execPath, compare) === 0;
process.exit(ok && browsersOk ? 0 : 1);
