import type { Catalog, Document, Group, Mark, View } from "../packages/oilpaint/src/author.ts";
export const studyLight: View = {
  mode: "lit",
  direction: [-0.28, -0.48, 0.78],
  bump: 0.65,
  contrast: 0.22,
  specular: 0.07,
};
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
  pressure: [number, number] = [1, 1],
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
function roundMass(color: RGB, controls: Record<string, number>, r = 0.13, id = "continuous-underpainting"): Mark {
  const path: [number, number, number][] = [];
  const k = 0.55228475;
  for (let quadrant = 0; quadrant < 6; quadrant++)
    for (let j = 0; j <= 24; j++) {
      const t = j / 24,
        q = 1 - t;
      const x = r * (q * q * q + 3 * q * q * t + 3 * q * t * t * k);
      const y = r * (3 * q * q * t * k + 3 * q * t * t + t * t * t);
      const [dx, dy] = [
        [x, y],
        [-y, x],
        [-x, -y],
        [y, -x],
      ][quadrant % 4];
      path.push([0.5 + dx, 0.36 + dy, 1]);
    }
  return { id, preset: "rounded-dab", width: 0.12, path, color, controls };
}
export function orangeStudy(engine: string, catalog: Catalog): Document {
  const base: RGB = [0.94, 0.46, 0.035];
  const thin = { load: 1, deplete: 0, hgain: 0.025, pickup: 0, flatten: 0.9, streak: 0.035 };
  const blend = { load: 0.9, deplete: 0.01, hgain: 0.035, pickup: 0.25, opacity: 0.42, hardness: 0.3, streak: 0.07 };
  const groups: Group[] = [
    {
      id: "shadow",
      name: "Contact and soft cast shadow",
      visible: true,
      dryAfter: 0,
      strokes: [
        mark(
          "shadow-soft",
          "rounded-dab",
          0.092,
          [0.36, 0.551],
          [0.54, 0.585],
          [0.69, 0.55],
          [0.78, 0.75, 0.69],
          { load: 1, deplete: 0, hgain: 0, pickup: 0, opacity: 0.2, hardness: 0.15, streak: 0 },
          [0.8, 0.7],
        ),
        mark(
          "shadow-contact",
          "rounded-dab",
          0.03,
          [0.4, 0.535],
          [0.51, 0.558],
          [0.61, 0.534],
          [0.43, 0.28, 0.17],
          { load: 1, deplete: 0, hgain: 0, pickup: 0, opacity: 0.25, streak: 0 },
          [0.8, 0.65],
        ),
      ],
    },
    {
      id: "orange-ground",
      name: "One connected orange mass",
      visible: true,
      dryAfter: 1,
      strokes: [
        roundMass(base, thin),
        mark("base-core-a", "rounded-dab", 0.12, [0.44, 0.315], [0.5, 0.36], [0.56, 0.405], base, thin),
        roundMass(base, thin, 0.055, "inner-underpainting"),
      ],
    },
    {
      id: "form",
      name: "Light, half-tone and shadow turn",
      visible: true,
      dryAfter: 1,
      strokes: [
        mark(
          "right-turn",
          "wet-mixing",
          0.115,
          [0.576, 0.264],
          [0.646, 0.414],
          [0.536, 0.51],
          [0.75, 0.3, 0.02],
          { ...blend, opacity: 0.42 },
          [0.8, 0.65],
        ),
        mark(
          "bottom-turn",
          "wet-mixing",
          0.086,
          [0.383, 0.444],
          [0.497, 0.548],
          [0.615, 0.444],
          [0.79, 0.33, 0.02],
          { ...blend, opacity: 0.34 },
          [0.7, 0.7],
        ),
        mark(
          "upper-light",
          "wet-mixing",
          0.12,
          [0.419, 0.278],
          [0.475, 0.232],
          [0.566, 0.292],
          [1, 0.66, 0.08],
          { ...blend, opacity: 0.5, pickup: 0.28 },
          [0.9, 0.65],
        ),
        mark(
          "front-light",
          "wet-mixing",
          0.105,
          [0.402, 0.324],
          [0.47, 0.26],
          [0.543, 0.327],
          [1, 0.555, 0.05],
          { ...blend, opacity: 0.32 },
          [0.8, 0.55],
        ),
        mark(
          "join-light",
          "wet-mixing",
          0.078,
          [0.405, 0.35],
          [0.48, 0.36],
          [0.553, 0.359],
          [0.97, 0.505, 0.04],
          { ...blend, opacity: 0.24, pickup: 0.46 },
          [0.75, 0.45],
        ),
        mark(
          "connect-halftone",
          "wet-mixing",
          0.115,
          [0.415, 0.325],
          [0.49, 0.375],
          [0.584, 0.397],
          [0.94, 0.46, 0.035],
          { load: 0.4, deplete: 0, hgain: 0.02, pickup: 0.7, opacity: 0.65, hardness: 0.25, streak: 0.035 },
          [0.7, 0.5],
        ),
        mark(
          "reflected-warmth",
          "wet-mixing",
          0.042,
          [0.415, 0.503],
          [0.501, 0.548],
          [0.584, 0.49],
          [0.98, 0.5, 0.06],
          { ...blend, opacity: 0.34 },
          [0.65, 0.5],
        ),
      ],
    },
    {
      id: "accents",
      name: "Selective highlight and small crown depression",
      visible: true,
      dryAfter: 1,
      strokes: [
        mark(
          "highlight-bed",
          "rounded-dab",
          0.036,
          [0.441, 0.287],
          [0.466, 0.275],
          [0.484, 0.289],
          [1, 0.7, 0.22],
          { ...thin, hgain: 0.13, opacity: 0.42, hardness: 0.25 },
          [0.65, 0.35],
        ),
        mark(
          "highlight",
          "impasto-accent",
          0.008,
          [0.449, 0.28],
          [0.458, 0.274],
          [0.467, 0.28],
          [1, 0.89, 0.59],
          { load: 1, deplete: 0, hgain: 0.95, pickup: 0.015, opacity: 0.8 },
          [0.85, 0.3],
        ),
        mark(
          "crown-hollow",
          "fine-detail",
          0.009,
          [0.494, 0.213],
          [0.502, 0.216],
          [0.51, 0.211],
          [0.68, 0.39, 0.09],
          { hgain: 0.025, pickup: 0.05, opacity: 0.6 },
          [0.7, 0.35],
        ),
      ],
    },
  ];
  return {
    format: "oil-author",
    version: 2,
    engine,
    seed: 19071004,
    aspect: [3, 2],
    ground: [0.89, 0.895, 0.886],
    catalog,
    groups,
  };
}
