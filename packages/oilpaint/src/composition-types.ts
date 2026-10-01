// Generated from oil-author::Composition / spec/composition-1.schema.json. Do not edit.

export interface Composition {
  /**
   * @minItems 2
   * @maxItems 2
   */
  aspect: [number, number];
  engine: string;
  format: string;
  /**
   * @minItems 3
   * @maxItems 3
   */
  ground: [number, number, number];
  groups: PlannedGroup[];
  sourceHash?: string | null;
  title?: string | null;
  version: number;
}
/**
 * This interface was referenced by `Composition`'s JSON-Schema
 * via the `definition` "PlannedGroup".
 */
export interface PlannedGroup {
  dryAfter?: number | null;
  hblurSigma?: number | null;
  id: string;
  name: string;
  pass: string;
  region?: string | null;
  strokes: ResolvedStroke[];
  visible: boolean;
}
/**
 * This interface was referenced by `Composition`'s JSON-Schema
 * via the `definition` "ResolvedStroke".
 */
export interface ResolvedStroke {
  brush: BrushParams;
  /**
   * @minItems 3
   * @maxItems 3
   */
  color: [number, number, number];
  /**
   * @minItems 3
   * @maxItems 3
   */
  color2: [number, number, number];
  id: string;
  points: [number, number, number, number][];
  preset?: string | null;
  streakAmount: number;
}
/**
 * This interface was referenced by `Composition`'s JSON-Schema
 * via the `definition` "BrushParams".
 */
export interface BrushParams {
  blob: number;
  body: number;
  deplete: number;
  dropout: number;
  dryThresh: number;
  dryWidth: number;
  flatten: number;
  furrow: number;
  grain: number;
  hardness: number;
  hgain: number;
  levee: number;
  load: number;
  marble: number;
  mode: number;
  nb: number;
  opacity: number;
  pickup: number;
  ragged: number;
  release: number;
  ridge: number;
  seed: number;
  splay: number;
  stiff: number;
  streak: number;
  streakMix: number;
  vdry: number;
}
