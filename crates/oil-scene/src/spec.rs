//! ScenePlan v1 types (spec/SCENEPLAN_V1.md). serde reads and writes the JSON; schemars derives the JSON Schema
//! (`spec/sceneplan-1.schema.json`), which generates the TypeScript types: one source of truth. Doc comments become
//! the schema's descriptions, so they double as the parameter catalogue for agents.
//!
//! Units: coordinates and sizes are in canvas widths (cw), y points down, angles are degrees (0 = +x, 90 = down).
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A point (x, y) in canvas widths.
pub type Point = [f64; 2];

macro_rules! defaults {
    ($($name:ident: $t:ty = $v:expr;)*) => { $(fn $name() -> $t { $v })* };
}

defaults! {
    d_edge: f64 = 0.02;
    d_blob_soft: f64 = 0.5;
    d_poly_soft: f64 = 0.01;
    d_one: f64 = 1.0;
    d_two: f64 = 2.0;
    d_beam_strength: f64 = 0.4;
    d_beam_soft: f64 = 0.3;
    d_bands_soft: f64 = 0.008;
    d_seed_blob: u32 = 21;
    d_seed_poly: u32 = 22;
    d_seed_noisy: u32 = 11;
    d_noisy_amount: f64 = 0.3;
    d_noisy_scale: f64 = 0.08;
    d_seed1: u32 = 1;
    d_sweep_angle: f64 = -12.0;
    d_sweep_curl: f64 = 0.4;
    d_sweep_noise: f64 = 0.25;
    d_sweep_scale: f64 = 0.5;
    d_seed2: u32 = 2;
    d_waves_amp: f64 = 18.0;
    d_waves_wl: f64 = 0.12;
    d_waves_noise: f64 = 0.3;
    d_true: bool = true;
    d_half: f64 = 0.5;
    d_seed3: u32 = 3;
    d_swirl_strength: f64 = 0.7;
    d_swirl_noise: f64 = 0.3;
    d_seed4: u32 = 4;
    d_radial_noise: f64 = 0.1;
    d_seed5: u32 = 5;
    d_upward_noise: f64 = 0.6;
    d_seed6: u32 = 6;
    d_contour_noise: f64 = 0.35;
    d_seed7: u32 = 7;
    d_error_threshold: f64 = 18.0;
    d_spacing: NumOrMap = NumOrMap::Num(1.6);
    d_coverage: NumOrMap = NumOrMap::Num(1.0);
    d_hblur: f64 = 0.01;
    d_gap_cover: f64 = 0.15;
    d_regions: RegionSel = RegionSel::Name("all".into());
}

/// A colour: `"#rrggbb"`, a tube name (for example `"lead_white"`, `"cobalt_blue"`), an sRGB triple with channels in
/// [0, 1], or `["a", "b", t]`, a mix of two colours at t in [0, 1] made with the active mixer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ColorSpec {
    Named(String),
    Mix(String, String, f64),
    Rgb([f64; 3]),
}

/// A ScenePlan: the declarative description of a painting. With the plan options `{seed, planWidth, mixer}` and an
/// engine version it determines the StrokeList bit for bit on every host.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ScenePlan {
    /// Ignored by the engine; lets editors validate.
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    /// Schema generation: 1.
    pub sceneplan: u32,
    /// Engine version the scene was authored and tuned with. A different engine warns (`ENGINE_VERSION_DIFFERS`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub canvas: Canvas,
    /// Target-image operations, applied in order: the reference colours the planner paints toward.
    pub target: Vec<TargetOp>,
    /// Regions, in order: later regions override earlier ones in the hard region map.
    pub regions: Vec<Region>,
    /// Contributions to the light map (drives `warmth` and `opacityByLight`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lights: Vec<Light>,
    /// Style per region, keyed by region name.
    pub styles: BTreeMap<String, Style>,
    /// The layer schedule, painted in order.
    pub layers: Vec<Layer>,
    /// Declarations of sampled fields (escape hatch): the arrays are passed next to the spec.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, FieldDecl>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Canvas {
    /// Aspect ratio as positive integers [width, height], for example [4, 5].
    pub aspect: [u32; 2],
    /// Ground (canvas) colour.
    pub ground: ColorSpec,
}

// ------------------------------------------------------------------ shapes

/// A mask in [0, 1], as a single-key object.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Shape {
    /// Everywhere (`{"all": true}`).
    All(bool),
    /// Above a y (y < value).
    Above(f64),
    /// At or below a y (y >= value).
    Below(f64),
    Ellipse(Ellipse),
    Disc(Disc),
    /// A polygon (at least 3 points), filled.
    Polygon(Vec<Point>),
    /// A soft sector from `apex` pointing at `angle`, `spread` degrees either side, `length` long.
    Wedge(Wedge),
    /// Pixels within `dist` of a polygon's outline.
    BandAround(BandAround),
    /// Sum of the shapes, clamped to 1.
    Union(Vec<Shape>),
    /// Product of the shapes.
    Intersect(Vec<Shape>),
    /// 1 - shape.
    Not(Box<Shape>),
    /// A shape whose boundary is perturbed by noise.
    Noisy(Box<Noisy>),
    /// A sampled mask (escape hatch), by name.
    Field(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Ellipse {
    pub c: Point,
    /// Radii [rx, ry] in cw.
    pub r: [f64; 2],
    /// 0 = hard edge; otherwise the width of the fade, relative to the radius.
    #[serde(default)]
    pub softness: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Disc {
    pub c: Point,
    pub r: f64,
    #[serde(default)]
    pub softness: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Wedge {
    pub apex: Point,
    pub angle: f64,
    pub spread: f64,
    pub length: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BandAround {
    pub polygon: Vec<Point>,
    pub dist: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Noisy {
    pub shape: Shape,
    #[serde(default = "d_noisy_amount")]
    pub amount: f64,
    /// Size of the largest noise feature, in cw.
    #[serde(default = "d_noisy_scale")]
    pub scale: f64,
    #[serde(default = "d_seed_noisy")]
    pub seed: u32,
}

// ------------------------------------------------------------------ target

/// A target-image operation, as a single-key object.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum TargetOp {
    /// Replace (inside `mask`, if given) with a colour or a vertical gradient.
    Fill(Fill),
    /// An elliptical blob, alpha-blended.
    Blob(Blob),
    /// A polygon, alpha-blended.
    Polygon(PolygonOp),
    /// Horizontal colour bands inside a polygon; band positions are fractions of the polygon's height.
    Bands(Bands),
    /// Additive light around a point.
    Glow(Glow),
    /// Additive light in a wedge.
    Beam(Beam),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Fill {
    /// A solid colour (give this or `gradientV`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<ColorSpec>,
    /// A vertical gradient: stops [y, colour], linear between them, clamped outside.
    #[serde(rename = "gradientV", default, skip_serializing_if = "Option::is_none")]
    pub gradient_v: Option<Vec<(f64, ColorSpec)>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<Shape>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Blob {
    pub c: Point,
    pub r: [f64; 2],
    pub color: ColorSpec,
    #[serde(default = "d_blob_soft")]
    pub softness: f64,
    #[serde(default)]
    pub noise: f64,
    #[serde(default = "d_one")]
    pub strength: f64,
    #[serde(default = "d_seed_blob")]
    pub seed: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PolygonOp {
    pub points: Vec<Point>,
    pub color: ColorSpec,
    /// Edge blur in cw.
    #[serde(default = "d_poly_soft")]
    pub softness: f64,
    #[serde(default)]
    pub noise: f64,
    #[serde(default = "d_one")]
    pub strength: f64,
    #[serde(default = "d_seed_poly")]
    pub seed: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Bands {
    pub points: Vec<Point>,
    /// [from, to, colour] with from/to as fractions of the polygon's height.
    pub bands: Vec<(f64, f64, ColorSpec)>,
    #[serde(default = "d_bands_soft")]
    pub softness: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Glow {
    pub c: Point,
    pub r: f64,
    pub color: ColorSpec,
    #[serde(default = "d_one")]
    pub strength: f64,
    #[serde(default = "d_two")]
    pub power: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Beam {
    pub apex: Point,
    pub angle: f64,
    pub spread: f64,
    pub length: f64,
    pub color: ColorSpec,
    #[serde(default = "d_beam_strength")]
    pub strength: f64,
    #[serde(default = "d_beam_soft")]
    pub softness: f64,
}

// ------------------------------------------------------------------ regions and flows

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Region {
    pub name: String,
    pub shape: Shape,
    /// Soft-mask blur in cw.
    #[serde(default = "d_edge")]
    pub edge: f64,
    /// Stroke direction field; without one the region follows the target's structure tensor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow: Option<Flow>,
}

/// A unit direction field, as a single-key object.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Flow {
    Constant(ConstantFlow),
    /// Long arcs: base direction plus a slowly varying bend.
    Sweep(SweepFlow),
    /// Near-horizontal with an undulation whose wavelength shrinks toward the horizon.
    Waves(WavesFlow),
    /// Tangential flow around ellipse centres [cx, cy, rx, ry].
    SwirlAround(SwirlFlow),
    RadialFrom(RadialFlow),
    Upward(UpwardFlow),
    /// Tangent of a polygon's distance field: strokes follow its outline.
    Contour(ContourFlow),
    /// A sampled flow (escape hatch), by name.
    Field(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ConstantFlow {
    #[serde(default)]
    pub angle: f64,
    #[serde(default)]
    pub noise: f64,
    #[serde(default = "d_seed1")]
    pub seed: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SweepFlow {
    #[serde(default = "d_sweep_angle")]
    pub angle: f64,
    #[serde(default = "d_sweep_curl")]
    pub curl: f64,
    #[serde(default = "d_sweep_noise")]
    pub noise: f64,
    #[serde(default = "d_sweep_scale")]
    pub scale: f64,
    #[serde(default = "d_seed2")]
    pub seed: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct WavesFlow {
    #[serde(default)]
    pub angle: f64,
    #[serde(default = "d_waves_amp")]
    pub amplitude: f64,
    #[serde(default = "d_waves_wl")]
    pub wavelength: f64,
    #[serde(default = "d_waves_noise")]
    pub noise: f64,
    #[serde(default = "d_true")]
    pub perspective: bool,
    #[serde(default = "d_half")]
    pub horizon: f64,
    #[serde(default = "d_seed3")]
    pub seed: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SwirlFlow {
    pub centres: Vec<[f64; 4]>,
    #[serde(default = "d_swirl_strength")]
    pub strength: f64,
    #[serde(default = "d_swirl_noise")]
    pub noise: f64,
    #[serde(default = "d_seed4")]
    pub seed: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RadialFlow {
    pub c: Point,
    #[serde(default = "d_radial_noise")]
    pub noise: f64,
    #[serde(default = "d_seed5")]
    pub seed: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UpwardFlow {
    #[serde(default = "d_upward_noise")]
    pub noise: f64,
    #[serde(default = "d_seed6")]
    pub seed: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ContourFlow {
    pub polygon: Vec<Point>,
    #[serde(default = "d_contour_noise")]
    pub noise: f64,
    #[serde(default = "d_seed7")]
    pub seed: u32,
}

// ------------------------------------------------------------------ lights

/// A light-map contribution, as a single-key object.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Light {
    /// An ellipse of light, 2.5x flatter than wide.
    Glow(LightGlow),
    Beam(LightBeam),
    Lamp(LightLamp),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LightGlow {
    pub c: Point,
    pub r: f64,
    #[serde(default = "d_one")]
    pub strength: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LightBeam {
    pub apex: Point,
    pub angle: f64,
    pub spread: f64,
    pub length: f64,
    #[serde(default = "d_one")]
    pub strength: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LightLamp {
    pub c: Point,
    pub r: f64,
    #[serde(default = "d_one")]
    pub strength: f64,
}

// ------------------------------------------------------------------ styles and layers

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Paint,
    Scumble,
    Smudge,
    Glaze,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum Placement {
    /// Where the canvas differs most from the target.
    #[default]
    Error,
    /// Even coverage at `spacing`.
    Density,
    /// Along given curves (`curve`).
    Curve,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum ColorFrom {
    /// Sample the target image.
    #[default]
    Reference,
    /// Pick from the style's `colors`.
    Palette,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum Order {
    #[default]
    Sweep,
    Random,
}

/// A number for all regions, or one per region name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum NumOrMap {
    Num(f64),
    Map(BTreeMap<String, f64>),
}

/// `"all"` or a list of region names.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum RegionSel {
    Name(String),
    List(Vec<String>),
}

/// `"boundary"` (the region's outline) or a polyline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum CurveSpec {
    Named(String),
    Points(Vec<Point>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum FieldKind {
    Mask,
    Flow,
    Rgb,
}

/// A sampled field: little-endian f32 values (1 channel for a mask, 2 for a flow, 3 for rgb), row-major, passed next
/// to the spec and checked against `sha256`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FieldDecl {
    pub kind: FieldKind,
    pub width: u32,
    pub height: u32,
    pub sha256: String,
}

/// Declares a struct with the given fields plus every style key (all optional). Styles and layers share the keys: a
/// layer's value overrides the region styles for that layer.
macro_rules! with_style_keys {
    ($(#[$m:meta])* pub struct $name:ident { $($(#[$fm:meta])* pub $f:ident : $t:ty,)* }) => {
        $(#[$m])*
        pub struct $name {
            $($(#[$fm])* pub $f: $t,)*
            /// Palette-snapping targets.
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub colors: Option<Vec<ColorSpec>>,
            /// Complementary touches: [colour, probability].
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub flecks: Option<Vec<(ColorSpec, f64)>>,
            /// Stroke width range [min, max] in cw (default [0.015, 0.03]).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub width: Option<[f64; 2]>,
            /// Stroke length range [min, max] in cw (default [0.04, 0.10]).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub length: Option<[f64; 2]>,
            /// 0 = straight, 1 = follows every turn of the flow (default 0.3).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub curvature: Option<f64>,
            /// 1 = strictly along the flow, 0 = random directions (default 0.85).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub align: Option<f64>,
            /// Opacity range [min, max] (default [0.85, 1.0]).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub opacity: Option<[f64; 2]>,
            /// Share of wet paint picked up from the canvas (default 0.12).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub pickup: Option<f64>,
            /// Paint on the brush at the start (default 1.0).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub load: Option<f64>,
            /// Load lost per unit length (default 0.02).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub deplete: Option<f64>,
            /// How fast the tail dries out (default 0.25).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub vdry: Option<f64>,
            /// Paint thickness (default 1.0).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub hgain: Option<f64>,
            /// Share of the surface the stroke replaces (default 0.6).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub flatten: Option<f64>,
            /// Per-bristle colour streaks (default 0.25).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub streak: Option<f64>,
            /// Default 0.8.
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub streak_mix: Option<f64>,
            /// Paint body between bristle lanes (default 0.9).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub body: Option<f64>,
            /// Edge crispness (default 0.75).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub hardness: Option<f64>,
            /// Canvas-grain catch (default 0.10).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub grain: Option<f64>,
            /// Bristle lanes = nbBase + nbPerCw x width (default 450).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub nb_per_cw: Option<f64>,
            /// Default 5.
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub nb_base: Option<f64>,
            /// Default 0.3.
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub release: Option<f64>,
            /// Default 0.02.
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub dropout: Option<f64>,
            /// Outline raggedness (default 0.5).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub ragged: Option<f64>,
            /// Lane ridge/furrow relief, x hgain (default 0.5).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub ridge: Option<f64>,
            /// Raised paint along both edges, x hgain (default 0.35).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub levee: Option<f64>,
            /// Trough along the centre, x hgain (default 0.15).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub furrow: Option<f64>,
            /// Extra paint where the stroke starts, x hgain (default 0.3).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub blob: Option<f64>,
            /// Multi-scale roughness of stiff paint, x hgain (default 0.25).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub stiff: Option<f64>,
            /// Two-colour load: share of colour B in its lanes (default 0).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub marble: Option<f64>,
            /// Colour B for marbling (default: a lighter/warmer variant).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub load2: Option<ColorSpec>,
            /// Stray hairs at the outline, 0..3 (default 1.0).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub splay: Option<f64>,
            /// Pull toward the nearest palette mixture (default 0.85).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub snap: Option<f64>,
            /// Lab jitter [L, a/b] between strokes (default [5, 4]).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub jitter: Option<[f64; 2]>,
            /// How much the light map warms the colour toward warmColor (default 0).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub warmth: Option<f64>,
            /// Default "#f6d09a".
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub warm_color: Option<ColorSpec>,
            /// Order among regions within a layer, low first (default 0).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub priority: Option<i32>,
            /// Chance to run against the flow (default 0).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub reverse_p: Option<f64>,
            /// Chance to keep painting across a region edge (default 0.15).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub spill: Option<f64>,
            /// Chance to stop where the soft mask fades (default 0.9).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub stop_at_edge: Option<f64>,
            /// Length >= minAspect x width unless the layer sets `dab` (default 2.5).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub min_aspect: Option<f64>,
            /// Width factor at the stroke's end (default 0.5).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub end_width: Option<f64>,
            /// Pressure at the stroke's end (default 0.15).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub end_pressure: Option<f64>,
            /// +- relative thickness variation (default 0.25).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub hgain_jitter: Option<f64>,
            /// [y0, y1, s0, s1]: scale width and length by s0 at y0 .. s1 at y1 (perspective).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub size_by_y: Option<[f64; 4]>,
            /// Fade strokes away from the light: opacity x (1 - k (1 - light)) (default 0).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub opacity_by_light: Option<f64>,
            /// Stroke colours are never darker than this L* (default 20).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub l_floor: Option<f64>,
        }
    };
}

with_style_keys! {
    /// A region's painting style. Every key is optional; defaults are v1's.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    pub struct Style {
        /// Per-region override of the layer mode.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub mode: Option<Mode>,
    }
}

with_style_keys! {
    /// A layer of the schedule. Besides its own keys it accepts every style key, overriding the region styles for this
    /// layer.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    pub struct Layer {
        pub name: String,
        /// `"all"` or region names.
        #[serde(default = "d_regions")]
        pub regions: RegionSel,
        #[serde(default)]
        pub placement: Placement,
        /// paint, scumble, smudge or glaze (default paint).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub mode: Option<Mode>,
        /// Lab error that triggers a stroke (error placement; v1 `T`).
        #[serde(default = "d_error_threshold")]
        pub error_threshold: f64,
        /// Proposal grid cell in stroke widths (v1 `fg`).
        #[serde(default = "d_one")]
        pub grid_factor: f64,
        /// Blur of the reference in stroke widths (v1 `fs`).
        #[serde(default = "d_half")]
        pub reference_blur: f64,
        /// Density placement spacing in stroke widths: a number or per region.
        #[serde(default = "d_spacing")]
        pub spacing: NumOrMap,
        /// Share of the region to cover: a number or per region.
        #[serde(default = "d_coverage")]
        pub coverage: NumOrMap,
        /// Wetness factor applied after the layer.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub dry_after: Option<f64>,
        #[serde(default)]
        pub color_from: ColorFrom,
        #[serde(default)]
        pub order: Order,
        #[serde(default = "d_half")]
        pub jitter_pos: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub max_strokes: Option<u32>,
        #[serde(default = "d_true")]
        pub enabled: bool,
        #[serde(default)]
        pub seed_offset: i64,
        /// Height blur before scumble, in cw.
        #[serde(default = "d_hblur")]
        pub hblur_sigma: f64,
        /// Thickness multiplier for the layer.
        #[serde(default = "d_one")]
        pub relief: f64,
        /// Skip sites already covered more than this.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub max_cover: Option<f64>,
        #[serde(default = "d_true")]
        pub gap_fill: bool,
        #[serde(default = "d_gap_cover")]
        pub gap_cover: f64,
        /// Scumble: height-above-blur threshold where paint catches (default 0).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub dry_thresh: Option<f64>,
        /// Scumble: smoothstep width around `dryThresh` (default 0.15).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub dry_width: Option<f64>,
        /// Allow short, round dabs.
        #[serde(default)]
        pub dab: bool,
        /// Curve placement: per region, "boundary" or a polyline.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub curve: Option<BTreeMap<String, CurveSpec>>,
        #[serde(default)]
        pub curve_offset: f64,
        #[serde(default = "d_one")]
        pub curve_spacing: f64,
        #[serde(default = "d_half")]
        pub curve_jitter: f64,
    }
}
