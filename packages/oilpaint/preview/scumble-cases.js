// Focused scumble inputs; all other brush definitions and paint paths are unchanged.
export function scumbleCases(author) {
  const mk = (id, preset, width, a, b, c, color, controls = {}, pressure = [0.9, 0.75]) => ({
    id,
    preset,
    width,
    color,
    controls,
    path: Array.from({ length: 37 }, (_, i) => {
      const t = i / 36,
        q = 1 - t;
      return [
        q * q * a[0] + 2 * q * t * b[0] + t * t * c[0],
        q * q * a[1] + 2 * q * t * b[1] + t * t * c[1],
        pressure[0] + (pressure[1] - pressure[0]) * t,
      ];
    }),
  });
  const group = (id, strokes, dryAfter = 0) => ({ id, name: id, visible: true, dryAfter, strokes });
  const doc = (groups) => ({ ...author.document(719), aspect: [2, 1], groups });
  const substrate = [
    mk(
      "slab-left",
      "loaded-flat",
      0.11,
      [0.1, 0.16],
      [0.25, 0.13],
      [0.37, 0.32],
      [0.26, 0.36, 0.42],
      { hgain: 0.35 },
      [0.85, 0.7],
    ),
    mk(
      "slab-middle",
      "loaded-flat",
      0.12,
      [0.28, 0.34],
      [0.4, 0.14],
      [0.59, 0.18],
      [0.32, 0.4, 0.44],
      { hgain: 0.7 },
      [0.7, 1],
    ),
    mk(
      "slab-right",
      "rounded-dab",
      0.105,
      [0.66, 0.14],
      [0.66, 0.29],
      [0.85, 0.36],
      [0.28, 0.39, 0.46],
      { hgain: 0.45 },
      [1, 0.65],
    ),
    mk(
      "raised-catch",
      "impasto-accent",
      0.065,
      [0.4, 0.26],
      [0.47, 0.19],
      [0.56, 0.28],
      [0.38, 0.45, 0.46],
      { hgain: 1.1 },
      [0.75, 0.45],
    ),
    mk(
      "cross-grain",
      "dry-drag",
      0.06,
      [0.15, 0.35],
      [0.45, 0.36],
      [0.78, 0.17],
      [0.34, 0.42, 0.43],
      { body: 0.5, load: 0.9, deplete: 0.02, hgain: 0.65 },
      [1, 0.6],
    ),
  ];
  const scumble = [
    mk("broad-scumble", "scumble", 0.1, [0.1, 0.16], [0.47, 0.35], [0.89, 0.19], [0.88, 0.44, 0.16]),
    mk(
      "narrow-scumble",
      "scumble",
      0.044,
      [0.15, 0.38],
      [0.39, 0.4],
      [0.61, 0.365],
      [0.88, 0.44, 0.16],
      {},
      [0.95, 0.55],
    ),
  ];
  const flat = doc([group("scumble", scumble)]);
  const irregular = doc([group("dry-irregular-underpaint", substrate), group("scumble", scumble)]);
  const overlap = structuredClone(irregular);
  overlap.groups.push(
    group("crossing-scumble", [
      mk(
        "crossing-scumble",
        "scumble",
        0.076,
        [0.75, 0.11],
        [0.48, 0.4],
        [0.29, 0.2],
        [0.92, 0.68, 0.33],
        { load: 0.88 },
        [0.85, 0.6],
      ),
    ]),
  );
  return [
    { id: "flat", title: "Two widths on flat ground", document: flat },
    { id: "irregular", title: "Same strokes on irregular dry paint", document: irregular },
    { id: "overlap", title: "Short crossing pass over existing scumble", document: overlap },
  ];
}
export function stableBrushCases(author) {
  return author
    .catalog()
    .presets.filter((p) => p.id !== "scumble")
    .map((p) => {
      const d = { ...author.document(901), aspect: [2, 1] };
      const mark = (id, preset, width, path, color) => ({ id, preset, width, path, color, controls: {} });
      d.groups = [
        {
          id: "wet-base",
          name: "Wet base",
          visible: true,
          dryAfter: 1,
          strokes: [
            mark(
              "blue",
              "loaded-flat",
              0.09,
              [
                [0.46, 0.07, 1],
                [0.46, 0.43, 1],
              ],
              [0.12, 0.32, 0.68],
            ),
          ],
        },
        {
          id: "candidate",
          name: p.name,
          visible: true,
          dryAfter: 0.7,
          strokes: [
            mark(
              "candidate",
              p.id,
              Math.min(p.width[2], 0.065),
              [
                [0.12, 0.21, 0.4],
                [0.35, 0.27, 1],
                [0.63, 0.27, 0.8],
                [0.9, 0.2, 0.3],
              ],
              [0.88, 0.44, 0.16],
            ),
          ],
        },
      ];
      return { id: p.id, document: d };
    });
}
