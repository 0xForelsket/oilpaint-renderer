// node run_wasm.mjs <wasm> <corpus> <W> <reps> [dumpPrefix]  -> replay + relight timings; optional raw dumps
import fs from 'fs';
const [wasmPath, corpusPath, Ws, repsS, dump] = process.argv.slice(2);
const W = +Ws, reps = +(repsS || 1);
const bytes = fs.readFileSync(wasmPath);
const { instance } = await WebAssembly.instantiate(bytes, { env: { now_ms: () => performance.now() } });
const ex = instance.exports;
const corpus = fs.readFileSync(corpusPath);
const aspect = new DataView(corpus.buffer, corpus.byteOffset).getFloat32(24, true);
const H = Math.round(W * aspect), n = W * H;
const pc = ex.oc_alloc(corpus.length); new Uint8Array(ex.memory.buffer, pc, corpus.length).set(corpus);
const prgb = ex.oc_alloc(n * 12), ph = ex.oc_alloc(n * 4), pt = ex.oc_alloc(16), plit = ex.oc_alloc(n * 12), pp = ex.oc_alloc(72);
for (let r = 0; r < reps; r++) {
  const t0 = performance.now();
  const npx = ex.oc_replay(pc, corpus.length, W, H, prgb, ph, pt);
  const t1 = performance.now();
  const times = new Float64Array(ex.memory.buffer, pt, 2);
  const lp = new Float32Array(ex.memory.buffer, pp, 18);
  lp.set([-0.5, -0.6, 0.62, 1.4, 1.8, 0.45, 0.10, 22.0, 0.25, 1.4, 0.06, 0.0004, 0.004, 0.0006, 0.06, 0.0, 1.2]);
  new Int32Array(ex.memory.buffer, pp + 68, 1)[0] = 0;
  const t2 = performance.now();
  ex.oc_relight(prgb, ph, 0, W, H, pp, plit);
  const t3 = performance.now();
  console.log(`${wasmPath.split('/').pop()} node ${W}x${H} npx ${Number(npx)} kernel ${(times[0] / 1000).toFixed(3)}s replay ${((t1 - t0) / 1000).toFixed(3)}s relight ${((t3 - t2) / 1000).toFixed(3)}s mem ${(ex.memory.buffer.byteLength / 1e6).toFixed(0)}MB`);
}
if (dump) {
  fs.writeFileSync(dump + '_rgb.f32', Buffer.from(ex.memory.buffer, prgb, n * 12));
  fs.writeFileSync(dump + '_h.f32', Buffer.from(ex.memory.buffer, ph, n * 4));
  fs.writeFileSync(dump + '_lit.f32', Buffer.from(ex.memory.buffer, plit, n * 12));
}
