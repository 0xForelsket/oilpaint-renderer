// Runs the oil-xhost WASM cases in any JS host (Node or a browser page) and hashes each case's output bytes with
// the host's own SHA-256. The WASM has no imports and no bindgen glue: results come back through one buffer
// (a call returns a byte length; xhost_buf_ptr() gives its address in linear memory).

export async function runXhost(instance, sha256Hex, now) {
  const e = instance.exports;
  const u32 = (v) => v >>> 0;
  const view = (len) => new Uint8Array(e.memory.buffer, u32(e.xhost_buf_ptr()), len);
  const text = (len) => new TextDecoder().decode(view(len).slice());
  const engine = text(u32(e.xhost_engine_version()));
  const cases = {};
  const n = u32(e.xhost_case_count());
  for (let i = 0; i < n; i++) {
    const name = text(u32(e.xhost_case_name(i)));
    const expect = u32(e.xhost_case_expect(i)) === 0 ? "identical" : "may_differ";
    const t0 = now();
    const len = u32(e.xhost_run(i));
    const ms = now() - t0;
    cases[name] = { sha256: await sha256Hex(view(len), name), bytes: len, expect, ms: Math.round(ms * 1000) / 1000 };
  }
  return { engine, cases };
}

export function importsOf(module) {
  return WebAssembly.Module.imports(module).map((i) => `${i.module}.${i.name}`);
}
