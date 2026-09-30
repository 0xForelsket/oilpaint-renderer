//! The planner: a ScenePlan, plan options and a mixer become a StrokeList (plan sections 3 and 11, L3).
//!
//! `plan` is the composed call: compile the guides, plan every layer on a proxy canvas, assemble and validate the
//! StrokeList, and report what happened (per layer and region, coverage, timings, warnings). The stages are public
//! for users who write their own loop: `Planner::plan_layer`, the site proposers, `color::Palette`, `rng::Rng`.
#![forbid(unsafe_code)]
// Structured errors are large (136 bytes) but only travel on cold paths.
#![allow(clippy::result_large_err)]

pub mod boundary;
pub mod color;
pub mod planner;
pub mod rng;
pub mod style;

pub use planner::{LayerReport, Planner, RegionReport};

use oil_errors::Error;
use oil_mix::Mixer;
use oil_scene::{FieldData, ScenePlan};
use oil_strokes::{Inputs, Meta, PlanMeta, StrokeList};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Plan options (not part of the ScenePlan).
#[derive(Clone, Copy, Debug)]
pub struct PlanOptions {
    pub seed: u32,
    /// Width of the guides and the proxy canvas, in pixels.
    pub plan_width: u32,
    /// Refuse a plan authored for another engine version instead of warning.
    pub strict_engine: bool,
}

impl Default for PlanOptions {
    fn default() -> Self {
        PlanOptions { seed: 1907, plan_width: 600, strict_engine: false }
    }
}

/// Coverage of one region after planning.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionCoverage {
    pub region: String,
    /// Share of the region's soft-mask area painted at all (proxy cover > 0.05).
    pub covered: f64,
    pub strokes: usize,
    /// Drawn stroke widths (cw) for this region, min and max: the style's range, scaled by sizeByY, and 0.8x for gap
    /// fill.
    pub width: [f64; 2],
    /// Strokes whose path stopped after one step (two points: a tapered stub).
    pub stubs: usize,
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanTimings {
    pub guides_ms: f64,
    pub plan_ms: f64,
    pub total_ms: f64,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanReport {
    pub engine: &'static str,
    pub mixer: &'static str,
    pub size: [usize; 2],
    pub seed: u32,
    pub strokes: usize,
    pub points: usize,
    pub layers: Vec<LayerReport>,
    pub regions: Vec<RegionCoverage>,
    /// Share of the canvas never painted (proxy cover < 0.05).
    pub bare: f64,
    pub timings: PlanTimings,
    pub warnings: Vec<Error>,
}

fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// Plan `plan` with mixer `m`. `fields` supplies the sampled fields the plan declares; `clock` returns
/// milliseconds (for timings only).
pub fn plan<M: Mixer>(
    m: &M,
    plan: &ScenePlan,
    opts: &PlanOptions,
    fields: &BTreeMap<String, FieldData>,
    clock: &dyn Fn() -> f64,
) -> Result<(StrokeList, PlanReport), Vec<Error>> {
    let t0 = clock();
    let report = oil_scene::validate(plan);
    if !report.errors.is_empty() {
        return Err(report.errors);
    }
    let mut warnings = report.warnings;
    if opts.strict_engine {
        if let Some(i) = warnings.iter().position(|w| w.code == "ENGINE_VERSION_DIFFERS") {
            return Err(vec![warnings.remove(i)]);
        }
    }
    if !plan.fields.is_empty() {
        warnings.push(
            Error::new("NON_PORTABLE_PLAN", "the plan uses sampled fields, so another JS engine may plan it differently (painting is unaffected)")
                .path("/fields"),
        );
    }
    let (guides, _) = oil_scene::compile(plan, opts.plan_width, m, fields, clock)?;
    let t1 = clock();
    let mut p = Planner::new(m, plan, guides, opts.seed);
    for (li, layer) in plan.layers.iter().enumerate() {
        p.plan_layer(li, layer, clock);
    }
    let t2 = clock();

    // coverage per region and bare canvas
    let (w, h) = (p.g.w, p.g.h);
    let bare = p.cv.cover.iter().filter(|c| **c < 0.05).count() as f64 / (w * h) as f64;
    let mut counts = vec![0usize; p.g.names.len()];
    for s in &p.strokes {
        counts[s.region as usize] += 1;
    }
    let regions = p
        .g
        .names
        .iter()
        .enumerate()
        .map(|(r, name)| {
            let mk = &p.g.masks[r];
            let (mut area, mut painted) = (0.0f64, 0.0f64);
            for j in mk.y0..mk.y0 + mk.h {
                for i in mk.x0..mk.x0 + mk.w {
                    let v = mk.at(i, j) as f64;
                    area += v;
                    if p.cv.cover[j * w + i] > 0.05 {
                        painted += v;
                    }
                }
            }
            let n = counts[r];
            let (wr, stubs) = p.drawn[r];
            RegionCoverage { region: name.clone(), covered: if area > 0.0 { painted / area } else { 1.0 }, strokes: n, width: if n > 0 { wr } else { [0.0, 0.0] }, stubs }
        })
        .collect();

    let plan_json = serde_json::to_vec(plan).unwrap_or_default();
    let meta = Meta {
        generator: format!("oilpaint planner (engine {})", oil_kernel::ENGINE_VERSION),
        title: plan.title.clone(),
        plan: Some(PlanMeta { mixer: M::ID.to_string(), plan_width: opts.plan_width, seed: opts.seed, portable: plan.fields.is_empty() }),
        inputs: Inputs {
            sceneplan: Some(hex(&Sha256::digest(&plan_json))),
            fields: plan.fields.iter().map(|(k, f)| (k.clone(), f.sha256.clone())).collect(),
            hooks: Vec::new(),
        },
        layers: plan.layers.iter().map(|l| l.name.clone()).collect(),
        regions: p.g.names.clone(),
    };
    let list = StrokeList {
        meta,
        aspect: plan.canvas.aspect,
        ground: p.g.ground,
        layers: p.layers.clone(),
        offsets: p.offsets.clone(),
        points: p.points.clone(),
        strokes: p.strokes.clone(),
    };
    let errs = list.validate();
    if !errs.is_empty() {
        return Err(errs);
    }
    let rep = PlanReport {
        engine: oil_kernel::ENGINE_VERSION,
        mixer: M::ID,
        size: [w, h],
        seed: opts.seed,
        strokes: list.strokes.len(),
        points: list.points.len(),
        layers: p.report.clone(),
        regions,
        bare,
        timings: PlanTimings { guides_ms: t1 - t0, plan_ms: t2 - t1, total_ms: t2 - t0 },
        warnings,
    };
    Ok((list, rep))
}
