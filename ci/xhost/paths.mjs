// Shared paths and a tiny --key value argument parser for the xhost scripts.
export const WASM_DEFAULT = "target/wasm32-unknown-unknown/release/oil_xhost.wasm";
export const OUT_DIR = "out/xhost";

export function args(argv, defaults) {
  const opt = { ...defaults };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (!a.startsWith("--")) throw new Error(`unexpected argument ${a}`);
    const key = a.slice(2).replace(/-([a-z])/g, (_, c) => c.toUpperCase());
    if (!(key in defaults)) throw new Error(`unknown option ${a}; known: ${Object.keys(defaults).join(", ")}`);
    if (typeof defaults[key] === "boolean") opt[key] = true;
    else opt[key] = argv[++i];
  }
  return opt;
}
