//! Frozen planned marks, grouped for precise local editing. This format never reapplies a brush profile.
use oil_kernel::BrushParams;
use oil_strokes::{Layer, Meta, Stroke, StrokeList, NO_REGION};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(remote = "BrushParams", rename_all = "camelCase", deny_unknown_fields)]
pub struct BrushRecord {
    pub mode: i32,
    pub opacity: f32,
    pub pickup: f32,
    pub load: f32,
    pub deplete: f32,
    pub vdry: f32,
    pub hgain: f32,
    pub flatten: f32,
    pub streak: f32,
    pub hardness: f32,
    pub grain: f32,
    pub dry_thresh: f32,
    pub dry_width: f32,
    pub nb: i32,
    pub seed: u32,
    pub dropout: f32,
    pub ragged: f32,
    pub body: f32,
    pub release: f32,
    pub streak_mix: f32,
    pub ridge: f32,
    pub levee: f32,
    pub furrow: f32,
    pub blob: f32,
    pub stiff: f32,
    pub marble: f32,
    pub splay: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolvedStroke {
    pub id: String,
    pub points: Vec<[f32; 4]>,
    pub color: [f32; 3],
    pub color2: [f32; 3],
    pub streak_amount: f32,
    #[serde(with = "BrushRecord")]
    #[schemars(with = "BrushRecord")]
    pub brush: BrushParams,
    pub preset: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlannedGroup {
    pub id: String,
    pub name: String,
    pub region: Option<String>,
    pub pass: String,
    pub visible: bool,
    pub hblur_sigma: Option<f32>,
    pub dry_after: Option<f32>,
    pub strokes: Vec<ResolvedStroke>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Composition {
    pub format: String,
    pub version: u32,
    pub engine: String,
    pub title: Option<String>,
    pub source_hash: Option<String>,
    pub aspect: [u32; 2],
    pub ground: [f32; 3],
    pub groups: Vec<PlannedGroup>,
}
impl Composition {
    pub fn compile(&self) -> Result<StrokeList, String> {
        if self.format != "oil-composition" || self.version != 1 {
            return Err("COMPOSITION_VERSION_MISMATCH".into());
        }
        if self.engine != oil_kernel::ENGINE_VERSION {
            return Err(format!(
                "ENGINE_VERSION_MISMATCH: file {}, running {}",
                self.engine,
                oil_kernel::ENGINE_VERSION
            ));
        }
        let regions: Vec<String> = self
            .groups
            .iter()
            .filter_map(|g| g.region.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut list = StrokeList {
            meta: Meta {
                generator: "oil-composition/1".into(),
                title: self.title.clone(),
                regions: regions.clone(),
                ..Meta::default()
            },
            aspect: self.aspect,
            ground: self.ground,
            layers: vec![],
            offsets: vec![0],
            points: vec![],
            strokes: vec![],
        };
        let (mut gids, mut ids) = (BTreeSet::new(), BTreeSet::new());
        for g in &self.groups {
            if g.id.is_empty() || g.name.is_empty() || !gids.insert(&g.id) {
                return Err("INVALID_GROUP_ID".into());
            }
            let start = list.strokes.len() as u32;
            for s in &g.strokes {
                if s.id.is_empty() || !ids.insert(&s.id) || s.points.len() < 2 {
                    return Err("INVALID_STROKE_ID_OR_POINTS".into());
                }
                // Hidden groups are validated too: no malformed state may be saved behind visibility.
                list.points.extend_from_slice(&s.points);
                list.offsets.push(list.points.len() as u32);
                list.strokes.push(Stroke {
                    layer: list.layers.len() as u32,
                    region: g
                        .region
                        .as_ref()
                        .and_then(|r| regions.iter().position(|n| n == r))
                        .map_or(NO_REGION, |r| r as u32),
                    color: s.color,
                    color2: s.color2,
                    streak_amount: s.streak_amount,
                    brush: s.brush,
                });
            }
            list.layers.push(Layer {
                start,
                end: list.strokes.len() as u32,
                hblur_sigma: g.hblur_sigma,
                dry_after: g.dry_after,
            });
            list.meta.layers.push(g.name.clone());
        }
        let errors = list.validate();
        if !errors.is_empty() {
            return Err(format!("INVALID_COMPOSITION: {errors:?}"));
        }
        if self.groups.iter().all(|g| g.visible) {
            return Ok(list);
        }
        let mut visible = self.clone();
        for g in &mut visible.groups {
            if !g.visible {
                g.strokes.clear();
                g.visible = true;
            }
        }
        visible.compile()
    }
}

/// Convert a planned list using the planner's per-layer ordered region partitions. Empty groups stay addressable.
pub fn from_plan(
    list: &StrokeList,
    partitions: &[Vec<(String, usize, Option<String>)>],
) -> Composition {
    let mut groups = Vec::new();
    for (li, l) in list.layers.iter().enumerate() {
        let pass = &list.meta.layers[li];
        let mut start = l.start as usize;
        let empty = vec![(String::new(), 0, None)];
        let parts = if partitions[li].is_empty() {
            &empty
        } else {
            &partitions[li]
        };
        for (gi, (region, count, preset)) in parts.iter().enumerate() {
            let id = format!("{}:{}|{}:{}", pass.len(), pass, region.len(), region);
            let strokes = (start..start + count)
                .enumerate()
                .map(|(ordinal, i)| {
                    let s = &list.strokes[i];
                    ResolvedStroke {
                        id: format!("{id}#{ordinal}"),
                        points: list.stroke_points(i).to_vec(),
                        color: s.color,
                        color2: s.color2,
                        streak_amount: s.streak_amount,
                        brush: s.brush,
                        preset: preset.clone(),
                    }
                })
                .collect();
            groups.push(PlannedGroup {
                id,
                name: if region.is_empty() {
                    pass.clone()
                } else {
                    format!("{pass} / {region}")
                },
                region: if region.is_empty() {
                    None
                } else {
                    Some(region.clone())
                },
                pass: pass.clone(),
                visible: true,
                hblur_sigma: if gi == 0 { l.hblur_sigma } else { None },
                dry_after: if gi + 1 == parts.len() {
                    l.dry_after
                } else {
                    None
                },
                strokes,
            });
            start += count;
        }
        assert_eq!(
            start, l.end as usize,
            "planner partitions must cover the layer"
        );
    }
    Composition {
        format: "oil-composition".into(),
        version: 1,
        engine: oil_kernel::ENGINE_VERSION.into(),
        title: list.meta.title.clone(),
        source_hash: list.meta.inputs.sceneplan.clone(),
        aspect: list.aspect,
        ground: list.ground,
        groups,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn schema_matches_source() {
        let expected: serde_json::Value =
            serde_json::from_str(include_str!("../../../spec/composition-1.schema.json")).unwrap();
        assert_eq!(
            serde_json::to_value(schemars::schema_for!(super::Composition)).unwrap(),
            expected
        );
    }
}
