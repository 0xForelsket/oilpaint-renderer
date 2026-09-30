// Still life: lemons and a white bowl on a blue cloth. The L3 acceptance's second scene: no preset (the engine's
// neutral defaults), a landscape canvas, and round forms, cloth folds and wood instead of Storm Light's sea and sky.
// Light comes from the upper left.
import {
  above, below, constant, contour, ellipse, gradientV, intersect, not, polygon, scene, sweep, union,
  type Point,
} from "oilpaint/scene";

const TABLE = 0.46; // table edge, in cw (the canvas is 1.0 x 0.8)
const CLOTH: Point[] = [[0.08, 0.8], [0.14, 0.52], [0.3, 0.49], [0.52, 0.53], [0.7, 0.5], [0.86, 0.56], [0.92, 0.8]];
const BOWL = { c: [0.47, 0.49] as Point, r: [0.17, 0.085] as Point };
const LEMONS: [number, number, number, number][] = [[0.4, 0.43, 0.065, 0.045], [0.52, 0.415, 0.07, 0.048], [0.72, 0.6, 0.066, 0.046]];

export default scene((S) => {
  S.meta({ title: "Still life with lemons", engine: "2.0.0-dev.3" });
  S.canvas({ aspect: [5, 4], ground: "#d9cdb8" });

  const T = S.target;
  // wall: warm grey-green, lighter at the upper left where the light comes from
  T.fill(gradientV([[0, "#6f7462"], [0.3, "#8a8a72"], [TABLE, "#9a9478"]]));
  T.blob(0.15, 0.1, 0.4, 0.3, "#b5ad8e", { softness: 0.9, strength: 0.6, seed: 3 });
  // table top, wood
  T.fill(gradientV([[TABLE, "#7a5234"], [0.8, "#4e321f"]]), { mask: below(TABLE) });
  // the cloth, with a darker fold and a lit ridge
  T.polygon(CLOTH, "#3f5f8f", { softness: 0.006 });
  T.polygon([[0.22, 0.8], [0.3, 0.6], [0.36, 0.8]], "#2c4570", { softness: 0.02, strength: 0.8 });
  T.polygon([[0.64, 0.8], [0.7, 0.62], [0.74, 0.8]], "#6a8cbd", { softness: 0.02, strength: 0.7 });
  // the bowl: white body, shadowed underside, rim
  T.blob(BOWL.c[0], BOWL.c[1], BOWL.r[0], BOWL.r[1], "#e8e2d6", { softness: 0.04, seed: 5 });
  T.blob(BOWL.c[0] + 0.03, BOWL.c[1] + 0.03, 0.14, 0.05, "#a8a4a8", { softness: 0.6, strength: 0.8, seed: 6 });
  T.blob(BOWL.c[0] - 0.05, BOWL.c[1] - 0.03, 0.08, 0.025, "#fbf7ee", { softness: 0.8, strength: 0.7, seed: 7 });
  // lemons, each with a shadow side and a highlight
  LEMONS.forEach(([cx, cy, rx, ry], i) => {
    T.blob(cx + 0.015, cy + 0.035, rx * 1.1, ry * 0.45, "#2e2a30", { softness: 0.6, strength: 0.55, seed: 10 + i }); // cast shadow
    T.blob(cx, cy, rx, ry, "#e8c21f", { softness: 0.05, seed: 20 + i });
    T.blob(cx + rx * 0.35, cy + ry * 0.35, rx * 0.7, ry * 0.6, "#b08a12", { softness: 0.8, strength: 0.6, seed: 30 + i });
    T.glow([cx - rx * 0.35, cy - ry * 0.4], rx * 0.3, "#fff6c8", { strength: 0.7 });
  });
  S.light({ lamp: { c: [0.15, 0.1], r: 0.6, strength: 0.8 } });

  // ---- regions
  const lemons = union(...LEMONS.map(([cx, cy, rx, ry]) => ellipse(cx, cy, rx, ry)));
  S.region("wall", above(TABLE), { edge: 0.02, flow: sweep({ angle: -20, curl: 0.3, noise: 0.3, scale: 0.4 }) });
  S.region("table", intersect(below(TABLE), not(polygon(CLOTH))), { edge: 0.01, flow: constant(2, { noise: 0.08 }) });
  S.region("cloth", polygon(CLOTH), { edge: 0.008, flow: contour(CLOTH, { noise: 0.5 }) });
  // the round forms have no authored flow: their strokes follow the target's own shading (structure-tensor flow)
  S.region("bowl", ellipse(BOWL.c[0], BOWL.c[1], BOWL.r[0], BOWL.r[1]), { edge: 0.006 });
  S.region("lemons", lemons, { edge: 0.005 });

  // ---- styles (neutral defaults otherwise)
  S.style("wall", { width: [0.03, 0.05], length: [0.06, 0.16], colors: ["#6f7462", "#8a8a72", "#b5ad8e", "#9a9478"], snap: 0.5, lengthSkew: 0.5 });
  S.style("table", { width: [0.015, 0.03], length: [0.08, 0.2], colors: ["#7a5234", "#5e3c24", "#4e321f", "#9a6a44"], curvature: 0.1, align: 0.95 });
  S.style("cloth", { width: [0.015, 0.03], length: [0.04, 0.1], colors: ["#3f5f8f", "#2c4570", "#6a8cbd", "#8fa9cf"], sizeByDetail: 0.5 });
  S.style("bowl", { width: [0.008, 0.016], length: [0.02, 0.05], colors: ["#e8e2d6", "#a8a4a8", "#fbf7ee", "#c9c2b8"], curvature: 0.6, priority: 1, spill: 0.02, stopAtEdge: 0.98 });
  S.style("lemons", {
    width: [0.006, 0.012], length: [0.015, 0.035], colors: ["#e8c21f", "#b08a12", "#f4dd5a", "#8a6a10"], curvature: 0.7,
    priority: 2, reliefByValue: 0.5, dabShare: 0.25, flecks: [["#6a8cbd", 0.05]], spill: 0.02, stopAtEdge: 0.98,
  });

  S.layers([
    { name: "Block in", regions: "all", errorThreshold: 14, referenceBlur: 0.7, width: [0.04, 0.07], length: [0.1, 0.25], opacity: [0.8, 0.95], relief: 0.4, dryAfter: 0.4 },
    { name: "Forms", regions: ["wall", "table", "cloth", "bowl", "lemons"], errorThreshold: 9, referenceBlur: 0.45 },
    { name: "Details", regions: ["bowl", "lemons", "cloth"], errorThreshold: 6, referenceBlur: 0.3, gridFactor: 0.9 },
    { name: "Accents", regions: ["lemons", "bowl"], placement: "density", spacing: { lemons: 2.2, bowl: 2.6 }, coverage: 0.35, relief: 1.6, dab: true, colorFrom: "reference", snap: 0.7 },
  ]);
});
