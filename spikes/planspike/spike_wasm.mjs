// node spike_wasm.mjs : the Rust planner hot loops compiled to wasm32, driven from JS
import fs from 'fs';
const D = new URL('./data/', import.meta.url).pathname;
const meta = JSON.parse(fs.readFileSync(D + 'meta.json')); const { W, H } = meta;
const { instance } = await WebAssembly.instantiate(fs.readFileSync(new URL('./rs/target/wasm32-unknown-unknown/release/planspike.wasm', import.meta.url).pathname), {});
const ex = instance.exports; ex.ps_init();
const put = (name) => { const b = fs.readFileSync(D + name + '.bin'); const p = ex.ps_alloc(b.length); new Uint8Array(ex.memory.buffer, p, b.length).set(b); return p; };
const ref = put('ref'), proxy = put('proxy'), mask = put('mask'), flow = put('flow'), palLab = put('pal_lab'), palLat = put('pal_lat'), lut = put('lut'), jobs = put('jobs'), q = put('queries');
const E = ex.ps_alloc(W * H * 4), cells = ex.ps_alloc(W * H * 4), out = ex.ps_alloc(meta.n_q * 12), pts = ex.ps_alloc(4096 * 4);
for (let rep = 0; rep < 2; rep++) {
  let t0 = performance.now(), nc = 0;
  for (let i = 0; i < 27; i++) nc += ex.ps_error_cells(ref, proxy, mask, W, H, 8 + (i % 5) * 3, 12, E, cells);
  const tErr = (performance.now() - t0) / 1000;
  t0 = performance.now(); ex.ps_snap(q, meta.n_q, palLab, palLat, meta.n_pal, lut, 0.85, out); const tSnap = (performance.now() - t0) / 1000;
  t0 = performance.now(); const tot = ex.ps_paths(jobs, meta.n_jobs, flow, mask, W, H, 0.80, 0.35, 0.9, 7n, pts); const tPath = (performance.now() - t0) / 1000;
  console.log(`Rust->WASM(node) rep ${rep}: error map x27 ${tErr.toFixed(3)}s (${nc} cells) | snap x${meta.n_q} ${tSnap.toFixed(3)}s | paths x${meta.n_jobs} ${tPath.toFixed(3)}s (${tot} points)`);
}
