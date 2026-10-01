// Local-only development server; TypeScript is stripped by Node 24.
import http from "node:http";
import fs from "node:fs/promises";
import path from "node:path";
import { stripTypeScriptTypes } from "node:module";
const root = process.cwd();
export const server = http.createServer(async (req, res) => {
  try {
    const pathname = decodeURIComponent(new URL(req.url, "http://localhost").pathname);
    const file = path.resolve(root, "." + (pathname === "/" ? "/packages/oilpaint/preview/index.html" : pathname));
    if (!file.startsWith(root + path.sep)) throw Error("outside root");
    const ext = path.extname(file);
    let body = await fs.readFile(file);
    if (ext === ".ts") body = Buffer.from(stripTypeScriptTypes(body.toString(), { mode: "strip" }));
    res.setHeader(
      "Content-Type",
      {
        ".ts": "text/javascript",
        ".js": "text/javascript",
        ".html": "text/html",
        ".css": "text/css",
        ".wasm": "application/wasm",
        ".json": "application/json",
        ".png": "image/png",
      }[ext] ?? "application/octet-stream",
    );
    res.setHeader("Cache-Control", "no-store");
    res.end(body);
  } catch (e) {
    res.statusCode = 404;
    res.end(String(e));
  }
});
server.listen(Number(process.env.PORT ?? 4173), "127.0.0.1", () =>
  console.log("Brush preview: http://127.0.0.1:" + server.address().port),
);
