import type { Composition } from "./composition-types.ts";
export type { Composition, PlannedGroup, ResolvedStroke, BrushParams } from "./composition-types.ts";
import type { Engine, MixerId, RgbImage } from "./engine.ts";
import type { Document, Catalog, Group, Mark } from "./author-types.ts";
export type { Document, Catalog, Group, Mark, Preset } from "./author-types.ts";
export interface View {
  mode: "lit" | "unlit" | "height";
  direction: [number, number, number];
  bump: number;
  contrast: number;
  specular: number;
}
export const softView: View = {
  mode: "lit",
  direction: [-0.5, -0.6, 0.62],
  bump: 0.65,
  contrast: 0.19,
  specular: 0.025,
};
export const defaultView: View = {
  mode: "lit",
  direction: [-0.55, -0.6, 0.35],
  bump: 1.05,
  contrast: 0.45,
  specular: 0.12,
};
export const rakingView: View = {
  mode: "lit",
  direction: [-0.8, -0.35, 0.18],
  bump: 1.05,
  contrast: 0.6,
  specular: 0.25,
};
export function createAuthor(engine: Engine) {
  return {
    catalog: (): Catalog => engine.author({ op: "catalog" }),
    validateCatalog: (catalog: unknown): Catalog => engine.author({ op: "validateCatalog", catalog }),
    document(seed = 1907): Document {
      return {
        format: "oil-author",
        version: 2,
        engine: engine.version,
        seed,
        aspect: [3, 2],
        ground: [0.68, 0.65, 0.59],
        catalog: this.catalog(),
        groups: [],
      };
    },
    compile(document: Document | Composition): Uint8Array {
      engine.author({ op: "compile", document });
      return engine.authorBytes("strokes");
    },
    render(
      document: Document | Composition,
      width = 320,
      mixer: MixerId = "ochrell",
    ): { width: number; height: number; reusedGroups: number } {
      return engine.author({ op: "render", document, width, mixer });
    },
    view(view: View = defaultView): RgbImage {
      const size = engine.author({ op: "view", view });
      return { ...size, data: engine.authorBytes("image") };
    },
    hashes: (): Record<string, string> => engine.author({ op: "hashes" }),
    parse(text: string): Document | Composition {
      const d = JSON.parse(text);
      this.compile(d);
      return d;
    },
  };
}
/** Immutable, ID-selected group update. Unselected groups and marks keep identical authored values. */
export function editGroup(d: Document, id: string, edit: (g: Group) => void): Document {
  const next = structuredClone(d);
  const g = next.groups.find((g) => g.id === id);
  if (!g) throw new Error(`UNKNOWN_GROUP: ${id}`);
  edit(g);
  return next;
}
export function moveGroup(d: Document, id: string, index: number): Document {
  if (!Number.isInteger(index) || index < 0 || index >= d.groups.length) throw new Error("INVALID_ORDER");
  const next = structuredClone(d);
  const old = next.groups.findIndex((g) => g.id === id);
  if (old < 0) throw new Error(`UNKNOWN_GROUP: ${id}`);
  const [g] = next.groups.splice(old, 1);
  next.groups.splice(index, 0, g);
  return next;
}
/** Independent counter draw for procedural authors. Give each subject a permanent ID. */
export function scopedRandom(seed: number, scopes: string[], counter = 0): number {
  let h = (2166136261 ^ seed) >>> 0;
  for (const s of [...scopes, String(counter)]) {
    const b = new TextEncoder().encode(s);
    const len = new Uint8Array(4);
    new DataView(len.buffer).setUint32(0, b.length, true);
    for (const v of [...len, ...b]) h = Math.imul(h ^ v, 16777619) >>> 0;
  }
  return h / 4294967296;
}
export function sampleMark(id: string, preset: string, width: number, kind = "straight", y = 0.3): Mark {
  const path: [number, number, number][] = [];
  for (let i = 0; i <= 32; i++) {
    const t = i / 32;
    path.push([
      kind === "twist"
        ? 0.4 + 0.18 * t
        : kind === "dab"
          ? 0.5 + (t - 0.5) * width * 1.2
          : kind === "reverse"
            ? 0.88 - 0.76 * t
            : 0.12 + 0.76 * t,
      y +
        (kind === "twist"
          ? 0.15 * (2 * t - 1) * (1 - (2 * t - 1) * (2 * t - 1))
          : kind === "curve"
            ? 0.16 * 4 * t * (1 - t)
            : 0),
      kind === "dab" || kind === "twist"
        ? 0.1 + 0.9 * 4 * t * (1 - t)
        : kind === "pressure"
          ? 0.08 + 0.92 * t
          : kind === "taper"
            ? 1 - 0.98 * t
            : 1,
    ]);
  }
  return {
    id,
    preset,
    width,
    path,
    color: [0.87, 0.42, 0.2],
    controls: kind === "runout" ? { load: 0.65, deplete: 0.12 } : {},
  };
}
