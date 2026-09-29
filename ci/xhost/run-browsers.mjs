// Browser side of the cross-host check, through Playwright:
//   node ci/xhost/run-browsers.mjs [--browsers chromium,firefox,webkit] [--wasm FILE] [--out-dir DIR]
// Serves page.html, wasm-host.mjs and the WASM from 127.0.0.1 (a secure context, so crypto.subtle exists), loads
// the WASM with instantiateStreaming as a CDN build would, and hashes with WebCrypto. WebKit is best-effort: a
// launch failure is reported and skipped; a hash difference in any browser that ran is not.
import { createServer } from "node:http";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { args, OUT_DIR, WASM_DEFAULT } from "./paths.mjs";

const opt = args(process.argv.slice(2), { browsers: "chromium,firefox,webkit", wasm: WASM_DEFAULT, outDir: OUT_DIR });
const here = dirname(fileURLToPath(import.meta.url));
const files = {
  "/page.html": [join(here, "page.html"), "text/html; charset=utf-8"],
  "/wasm-host.mjs": [join(here, "wasm-host.mjs"), "text/javascript; charset=utf-8"],
  "/oil_xhost.wasm": [opt.wasm, "application/wasm"],
};
const server = createServer((req, res) => {
  const f = files[new URL(req.url, "http://x").pathname];
  if (!f) return res.writeHead(404).end();
  res.writeHead(200, { "content-type": f[1], "cache-control": "no-store" }).end(readFileSync(f[0]));
});
await new Promise((ok) => server.listen(0, "127.0.0.1", ok));
const url = `http://127.0.0.1:${server.address().port}/page.html`;

const pw = await import("playwright");
const bestEffort = new Set(["webkit"]);
let failed = false;
for (const name of opt.browsers.split(",").filter(Boolean)) {
  let browser;
  try {
    browser = await pw[name].launch();
  } catch (err) {
    console.error(`xhost: ${name} did not launch: ${String(err.message).split("\n")[0]}`);
    if (!bestEffort.has(name)) failed = true;
    continue;
  }
  try {
    const page = await browser.newPage();
    const errors = [];
    page.on("pageerror", (e) => errors.push(String(e)));
    await page.goto(url);
    await page.waitForFunction(() => window.xhostReady === true);
    const result = await page.evaluate(() => window.runXhost());
    if (errors.length) throw new Error(errors.join("; "));
    const host = `${name}-${browser.version()}`;
    const out = join(opt.outDir, `${host}.json`);
    mkdirSync(opt.outDir, { recursive: true });
    writeFileSync(out, JSON.stringify({ host, runtime: "wasm", ...result }) + "\n");
    console.error(`xhost: ${Object.keys(result.cases).length} cases in ${host} -> ${out}`);
  } catch (err) {
    console.error(`xhost: ${name} failed: ${err.message}`);
    failed = true;
  } finally {
    await browser.close();
  }
}
server.close();
process.exit(failed ? 1 : 0);
