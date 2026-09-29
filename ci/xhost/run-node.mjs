// Node side of the cross-host check: node ci/xhost/run-node.mjs [--wasm FILE] [--out FILE] [--dump-dir DIR]
// (--dump-dir also writes each case's bytes, to diagnose a difference against the native dump)
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { performance } from "node:perf_hooks";
import { importsOf, runXhost } from "./wasm-host.mjs";
import { args, WASM_DEFAULT } from "./paths.mjs";

const opt = args(process.argv.slice(2), { wasm: WASM_DEFAULT, out: null, dumpDir: null });
const host = `node-${process.version}-${process.platform}-${process.arch}`;
const module = new WebAssembly.Module(readFileSync(opt.wasm));
const imports = importsOf(module);
if (imports.length) throw new Error(`oil_xhost.wasm must not import anything, found: ${imports.join(", ")}`);
const instance = new WebAssembly.Instance(module, {});
const sha = async (bytes, name) => {
  if (opt.dumpDir) {
    mkdirSync(opt.dumpDir, { recursive: true });
    writeFileSync(join(opt.dumpDir, `${name}.bin`), bytes);
  }
  return createHash("sha256").update(bytes).digest("hex");
};
const report = { host, runtime: "wasm", ...(await runXhost(instance, sha, () => performance.now())) };
const out = opt.out ?? `out/xhost/${host}.json`;
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, JSON.stringify(report) + "\n");
console.error(`xhost: ${Object.keys(report.cases).length} cases in ${host} -> ${out}`);
