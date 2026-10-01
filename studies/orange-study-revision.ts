import type { Catalog, Document, Mark } from "../packages/oilpaint/src/author.ts";
import { orangeStudy } from "./orange-study.ts";
export { studyLight } from "./orange-study.ts";
type XY = [number, number];
type RGB = [number, number, number];
function mark(
  id: string,
  preset: string,
  width: number,
  a: XY,
  b: XY,
  c: XY,
  color: RGB,
  controls: Record<string, number>,
  pressure: [number, number] = [0.8, 0.65],
): Mark {
  const path: [number, number, number][] = [];
  for (let i = 0; i <= 36; i++) {
    const t = i / 36,
      q = 1 - t;
    path.push([
      q * q * a[0] + 2 * q * t * b[0] + t * t * c[0],
      q * q * a[1] + 2 * q * t * b[1] + t * t * c[1],
      pressure[0] + (pressure[1] - pressure[0]) * t,
    ]);
  }
  return { id, preset, width, path, color, controls };
}
export function orangeRevision(engine: string, catalog: Catalog): Document {
  const d = orangeStudy(engine, catalog);
  const dryShadow = { load: 1, deplete: 0, hgain: 0, pickup: 0, streak: 0, hardness: 0.1 };
  const shadows = d.groups.find((g) => g.id === "shadow")!;
  shadows.strokes = [];
  // Build falloff with thin pigment glazes, each slightly smaller and nearer the contact.
  // The image is never blurred; these are ten real paint strokes plus the dark contact.
  for (let i = 0; i < 10; i++) {
    const t = i / 9,
      w = 0.092 - 0.069 * t,
      length = 0.35 - 0.205 * t,
      cx = 0.527 - 0.012 * t,
      cy = 0.559 - 0.016 * t;
    shadows.strokes.push(
      mark(
        "shadow-wash-" + i,
        "rounded-dab",
        w,
        [cx - length / 2, cy - 0.005],
        [cx, cy + 0.005],
        [cx + length / 2, cy - 0.005],
        [0.37, 0.25, 0.145],
        { ...dryShadow, opacity: 0.006 + 0.022 * t * t },
        [0.85, 0.8],
      ),
    );
  }
  shadows.strokes.push(
    mark(
      "shadow-contact-transition",
      "rounded-dab",
      0.032,
      [0.442, 0.53],
      [0.512, 0.556],
      [0.601, 0.533],
      [0.28, 0.16, 0.075],
      { ...dryShadow, opacity: 0.22 },
      [0.9, 0.85],
    ),
  );
  shadows.strokes.push(
    mark(
      "shadow-contact",
      "rounded-dab",
      0.014,
      [0.455, 0.528],
      [0.513, 0.553],
      [0.57, 0.529],
      [0.12, 0.055, 0.018],
      { ...dryShadow, opacity: 0.85, body: 1 },
      [1, 1],
    ),
  );
  // Preserve all approved underpainting paths, widths, pressure and loading. Bias the mass
  // toward the shadow half-tone; model the light on top so an untouched bright rim cannot remain.
  for (const s of d.groups.find((g) => g.id === "orange-ground")!.strokes) s.color = [0.915, 0.405, 0.027];
  const glaze = { load: 0.85, deplete: 0.005, hgain: 0.025, pickup: 0.25, opacity: 0.2, hardness: 0.18, streak: 0.045 };
  d.groups.find((g) => g.id === "form")!.strokes = [
    mark("right-plane", "wet-mixing", 0.115, [0.575, 0.263], [0.645, 0.365], [0.58, 0.472], [0.8, 0.315, 0.022], {
      ...glaze,
      opacity: 0.24,
    }),
    mark("lower-plane", "wet-mixing", 0.11, [0.393, 0.433], [0.501, 0.513], [0.624, 0.429], [0.81, 0.328, 0.024], {
      ...glaze,
      opacity: 0.19,
    }),
    mark(
      "underside-turn",
      "wet-mixing",
      0.045,
      [0.426, 0.49],
      [0.503, 0.54],
      [0.586, 0.488],
      [0.67, 0.26, 0.024],
      { ...glaze, opacity: 0.24, pickup: 0.4 },
      [0.75, 0.6],
    ),
    mark(
      "broad-light-left",
      "wet-mixing",
      0.12,
      [0.389, 0.374],
      [0.414, 0.255],
      [0.535, 0.246],
      [0.995, 0.635, 0.08],
      { ...glaze, opacity: 0.25 },
      [0.8, 0.65],
    ),
    mark(
      "broad-light-front",
      "wet-mixing",
      0.12,
      [0.408, 0.374],
      [0.45, 0.278],
      [0.563, 0.312],
      [1, 0.61, 0.075],
      { ...glaze, opacity: 0.28 },
      [0.8, 0.65],
    ),
    mark("upper-light-turn", "wet-mixing", 0.105, [0.4, 0.299], [0.469, 0.244], [0.556, 0.295], [1, 0.68, 0.12], {
      ...glaze,
      opacity: 0.17,
    }),
    mark(
      "light-into-halftone",
      "wet-mixing",
      0.115,
      [0.441, 0.304],
      [0.516, 0.351],
      [0.63, 0.408],
      [0.95, 0.485, 0.04],
      { ...glaze, load: 0.4, pickup: 0.65, opacity: 0.32 },
      [0.8, 0.5],
    ),
    mark(
      "halftone-face",
      "wet-mixing",
      0.12,
      [0.416, 0.376],
      [0.515, 0.4],
      [0.599, 0.405],
      [0.97, 0.49, 0.035],
      { ...glaze, opacity: 0.2 },
      [0.75, 0.55],
    ),
    mark(
      "reflected-bottom",
      "wet-mixing",
      0.04,
      [0.437, 0.493],
      [0.504, 0.525],
      [0.584, 0.488],
      [0.965, 0.47, 0.052],
      { ...glaze, opacity: 0.16 },
      [0.65, 0.45],
    ),
    mark(
      "quiet-light-field",
      "wet-mixing",
      0.082,
      [0.413, 0.292],
      [0.458, 0.249],
      [0.511, 0.287],
      [1, 0.7, 0.18],
      { ...glaze, opacity: 0.16 },
      [0.8, 0.65],
    ),
    mark(
      "light-field-core",
      "wet-mixing",
      0.054,
      [0.429, 0.282],
      [0.464, 0.262],
      [0.493, 0.287],
      [1, 0.745, 0.255],
      { ...glaze, opacity: 0.13 },
      [0.8, 0.65],
    ),
  ];
  const accents = d.groups.find((g) => g.id === "accents")!;
  const crown = accents.strokes.find((s) => s.id === "crown-hollow")!;
  accents.strokes = [
    mark(
      "selective-highlight",
      "impasto-accent",
      0.0065,
      [0.449, 0.278],
      [0.456, 0.273],
      [0.465, 0.279],
      [1, 0.865, 0.53],
      { load: 1, deplete: 0, hgain: 0.65, pickup: 0.015, opacity: 0.62 },
      [0.85, 0.25],
    ),
    crown,
  ];
  return d;
}
