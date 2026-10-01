import {
  scene,
  ellipse,
  polygon,
  union,
  constant,
  swirlAround,
  gradientV,
} from "../packages/oilpaint/src/scene.ts";
import type { Catalog } from "../packages/oilpaint/src/author.ts";
export function catalogStillLife(catalog: Catalog) {
  return scene((S) => {
    S.meta({ title: "Catalog still life — orange, leaf and stone" })
      .canvas({ aspect: [3, 2], ground: "#d8d4c8" })
      .brushCatalog(catalog);
    const leaf = polygon([
      [0.075, 0.45],
      [0.09, 0.31],
      [0.16, 0.225],
      [0.235, 0.205],
      [0.255, 0.26],
      [0.212, 0.385],
      [0.135, 0.455],
    ]);
    const orange = ellipse(0.505, 0.325, 0.145, 0.153, 0.05);
    const rockPts: [number, number][] = [
      [0.73, 0.43],
      [0.713, 0.345],
      [0.772, 0.274],
      [0.85, 0.285],
      [0.906, 0.35],
      [0.882, 0.432],
      [0.806, 0.464],
    ];
    const stone = polygon(rockPts);
    const shadows = union(
      ellipse(0.18, 0.465, 0.13, 0.027, 0.65),
      ellipse(0.53, 0.489, 0.155, 0.032, 0.6),
      ellipse(0.835, 0.472, 0.12, 0.022, 0.65),
    );
    S.target.fill("#d8d4c8");
    S.target.fill("#a39680", { mask: shadows });
    S.target.fill(
      gradientV([
        [0.2, "#658147"],
        [0.34, "#315e30"],
        [0.46, "#24482b"],
      ]),
      { mask: leaf },
    );
    S.target.fill(
      gradientV([
        [0.17, "#edac30"],
        [0.27, "#ee971a"],
        [0.4, "#d6790c"],
        [0.49, "#ad570d"],
      ]),
      { mask: orange },
    );
    S.target.blob(0.46, 0.264, 0.07, 0.065, "#ffc64b", { softness: 1, strength: 0.7 });
    S.target.fill(
      gradientV([
        [0.27, "#9a9d90"],
        [0.36, "#6f7877"],
        [0.47, "#485761"],
      ]),
      { mask: stone },
    );
    S.target.polygon(
      [
        [0.73, 0.345],
        [0.78, 0.282],
        [0.85, 0.29],
        [0.81, 0.362],
      ],
      "#b1ac96",
      { softness: 0.015, strength: 0.7 },
    );
    S.region("shadows", shadows, { edge: 0.008, flow: constant(0, { noise: 0.08, seed: 31 }) });
    S.region("leaf", leaf, { edge: 0.003, flow: constant(-61, { noise: 0.05, seed: 17 }) });
    S.region("orange", orange, {
      edge: 0.005,
      flow: swirlAround([[0.35, 0.43, 0.24, 0.25]], { strength: 0.65, noise: 0.08, seed: 19 }),
    });
    S.region("stone", stone, { edge: 0.003, flow: constant(18, { noise: 0.13, seed: 23 }) });
    S.style("shadows", {
      brushPreset: "loaded-flat",
      width: [0.027, 0.048],
      length: [0.05, 0.13],
      opacity: [0.1, 0.2],
      hgain: 0,
      grain: 0,
      streak: 0,
      pickup: 0,
      snap: 0,
      jitter: [0, 0],
      align: 0.98,
      curvature: 0.8,
      edgeFade: 0.95,
      edgeInset: 0.25,
      spill: 0,
      stopAtEdge: 0.92,
    });
    S.style("leaf", {
      brushPreset: "loaded-flat",
      width: [0.025, 0.05],
      length: [0.075, 0.16],
      snap: 0,
      jitter: [1.4, 0.8],
      align: 0.97,
      curvature: 0.85,
      reverseP: 0.25,
      hgain: 0.12,
      edgeFade: 0.55,
      edgeInset: 1,
      spill: 0,
      stopAtEdge: 0.96,
    });
    S.style("orange", {
      brushPreset: "rounded-dab",
      width: [0.03, 0.058],
      length: [0.045, 0.125],
      snap: 0,
      jitter: [1.0, 0.5],
      align: 0.96,
      curvature: 0.92,
      reverseP: 0.45,
      hgain: 0.1,
      edgeFade: 0.58,
      edgeInset: 1,
      spill: 0,
      stopAtEdge: 0.95,
      reliefByValue: 0.55,
    });
    S.style("stone", {
      brushPreset: "loaded-flat",
      width: [0.035, 0.061],
      length: [0.06, 0.14],
      snap: 0,
      jitter: [1.5, 0.8],
      align: 0.87,
      curvature: 0.48,
      reverseP: 0.3,
      hgain: 0.12,
      edgeFade: 0.35,
      edgeInset: 0.4,
      spill: 0,
      stopAtEdge: 0.98,
    });
    S.layers([
      {
        name: "Cast shadows",
        regions: ["shadows"],
        brushPreset: "rounded-dab",
        placement: "density",
        spacing: 0.45,
        coverage: 1,
        maxStrokes: 90,
        gapFill: false,
        dryAfter: 0,
      },
      {
        name: "Block in",
        regions: ["leaf", "orange", "stone"],
        brushPreset: "loaded-flat",
        placement: "error",
        errorThreshold: 6,
        referenceBlur: 0.4,
        gridFactor: 1.15,
        width: [0.05, 0.072],
        length: [0.07, 0.15],
        hgain: 0.08,
        opacity: [0.96, 1],
        jitter: [0.4, 0.3],
        edgeFade: 0.3,
        edgeInset: 1,
        reliefByValue: 0.4,
        maxStrokes: 150,
        dryAfter: 0.85,
      },
      {
        name: "Describe form",
        regions: ["leaf", "orange", "stone"],
        placement: "error",
        errorThreshold: 3.8,
        referenceBlur: 0.25,
        gridFactor: 1.0,
        maxStrokes: 140,
        dryAfter: 0.75,
      },
      {
        name: "Shape transitions",
        regions: ["leaf", "orange", "stone"],
        placement: "error",
        errorThreshold: 4,
        referenceBlur: 0.2,
        gridFactor: 0.75,
        width: [0.012, 0.023],
        length: [0.025, 0.055],
        opacity: [0.65, 0.9],
        hgain: 0.035,
        edgeFade: 0.65,
        edgeInset: 0.3,
        maxStrokes: 140,
        jitter: [0.4, 0.2],
        dryAfter: 0.8,
      },
      {
        name: "Leaf vein",
        regions: ["leaf"],
        brushPreset: "fine-detail",
        placement: "curve",
        curve: {
          leaf: [
            [0.122, 0.433],
            [0.178, 0.323],
            [0.232, 0.229],
          ],
        },
        curveSpacing: 1.6,
        curveJitter: 0.1,
        width: [0.003, 0.0045],
        length: [0.018, 0.034],
        colors: ["#839956"],
        colorFrom: "palette",
        snap: 1,
        jitter: [0, 0],
        opacity: [0.75, 0.9],
        hgain: 0.16,
        maxStrokes: 15,
        gapFill: false,
        dryAfter: 1,
      },
      {
        name: "Light accents",
        regions: ["orange"],
        brushPreset: "impasto-accent",
        placement: "density",
        spacing: 2.8,
        coverage: 0.1,
        width: [0.006, 0.011],
        length: [0.012, 0.025],
        hgain: 0.55,
        opacity: [0.4, 0.65],
        opacityByLight: 0.9,
        reliefByValue: 0.8,
        maxStrokes: 8,
        gapFill: false,
      },
      {
        name: "Stone catches",
        regions: ["stone"],
        brushPreset: "scumble",
        placement: "density",
        spacing: 2.6,
        coverage: 0.15,
        width: [0.012, 0.028],
        length: [0.03, 0.05],
        hgain: 0.18,
        opacity: [0.4, 0.7],
        maxStrokes: 8,
        gapFill: false,
      },
    ]);
    S.light({ lamp: { c: [0.45, 0.25], r: 0.11, strength: 1 } });
  });
}
