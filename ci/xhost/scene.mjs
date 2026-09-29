// Paint a real StrokeList in every host: time it (single thread) and check that the display plane is bit-identical
// to the native CLI's (G1 on a full scene, not just the test sheet).
//   node ci/xhost/scene.mjs --strokes FILE.oilstrokes [--width 600] [--mixer ochrell|rgb|mixbox]
//                           [--hosts native,node,chromium,firefox] [--out out/xhost/scene.json]
// The native side is `oil paint` (build it with --features mixbox for the mixbox mixer); the WASM side is
// oil_xhost.wasm's xhost_input/xhost_paint exports.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { createServer } from "node:http";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { performance } from "node:perf_hooks";
import { args, WASM_DEFAULT } from "./paths.mjs";

const opt = args(process.argv.slice(2), { strokes: null, width: "600", mixer: "ochrell", hosts: "native,node,chromium,firefox", out: "out/xhost/scene.json", wasm: WASM_DEFAULT });
if (!opt.strokes) throw new Error("--strokes FILE is required");
const MIXERS = { ochrell: 0, rgb: 1, mixbox: 2 };
const width = Number(opt.width);
const here = dirname(fileURLToPath(import.meta.url));
const results = [];

// Runs inside Node and inside the browser page (serialised with toString), so it must be self-contained.
async function paintInWasm(wasmBytes, strokes, width, mixer, sha256Hex, now) {
  const { instance } = await WebAssembly.instantiate(wasmBytes, {});
  const e = instance.exports;
  const u32 = (v) => v >>> 0;
  const ptr = u32(e.xhost_input(strokes.length));
  new Uint8Array(e.memory.buffer, ptr, strokes.length).set(strokes);
  const t0 = now();
  const len = u32(e.xhost_paint(width, mixer));
  const ms = now() - t0;
  const out = new Uint8Array(e.memory.buffer, u32(e.xhost_buf_ptr()), len);
  if (len === 0) throw new Error("paint failed: " + new TextDecoder().decode(new Uint8Array(e.memory.buffer, u32(e.xhost_buf_ptr()), 400)));
  return { ms, sha256: await sha256Hex(out), bytes: len, memoryMB: e.memory.buffer.byteLength / 1048576 };
}

for (const host of opt.hosts.split(",").filter(Boolean)) {
  try {
    if (host === "native") {
      const exe = process.platform === "win32" ? "target/release/oil.exe" : "target/release/oil";
      const r = spawnSync(exe, ["paint", opt.strokes, "--width", String(width), "--mixer", opt.mixer, "--light", "none"], { encoding: "utf8", maxBuffer: 1 << 24 });
      const rep = JSON.parse(r.stdout);
      if (rep.error) throw new Error(rep.error.message);
      results.push({ host: `native-${process.platform}-${process.arch}`, ms: rep.timings.paintSeconds * 1e3, sha256: rep.sha256.rgb });
    } else if (host === "node") {
      const res = await paintInWasm(readFileSync(opt.wasm), readFileSync(opt.strokes), width, MIXERS[opt.mixer],
        async (b) => createHash("sha256").update(b).digest("hex"), () => performance.now());
      results.push({ host: `node-${process.version}`, ...res });
    } else {
      const files = { "/x.wasm": [opt.wasm, "application/wasm"], "/s.bin": [opt.strokes, "application/octet-stream"], "/": [join(here, "page.html"), "text/html"] };
      const server = createServer((req, res) => {
        const f = files[new URL(req.url, "http://x").pathname];
        if (!f) return res.writeHead(404).end();
        res.writeHead(200, { "content-type": f[1] }).end(readFileSync(f[0]));
      });
      await new Promise((ok) => server.listen(0, "127.0.0.1", ok));
      const pw = await import("playwright");
      const browser = host === "msedge" || host === "chrome" ? await pw.chromium.launch({ channel: host }) : await pw[host].launch();
      try {
        const page = await browser.newPage();
        await page.goto(`http://127.0.0.1:${server.address().port}/`);
        const res = await page.evaluate(async ([fn, width, mixer]) => {
          const paint = new Function(`return (${fn})`)();
          const [w, s] = await Promise.all(["/x.wasm", "/s.bin"].map((u) => fetch(u).then((r) => r.arrayBuffer())));
          const hex = (buf) => [...new Uint8Array(buf)].map((b) => b.toString(16).padStart(2, "0")).join("");
          return paint(w, new Uint8Array(s), width, mixer, async (b) => hex(await crypto.subtle.digest("SHA-256", b.slice())), () => performance.now());
        }, [paintInWasm.toString(), width, MIXERS[opt.mixer]]);
        results.push({ host: `${host}-${browser.version()}`, ...res });
      } finally {
        await browser.close();
        server.close();
      }
    }
  } catch (err) {
    results.push({ host, error: String(err.message).split("\n")[0] });
  }
}

const ref = results.find((r) => r.host.startsWith("native"))?.sha256;
console.log(`Storm-scale check: ${opt.strokes} at ${width} px, mixer ${opt.mixer}`);
for (const r of results) {
  const same = r.sha256 && ref ? (r.sha256 === ref ? "identical to native" : "DIFFERENT") : "";
  console.log(r.error ? `  ${r.host}: ERROR ${r.error}` : `  ${r.host.padEnd(26)} paint ${(r.ms / 1000).toFixed(2).padStart(7)} s  ${r.memoryMB ? `wasm memory ${r.memoryMB.toFixed(0)} MB  ` : ""}${r.sha256.slice(0, 12)}  ${same}`);
}
mkdirSync(dirname(opt.out), { recursive: true });
writeFileSync(opt.out, JSON.stringify({ strokes: opt.strokes, width, mixer: opt.mixer, results }, null, 1) + "\n");
const bad = results.some((r) => r.sha256 && ref && r.sha256 !== ref);
process.exit(bad ? 1 : 0);
