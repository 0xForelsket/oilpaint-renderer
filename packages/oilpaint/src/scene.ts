// The scene DSL: builds a ScenePlan (spec/SCENEPLAN_V1.md), which is plain JSON. The helpers mirror v1's Python
// DSL (oilpaint/scene.py) with camelCase names, so scenes port mechanically. Coordinates are canvas widths (cw):
// x in [0, 1], y down to the aspect height (1.25 for 4:5); angles in degrees, 0 = +x, 90 = down.
//
//   export default scene((S) => {
//     S.canvas({ aspect: [4, 5], ground: "#e9e1d6" });
//     S.target.fill(gradientV([[0, "#232a58"], [0.6, "#e8cbb8"]]));
//     S.region("sky", above(0.6), { edge: 0.03, flow: sweep({ angle: -14 }) });
//     S.style("sky", { width: [0.025, 0.04] });
//     S.layers([{ name: "Sky", regions: ["sky"], errorThreshold: 12 }]);
//   });
import type {
  Beam, Blob, ColorSpec, ContourFlow, FieldKind, Flow, Glow, Layer, Light, LightBeam, LightGlow, LightLamp, Noisy,
  Fit, ImageOp, PolygonOp, Region, ScenePlan, Shape, Style, SweepFlow, TargetOp, WavesFlow,
} from "./sceneplan.ts";
import { sha256Hex } from "./sha256.ts";

export type * from "./sceneplan.ts";

export type Point = [number, number];
type Opt<T, K extends keyof T> = Partial<Omit<T, K>>;

/** The ScenePlan generation this DSL writes. */
export const SCENEPLAN = 1;

/** Drop undefined keys so the spec only carries what the author set (the engine fills defaults). */
function clean<T extends object>(o: T): T {
  return Object.fromEntries(Object.entries(o).filter(([, v]) => v !== undefined)) as T;
}

// ------------------------------------------------------------------ shapes (masks in [0, 1])

export const all = (): Shape => ({ all: true });
/** y < y0 */
export const above = (y0: number): Shape => ({ above: y0 });
/** y >= y0 */
export const below = (y0: number): Shape => ({ below: y0 });
export const ellipse = (cx: number, cy: number, rx: number, ry: number, softness?: number): Shape =>
  ({ ellipse: clean({ c: [cx, cy] as Point, r: [rx, ry] as Point, softness }) });
export const disc = (c: Point, r: number, softness?: number): Shape => ({ disc: clean({ c, r, softness }) });
export const polygon = (pts: Point[]): Shape => ({ polygon: pts });
/** A soft sector from `apex` pointing at `angle`, `spread` degrees either side, `length` long. */
export const wedge = (apex: Point, angle: number, spread: number, length: number): Shape =>
  ({ wedge: { apex, angle, spread, length } });
/** Pixels within `dist` of a polygon's outline. */
export const bandAround = (pts: Point[], dist: number): Shape => ({ bandAround: { polygon: pts, dist } });
export const union = (...shapes: Shape[]): Shape => ({ union: shapes });
export const intersect = (...shapes: Shape[]): Shape => ({ intersect: shapes });
export const not = (shape: Shape): Shape => ({ not: shape });
/** Perturb a shape's boundary with noise (breaks straight edges). */
export const noisy = (shape: Shape, o: Opt<Noisy, "shape"> = {}): Shape => ({ noisy: clean({ shape, ...o }) });
/** A sampled mask declared with `S.field(name, "mask", ...)`. */
export const fieldShape = (name: string): Shape => ({ field: name });

// ------------------------------------------------------------------ flows (unit direction fields)

export const constant = (angle: number, o: { noise?: number; seed?: number } = {}): Flow =>
  ({ constant: clean({ angle, ...o }) });
/** Long arcs: base direction plus a slowly varying bend. */
export const sweep = (o: Partial<SweepFlow> = {}): Flow => ({ sweep: clean({ ...o }) });
/** Near-horizontal with an undulation whose wavelength shrinks toward the horizon. */
export const waves = (o: Partial<WavesFlow> = {}): Flow => ({ waves: clean({ ...o }) });
/** Tangential flow around ellipse centres [cx, cy, rx, ry]. */
export const swirlAround = (centres: [number, number, number, number][], o: { strength?: number; noise?: number; seed?: number } = {}): Flow =>
  ({ swirlAround: clean({ centres, ...o }) });
export const radialFrom = (c: Point, o: { noise?: number; seed?: number } = {}): Flow => ({ radialFrom: clean({ c, ...o }) });
export const upward = (o: { noise?: number; seed?: number } = {}): Flow => ({ upward: clean({ ...o }) });
/** Strokes follow a polygon's outline. */
export const contour = (pts: Point[], o: Opt<ContourFlow, "polygon"> = {}): Flow => ({ contour: clean({ polygon: pts, ...o }) });
/** A sampled flow declared with `S.field(name, "flow", ...)`. */
export const fieldFlow = (name: string): Flow => ({ field: name });

// ------------------------------------------------------------------ target

/** A vertical gradient from [y, colour] stops (increasing y). */
export const gradientV = (stops: [number, ColorSpec][]): { gradientV: [number, ColorSpec][] } => ({ gradientV: stops });

/** Target-image operations, applied in order (the reference the planner paints toward). */
export class TargetBuilder {
  readonly ops: TargetOp[] = [];

  /** Replace everything (or blend inside `mask`) with a colour or a `gradientV(...)`. */
  fill(fill: ColorSpec | { gradientV: [number, ColorSpec][] }, o: { mask?: Shape } = {}): this {
    const body = typeof fill === "object" && !Array.isArray(fill) ? fill : { color: fill };
    this.ops.push({ fill: clean({ ...body, mask: o.mask }) });
    return this;
  }

  /** An elliptical blob, alpha-blended (v1: blob(cx, cy, rx, ry, color, ...)). */
  blob(cx: number, cy: number, rx: number, ry: number, color: ColorSpec, o: Opt<Blob, "c" | "r" | "color"> = {}): this {
    this.ops.push({ blob: clean({ c: [cx, cy] as Point, r: [rx, ry] as Point, color, ...o }) });
    return this;
  }

  polygon(points: Point[], color: ColorSpec, o: Opt<PolygonOp, "points" | "color"> = {}): this {
    this.ops.push({ polygon: clean({ points, color, ...o }) });
    return this;
  }

  /** Horizontal colour bands inside a polygon; [from, to] are fractions of the polygon's height. */
  bands(points: Point[], bands: [number, number, ColorSpec][], o: { softness?: number } = {}): this {
    this.ops.push({ bands: clean({ points, bands, ...o }) });
    return this;
  }

  /** Additive light around a point. */
  glow(c: Point, r: number, color: ColorSpec, o: Opt<Glow, "c" | "r" | "color"> = {}): this {
    this.ops.push({ glow: clean({ c, r, color, ...o }) });
    return this;
  }

  /** Additive light in a wedge. */
  /** A picture (for example a photo reference) declared with `S.picture(name, ...)`, replacing the target (inside
   *  `mask`, if given). `fit`: "cover" (default), "contain" or "stretch". */
  image(field: string, o: { fit?: Fit; strength?: number; mask?: Shape } = {}): this {
    this.ops.push({ image: clean<ImageOp>({ field, ...o }) });
    return this;
  }

  beam(apex: Point, angle: number, spread: number, length: number, color: ColorSpec, o: Opt<Beam, "apex" | "angle" | "spread" | "length" | "color"> = {}): this {
    this.ops.push({ beam: clean({ apex, angle, spread, length, color, ...o }) });
    return this;
  }
}

// ------------------------------------------------------------------ sampled fields (escape hatch)

/** A field function: (x, y) in cw to a mask value, an [dx, dy] flow, or an [r, g, b] colour. */
export type FieldFn = (x: number, y: number) => number | [number, number] | [number, number, number];

const CHANNELS: Record<FieldKind, number> = { mask: 1, flow: 2, rgb: 3 };

/** Sample `fn` at the pixel centres of a `width` x `height` grid one canvas width across. */
export function sampleField(kind: FieldKind, fn: FieldFn, width: number, height: number): Float32Array {
  const nc = CHANNELS[kind];
  const out = new Float32Array(width * height * nc);
  for (let j = 0; j < height; j++) {
    for (let i = 0; i < width; i++) {
      const v = fn((i + 0.5) / width, (j + 0.5) / width);
      const k = (j * width + i) * nc;
      if (typeof v === "number") out[k] = v;
      else for (let c = 0; c < nc; c++) out[k + c] = v[c] ?? 0;
    }
  }
  return out;
}

function f32Bytes(a: Float32Array): Uint8Array {
  const out = new Uint8Array(a.length * 4);
  const dv = new DataView(out.buffer);
  a.forEach((v, i) => dv.setFloat32(i * 4, v, true));
  return out;
}

// ------------------------------------------------------------------ the builder

export interface RegionOptions {
  /** Soft-mask blur in cw (default 0.02). */
  edge?: number;
  flow?: Flow;
}

export class SceneBuilder {
  readonly target = new TargetBuilder();
  private plan: Omit<ScenePlan, "target" | "regions" | "styles" | "layers"> = {
    sceneplan: SCENEPLAN,
    canvas: { aspect: [4, 5], ground: "#f2ede3" },
  };
  private regions: Region[] = [];
  private flows = new Map<string, Flow>();
  private lights: Light[] = [];
  private styles: Record<string, Style> = {};
  private layerList: Layer[] = [];
  readonly fields = new Map<string, Float32Array>();

  /** Title and the engine version the scene is tuned for (a different engine warns). */
  /** A style preset (for example "impressionist"): its values sit between the engine's defaults and the region
   *  styles. */
  preset(name: string): this {
    this.plan.preset = name;
    return this;
  }

  /** Embed the shared catalog to pin named brushes in this scene source. */
  brushCatalog(catalog: NonNullable<ScenePlan["brushCatalog"]>): this {
    this.plan.brushCatalog = structuredClone(catalog);
    return this;
  }

  meta(o: { title?: string; engine?: string }): this {
    Object.assign(this.plan, clean(o));
    return this;
  }

  canvas(o: { aspect?: [number, number]; ground?: ColorSpec }): this {
    this.plan.canvas = { ...this.plan.canvas, ...clean(o) };
    return this;
  }

  /** Declare a region; later regions override earlier ones in the hard region map. */
  region(name: string, shape: Shape, o: RegionOptions = {}): this {
    this.regions.push(clean({ name, shape, edge: o.edge, flow: o.flow }));
    return this;
  }

  /** Set a region's flow (v1's S.flow); the region may be declared before or after. */
  flow(region: string, flow: Flow): this {
    this.flows.set(region, flow);
    return this;
  }

  light(l: { glow?: LightGlow; beam?: LightBeam; lamp?: LightLamp }): this {
    if (l.glow) this.lights.push({ glow: l.glow });
    if (l.beam) this.lights.push({ beam: l.beam });
    if (l.lamp) this.lights.push({ lamp: l.lamp });
    return this;
  }

  style(region: string, style: Style): this {
    this.styles[region] = clean(style);
    return this;
  }

  layers(layers: Layer[]): this {
    this.layerList.push(...layers.map((l) => clean(l)));
    return this;
  }

  /** Sample a function into a field at `width` x round(width x aspect) and declare it (escape hatch: the plan is
   *  then marked non-portable, since JS maths may differ between engines). */
  /** Declare a picture as an rgb field: 8-bit RGB or RGBA pixels (as from a canvas's ImageData or a decoded PNG),
   *  or floats in [0, 1]. Use it as the target with `S.target.image(name)`. */
  picture(name: string, img: { width: number; height: number; data: Uint8Array | Uint8ClampedArray | Float32Array; channels?: 3 | 4 }): this {
    const n = img.width * img.height;
    const ch = img.channels ?? (img.data.length === n * 4 ? 4 : 3);
    if (img.data.length !== n * ch) throw new SceneError("RANGE", `picture ${name}: expected ${n * ch} values, got ${img.data.length}`, "/fields/" + name);
    const scale = img.data instanceof Float32Array ? 1 : 1 / 255;
    const data = new Float32Array(n * 3);
    for (let i = 0; i < n; i++) for (let c = 0; c < 3; c++) data[i * 3 + c] = img.data[i * ch + c] * scale;
    this.fields.set(name, data);
    this.plan.fields = { ...this.plan.fields, [name]: { kind: "rgb", width: img.width, height: img.height, sha256: sha256Hex(f32Bytes(data)) } };
    return this;
  }

  field(name: string, kind: FieldKind, fn: FieldFn, width = 300): this {
    const [aw, ah] = this.plan.canvas.aspect;
    const height = Math.floor((width * ah * 2 + aw) / (2 * aw));
    const data = sampleField(kind, fn, width, height);
    this.fields.set(name, data);
    this.plan.fields = { ...this.plan.fields, [name]: { kind, width, height, sha256: sha256Hex(f32Bytes(data)) } };
    return this;
  }

  build(): Scene {
    const names = new Set(this.regions.map((r) => r.name));
    for (const name of this.flows.keys()) {
      if (!names.has(name)) throw new SceneError("UNKNOWN_REGION", `flow for region ${name}, which is not declared`, `/regions`, name);
    }
    const regions = this.regions.map((r) => (this.flows.has(r.name) ? { ...r, flow: this.flows.get(r.name) } : r));
    const spec: ScenePlan = {
      ...this.plan,
      target: this.target.ops,
      regions,
      ...(this.lights.length ? { lights: this.lights } : {}),
      styles: this.styles,
      layers: this.layerList,
    };
    return { spec, fields: this.fields };
  }
}

/** A built scene: the ScenePlan JSON and any sampled field arrays. */
export interface Scene {
  spec: ScenePlan;
  fields: Map<string, Float32Array>;
}

export class SceneError extends Error {
  readonly code: string;
  readonly path?: string;
  readonly got?: unknown;
  constructor(code: string, message: string, path?: string, got?: unknown) {
    super(`${code}: ${message}`);
    this.code = code;
    this.path = path;
    this.got = got;
  }
}

/** Build a scene with the DSL. */
export function scene(build: (S: SceneBuilder) => void): Scene {
  const S = new SceneBuilder();
  build(S);
  return S.build();
}
