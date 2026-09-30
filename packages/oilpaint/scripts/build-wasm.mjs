// Build crates/oil-wasm for wasm32 and copy it to packages/oilpaint/wasm/oil.wasm.
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../../", import.meta.url));
execFileSync("cargo", ["build", "--release", "-p", "oil-wasm", "--lib", "--target", "wasm32-unknown-unknown"], { cwd: root, stdio: "inherit" });
const src = `${root}target/wasm32-unknown-unknown/release/oil_wasm.wasm`;
const dst = fileURLToPath(new URL("../wasm/oil.wasm", import.meta.url));
mkdirSync(fileURLToPath(new URL("../wasm/", import.meta.url)), { recursive: true });
copyFileSync(src, dst);
console.log(JSON.stringify({ wasm: "packages/oilpaint/wasm/oil.wasm", bytes: statSync(dst).size }));
