// WASM boundary cost for a planner stage: 13,795 path traces as one batched call vs one call per stroke
import fs from 'fs';
const D = new URL('./data/', import.meta.url).pathname;
const meta = JSON.parse(fs.readFileSync(D + 'meta.json')); const { W, H } = meta;
const { instance } = await WebAssembly.instantiate(fs.readFileSync(new URL('./rs/target/wasm32-unknown-unknown/release/planspike.wasm', import.meta.url).pathname), {});
const ex = instance.exports; ex.ps_init();
const put = (name) => { const b = fs.readFileSync(D + name + '.bin'); const p = ex.ps_alloc(b.length); new Uint8Array(ex.memory.buffer, p, b.length).set(b); return p; };
const mask = put('mask'), flow = put('flow'), jobs = put('jobs'); const pts = ex.ps_alloc(4096 * 4);
for (let rep = 0; rep < 3; rep++) {
  let t0 = performance.now(); const a = ex.ps_paths(jobs, meta.n_jobs, flow, mask, W, H, 0.8, 0.35, 0.9, 7n, pts); const tb = performance.now() - t0;
  t0 = performance.now(); let b = 0n;
  for (let j = 0; j < meta.n_jobs; j++) {
    b += ex.ps_paths(jobs + 16 * j, 1, flow, mask, W, H, 0.8, 0.35, 0.9, BigInt(7 + j), pts);
    const view = new Float32Array(ex.memory.buffer, pts, 64); if (view[0] === 12345) console.log('');  // read result back each call
  }
  const ts = performance.now() - t0;
  console.log(`rep ${rep}: batched ${tb.toFixed(1)} ms (${a} pts) | per-stroke calls ${ts.toFixed(1)} ms (${b} pts) -> ${(1000 * (ts - tb) / meta.n_jobs).toFixed(2)} us overhead per call`);
}
