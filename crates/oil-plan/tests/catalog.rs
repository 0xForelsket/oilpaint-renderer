use oil_author::Replay;
use oil_mix::{Mixer, OchrellMixer, RgbMixer};
use oil_plan::{PlanOptions, Planner};
use std::collections::BTreeMap;
fn scene() -> oil_scene::ScenePlan {
    oil_scene::parse(r##"{"sceneplan":1,"canvas":{"aspect":[3,2],"ground":"#c8c0b0"},"target":[{"fill":{"color":"#698547"}}],"regions":[{"name":"leaf","shape":{"ellipse":{"c":[0.25,0.3],"r":[0.14,0.18]}},"edge":0.015,"flow":{"constant":{"angle":-60}}},{"name":"orange","shape":{"ellipse":{"c":[0.65,0.3],"r":[0.17,0.16]}},"flow":{"constant":{"angle":20}}}],"styles":{"leaf":{"brushPreset":"loaded-flat","width":[0.025,0.04],"length":[0.05,0.10],"edgeFade":0.7,"edgeInset":0.3,"pressure":[0.7,1]},"orange":{"brushPreset":"rounded-dab","width":[0.03,0.05],"length":[0.05,0.12]}},"layers":[{"name":"Body","regions":["leaf","orange"],"placement":"density","spacing":0.65,"dryAfter":0.8},{"name":"Leaf detail","regions":["leaf"],"brushPreset":"fine-detail","width":[0.004,0.006],"length":[0.01,0.03],"placement":"density","spacing":3,"maxStrokes":12},{"name":"Stone-like catch","regions":["orange"],"brushPreset":"scumble","width":[0.02,0.03],"placement":"density","spacing":2,"maxStrokes":8}]}"##).unwrap()
}
fn equal<M: Mixer>(a: &oil_kernel::Canvas<M>, b: &oil_kernel::Canvas<M>) {
    for p in oil_paint::PLANES {
        assert_eq!(
            oil_paint::plane_bytes(a, p),
            oil_paint::plane_bytes(b, p),
            "{p}"
        );
    }
    assert_eq!(a.hblur, b.hblur);
}
fn check<M: Mixer>(m: M) {
    let sp = scene();
    let opts = PlanOptions {
        seed: 41,
        plan_width: 96,
        strict_engine: false,
    };
    let (list, report) = oil_plan::plan(&m, &sp, &opts, &BTreeMap::new(), &|| 0.).unwrap();
    let d = oil_plan::composition(&list, &report);
    assert!(!d.groups.is_empty());
    assert!(d.groups.iter().any(|g| g.region.as_deref() == Some("leaf")));
    let (full, _) = oil_paint::paint(&m, &list, 96, |_, _| {});
    let (resolved, _) = oil_paint::paint(&m, &d.compile().unwrap(), 96, |_, _| {});
    equal(&full, &resolved);
    let (g, _) = oil_scene::compile(&sp, 96, &m, &BTreeMap::new(), &|| 0.).unwrap();
    let mut p = Planner::new(&m, &sp, g, 41);
    for (i, l) in sp.layers.iter().enumerate() {
        p.plan_layer(i, l, &|| 0.);
    }
    equal(&p.cv, &full);
    let encoded = serde_json::to_string(&d).unwrap();
    let restored: oil_author::Composition = serde_json::from_str(&encoded).unwrap();
    assert_eq!(d, restored);
    let mut changed = d.clone();
    for group in &mut changed.groups {
        if group.region.as_deref() == Some("leaf") {
            for s in &mut group.strokes {
                s.color = [0.6, 0.55, 0.18];
            }
        }
    }
    for (a, b) in d.groups.iter().zip(&changed.groups) {
        if a.region.as_deref() != Some("leaf") {
            assert_eq!(a, b);
        }
    }
    let mut cached = Replay::default();
    cached.render(&m, d.compile().unwrap(), 96).unwrap();
    cached.render(&m, changed.compile().unwrap(), 96).unwrap();
    let (fresh, _) = oil_paint::paint(&m, &changed.compile().unwrap(), 96, |_, _| {});
    equal(cached.canvas.as_ref().unwrap(), &fresh);
    // Changing only the final group must exercise a nonzero checkpoint prefix.
    let mut late = changed.clone();
    late.groups.last_mut().unwrap().visible = false;
    assert!(cached.render(&m, late.compile().unwrap(), 96).unwrap() > 0);
    let (fresh, _) = oil_paint::paint(&m, &late.compile().unwrap(), 96, |_, _| {});
    equal(cached.canvas.as_ref().unwrap(), &fresh);
}
#[test]
fn brush_plans_and_local_edits_replay_rgb() {
    check(RgbMixer);
}
#[test]
fn brush_plans_and_local_edits_replay_ochrell() {
    check(OchrellMixer);
}
#[test]
fn brush_resolution_precedence_and_validation() {
    let mut sp = scene();
    assert!(oil_scene::validate(&sp).errors.is_empty());
    let catalog = oil_brush::catalog();
    let st = oil_plan::style::resolve_catalog(
        &RgbMixer,
        None,
        sp.styles.get("leaf"),
        &sp.layers[1],
        &catalog,
    );
    assert_eq!(st.brush.unwrap().id, "fine-detail");
    assert_eq!(st.nb_base, 7.);
    assert_eq!(st.width, [0.004, 0.006]);
    sp.layers[0].brush_preset = Some("unknown".into());
    assert!(oil_scene::validate(&sp)
        .errors
        .iter()
        .any(|e| e.code == "UNKNOWN_BRUSH_PRESET"));
    sp.layers[0].brush_preset = Some("fine-detail".into());
    assert!(oil_scene::validate(&sp)
        .errors
        .iter()
        .any(|e| e.code == "BRUSH_WIDTH_RANGE"));
    sp.layers[0].brush_preset = None;
    sp.styles.get_mut("leaf").unwrap().edge_fade = Some(2.);
    assert!(oil_scene::validate(&sp)
        .errors
        .iter()
        .any(|e| e.code == "RANGE"));
}
#[test]
fn inserting_a_disjoint_region_does_not_reroll_named_scope() {
    let mut sp = scene();
    sp.layers.truncate(1);
    sp.layers[0].placement = oil_scene::spec::Placement::Density;
    let opts = PlanOptions {
        seed: 41,
        plan_width: 96,
        strict_engine: false,
    };
    let (a, r) = oil_plan::plan(&RgbMixer, &sp, &opts, &BTreeMap::new(), &|| 0.).unwrap();
    let a = oil_plan::composition(&a, &r);
    let extra: oil_scene::spec::Region = serde_json::from_value(
        serde_json::json!({"name":"extra","shape":{"ellipse":{"c":[0.93,0.58],"r":[0.01,0.01]}}}),
    )
    .unwrap();
    sp.regions.insert(0, extra);
    let (b, r) = oil_plan::plan(&RgbMixer, &sp, &opts, &BTreeMap::new(), &|| 0.).unwrap();
    let b = oil_plan::composition(&b, &r);
    assert_eq!(a.groups, b.groups);
}
