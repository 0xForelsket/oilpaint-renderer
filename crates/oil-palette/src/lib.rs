//! Code-driven painting with persistent palette recipes. OPJ1/OPJ2 embed the
//! palette, prepared decoder, version-gated stroke geometry and explicit loads.
//! Existing RGB StrokeLists and the default renderer remain unchanged.
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod codec;
use oil_kernel::brush::{length_in_widths, render_stroke_len, render_stroke_len_materials};
use oil_kernel::{Canvas, Load};
pub use oil_mix::palette::{
    ForwardDecoder, PaletteMixer, PaletteMixerN, TargetMatch, TargetMatchN,
};
use oil_mix::Mixer;
pub use oil_paint::PaintStats;
use oil_strokes::StrokeList;

#[derive(Debug)]
pub struct Error(String);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}
impl From<oil_mix::palette::PaletteError> for Error {
    fn from(e: oil_mix::palette::PaletteError) -> Self {
        Self(e.to_string())
    }
}
impl From<oil_mix::palette::PrepareError> for Error {
    fn from(e: oil_mix::palette::PrepareError) -> Self {
        Self(e.to_string())
    }
}

/// Normalized main and secondary loads, plus a signed zero-sum streak direction.
/// `dz` already includes the desired streak amount; no RGB matching occurs in paint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RecipeLoadN<const N: usize> {
    pub main: [f32; N],
    pub secondary: [f32; N],
    pub dz: [f32; N],
}
impl<const N: usize> RecipeLoadN<N> {
    pub fn solid(recipe: [f32; N]) -> Self {
        Self {
            main: recipe,
            secondary: recipe,
            dz: [0.0; N],
        }
    }
}

pub struct MatchReportN<const N: usize> {
    pub ground: TargetMatchN<N>,
    /// Main and secondary load reports in stroke order. Streak variants are
    /// matched once too, but these errors describe the two principal loads.
    pub strokes: Vec<[TargetMatchN<N>; 2]>,
}

pub struct PaletteJobN<const N: usize, const PREPARED: bool = false, const B: usize = 81> {
    mixer: PaletteMixerN<N, PREPARED, B>,
    geometry: StrokeList,
    ground: [f32; N],
    loads: Vec<RecipeLoadN<N>>,
}

fn normalized<const N: usize>(z: &[f32; N]) -> bool {
    z.iter().all(|v| v.is_finite() && *v >= 0.0)
        && (z.iter().map(|v| *v as f64).sum::<f64>() - 1.0).abs() <= 1e-5
}

pub type PaletteJob = PaletteJobN<4, true>;
pub type RecipeLoad = RecipeLoadN<4>;
pub type MatchReport = MatchReportN<4>;

impl<const N: usize, const PREPARED: bool, const B: usize> PaletteJobN<N, PREPARED, B> {
    /// Geometry's RGB fields remain authoring provenance; the supplied recipe
    /// loads and ground are the only material inputs consumed by painting.
    pub fn new(
        mixer: PaletteMixerN<N, PREPARED, B>,
        geometry: StrokeList,
        ground: [f32; N],
        loads: Vec<RecipeLoadN<N>>,
    ) -> Result<Self, Error> {
        let errors = geometry.validate();
        if !errors.is_empty() {
            return Err(Error(format!("Invalid stroke geometry: {errors:?}")));
        }
        if !normalized(&ground) || loads.len() != geometry.strokes.len() {
            return Err(Error(
                "Expected a normalized ground and one recipe load per stroke".into(),
            ));
        }
        for (i, load) in loads.iter().enumerate() {
            if !normalized(&load.main)
                || !normalized(&load.secondary)
                || load.dz.iter().any(|v| !v.is_finite() || v.abs() > 1.0)
                || load.dz.iter().map(|v| *v as f64).sum::<f64>().abs() > 1e-5
            {
                return Err(Error(format!(
                    "Invalid normalized load or zero-sum streak at stroke {i}"
                )));
            }
        }
        Ok(Self {
            mixer,
            geometry,
            ground,
            loads,
        })
    }

    /// Optional import path: match existing authored RGB once, then retain the
    /// resulting recipes. Call `new` to author recipes directly with no search.
    pub fn from_rgb(
        mixer: PaletteMixerN<N, PREPARED, B>,
        geometry: StrokeList,
    ) -> Result<(Self, MatchReportN<N>), Error> {
        let errors = geometry.validate();
        if !errors.is_empty() {
            return Err(Error(format!("Invalid stroke geometry: {errors:?}")));
        }
        let ground = mixer.match_target(geometry.ground)?;
        let mut strokes = Vec::with_capacity(geometry.strokes.len());
        let mut loads = Vec::with_capacity(geometry.strokes.len());
        for s in &geometry.strokes {
            let main = mixer.match_target(s.color)?;
            let secondary = if s.color2 == s.color {
                main
            } else {
                mixer.match_target(s.color2)?
            };
            let dz = if s.streak_amount == 0.0 {
                [0.0; N]
            } else {
                oil_paint::streak_vector(&mixer, &main.recipe, s.streak_amount)
            };
            loads.push(RecipeLoadN {
                main: main.recipe,
                secondary: secondary.recipe,
                dz,
            });
            strokes.push([main, secondary]);
        }
        Ok((
            Self::new(mixer, geometry, ground.recipe, loads)?,
            MatchReportN { ground, strokes },
        ))
    }
    pub fn mixer(&self) -> &PaletteMixerN<N, PREPARED, B> {
        &self.mixer
    }
    /// Change only the versioned display evaluator, preserving every authored
    /// recipe and optical coefficient. The choice is saved in direct OPJ2 jobs.
    pub fn with_forward_decoder(mut self, method: ForwardDecoder) -> Result<Self, Error> {
        self.mixer = self.mixer.with_forward_decoder(method)?;
        Ok(self)
    }
    pub fn geometry(&self) -> &StrokeList {
        &self.geometry
    }
    pub fn ground(&self) -> [f32; N] {
        self.ground
    }
    pub fn loads(&self) -> &[RecipeLoadN<N>] {
        &self.loads
    }

    /// Paint explicit recipe states through the existing brush/transport kernel.
    /// This native entry point allows width <=16384 and <=16M pixels per canvas.
    pub fn paint(
        &self,
        width: u32,
        after_layer: impl FnMut(usize, &Canvas<PaletteMixerN<N, PREPARED, B>>),
    ) -> Result<(Canvas<PaletteMixerN<N, PREPARED, B>>, PaintStats), Error> {
        self.paint_impl::<false>(width, after_layer)
    }

    /// Render only the final image, evaluating the unchanged decoder once per
    /// pixel that received paint. Material transport is identical to `paint`.
    /// No intermediate layer previews are produced. Saved formats are unchanged.
    pub fn paint_final(
        &self,
        width: u32,
    ) -> Result<(Canvas<PaletteMixerN<N, PREPARED, B>>, PaintStats), Error> {
        self.paint_impl::<true>(width, |_, _| {})
    }

    fn paint_impl<const DEFER_DISPLAY: bool>(
        &self,
        width: u32,
        mut after_layer: impl FnMut(usize, &Canvas<PaletteMixerN<N, PREPARED, B>>),
    ) -> Result<(Canvas<PaletteMixerN<N, PREPARED, B>>, PaintStats), Error> {
        if width == 0 || width > 16384 {
            return Err(Error("Canvas width must be in 1..=16384".into()));
        }
        let [aw, ah] = self.geometry.aspect.map(|v| v as u64);
        let height = (2 * width as u64 * ah + aw) / (2 * aw);
        if height == 0 || height > 16384 || width as u64 * height > 16_777_216 {
            return Err(Error(
                "Canvas exceeds native palette dimensions or 16M-pixel limit".into(),
            ));
        }
        let (w, h) = (width as usize, height as usize);
        let n = w * h;
        let m = &self.mixer;
        let mut canvas = Canvas {
            w,
            h,
            lat: vec![self.ground; n],
            rgb: vec![m.decode_srgb(&self.ground); n],
            hgt: vec![0.0; n],
            wet: vec![0.0; n],
            cover: vec![0.0; n],
            hblur: Vec::new(),
        };
        let mut stats = PaintStats::default();
        let mut points = Vec::new();
        for (li, layer) in self.geometry.layers.iter().enumerate() {
            if let Some(sigma) = layer.hblur_sigma {
                canvas.hblur =
                    oil_image::blur(&canvas.hgt, w, h, (sigma as f64 * width as f64).max(1.0));
            }
            for i in layer.start as usize..layer.end as usize {
                let stroke = &self.geometry.strokes[i];
                let path = self.geometry.stroke_points(i);
                points.clear();
                points.extend(path.iter().map(|p| {
                    [
                        p[0] * width as f32,
                        p[1] * width as f32,
                        p[2] * width as f32,
                        p[3],
                    ]
                }));
                let recipe = self.loads[i];
                let load = Load {
                    zcol: recipe.main,
                    zcol2: recipe.secondary,
                    dz: recipe.dz,
                };
                let st = if DEFER_DISPLAY {
                    render_stroke_len_materials(
                        m,
                        &mut canvas.planes(),
                        &points,
                        &load,
                        &stroke.brush,
                        length_in_widths(path),
                    )
                } else {
                    render_stroke_len(
                        m,
                        &mut canvas.planes(),
                        &points,
                        &load,
                        &stroke.brush,
                        length_in_widths(path),
                    )
                };
                stats.strokes += 1;
                stats.painted_pixels += st.pixels;
                stats.alpha += st.alpha;
            }
            if let Some(d) = layer.dry_after {
                canvas.dry(d);
            }
            if !DEFER_DISPLAY {
                after_layer(li, &canvas);
            }
        }
        if DEFER_DISPLAY {
            // Every deposit reaching the former RGB update also increments cover
            // by positive alpha (>0.002). Untouched pixels retain the exact ground
            // display; each touched pixel uses its final, unchanged recipe.
            for ((rgb, state), cover) in canvas.rgb.iter_mut().zip(&canvas.lat).zip(&canvas.cover) {
                if *cover > 0. {
                    *rgb = m.decode_srgb(state);
                }
            }
        }
        Ok((canvas, stats))
    }
}
