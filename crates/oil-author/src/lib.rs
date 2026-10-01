//! Explicit authored documents. Additive to StrokeList v2; no existing file is reinterpreted.
#![forbid(unsafe_code)]
use oil_kernel::{BrushParams, Canvas};
use oil_mix::Mixer;
use oil_strokes::{Layer, Meta, Stroke, StrokeList, NO_REGION};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const CATALOG: &str = include_str!("../catalog.json");
pub const AUTHOR_VERSION: u32 = 1;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub status: String,
    pub mode: i32,
    pub width: [f32; 3],         // min, default, max (cw)
    pub width_profile: [f32; 3], // landing, body, tail
    pub pressure_profile: [f32; 3],
    pub depletion: f32, // total depletion per path length in half-widths
    pub paint: BTreeMap<String, f32>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Catalog {
    pub version: u32,
    pub presets: Vec<Preset>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Mark {
    pub id: String,
    /// x,y,pressure in cw. Pressure is multiplied by the preset envelope.
    pub path: Vec<[f32; 3]>,
    pub width: f32,
    pub color: [f32; 3],
    pub preset: String,
    pub controls: BTreeMap<String, f32>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub visible: bool,
    /// Wetness multiplier after this group: 0 dry, 1 wet.
    pub dry_after: f32,
    pub strokes: Vec<Mark>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Document {
    pub format: String,
    pub version: u32,
    pub engine: String,
    pub seed: u32,
    pub aspect: [u32; 2],
    pub ground: [f32; 3],
    /// Embedded immutable definitions; catalog changes cannot silently change a saved painting.
    pub catalog: Catalog,
    pub groups: Vec<Group>,
}
pub fn catalog() -> Catalog {
    serde_json::from_str(CATALOG).expect("built-in catalog")
}
fn unit(v: f32) -> bool {
    (0.0..=1.0).contains(&v)
}
fn range(v: f32, lo: f32, hi: f32) -> bool {
    v.is_finite() && v >= lo && v <= hi
}
pub fn control_range(k: &str) -> Option<(f32, f32)> {
    Some(match k {
        "opacity" | "pickup" | "flatten" | "hardness" | "dropout" | "body" | "release"
        | "streakMix" | "marble" => (0., 1.),
        "load" => (0., 1.5),
        "vdry" => (0.05, 1.),
        "hgain" => (0., 3.),
        "nb" => (2., 68.),
        "splay" => (0., 3.),
        "deplete" | "streak" | "grain" | "ragged" | "ridge" | "levee" | "furrow" | "blob"
        | "stiff" => (0., 3.),
        _ => return None,
    })
}
fn apply(b: &mut BrushParams, values: &BTreeMap<String, f32>) -> Result<(), String> {
    for (k, &v) in values {
        let (lo, hi) = control_range(k).ok_or_else(|| format!("UNKNOWN_CONTROL: {k}"))?;
        if !range(v, lo, hi) || (k == "nb" && v.fract() != 0.) {
            return Err(format!("INVALID_CONTROL: {k} must be {lo}..{hi}"));
        }
        match k.as_str() {
            "opacity" => b.opacity = v,
            "pickup" => b.pickup = v,
            "load" => b.load = v,
            "deplete" => b.deplete = v,
            "vdry" => b.vdry = v,
            "hgain" => b.hgain = v,
            "flatten" => b.flatten = v,
            "streak" => b.streak = v,
            "hardness" => b.hardness = v,
            "grain" => b.grain = v,
            "nb" => b.nb = v as i32,
            "dropout" => b.dropout = v,
            "ragged" => b.ragged = v,
            "body" => b.body = v,
            "release" => b.release = v,
            "streakMix" => b.streak_mix = v,
            "ridge" => b.ridge = v,
            "levee" => b.levee = v,
            "furrow" => b.furrow = v,
            "blob" => b.blob = v,
            "stiff" => b.stiff = v,
            "marble" => b.marble = v,
            "splay" => b.splay = v,
            _ => unreachable!(),
        }
    }
    Ok(())
}
impl Catalog {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("CATALOG_VERSION_MISMATCH".into());
        }
        let mut ids = BTreeSet::new();
        if self.presets.is_empty() {
            return Err("EMPTY_CATALOG".into());
        }
        for p in &self.presets {
            if p.id.is_empty()
                || !ids.insert(&p.id)
                || p.name.is_empty()
                || p.status != "candidate"
                || !(0..=3).contains(&p.mode)
                || !range(p.width[0], 0.0001, 0.2)
                || !(p.width[0] <= p.width[1] && p.width[1] <= p.width[2])
                || !range(p.width[2], 0.0001, 0.2)
                || !p.width_profile.iter().all(|&v| range(v, 0.01, 2.))
                || !p.pressure_profile.iter().all(|&v| unit(v))
                || !range(p.depletion, 0., 3.)
            {
                return Err(format!("INVALID_PRESET: {}", p.id));
            }
            apply(&mut BrushParams::default(), &p.paint)?;
        }
        Ok(())
    }
}
/// Stable FNV-1a seed over length-delimited UTF-8 scopes. No array indices or sequential shared RNG.
pub fn scoped_seed(seed: u32, scopes: &[&str]) -> u32 {
    let mut h = 2166136261u32 ^ seed;
    for s in scopes {
        for b in (s.len() as u32).to_le_bytes().iter().chain(s.as_bytes()) {
            h = (h ^ *b as u32).wrapping_mul(16777619);
        }
    }
    h
}
fn envelope(p: [f32; 3], t: f32) -> f32 {
    if t < 0.2 {
        p[0] + (p[1] - p[0]) * t / 0.2
    } else if t > 0.7 {
        p[1] + (p[2] - p[1]) * (t - 0.7) / 0.3
    } else {
        p[1]
    }
}
impl Document {
    pub fn compile(&self) -> Result<StrokeList, String> {
        if self.format != "oil-author" || self.version != AUTHOR_VERSION {
            return Err("DOCUMENT_VERSION_MISMATCH".into());
        }
        if self.engine != oil_kernel::ENGINE_VERSION {
            return Err(format!(
                "ENGINE_VERSION_MISMATCH: file {}, running {}; use the matching engine",
                self.engine,
                oil_kernel::ENGINE_VERSION
            ));
        }
        self.catalog.validate()?;
        if self.aspect.contains(&0) || !self.ground.iter().all(|&v| unit(v)) {
            return Err("INVALID_CANVAS".into());
        }
        let mut list = StrokeList {
            meta: Meta {
                generator: "oil-author/1".into(),
                ..Meta::default()
            },
            aspect: self.aspect,
            ground: self.ground,
            layers: vec![],
            offsets: vec![0],
            points: vec![],
            strokes: vec![],
        };
        let mut groups = BTreeSet::new();
        let mut strokes = BTreeSet::new();
        for g in &self.groups {
            if g.id.is_empty() || g.name.is_empty() || !groups.insert(&g.id) || !unit(g.dry_after) {
                return Err("INVALID_GROUP".into());
            }
            let start = list.strokes.len() as u32;
            for s in &g.strokes {
                if s.id.is_empty()
                    || !strokes.insert(&s.id)
                    || s.path.len() < 2
                    || s.path.len() > 10000
                    || !s.color.iter().all(|&v| unit(v))
                    || !s
                        .path
                        .iter()
                        .all(|p| range(p[0], -2., 3.) && range(p[1], -2., 3.) && unit(p[2]))
                {
                    return Err(format!("INVALID_STROKE: {}", s.id));
                }
                let p = self
                    .catalog
                    .presets
                    .iter()
                    .find(|p| p.id == s.preset)
                    .ok_or_else(|| format!("UNKNOWN_PRESET: {}", s.preset))?;
                if !range(s.width, p.width[0], p.width[2]) {
                    return Err(format!(
                        "WIDTH_RANGE: {} requires {}..{} cw",
                        p.id, p.width[0], p.width[2]
                    ));
                }
                let mut b = BrushParams {
                    mode: p.mode,
                    seed: scoped_seed(self.seed, &[&g.id, &s.id, "bristles"]),
                    ..BrushParams::default()
                };
                apply(&mut b, &p.paint)?;
                let mut lengths = vec![0f32];
                for pair in s.path.windows(2) {
                    let dx = pair[1][0] - pair[0][0];
                    let dy = pair[1][1] - pair[0][1];
                    lengths.push(lengths.last().unwrap() + (dx * dx + dy * dy).sqrt());
                }
                let len = *lengths.last().unwrap();
                b.deplete = p.depletion / (2. * len / s.width).max(1.);
                apply(&mut b, &s.controls)?;
                if !g.visible {
                    continue;
                }
                // Arc-length resampling avoids pointer event density changing the brush envelope.
                let n = ((len / (s.width * 0.2)).ceil() as usize).clamp(16, 2048);
                let mut seg = 0;
                for i in 0..=n {
                    let t = i as f32 / n as f32;
                    let d = t * len;
                    while seg + 2 < lengths.len() && lengths[seg + 1] < d {
                        seg += 1;
                    }
                    let f = if lengths[seg + 1] > lengths[seg] {
                        (d - lengths[seg]) / (lengths[seg + 1] - lengths[seg])
                    } else {
                        0.
                    };
                    let a = s.path[seg];
                    let z = s.path[seg + 1];
                    list.points.push([
                        a[0] + (z[0] - a[0]) * f,
                        a[1] + (z[1] - a[1]) * f,
                        s.width * envelope(p.width_profile, t),
                        (a[2] + (z[2] - a[2]) * f) * envelope(p.pressure_profile, t),
                    ]);
                }
                list.offsets.push(list.points.len() as u32);
                list.strokes.push(Stroke {
                    layer: list.layers.len() as u32,
                    region: NO_REGION,
                    color: s.color,
                    color2: s.color,
                    streak_amount: 0.75,
                    brush: b,
                });
            }
            list.meta.layers.push(g.name.clone());
            list.layers.push(Layer {
                start,
                end: list.strokes.len() as u32,
                hblur_sigma: Some(0.002),
                dry_after: Some(if g.visible { g.dry_after } else { 1. }),
            });
        }
        let errors = list.validate();
        if !errors.is_empty() {
            return Err(format!("INVALID_DOCUMENT: {errors:?}"));
        }
        Ok(list)
    }
}
fn copy_canvas<M: Mixer>(c: &Canvas<M>) -> Canvas<M> {
    Canvas {
        w: c.w,
        h: c.h,
        lat: c.lat.clone(),
        rgb: c.rgb.clone(),
        hgt: c.hgt.clone(),
        wet: c.wet.clone(),
        cover: c.cover.clone(),
        hblur: c.hblur.clone(),
    }
}
fn same_layer(a: &StrokeList, b: &StrokeList, i: usize) -> bool {
    let (x, y) = (&a.layers[i], &b.layers[i]);
    x.hblur_sigma == y.hblur_sigma
        && x.dry_after == y.dry_after
        && x.end - x.start == y.end - y.start
        && (x.start..x.end).zip(y.start..y.end).all(|(j, k)| {
            a.strokes[j as usize] == b.strokes[k as usize]
                && a.stroke_points(j as usize) == b.stroke_points(k as usize)
        })
}
/// At most two full-plane checkpoints, within a 64 MiB checkpoint budget. Replays every downstream group.
pub struct Replay<M: Mixer> {
    pub canvas: Option<Canvas<M>>,
    list: Option<StrokeList>,
    checkpoints: Vec<(usize, Canvas<M>)>,
}
impl<M: Mixer> Default for Replay<M> {
    fn default() -> Self {
        Self {
            canvas: None,
            list: None,
            checkpoints: vec![],
        }
    }
}
impl<M: Mixer> Replay<M> {
    pub fn render(&mut self, m: &M, list: StrokeList, w: u32) -> Result<usize, String> {
        let h = list.height_for(w);
        if w == 0
            || h == 0
            || w as u64 * h as u64 * Canvas::<M>::bytes_per_pixel() as u64 > 192 * 1024 * 1024
        {
            return Err("CANVAS_TOO_LARGE: author preview budget is 192 MiB; use native oil paint for large exports".into());
        }
        let mut common = 0;
        if let (Some(old), Some(cv)) = (&self.list, &self.canvas) {
            if cv.w == w as usize && old.aspect == list.aspect && old.ground == list.ground {
                while common < old.layers.len().min(list.layers.len())
                    && same_layer(old, &list, common)
                {
                    common += 1;
                }
                if common == old.layers.len() && common == list.layers.len() {
                    self.list = Some(list);
                    return Ok(common);
                }
            }
        }
        self.checkpoints.retain(|(n, _)| *n <= common);
        let (start, mut cv) = if let Some((n, c)) = self.checkpoints.last() {
            (*n, copy_canvas(c))
        } else {
            (0, Canvas::new(m, w as usize, h as usize, list.ground))
        };
        let limit =
            (64 * 1024 * 1024 / (w as usize * h as usize * Canvas::<M>::bytes_per_pixel())).min(2);
        oil_paint::paint_layers(m, &list, &mut cv, start, list.layers.len(), |i, c| {
            if limit > 0 {
                self.checkpoints.push((i + 1, copy_canvas(c)));
                while self.checkpoints.len() > limit {
                    self.checkpoints.remove(0);
                }
            }
        });
        self.canvas = Some(cv);
        self.list = Some(list);
        Ok(start)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct View {
    pub mode: String,
    pub direction: [f32; 3],
    pub bump: f32,
    pub contrast: f32,
    pub specular: f32,
}
pub fn image<M: Mixer>(c: &Canvas<M>, v: &View) -> Result<Vec<u8>, String> {
    if !["lit", "unlit", "height"].contains(&v.mode.as_str())
        || !v.direction.iter().all(|&x| range(x, -1., 1.))
        || v.direction.iter().map(|x| x * x).sum::<f32>() < 0.01
        || !range(v.bump, 0., 3.)
        || !unit(v.contrast)
        || !unit(v.specular)
    {
        return Err("INVALID_VIEW".into());
    }
    let rgb = match v.mode.as_str() {
        "unlit" => c.rgb.clone(),
        "height" => c.hgt.iter().map(|h| [(h / 3.).clamp(0., 1.); 3]).collect(),
        _ => oil_light::relight(
            &c.rgb,
            &c.hgt,
            c.w,
            c.h,
            &oil_light::LightParams {
                light_dir: v.direction,
                bump: v.bump,
                bump_fine: v.bump,
                contrast: v.contrast,
                spec: v.specular,
                cavity: 0.015,
                tint: 0.02,
                weave_amp: 0.06,
                ..oil_light::LightParams::painting()
            },
        ),
    };
    Ok(rgb
        .iter()
        .flatten()
        .map(|v| (v.clamp(0., 1.) * 255. + 0.5) as u8)
        .collect())
}

pub mod session;
#[cfg(test)]
mod tests {
    use super::*;
    use oil_mix::{OchrellMixer, RgbMixer};
    fn doc() -> Document {
        let mark = |id: &str, preset: &str, y: f32| Mark {
            id: id.into(),
            preset: preset.into(),
            width: 0.03,
            color: [0.8, 0.2, 0.1],
            path: vec![[0.1, y, 1.], [0.9, y, 0.7]],
            controls: BTreeMap::new(),
        };
        Document {
            format: "oil-author".into(),
            version: 1,
            engine: oil_kernel::ENGINE_VERSION.into(),
            seed: 7,
            aspect: [3, 2],
            ground: [0.6; 3],
            catalog: catalog(),
            groups: (0..4)
                .map(|i| Group {
                    id: format!("g{i}"),
                    name: format!("Group {i}"),
                    visible: true,
                    dry_after: 0.8,
                    strokes: vec![mark(
                        &format!("s{i}"),
                        if i == 3 { "wet-mixing" } else { "loaded-flat" },
                        0.3 + i as f32 * 0.008,
                    )],
                })
                .collect(),
        }
    }
    #[test]
    fn schema_matches_source() {
        let expected: serde_json::Value =
            serde_json::from_str(include_str!("../../../spec/author-1.schema.json")).unwrap();
        assert_eq!(
            serde_json::to_value(schemars::schema_for!(Document)).unwrap(),
            expected
        );
    }
    #[test]
    fn calibrated_controls_change_coverage_relief_and_wet_pickup() {
        let make = |preset: &str| {
            let mut d = doc();
            d.groups.truncate(1);
            d.groups[0].strokes[0].preset = preset.into();
            d.groups[0].strokes[0].width = 0.06;
            oil_paint::paint(&RgbMixer, &d.compile().unwrap(), 256, |_, _| {}).0
        };
        let loaded = make("loaded-flat");
        let dry = make("dry-drag");
        let impasto = make("impasto-accent");
        assert!(loaded.cover.iter().sum::<f32>() > dry.cover.iter().sum::<f32>() * 2.);
        assert!(impasto.hgt.iter().sum::<f32>() > loaded.hgt.iter().sum::<f32>());
        let mut d = doc();
        d.groups.truncate(2);
        d.groups[0].strokes[0].color = [0.05, 0.2, 0.8];
        d.groups[1].strokes[0].preset = "wet-mixing".into();
        d.groups[0].dry_after = 1.;
        let wet = oil_paint::paint(&RgbMixer, &d.compile().unwrap(), 256, |_, _| {}).0;
        d.groups[0].dry_after = 0.;
        let dry = oil_paint::paint(&RgbMixer, &d.compile().unwrap(), 256, |_, _| {}).0;
        let difference: f32 = wet
            .rgb
            .iter()
            .flatten()
            .zip(dry.rgb.iter().flatten())
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(
            difference > 0.1,
            "wet pickup must respond to substrate drying: {difference}"
        );
    }
    #[test]
    fn catalog_and_strict_serialization() {
        let d = doc();
        assert_eq!(
            serde_json::from_str::<Document>(&serde_json::to_string(&d).unwrap()).unwrap(),
            d
        );
        for p in &d.catalog.presets {
            let mut d = d.clone();
            d.groups[0].strokes[0].preset = p.id.clone();
            d.groups[0].strokes[0].width = p.width[1];
            assert!(d.compile().is_ok());
        }
        let mut bad = d.clone();
        bad.version = 2;
        assert!(bad.compile().unwrap_err().contains("VERSION"));
        bad = d.clone();
        bad.engine = "2.0.0-dev.2".into();
        assert!(bad.compile().unwrap_err().contains("ENGINE_VERSION"));
        bad = d.clone();
        bad.groups[1].strokes[0].id = "s0".into();
        assert!(bad.compile().is_err());
        bad = d.clone();
        bad.catalog.presets[0].paint.insert("typo".into(), 0.);
        assert!(bad.compile().is_err());
        bad = d.clone();
        bad.catalog.presets[0].paint.insert("nb".into(), 2.5);
        assert!(bad.compile().is_err());
        bad = d.clone();
        bad.groups[0].strokes[0]
            .controls
            .insert("pickup".into(), f32::NAN);
        assert!(bad.compile().is_err());
        bad = d.clone();
        bad.groups[0].visible = false;
        bad.groups[0].strokes[0].width = -1.;
        assert!(bad.compile().is_err());
        let mut v = serde_json::to_value(d).unwrap();
        v["extra"] = 1.into();
        assert!(serde_json::from_value::<Document>(v).is_err());
    }
    #[test]
    fn scoped_edits_preserve_authored_and_compiled_strokes() {
        let d = doc();
        let a = d.compile().unwrap();
        let mut e = d.clone();
        e.groups[0].strokes[0].color = [0., 0., 1.];
        let extra = Mark {
            id: "extra".into(),
            ..e.groups[0].strokes[0].clone()
        };
        e.groups[0].strokes.push(extra);
        let b = e.compile().unwrap();
        assert_eq!(&d.groups[1..], &e.groups[1..]);
        for i in 1..4 {
            assert_eq!(a.strokes[i], b.strokes[i + 1]);
            assert_eq!(a.stroke_points(i), b.stroke_points(i + 1));
        }
        assert_eq!(
            scoped_seed(7, &["g", "s", "p"]),
            scoped_seed(7, &["g", "s", "p"])
        );
        assert_ne!(scoped_seed(7, &["a", "bc"]), scoped_seed(7, &["ab", "c"]));
        assert_eq!(a, StrokeList::from_bytes(&a.to_bytes()).unwrap());
    }
    fn replay<M: Mixer>(m: &M) {
        let base = doc();
        let mut replay = Replay::default();
        replay.render(m, base.compile().unwrap(), 96).unwrap();
        for which in 0..9 {
            let mut d = base.clone();
            match which {
                0 => d.groups[3].strokes[0].color = [0.1, 0.2, 0.8],
                1 => d.groups[0].strokes[0].color = [0.1, 0.2, 0.8],
                2 => d.groups[1].visible = false,
                3 => d.groups.swap(0, 3),
                4 => d.groups[0].dry_after = 0.,
                5 => d.ground = [0.2; 3],
                6 => d.groups[2].strokes[0].path[0][0] = 0.4,
                7 => {
                    d.groups.remove(1);
                }
                _ => d.groups[0].strokes[0].preset = "scumble".into(),
            }
            let list = d.compile().unwrap();
            let reused = replay.render(m, list.clone(), 96).unwrap();
            if which == 0 {
                assert!(reused > 0);
            }
            let (full, _) = oil_paint::paint(m, &list, 96, |_, _| {});
            let actual = replay.canvas.as_ref().unwrap();
            for p in oil_paint::PLANES {
                assert_eq!(
                    oil_paint::plane_bytes(actual, p),
                    oil_paint::plane_bytes(&full, p),
                    "edit {which}, plane {p}"
                );
            }
            assert_eq!(actual.hblur, full.hblur);
        }
        let list = base.compile().unwrap();
        replay.render(m, list.clone(), 128).unwrap();
        let (full, _) = oil_paint::paint(m, &list, 128, |_, _| {});
        assert_eq!(replay.canvas.as_ref().unwrap().rgb, full.rgb);
    }
    #[test]
    fn incremental_equals_full_rgb() {
        replay(&RgbMixer);
    }
    #[test]
    fn incremental_equals_full_ochrell() {
        replay(&OchrellMixer);
    }
    #[test]
    fn swatches_width_resolution_and_repeat() {
        for p in catalog().presets {
            for w in [p.width[0].max(0.004), p.width[1], p.width[2]] {
                for resolution in [256, 512] {
                    let mut d = doc();
                    d.groups.truncate(1);
                    let s = &mut d.groups[0].strokes[0];
                    s.preset = p.id.clone();
                    s.width = w;
                    let list = d.compile().unwrap();
                    let (a, _) = oil_paint::paint(&RgbMixer, &list, resolution, |_, _| {});
                    let (b, _) = oil_paint::paint(&RgbMixer, &list, resolution, |_, _| {});
                    assert_eq!(a.rgb, b.rgb);
                    assert!(a.hgt.iter().all(|v| v.is_finite()));
                    assert!(
                        a.cover.iter().any(|&v| v > 0.),
                        "{} width {w} resolution {resolution}",
                        p.id
                    );
                }
            }
        }
    }
}
