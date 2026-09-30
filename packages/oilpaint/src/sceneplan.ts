// Generated from spec/sceneplan-1.schema.json by scripts/gen-types.mjs: do not edit.
// The schema is generated from the Rust types in crates/oil-scene/src/spec.rs.

/**
 * A colour: `"#rrggbb"`, a tube name (for example `"lead_white"`, `"cobalt_blue"`), an sRGB triple with channels in
 * [0, 1], or `["a", "b", t]`, a mix of two colours at t in [0, 1] made with the active mixer.
 *
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "ColorSpec".
 */
export type ColorSpec = string | [string, string, number] | [number, number, number];
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "FieldKind".
 */
export type FieldKind = "mask" | "flow" | "rgb";
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "ColorFrom".
 */
export type ColorFrom = "reference" | "palette";
/**
 * A number for all regions, or one per region name.
 *
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "NumOrMap".
 */
export type NumOrMap =
  | number
  | {
      [k: string]: number;
    };
/**
 * `"boundary"` (the region's outline) or a polyline.
 *
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "CurveSpec".
 */
export type CurveSpec = string | [number, number][];
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "Mode".
 */
export type Mode = "paint" | "scumble" | "smudge" | "glaze";
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "Order".
 */
export type Order = "sweep" | "random";
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "Placement".
 */
export type Placement = "error" | "density" | "curve";
/**
 * `"all"` or a list of region names.
 *
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "RegionSel".
 */
export type RegionSel = string | string[];
/**
 * A light-map contribution, as a single-key object.
 *
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "Light".
 */
export type Light =
  | {
      glow: LightGlow;
    }
  | {
      beam: LightBeam;
    }
  | {
      lamp: LightLamp;
    };
/**
 * A unit direction field, as a single-key object.
 *
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "Flow".
 */
export type Flow =
  | {
      constant: ConstantFlow;
    }
  | {
      sweep: SweepFlow;
    }
  | {
      waves: WavesFlow;
    }
  | {
      swirlAround: SwirlFlow;
    }
  | {
      radialFrom: RadialFlow;
    }
  | {
      upward: UpwardFlow;
    }
  | {
      contour: ContourFlow;
    }
  | {
      field: string;
    };
/**
 * A mask in [0, 1], as a single-key object.
 *
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "Shape".
 */
export type Shape =
  | {
      all: boolean;
    }
  | {
      above: number;
    }
  | {
      below: number;
    }
  | {
      ellipse: Ellipse;
    }
  | {
      disc: Disc;
    }
  | {
      polygon: [number, number][];
    }
  | {
      wedge: Wedge;
    }
  | {
      bandAround: BandAround;
    }
  | {
      union: Shape[];
    }
  | {
      intersect: Shape[];
    }
  | {
      not: Shape;
    }
  | {
      noisy: Noisy;
    }
  | {
      field: string;
    };
/**
 * A target-image operation, as a single-key object.
 *
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "TargetOp".
 */
export type TargetOp =
  | {
      fill: Fill;
    }
  | {
      blob: Blob;
    }
  | {
      polygon: PolygonOp;
    }
  | {
      bands: Bands;
    }
  | {
      glow: Glow;
    }
  | {
      beam: Beam;
    };

/**
 * A ScenePlan: the declarative description of a painting. With the plan options `{seed, planWidth, mixer}` and an
 * engine version it determines the StrokeList bit for bit on every host.
 */
export interface ScenePlan {
  /**
   * Ignored by the engine; lets editors validate.
   */
  $schema?: string | null;
  canvas: Canvas;
  /**
   * Engine version the scene was authored and tuned with. A different engine warns (`ENGINE_VERSION_DIFFERS`).
   */
  engine?: string | null;
  /**
   * Declarations of sampled fields (escape hatch): the arrays are passed next to the spec.
   */
  fields?: {
    [k: string]: FieldDecl;
  };
  /**
   * The layer schedule, painted in order.
   */
  layers: Layer[];
  /**
   * Contributions to the light map (drives `warmth` and `opacityByLight`).
   */
  lights?: Light[];
  /**
   * Regions, in order: later regions override earlier ones in the hard region map.
   */
  regions: Region[];
  /**
   * Schema generation: 1.
   */
  sceneplan: number;
  /**
   * Style per region, keyed by region name.
   */
  styles: {
    [k: string]: Style;
  };
  /**
   * Target-image operations, applied in order: the reference colours the planner paints toward.
   */
  target: TargetOp[];
  title?: string | null;
}
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "Canvas".
 */
export interface Canvas {
  /**
   * Aspect ratio as positive integers [width, height], for example [4, 5].
   *
   * @minItems 2
   * @maxItems 2
   */
  aspect: [number, number];
  /**
   * Ground (canvas) colour.
   */
  ground: ColorSpec;
}
/**
 * A sampled field: little-endian f32 values (1 channel for a mask, 2 for a flow, 3 for rgb), row-major, passed next
 * to the spec and checked against `sha256`.
 *
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "FieldDecl".
 */
export interface FieldDecl {
  height: number;
  kind: FieldKind;
  sha256: string;
  width: number;
}
/**
 * A layer of the schedule. Besides its own keys it accepts every style key, overriding the region styles for this
 * layer.
 *
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "Layer".
 */
export interface Layer {
  /**
   * 1 = strictly along the flow, 0 = random directions (default 0.85).
   */
  align?: number | null;
  /**
   * Extra paint where the stroke starts, x hgain (default 0.3).
   */
  blob?: number | null;
  /**
   * Paint body between bristle lanes (default 0.9).
   */
  body?: number | null;
  colorFrom?: ColorFrom;
  /**
   * Palette-snapping targets.
   */
  colors?: ColorSpec[] | null;
  /**
   * Share of the region to cover: a number or per region (regions not listed: 1.0).
   */
  coverage?: NumOrMap;
  /**
   * 0 = straight, 1 = follows every turn of the flow (default 0.3).
   */
  curvature?: number | null;
  /**
   * Curve placement: per region, "boundary" or a polyline (regions not listed, or no map: "boundary").
   */
  curve?: {
    [k: string]: CurveSpec;
  } | null;
  curveJitter?: number;
  curveOffset?: number;
  curveSpacing?: number;
  /**
   * Allow short, round dabs.
   */
  dab?: boolean;
  /**
   * Load lost per unit length (default 0.02).
   */
  deplete?: number | null;
  /**
   * Default 0.02.
   */
  dropout?: number | null;
  /**
   * Wetness factor applied after the layer.
   */
  dryAfter?: number | null;
  /**
   * Scumble: height-above-blur threshold where paint catches (default 0).
   */
  dryThresh?: number | null;
  /**
   * Scumble: smoothstep width around `dryThresh` (default 0.15).
   */
  dryWidth?: number | null;
  enabled?: boolean;
  /**
   * Pressure at the stroke's end (default 0.15).
   */
  endPressure?: number | null;
  /**
   * Width factor at the stroke's end (default 0.5).
   */
  endWidth?: number | null;
  /**
   * Lab error that triggers a stroke (error placement; v1 `T`).
   */
  errorThreshold?: number;
  /**
   * Share of the surface the stroke replaces (default 0.6).
   */
  flatten?: number | null;
  /**
   * Complementary touches: [colour, probability].
   */
  flecks?: [ColorSpec, number][] | null;
  /**
   * Trough along the centre, x hgain (default 0.15).
   */
  furrow?: number | null;
  gapCover?: number;
  gapFill?: boolean;
  /**
   * Canvas-grain catch (default 0.10).
   */
  grain?: number | null;
  /**
   * Proposal grid cell in stroke widths (v1 `fg`).
   */
  gridFactor?: number;
  /**
   * Edge crispness (default 0.75).
   */
  hardness?: number | null;
  /**
   * Height blur before scumble, in cw.
   */
  hblurSigma?: number;
  /**
   * Paint thickness (default 1.0).
   */
  hgain?: number | null;
  /**
   * +- relative thickness variation (default 0.25).
   */
  hgainJitter?: number | null;
  /**
   * Lab jitter [L, a/b] between strokes (default [5, 4]).
   *
   * @minItems 2
   * @maxItems 2
   */
  jitter?: [number, number] | null;
  jitterPos?: number;
  /**
   * Stroke colours are never darker than this L* (default 20).
   */
  lFloor?: number | null;
  /**
   * Stroke length range [min, max] in cw (default [0.04, 0.10]).
   *
   * @minItems 2
   * @maxItems 2
   */
  length?: [number, number] | null;
  /**
   * Raised paint along both edges, x hgain (default 0.35).
   */
  levee?: number | null;
  /**
   * Paint on the brush at the start (default 1.0).
   */
  load?: number | null;
  /**
   * Colour B for marbling (default: a lighter/warmer variant).
   */
  load2?: ColorSpec | null;
  /**
   * Two-colour load: share of colour B in its lanes (default 0).
   */
  marble?: number | null;
  /**
   * Skip sites already covered more than this.
   */
  maxCover?: number | null;
  maxStrokes?: number | null;
  /**
   * Length >= minAspect x width unless the layer sets `dab` (default 2.5).
   */
  minAspect?: number | null;
  /**
   * paint, scumble, smudge or glaze (default paint).
   */
  mode?: Mode | null;
  name: string;
  /**
   * Default 5.
   */
  nbBase?: number | null;
  /**
   * Bristle lanes = nbBase + nbPerCw x width (default 450).
   */
  nbPerCw?: number | null;
  /**
   * Opacity range [min, max] (default [0.85, 1.0]).
   *
   * @minItems 2
   * @maxItems 2
   */
  opacity?: [number, number] | null;
  /**
   * Fade strokes away from the light: opacity x (1 - k (1 - light)) (default 0).
   */
  opacityByLight?: number | null;
  order?: Order;
  /**
   * Share of wet paint picked up from the canvas (default 0.12).
   */
  pickup?: number | null;
  placement?: Placement;
  /**
   * Order among regions within a layer, low first (default 0).
   */
  priority?: number | null;
  /**
   * Outline raggedness (default 0.5).
   */
  ragged?: number | null;
  /**
   * Blur of the reference in stroke widths (v1 `fs`).
   */
  referenceBlur?: number;
  /**
   * `"all"` or region names.
   */
  regions?: RegionSel;
  /**
   * Default 0.3.
   */
  release?: number | null;
  /**
   * Thickness multiplier for the layer.
   */
  relief?: number;
  /**
   * Chance to run against the flow (default 0).
   */
  reverseP?: number | null;
  /**
   * Lane ridge/furrow relief, x hgain (default 0.5).
   */
  ridge?: number | null;
  seedOffset?: number;
  /**
   * [y0, y1, s0, s1]: scale width and length by s0 at y0 .. s1 at y1 (perspective).
   *
   * @minItems 4
   * @maxItems 4
   */
  sizeByY?: [number, number, number, number] | null;
  /**
   * Pull toward the nearest palette mixture (default 0.85).
   */
  snap?: number | null;
  /**
   * Density placement spacing in stroke widths: a number or per region (regions not listed: 1.6).
   */
  spacing?: NumOrMap;
  /**
   * Chance to keep painting across a region edge (default 0.15).
   */
  spill?: number | null;
  /**
   * Stray hairs at the outline, 0..3 (default 1.0).
   */
  splay?: number | null;
  /**
   * Multi-scale roughness of stiff paint, x hgain (default 0.25).
   */
  stiff?: number | null;
  /**
   * Chance to stop where the soft mask fades (default 0.9).
   */
  stopAtEdge?: number | null;
  /**
   * Per-bristle colour streaks (default 0.25).
   */
  streak?: number | null;
  /**
   * Default 0.8.
   */
  streakMix?: number | null;
  /**
   * How fast the tail dries out (default 0.25).
   */
  vdry?: number | null;
  /**
   * Default "#f6d09a".
   */
  warmColor?: ColorSpec | null;
  /**
   * How much the light map warms the colour toward warmColor (default 0).
   */
  warmth?: number | null;
  /**
   * Stroke width range [min, max] in cw (default [0.015, 0.03]).
   *
   * @minItems 2
   * @maxItems 2
   */
  width?: [number, number] | null;
}
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "LightGlow".
 */
export interface LightGlow {
  /**
   * @minItems 2
   * @maxItems 2
   */
  c: [number, number];
  r: number;
  strength?: number;
}
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "LightBeam".
 */
export interface LightBeam {
  angle: number;
  /**
   * @minItems 2
   * @maxItems 2
   */
  apex: [number, number];
  length: number;
  spread: number;
  strength?: number;
}
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "LightLamp".
 */
export interface LightLamp {
  /**
   * @minItems 2
   * @maxItems 2
   */
  c: [number, number];
  r: number;
  strength?: number;
}
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "Region".
 */
export interface Region {
  /**
   * Soft-mask blur in cw.
   */
  edge?: number;
  /**
   * Stroke direction field; without one the region follows the target's structure tensor.
   */
  flow?: Flow | null;
  name: string;
  shape: Shape;
}
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "ConstantFlow".
 */
export interface ConstantFlow {
  angle?: number;
  noise?: number;
  seed?: number;
}
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "SweepFlow".
 */
export interface SweepFlow {
  angle?: number;
  curl?: number;
  noise?: number;
  scale?: number;
  seed?: number;
}
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "WavesFlow".
 */
export interface WavesFlow {
  amplitude?: number;
  angle?: number;
  horizon?: number;
  noise?: number;
  perspective?: boolean;
  seed?: number;
  wavelength?: number;
}
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "SwirlFlow".
 */
export interface SwirlFlow {
  centres: [number, number, number, number][];
  noise?: number;
  seed?: number;
  strength?: number;
}
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "RadialFlow".
 */
export interface RadialFlow {
  /**
   * @minItems 2
   * @maxItems 2
   */
  c: [number, number];
  noise?: number;
  seed?: number;
}
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "UpwardFlow".
 */
export interface UpwardFlow {
  noise?: number;
  seed?: number;
}
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "ContourFlow".
 */
export interface ContourFlow {
  noise?: number;
  polygon: [number, number][];
  seed?: number;
}
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "Ellipse".
 */
export interface Ellipse {
  /**
   * @minItems 2
   * @maxItems 2
   */
  c: [number, number];
  /**
   * Radii [rx, ry] in cw.
   *
   * @minItems 2
   * @maxItems 2
   */
  r: [number, number];
  /**
   * 0 = hard edge; otherwise the width of the fade, relative to the radius.
   */
  softness?: number;
}
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "Disc".
 */
export interface Disc {
  /**
   * @minItems 2
   * @maxItems 2
   */
  c: [number, number];
  r: number;
  softness?: number;
}
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "Wedge".
 */
export interface Wedge {
  angle: number;
  /**
   * @minItems 2
   * @maxItems 2
   */
  apex: [number, number];
  length: number;
  spread: number;
}
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "BandAround".
 */
export interface BandAround {
  dist: number;
  polygon: [number, number][];
}
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "Noisy".
 */
export interface Noisy {
  amount?: number;
  /**
   * Size of the largest noise feature, in cw.
   */
  scale?: number;
  seed?: number;
  shape: Shape;
}
/**
 * A region's painting style. Every key is optional; defaults are v1's.
 *
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "Style".
 */
export interface Style {
  /**
   * 1 = strictly along the flow, 0 = random directions (default 0.85).
   */
  align?: number | null;
  /**
   * Extra paint where the stroke starts, x hgain (default 0.3).
   */
  blob?: number | null;
  /**
   * Paint body between bristle lanes (default 0.9).
   */
  body?: number | null;
  /**
   * Palette-snapping targets.
   */
  colors?: ColorSpec[] | null;
  /**
   * 0 = straight, 1 = follows every turn of the flow (default 0.3).
   */
  curvature?: number | null;
  /**
   * Load lost per unit length (default 0.02).
   */
  deplete?: number | null;
  /**
   * Default 0.02.
   */
  dropout?: number | null;
  /**
   * Pressure at the stroke's end (default 0.15).
   */
  endPressure?: number | null;
  /**
   * Width factor at the stroke's end (default 0.5).
   */
  endWidth?: number | null;
  /**
   * Share of the surface the stroke replaces (default 0.6).
   */
  flatten?: number | null;
  /**
   * Complementary touches: [colour, probability].
   */
  flecks?: [ColorSpec, number][] | null;
  /**
   * Trough along the centre, x hgain (default 0.15).
   */
  furrow?: number | null;
  /**
   * Canvas-grain catch (default 0.10).
   */
  grain?: number | null;
  /**
   * Edge crispness (default 0.75).
   */
  hardness?: number | null;
  /**
   * Paint thickness (default 1.0).
   */
  hgain?: number | null;
  /**
   * +- relative thickness variation (default 0.25).
   */
  hgainJitter?: number | null;
  /**
   * Lab jitter [L, a/b] between strokes (default [5, 4]).
   *
   * @minItems 2
   * @maxItems 2
   */
  jitter?: [number, number] | null;
  /**
   * Stroke colours are never darker than this L* (default 20).
   */
  lFloor?: number | null;
  /**
   * Stroke length range [min, max] in cw (default [0.04, 0.10]).
   *
   * @minItems 2
   * @maxItems 2
   */
  length?: [number, number] | null;
  /**
   * Raised paint along both edges, x hgain (default 0.35).
   */
  levee?: number | null;
  /**
   * Paint on the brush at the start (default 1.0).
   */
  load?: number | null;
  /**
   * Colour B for marbling (default: a lighter/warmer variant).
   */
  load2?: ColorSpec | null;
  /**
   * Two-colour load: share of colour B in its lanes (default 0).
   */
  marble?: number | null;
  /**
   * Length >= minAspect x width unless the layer sets `dab` (default 2.5).
   */
  minAspect?: number | null;
  /**
   * Per-region override of the layer mode.
   */
  mode?: Mode | null;
  /**
   * Default 5.
   */
  nbBase?: number | null;
  /**
   * Bristle lanes = nbBase + nbPerCw x width (default 450).
   */
  nbPerCw?: number | null;
  /**
   * Opacity range [min, max] (default [0.85, 1.0]).
   *
   * @minItems 2
   * @maxItems 2
   */
  opacity?: [number, number] | null;
  /**
   * Fade strokes away from the light: opacity x (1 - k (1 - light)) (default 0).
   */
  opacityByLight?: number | null;
  /**
   * Share of wet paint picked up from the canvas (default 0.12).
   */
  pickup?: number | null;
  /**
   * Order among regions within a layer, low first (default 0).
   */
  priority?: number | null;
  /**
   * Outline raggedness (default 0.5).
   */
  ragged?: number | null;
  /**
   * Default 0.3.
   */
  release?: number | null;
  /**
   * Chance to run against the flow (default 0).
   */
  reverseP?: number | null;
  /**
   * Lane ridge/furrow relief, x hgain (default 0.5).
   */
  ridge?: number | null;
  /**
   * [y0, y1, s0, s1]: scale width and length by s0 at y0 .. s1 at y1 (perspective).
   *
   * @minItems 4
   * @maxItems 4
   */
  sizeByY?: [number, number, number, number] | null;
  /**
   * Pull toward the nearest palette mixture (default 0.85).
   */
  snap?: number | null;
  /**
   * Chance to keep painting across a region edge (default 0.15).
   */
  spill?: number | null;
  /**
   * Stray hairs at the outline, 0..3 (default 1.0).
   */
  splay?: number | null;
  /**
   * Multi-scale roughness of stiff paint, x hgain (default 0.25).
   */
  stiff?: number | null;
  /**
   * Chance to stop where the soft mask fades (default 0.9).
   */
  stopAtEdge?: number | null;
  /**
   * Per-bristle colour streaks (default 0.25).
   */
  streak?: number | null;
  /**
   * Default 0.8.
   */
  streakMix?: number | null;
  /**
   * How fast the tail dries out (default 0.25).
   */
  vdry?: number | null;
  /**
   * Default "#f6d09a".
   */
  warmColor?: ColorSpec | null;
  /**
   * How much the light map warms the colour toward warmColor (default 0).
   */
  warmth?: number | null;
  /**
   * Stroke width range [min, max] in cw (default [0.015, 0.03]).
   *
   * @minItems 2
   * @maxItems 2
   */
  width?: [number, number] | null;
}
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "Fill".
 */
export interface Fill {
  /**
   * A solid colour (give this or `gradientV`).
   */
  color?: ColorSpec | null;
  /**
   * A vertical gradient: stops [y, colour], linear between them, clamped outside.
   */
  gradientV?: [number, ColorSpec][] | null;
  mask?: Shape | null;
}
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "Blob".
 */
export interface Blob {
  /**
   * @minItems 2
   * @maxItems 2
   */
  c: [number, number];
  color: ColorSpec;
  noise?: number;
  /**
   * @minItems 2
   * @maxItems 2
   */
  r: [number, number];
  seed?: number;
  softness?: number;
  strength?: number;
}
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "PolygonOp".
 */
export interface PolygonOp {
  color: ColorSpec;
  noise?: number;
  points: [number, number][];
  seed?: number;
  /**
   * Edge blur in cw.
   */
  softness?: number;
  strength?: number;
}
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "Bands".
 */
export interface Bands {
  /**
   * [from, to, colour] with from/to as fractions of the polygon's height.
   */
  bands: [number, number, ColorSpec][];
  points: [number, number][];
  softness?: number;
}
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "Glow".
 */
export interface Glow {
  /**
   * @minItems 2
   * @maxItems 2
   */
  c: [number, number];
  color: ColorSpec;
  power?: number;
  r: number;
  strength?: number;
}
/**
 * This interface was referenced by `ScenePlan`'s JSON-Schema
 * via the `definition` "Beam".
 */
export interface Beam {
  angle: number;
  /**
   * @minItems 2
   * @maxItems 2
   */
  apex: [number, number];
  color: ColorSpec;
  length: number;
  softness?: number;
  spread: number;
  strength?: number;
}
