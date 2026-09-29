// node run_chromium.cjs <wasm> <corpus> <W> [reps] : same replay inside headless Chromium (main thread), SHA-256 of rgb/h
const http = require('http'), fs = require('fs'), path = require('path'), crypto = require('crypto');
const puppeteer = require('/tmp/claude-0/-home-claude/cd4531f6-3314-512b-9953-4f74d55c5a55/scratchpad/render/node_modules/puppeteer-core');
const [wasmPath, corpusPath, Ws, repsS] = process.argv.slice(2);
const root = __dirname;
const srv = http.createServer((req, res) => {
  const p = path.join(root, decodeURIComponent(req.url.split('?')[0]));
  if (!p.startsWith(root) || !fs.existsSync(p)) { res.writeHead(404); return res.end(); }
  const type = p.endsWith('.wasm') ? 'application/wasm' : p.endsWith('.html') ? 'text/html' : 'application/octet-stream';
  res.writeHead(200, { 'Content-Type': type, 'Cross-Origin-Opener-Policy': 'same-origin', 'Cross-Origin-Embedder-Policy': 'require-corp' });
  fs.createReadStream(p).pipe(res);
}).listen(0, async () => {
  const port = srv.address().port;
  fs.writeFileSync(path.join(root, 'blank.html'), '<!doctype html><title>oilcore</title>');
  const browser = await puppeteer.launch({ executablePath: '/opt/pw-browsers/chromium-1194/chrome-linux/chrome', headless: true,
    args: ['--no-sandbox', '--js-flags=--max-old-space-size=4096'], protocolTimeout: 3600000 });
  const page = await browser.newPage();
  page.on('console', m => console.log('[page]', m.text()));
  await page.goto(`http://127.0.0.1:${port}/blank.html`);
  const r = await page.evaluate(async (wasmUrl, corpusUrl, W, reps) => {
    const [wb, cb] = await Promise.all([fetch(wasmUrl).then(r => r.arrayBuffer()), fetch(corpusUrl).then(r => r.arrayBuffer())]);
    const { instance } = await WebAssembly.instantiate(wb, { env: { now_ms: () => performance.now() } });
    const ex = instance.exports, corpus = new Uint8Array(cb);
    const aspect = new DataView(cb).getFloat32(24, true), H = Math.round(W * aspect), n = W * H;
    const pc = ex.oc_alloc(corpus.length); new Uint8Array(ex.memory.buffer, pc, corpus.length).set(corpus);
    const prgb = ex.oc_alloc(n * 12), ph = ex.oc_alloc(n * 4), pt = ex.oc_alloc(16);
    const out = [];
    for (let i = 0; i < reps; i++) {
      const t0 = performance.now(); ex.oc_replay(pc, corpus.length, W, H, prgb, ph, pt); const t1 = performance.now();
      out.push({ kernel: new Float64Array(ex.memory.buffer, pt, 2)[0] / 1000, replay: (t1 - t0) / 1000 });
    }
    const hex = async (p, len) => Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', new Uint8Array(ex.memory.buffer, p, len).slice()))).map(b => b.toString(16).padStart(2, '0')).join('').slice(0, 16);
    return { W, H, crossOriginIsolated: self.crossOriginIsolated, sab: typeof SharedArrayBuffer, runs: out, rgb: await hex(prgb, n * 12), h: await hex(ph, n * 4), mem: ex.memory.buffer.byteLength / 1e6,
             ua: navigator.userAgent.match(/Chrome\/[\d.]+/)[0], cores: navigator.hardwareConcurrency };
  }, '/' + wasmPath, '/' + corpusPath, +Ws, +(repsS || 1));
  const tag = path.basename(wasmPath);
  for (const x of r.runs) console.log(`${tag} chromium(${r.ua}) ${r.W}x${r.H} kernel ${x.kernel.toFixed(3)}s replay ${x.replay.toFixed(3)}s mem ${r.mem.toFixed(0)}MB coi=${r.crossOriginIsolated} cores=${r.cores}`);
  console.log(`sha256 rgb ${r.rgb} h ${r.h}`);
  await browser.close(); srv.close();
});
