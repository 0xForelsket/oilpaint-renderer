// Deliberate stroke placement for the third review. Geometry is authored with polynomial curves.
// These are painting studies, not a new preset or a procedural scene style.
export const denseDryControls = { body: 0.48, load: 0.85, deplete: 0.04, dropout: 0.055, vdry: 0.34 };
const green = [0.16, 0.36, 0.18],
  orange = [0.81, 0.34, 0.1],
  grey = [0.36, 0.39, 0.43],
  blue = [0.1, 0.26, 0.65];
function stroke(id, preset, width, a, b, c, color, controls = {}, pressure = [0.85, 0.55]) {
  const path = [];
  for (let i = 0; i <= 24; i++) {
    const t = i / 24,
      q = 1 - t;
    path.push([
      q * q * a[0] + 2 * q * t * b[0] + t * t * c[0],
      q * q * a[1] + 2 * q * t * b[1] + t * t * c[1],
      pressure[0] + (pressure[1] - pressure[0]) * t,
    ]);
  }
  return { id, preset, width, path, color, controls };
}
const group = (id, strokes, dryAfter = 1) => ({ id, name: id, visible: true, dryAfter, strokes });
const line = (id, preset, w, x, y, length, color, controls = {}, p = [1, 1]) =>
  stroke(id, preset, w, [x, y], [x + length / 2, y], [x + length, y], color, controls, p);
function doc(author, groups) {
  return { ...author.document(19071003), aspect: [2, 1], groups };
}
export function formStudy(author) {
  const leaf = [
    stroke(
      "leaf-dark",
      "loaded-flat",
      0.072,
      [0.12, 0.36],
      [0.1, 0.21],
      [0.23, 0.075],
      [0.1, 0.25, 0.13],
      {},
      [0.95, 0.25],
    ),
    stroke("leaf-right", "rounded-dab", 0.065, [0.13, 0.34], [0.26, 0.21], [0.23, 0.075], green, {}, [0.8, 0.2]),
    stroke(
      "leaf-middle",
      "loaded-flat",
      0.048,
      [0.13, 0.31],
      [0.2, 0.23],
      [0.22, 0.12],
      [0.25, 0.45, 0.2],
      {},
      [0.9, 0.35],
    ),
    stroke(
      "leaf-left-turn",
      "rounded-dab",
      0.034,
      [0.11, 0.28],
      [0.11, 0.2],
      [0.17, 0.16],
      [0.22, 0.39, 0.16],
      {},
      [0.65, 0.3],
    ),
    stroke(
      "leaf-vein",
      "fine-detail",
      0.007,
      [0.12, 0.37],
      [0.2, 0.2],
      [0.23, 0.07],
      [0.52, 0.57, 0.26],
      {},
      [1, 0.12],
    ),
    ...[
      [0.15, 0.3, 0.11, 0.245],
      [0.175, 0.25, 0.14, 0.19],
      [0.19, 0.2, 0.225, 0.185],
      [0.205, 0.155, 0.245, 0.14],
    ].map(([x, y, ex, ey], i) =>
      stroke(
        "vein-" + i,
        "fine-detail",
        0.003,
        [x, y],
        [(x + ex) / 2, (y + ey) / 2 - 0.007],
        [ex, ey],
        [0.42, 0.52, 0.24],
        {},
        [0.75, 0.08],
      ),
    ),
  ];
  const fruit = [
    // An overlapping thin foundation closes the silhouette before the form-following body marks.
    stroke(
      "fruit-ground-left",
      "rounded-dab",
      0.095,
      [0.445, 0.15],
      [0.4, 0.24],
      [0.46, 0.335],
      [0.58, 0.24, 0.075],
      { hgain: 0.15, body: 1, opacity: 1, deplete: 0 },
      [1, 1],
    ),
    stroke(
      "fruit-ground-center",
      "rounded-dab",
      0.12,
      [0.49, 0.095],
      [0.475, 0.23],
      [0.5, 0.37],
      [0.74, 0.29, 0.08],
      { hgain: 0.15, body: 1, opacity: 1, deplete: 0 },
      [1, 1],
    ),
    stroke(
      "fruit-ground-middle",
      "rounded-dab",
      0.12,
      [0.55, 0.36],
      [0.55, 0.22],
      [0.53, 0.1],
      [0.78, 0.33, 0.09],
      { hgain: 0.15, body: 1, opacity: 1, deplete: 0 },
      [1, 1],
    ),
    stroke(
      "fruit-ground-right",
      "rounded-dab",
      0.1,
      [0.57, 0.15],
      [0.635, 0.24],
      [0.575, 0.325],
      [0.57, 0.23, 0.07],
      { hgain: 0.15, body: 1, opacity: 1, deplete: 0 },
      [1, 1],
    ),
    stroke(
      "fruit-dark-left",
      "rounded-dab",
      0.105,
      [0.47, 0.105],
      [0.36, 0.26],
      [0.48, 0.36],
      [0.51, 0.19, 0.075],
      {},
      [0.8, 0.6],
    ),
    stroke("fruit-base", "rounded-dab", 0.12, [0.5, 0.12], [0.57, 0.23], [0.5, 0.35], orange, {}, [0.9, 0.72]),
    stroke(
      "fruit-top",
      "loaded-flat",
      0.065,
      [0.45, 0.12],
      [0.54, 0.06],
      [0.59, 0.18],
      [0.87, 0.4, 0.12],
      {},
      [0.65, 0.8],
    ),
    stroke(
      "fruit-right-plane",
      "rounded-dab",
      0.072,
      [0.55, 0.17],
      [0.65, 0.27],
      [0.54, 0.34],
      [0.64, 0.25, 0.08],
      {},
      [0.9, 0.5],
    ),
    stroke(
      "fruit-belly",
      "rounded-dab",
      0.09,
      [0.445, 0.22],
      [0.49, 0.32],
      [0.56, 0.275],
      [0.91, 0.44, 0.15],
      {},
      [0.7, 0.85],
    ),
    stroke(
      "fruit-turn",
      "wet-mixing",
      0.047,
      [0.44, 0.16],
      [0.43, 0.245],
      [0.475, 0.29],
      [0.92, 0.46, 0.17],
      { pickup: 0.3, load: 0.92, body: 0.94 },
      [0.75, 0.3],
    ),
    stroke(
      "fruit-light",
      "loaded-flat",
      0.049,
      [0.457, 0.145],
      [0.48, 0.135],
      [0.512, 0.17],
      [0.97, 0.54, 0.2],
      { hgain: 0.35 },
      [0.7, 0.3],
    ),
    stroke(
      "fruit-highlight",
      "impasto-accent",
      0.021,
      [0.466, 0.145],
      [0.48, 0.139],
      [0.491, 0.15],
      [0.98, 0.7, 0.35],
      { stiff: 0.1 },
      [0.85, 0.35],
    ),
    stroke(
      "fruit-stem",
      "fine-detail",
      0.009,
      [0.505, 0.105],
      [0.49, 0.063],
      [0.523, 0.054],
      [0.26, 0.24, 0.11],
      {},
      [0.9, 0.15],
    ),
    stroke(
      "fruit-tiny-highlight",
      "fine-detail",
      0.004,
      [0.459, 0.168],
      [0.462, 0.162],
      [0.467, 0.16],
      [0.98, 0.77, 0.42],
      {},
      [0.7, 0.1],
    ),
  ];
  const stone = [
    stroke(
      "stone-ground-low",
      "loaded-flat",
      0.105,
      [0.74, 0.305],
      [0.825, 0.34],
      [0.925, 0.295],
      [0.26, 0.3, 0.34],
      { hgain: 0.15, body: 1, opacity: 1, deplete: 0 },
      [1, 1],
    ),
    stroke(
      "stone-ground-middle",
      "loaded-flat",
      0.12,
      [0.74, 0.27],
      [0.82, 0.2],
      [0.902, 0.255],
      [0.34, 0.37, 0.4],
      { hgain: 0.15, body: 1, opacity: 1, deplete: 0 },
      [1, 1],
    ),
    stroke(
      "stone-ground-top",
      "loaded-flat",
      0.1,
      [0.765, 0.2],
      [0.818, 0.15],
      [0.885, 0.215],
      [0.41, 0.43, 0.43],
      { hgain: 0.15, body: 1, opacity: 1, deplete: 0 },
      [1, 1],
    ),
    stroke(
      "stone-base",
      "loaded-flat",
      0.11,
      [0.75, 0.3],
      [0.82, 0.34],
      [0.915, 0.3],
      [0.25, 0.29, 0.33],
      { hgain: 0.4 },
      [0.85, 0.75],
    ),
    stroke(
      "stone-left-plane",
      "loaded-flat",
      0.095,
      [0.748, 0.28],
      [0.77, 0.18],
      [0.81, 0.15],
      [0.43, 0.46, 0.46],
      { hgain: 0.5 },
      [0.95, 0.55],
    ),
    stroke(
      "stone-right-plane",
      "loaded-flat",
      0.093,
      [0.813, 0.17],
      [0.89, 0.22],
      [0.907, 0.295],
      grey,
      { hgain: 0.45 },
      [0.85, 0.65],
    ),
    stroke(
      "stone-top-plane",
      "loaded-flat",
      0.069,
      [0.778, 0.16],
      [0.825, 0.13],
      [0.87, 0.196],
      [0.57, 0.57, 0.51],
      { hgain: 0.35 },
      [0.7, 0.75],
    ),
    stroke(
      "stone-bottom-turn",
      "rounded-dab",
      0.05,
      [0.774, 0.32],
      [0.836, 0.35],
      [0.872, 0.31],
      [0.34, 0.37, 0.39],
      { hgain: 0.32 },
      [0.8, 0.4],
    ),
    stroke(
      "stone-dry-catch",
      "dry-drag",
      0.048,
      [0.782, 0.184],
      [0.83, 0.181],
      [0.88, 0.22],
      [0.65, 0.64, 0.56],
      denseDryControls,
      [0.7, 0.45],
    ),
    stroke(
      "stone-scumble",
      "scumble",
      0.06,
      [0.78, 0.24],
      [0.825, 0.22],
      [0.856, 0.245],
      [0.57, 0.58, 0.53],
      { load: 0.9, body: 0.94 },
      [0.8, 0.4],
    ),
    stroke(
      "stone-edge",
      "fine-detail",
      0.005,
      [0.752, 0.265],
      [0.761, 0.23],
      [0.777, 0.203],
      [0.6, 0.59, 0.52],
      {},
      [0.6, 0.08],
    ),
  ];
  return doc(author, [group("leaf", leaf, 0.55), group("fruit", fruit, 0.6), group("stone", stone, 0.5)]);
}
export function reviewCases(author) {
  const contact = doc(author, [
    group("impasto", [
      stroke("impasto-wide", "impasto-accent", 0.078, [0.08, 0.12], [0.36, 0.32], [0.63, 0.1], orange, {}, [0.9, 0.7]),
      line("impasto-low", "impasto-accent", 0.07, 0.1, 0.37, 0.55, orange, {}, [0.25, 0.95]),
    ]),
  ]);
  const ridges = Array.from({ length: 9 }, (_, i) =>
    stroke(
      "ridge-" + i,
      "loaded-flat",
      0.028,
      [0.12 + i * 0.082, 0.09],
      [0.1 + i * 0.082, 0.26],
      [0.16 + i * 0.082, 0.42],
      blue,
      { hgain: 1.1, ridge: 0.5 },
      [1, 1],
    ),
  );
  const scumble = (under) =>
    doc(author, [
      group("surface", under, 0),
      group("scumble", [
        stroke("drag", "scumble", 0.1, [0.1, 0.18], [0.45, 0.31], [0.89, 0.2], orange, {}, [0.8, 0.7]),
        line("short", "scumble", 0.09, 0.2, 0.38, 0.48, orange, {}, [0.9, 0.45]),
      ]),
    ]);
  const pressure = doc(author, [
    group(
      "blue",
      [
        line("blue-top", "loaded-flat", 0.12, 0.1, 0.105, 0.8, blue),
        line("blue-mid", "loaded-flat", 0.12, 0.1, 0.25, 0.8, blue),
        line("blue-bottom", "loaded-flat", 0.12, 0.1, 0.395, 0.8, blue),
      ],
      0,
    ),
    group(
      "loaded",
      [0.15, 0.4, 1].map((p, i) =>
        line("pressure-" + i, "loaded-flat", 0.09, 0.12, 0.105 + i * 0.145, 0.74, orange, { pickup: 0, deplete: 0 }, [
          p,
          p,
        ]),
      ),
    ),
  ]);
  const dry = doc(author, [
    group(
      "blue",
      [
        line("blue-top", "loaded-flat", 0.12, 0.08, 0.14, 0.82, blue),
        line("blue-bottom", "loaded-flat", 0.12, 0.08, 0.36, 0.82, blue),
      ],
      0,
    ),
    group("dry", [
      line("sparse", "dry-drag", 0.1, 0.1, 0.14, 0.78, orange),
      line("dense", "dry-drag", 0.1, 0.1, 0.36, 0.78, orange, denseDryControls),
    ]),
  ]);
  const twist = doc(author, [
    group("rounded", [
      stroke("turn-a", "rounded-dab", 0.1, [0.12, 0.3], [0.37, 0.07], [0.31, 0.35], orange, {}, [0.35, 0.95]),
      stroke("turn-b", "rounded-dab", 0.105, [0.56, 0.14], [0.46, 0.41], [0.79, 0.24], orange, {}, [1, 0.15]),
    ]),
  ]);
  const tail = doc(author, [
    group("blue", [stroke("cross", "loaded-flat", 0.075, [0.3, 0.08], [0.3, 0.25], [0.3, 0.43], blue, {}, [1, 1])]),
    group("pickup", [
      stroke(
        "long-tail",
        "wet-mixing",
        0.09,
        [0.07, 0.24],
        [0.45, 0.21],
        [0.94, 0.3],
        orange,
        { deplete: 0.014, load: 1 },
        [1, 0.85],
      ),
    ]),
  ]);
  const details = doc(author, [
    group("details", [
      stroke("stem", "fine-detail", 0.009, [0.14, 0.4], [0.13, 0.21], [0.22, 0.08], green, {}, [1, 0.1]),
      stroke("branch", "fine-detail", 0.004, [0.16, 0.24], [0.24, 0.18], [0.28, 0.14], green, {}, [0.9, 0.1]),
      stroke(
        "contour",
        "fine-detail",
        0.006,
        [0.4, 0.35],
        [0.31, 0.13],
        [0.55, 0.15],
        [0.36, 0.25, 0.17],
        {},
        [0.2, 0.8],
      ),
      stroke("cross-a", "fine-detail", 0.007, [0.65, 0.13], [0.79, 0.24], [0.9, 0.34], blue, {}, [0.85, 0.15]),
      stroke("cross-b", "fine-detail", 0.005, [0.68, 0.36], [0.74, 0.24], [0.87, 0.11], orange, {}, [0.9, 0.2]),
      line("tiny-light", "fine-detail", 0.004, 0.43, 0.18, 0.017, [0.98, 0.92, 0.7], {}, [1, 0.1]),
      line("tiny-dot", "fine-detail", 0.006, 0.48, 0.23, 0.005, [0.99, 0.93, 0.7], {}, [1, 0.5]),
    ]),
  ]);
  const overlap = doc(author, [
    group("overlap", [
      stroke("a", "loaded-flat", 0.085, [0.12, 0.16], [0.35, 0.31], [0.69, 0.12], [0.18, 0.36, 0.52], {}, [1, 0.4]),
      stroke("b", "rounded-dab", 0.1, [0.31, 0.11], [0.23, 0.31], [0.52, 0.38], [0.81, 0.42, 0.16], {}, [0.55, 0.9]),
      stroke(
        "c",
        "wet-mixing",
        0.074,
        [0.7, 0.11],
        [0.41, 0.31],
        [0.79, 0.38],
        [0.7, 0.65, 0.36],
        { pickup: 0.45 },
        [0.7, 0.5],
      ),
      stroke(
        "d",
        "dry-drag",
        0.05,
        [0.12, 0.39],
        [0.46, 0.2],
        [0.9, 0.31],
        [0.85, 0.73, 0.49],
        denseDryControls,
        [0.8, 0.3],
      ),
    ]),
  ]);
  return [
    { id: "scumble-flat", title: "Clustered scumble · flat surface", d: scumble([]), before: true },
    { id: "scumble-relief", title: "Clustered scumble · raised dry surface", d: scumble(ridges), before: true },
    { id: "impasto", title: "Impasto · local buildup and smoother body", d: contact, before: true },
    { id: "pressure", title: "Loaded paint · pressure 0.15 / 0.40 / 1.00", d: pressure, before: true },
    { id: "dry-density", title: "Dry drag · sparse / denser existing controls", d: dry, before: true },
    { id: "rounded-turn", title: "Rounded contact · short turns with pressure change", d: twist },
    { id: "pickup-tail", title: "Wet mixing · long outgoing pickup tail", d: tail, before: true },
    { id: "fine-detail", title: "Fine detail · stems, contour, crossings, tiny highlights", d: details },
    { id: "overlap", title: "Overlapping directions, spacing and pressure", d: overlap },
    { id: "form-following", title: "Form-following leaf, fruit and stone · all seven presets", d: formStudy(author) },
  ];
}
