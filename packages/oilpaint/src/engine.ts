// The Rust engine as WASM (crates/oil-wasm): ScenePlan validation and the guide compiler (L2). The module has no
// imports and no bindgen glue: input goes into a buffer (`oil_input`), a call returns a byte length, and the result
// is read at `oil_buf_ptr()`.
import type { ScenePlan } from "./sceneplan.ts";
import type { Scene } from "./scene.ts";

/** A structured error or warning (spec/ERRORS.md). */
export interface OilIssue {
  code: string;
  message: string;
  path?: string;
  got?: string;
  expected?: string;
  fix?: string;
}

export class OilError extends Error {
  readonly code: string;
  readonly path?: string;
  readonly issues: OilIssue[];
  constructor(issues: OilIssue[]) {
    super(issues.map((e) => `${e.code}: ${e.message}${e.path ? ` (at ${e.path})` : ""}${e.fix ? ` ${e.fix}` : ""}`).join("\n"));
    this.issues = issues;
    this.code = issues[0]?.code ?? "ERROR";
    this.path = issues[0]?.path;
  }
}

export interface Validation {
  valid: boolean;
  errors: OilIssue[];
  warnings: OilIssue[];
}

export const MIXERS = ["ochrell", "rgb"] as const;
export type MixerId = (typeof MIXERS)[number];

export interface GuidesSummary {
  engine: string;
  mixer: MixerId;
  size: [number, number];
  ground: [number, number, number];
  regions: { id: number; name: string; pixels: number; share: number; flow: boolean }[];
  sha256: Record<"target" | "regionId" | "masks" | "flow" | "regionFlows" | "light", string>;
  warnings: OilIssue[];
}

export interface RgbImage {
  width: number;
  height: number;
  /** RGB8, row-major. */
  data: Uint8Array;
}

export type PreviewName = "target" | "regions" | "flow" | "light" | "sheet";
const PREVIEWS: Record<PreviewName, number> = { target: 10, regions: 11, flow: 12, light: 13, sheet: 14 };

/** Guide maps of one compile. They live in the engine until the next compile, which makes this object stale. */
export interface Guides extends GuidesSummary {
  /** sRGB target, 3 floats per pixel. */
  target(): Float32Array;
  regionId(): Uint8Array;
  /** Soft masks, one plane per region in region order. */
  masks(): Float32Array;
  /** Unit flow, 2 floats per pixel. */
  flow(): Float32Array;
  light(): Float32Array;
  /** A region's own authored flow over the whole canvas, or null. */
  regionFlow(name: string): Float32Array | null;
  preview(name: PreviewName): RgbImage;
}

export interface Engine {
  readonly version: string;
  schema(): object;
  validate(spec: ScenePlan | Scene | string): Validation;
  /** Compile guides at `width` px. Throws OilError on invalid input. */
  guides(scene: ScenePlan | Scene | string, o?: { width?: number; mixer?: MixerId }): Guides;
}

type Exports = {
  memory: WebAssembly.Memory;
  oil_buf_ptr(): number;
  oil_input(len: number): number;
  oil_engine_version(): number;
  oil_scene_schema(): number;
  oil_scene_validate(): number;
  oil_field_add(): number;
  oil_fields_clear(): void;
  oil_guides(width: number, mixer: number): number;
  oil_guides_plane(which: number): number;
};

const isNode = typeof process !== "undefined" && !!process.versions?.node;

async function wasmBytes(source?: string | URL | BufferSource): Promise<BufferSource> {
  if (source && typeof source !== "string" && !(source instanceof URL)) return source;
  const url = source ? new URL(source, import.meta.url) : new URL("../wasm/oil.wasm", import.meta.url);
  if (isNode && url.protocol === "file:") {
    const { readFile } = await import("node:fs/promises");
    return readFile(url);
  }
  const res = await fetch(url);
  if (!res.ok) throw new OilError([{ code: "IO", message: `cannot load ${url}: HTTP ${res.status}` }]);
  return res.arrayBuffer();
}

function specOf(s: ScenePlan | Scene | string): { text: string; fields?: Map<string, Float32Array> } {
  if (typeof s === "string") return { text: s };
  if ("spec" in s) return { text: JSON.stringify(s.spec), fields: s.fields };
  return { text: JSON.stringify(s) };
}

/** Load the engine. `source`: a path or URL of oil.wasm, or its bytes (default: the package's wasm/oil.wasm). */
export async function loadEngine(source?: string | URL | BufferSource): Promise<Engine> {
  const { instance } = await WebAssembly.instantiate(await wasmBytes(source), {});
  const e = instance.exports as unknown as Exports;
  const bytes = (len: number) => new Uint8Array(e.memory.buffer, e.oil_buf_ptr() >>> 0, len >>> 0).slice();
  const text = (len: number) => new TextDecoder().decode(bytes(len));
  const json = (len: number) => JSON.parse(text(len));
  const input = (data: Uint8Array) => {
    const ptr = e.oil_input(data.length) >>> 0; // may grow the memory: take the buffer only after the call
    new Uint8Array(e.memory.buffer, ptr, data.length).set(data);
  };
  const version = text(e.oil_engine_version());
  let generation = 0;

  const f32 = (b: Uint8Array) => new Float32Array(b.buffer, b.byteOffset, b.byteLength / 4);

  return {
    version,
    schema: () => json(e.oil_scene_schema()),
    validate(s) {
      input(new TextEncoder().encode(specOf(s).text));
      return json(e.oil_scene_validate());
    },
    guides(s, o = {}) {
      const { text: spec, fields } = specOf(s);
      e.oil_fields_clear();
      for (const [name, data] of fields ?? []) {
        const nb = new TextEncoder().encode(name);
        const buf = new Uint8Array(4 + nb.length + data.length * 4);
        const dv = new DataView(buf.buffer);
        dv.setUint32(0, nb.length, true);
        buf.set(nb, 4);
        data.forEach((v, i) => dv.setFloat32(4 + nb.length + i * 4, v, true));
        input(buf);
        e.oil_field_add();
      }
      input(new TextEncoder().encode(spec));
      const mixer = MIXERS.indexOf(o.mixer ?? "ochrell");
      if (mixer < 0) throw new OilError([{ code: "UNKNOWN_MIXER", message: `mixer ${o.mixer} is not in this build`, got: String(o.mixer), expected: MIXERS.join(", ") }]);
      const summary = json(e.oil_guides(o.width ?? 600, mixer));
      if (summary.errors) throw new OilError(summary.errors);
      const gen = ++generation;
      const plane = (which: number) => {
        if (gen !== generation) throw new OilError([{ code: "STALE_GUIDES", message: "these guides were replaced by a later compile", fix: "copy planes out before compiling again" }]);
        return bytes(e.oil_guides_plane(which));
      };
      const s2 = summary as GuidesSummary;
      return {
        ...s2,
        target: () => f32(plane(0)),
        regionId: () => plane(1),
        masks: () => f32(plane(2)),
        flow: () => f32(plane(3)),
        light: () => f32(plane(4)),
        regionFlow(name) {
          const r = s2.regions.find((r) => r.name === name);
          return r && r.flow ? f32(plane(100 + r.id)) : null;
        },
        preview(name) {
          const b = plane(PREVIEWS[name]);
          const dv = new DataView(b.buffer, b.byteOffset);
          return { width: dv.getUint32(0, true), height: dv.getUint32(4, true), data: b.subarray(8) };
        },
      };
    },
  };
}
