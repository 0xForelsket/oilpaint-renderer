#!/usr/bin/env node
// oilpaint CLI (Node 24+). Every command prints one JSON document on stdout; errors are JSON too
// ({"error": {...}, "errors": [...]}) with exit code 1, usage errors exit with 2.
//
//   oilpaint spec SCENE [--out FILE]              the ScenePlan JSON of a scene module (.ts/.js) or file (.json)
//   oilpaint validate SCENE                       {valid, errors, warnings}
//   oilpaint guides SCENE [--width 600] [--mixer ochrell|rgb] [--out DIR]
//                                                 compile guides; writes guide_*.png, guides_sheet.png, guides.json
//   oilpaint plan SCENE --out FILE.oilstrokes [--width 600] [--seed 1907] [--mixer ochrell|rgb] [--strict-engine]
//                                                 plan a scene into a StrokeList; prints the plan report
//   oilpaint schema                               the ScenePlan v1 JSON Schema
//   oilpaint version
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { extname, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { loadEngine, OilError, type PreviewName } from "./engine.ts";
import { encodePng } from "./png.ts";
import type { Scene } from "./scene.ts";

function usage(msg: string): never {
  console.error(`${msg}\nusage: oilpaint spec SCENE [--out FILE] | validate SCENE | plan SCENE --out FILE [--width W] [--seed N] [--mixer ID] | guides SCENE [--width W] [--mixer ID] [--out DIR] | schema | version`);
  process.exit(2);
}

function fail(err: unknown): never {
  const errors = err instanceof OilError ? err.issues : [{ code: (err as { code?: string }).code ?? "ERROR", message: String((err as Error).message ?? err) }];
  console.log(JSON.stringify({ error: errors[0], errors }));
  process.exit(1);
}

/** A scene module's default export (a Scene from `scene(...)`), or a ScenePlan JSON file. */
export async function loadScene(path: string): Promise<{ scene: Scene; buildMs: number }> {
  const abs = resolve(path);
  const t0 = performance.now();
  if (extname(abs) === ".json") {
    return { scene: { spec: JSON.parse(readFileSync(abs, "utf8")), fields: new Map() }, buildMs: performance.now() - t0 };
  }
  const mod = await import(pathToFileURL(abs).href);
  const scene = mod.default as Scene | undefined;
  if (!scene || typeof scene !== "object" || !("spec" in scene)) {
    throw new OilError([{ code: "NOT_A_SCENE", message: `${path} has no default export from scene(...)`, fix: "export default scene((S) => { ... })" }]);
  }
  return { scene, buildMs: performance.now() - t0 };
}

async function main(argv: string[]) {
  const [cmd, file] = argv;
  const opt = (name: string) => {
    const i = argv.indexOf(name);
    return i >= 0 ? argv[i + 1] : undefined;
  };
  const engine = await loadEngine();
  switch (cmd) {
    case "version":
      console.log(JSON.stringify({ engine: engine.version }));
      return;
    case "schema":
      console.log(JSON.stringify(engine.schema(), null, 2));
      return;
    case "spec": {
      if (!file) usage("spec needs a scene");
      const { scene } = await loadScene(file);
      const text = JSON.stringify(scene.spec, null, 1) + "\n";
      const out = opt("--out");
      if (out) {
        writeFileSync(out, text);
        console.log(JSON.stringify({ file: out, bytes: text.length }));
      } else process.stdout.write(text);
      return;
    }
    case "validate": {
      if (!file) usage("validate needs a scene");
      const { scene } = await loadScene(file);
      const v = engine.validate(scene);
      console.log(JSON.stringify(v));
      if (!v.valid) process.exit(1);
      return;
    }
    case "plan": {
      if (!file) usage("plan needs a scene");
      const out = opt("--out");
      if (!out) usage("plan needs --out FILE.oilstrokes");
      const { scene, buildMs } = await loadScene(file);
      const t0 = performance.now();
      const p = engine.plan(scene, { width: Number(opt("--width") ?? 600), seed: Number(opt("--seed") ?? 1907), mixer: (opt("--mixer") ?? "ochrell") as "ochrell" | "rgb", strictEngine: argv.includes("--strict-engine") });
      const planMs = performance.now() - t0;
      writeFileSync(out, p.strokes);
      console.log(JSON.stringify({ ...p.report, file: out, timings: { buildSpecMs: buildMs, planMs }, host: `node ${process.versions.node}` }));
      return;
    }
    case "guides": {
      if (!file) usage("guides needs a scene");
      const width = Number(opt("--width") ?? 600);
      const mixer = (opt("--mixer") ?? "ochrell") as "ochrell" | "rgb";
      const { scene, buildMs } = await loadScene(file);
      const t0 = performance.now();
      const g = engine.guides(scene, { width, mixer });
      const compileMs = performance.now() - t0;
      const files: string[] = [];
      const out = opt("--out");
      if (out) {
        mkdirSync(out, { recursive: true });
        const names: [PreviewName, string][] = [["target", "guide_target.png"], ["regions", "guide_regions.png"], ["flow", "guide_flow.png"], ["light", "guide_light.png"], ["sheet", "guides_sheet.png"]];
        for (const [name, fname] of names) {
          const img = g.preview(name);
          writeFileSync(resolve(out, fname), encodePng(img.width, img.height, img.data));
          files.push(fname);
        }
      }
      const { target, regionId, masks, flow, light, regionFlow, preview, ...summary } = g;
      void [target, regionId, masks, flow, light, regionFlow, preview];
      const report = { ...summary, timings: { buildSpecMs: buildMs, compileMs }, files, host: `node ${process.versions.node}` };
      if (out) writeFileSync(resolve(out, "guides.json"), JSON.stringify(report, null, 1) + "\n");
      console.log(JSON.stringify(report));
      return;
    }
    default:
      usage(cmd ? `unknown command ${cmd}` : "no command");
  }
}

if (import.meta.main) main(process.argv.slice(2)).catch(fail);
