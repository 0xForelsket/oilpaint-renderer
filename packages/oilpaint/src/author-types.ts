// Generated from Rust oil-author via spec/author-1.schema.json. Do not edit.

export interface Document {
  /**
   * @minItems 2
   * @maxItems 2
   */
  aspect: [number, number];
  catalog: Catalog;
  engine: string;
  format: string;
  /**
   * @minItems 3
   * @maxItems 3
   */
  ground: [number, number, number];
  groups: Group[];
  seed: number;
  version: number;
}
/**
 * Embedded immutable definitions; catalog changes cannot silently change a saved painting.
 */
export interface Catalog {
  presets: Preset[];
  version: number;
}
/**
 * This interface was referenced by `Document`'s JSON-Schema
 * via the `definition` "Preset".
 */
export interface Preset {
  depletion: number;
  id: string;
  mode: number;
  name: string;
  paint: {
    [k: string]: number;
  };
  /**
   * @minItems 3
   * @maxItems 3
   */
  pressureProfile: [number, number, number];
  status: string;
  /**
   * @minItems 3
   * @maxItems 3
   */
  width: [number, number, number];
  /**
   * @minItems 3
   * @maxItems 3
   */
  widthProfile: [number, number, number];
}
/**
 * This interface was referenced by `Document`'s JSON-Schema
 * via the `definition` "Group".
 */
export interface Group {
  /**
   * Wetness multiplier after this group: 0 dry, 1 wet.
   */
  dryAfter: number;
  id: string;
  name: string;
  strokes: Mark[];
  visible: boolean;
}
/**
 * This interface was referenced by `Document`'s JSON-Schema
 * via the `definition` "Mark".
 */
export interface Mark {
  /**
   * @minItems 3
   * @maxItems 3
   */
  color: [number, number, number];
  controls: {
    [k: string]: number;
  };
  id: string;
  /**
   * x,y,pressure in cw. Pressure is multiplied by the preset envelope.
   */
  path: [number, number, number][];
  preset: string;
  width: number;
}
/**
 * This interface was referenced by `Document`'s JSON-Schema
 * via the `definition` "Catalog".
 */
export interface Catalog1 {
  presets: Preset[];
  version: number;
}
