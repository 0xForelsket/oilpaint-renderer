//! A plan replayed at its plan width reproduces the planner's proxy canvas bit for bit, and planning is repeatable.
use oil_mix::{Mixer, RgbMixer};
use oil_plan::{plan, PlanOptions, Planner};
use std::collections::BTreeMap;

fn scene() -> oil_scene::ScenePlan {
    let json = r##"{
      "sceneplan": 1, "preset": "impressionist",
      "canvas": {"aspect": [4, 5], "ground": "#e9e1d6"},
      "target": [
        {"fill": {"gradientV": [[0, "#2c3266"], [0.6, "#e8cbb8"], [1.25, "#3a67a0"]]}},
        {"blob": {"c": [0.5, 0.55], "r": [0.2, 0.12], "color": "#c8503f", "noise": 0.4}},
        {"polygon": {"points": [[0.2, 0.9], [0.8, 0.85], [0.7, 1.1], [0.3, 1.15]], "color": "#4a4a7c"}}
      ],
      "regions": [
        {"name": "all", "shape": {"all": true}, "flow": {"sweep": {"angle": -10}}},
        {"name": "blob", "shape": {"ellipse": {"c": [0.5, 0.55], "r": [0.2, 0.12]}}, "edge": 0.03, "flow": {"swirlAround": {"centres": [[0.5, 0.55, 0.2, 0.12]]}}},
        {"name": "rock", "shape": {"polygon": [[0.2, 0.9], [0.8, 0.85], [0.7, 1.1], [0.3, 1.15]]}, "flow": {"contour": {"polygon": [[0.2, 0.9], [0.8, 0.85], [0.7, 1.1], [0.3, 1.15]]}}}
      ],
      "styles": {
        "all": {"width": [0.04, 0.06], "length": [0.1, 0.2]},
        "blob": {"colors": ["#c8503f", "#e8a08f", "cadmium_red"], "width": [0.02, 0.035], "marble": 0.3, "flecks": [["#3a67a0", 0.1]]},
        "rock": {"width": [0.02, 0.03], "length": [0.03, 0.06], "warmth": 0.4}
      },
      "lights": [{"lamp": {"c": [0.5, 0.5], "r": 0.3}}],
      "layers": [
        {"name": "block in", "regions": "all", "errorThreshold": 12, "dryAfter": 0.5},
        {"name": "scumble", "regions": ["blob"], "placement": "density", "mode": "scumble", "spacing": 1.5, "opacity": [0.3, 0.5]},
        {"name": "details", "regions": ["blob", "rock"], "errorThreshold": 8, "referenceBlur": 0.3},
        {"name": "edges", "regions": ["rock"], "placement": "curve", "curveSpacing": 1.0}
      ]
    }"##;
    oil_scene::load(json).expect("valid").0
}

#[test]
fn replay_reproduces_the_proxy_and_planning_repeats() {
    let m = RgbMixer;
    let sp = scene();
    let opts = PlanOptions { seed: 11, plan_width: 120, strict_engine: false };
    let clock = || 0.0;
    let (list, rep) = plan(&m, &sp, &opts, &BTreeMap::new(), &clock).expect("plans");
    assert!(rep.strokes > 50, "{} strokes", rep.strokes);
    assert!(list.layers.iter().all(|l| l.end >= l.start));
    assert!(list.layers[1].hblur_sigma.is_some(), "the scumble layer records its height blur");

    // the planner's own proxy, planned again through the stages
    let (guides, _) = oil_scene::compile(&sp, 120, &m, &BTreeMap::new(), &clock).unwrap();
    let mut p = Planner::new(&m, &sp, guides, 11);
    for (li, l) in sp.layers.iter().enumerate() {
        p.plan_layer(li, l, &clock);
    }
    let (cv, _) = oil_paint::paint(&m, &list, 120, |_, _| {});
    let same = |a: &[f32], b: &[f32]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits());
    let lat = |c: &oil_kernel::Canvas<RgbMixer>| c.lat.iter().flat_map(|z| z.as_slice().to_vec()).collect::<Vec<f32>>();
    assert!(same(&lat(&cv), &lat(&p.cv)), "state plane differs from the proxy");
    assert!(same(&cv.hgt, &p.cv.hgt) && same(&cv.wet, &p.cv.wet) && same(&cv.cover, &p.cv.cover), "height, wetness or cover differs");

    let (again, _) = plan(&m, &sp, &opts, &BTreeMap::new(), &clock).unwrap();
    assert_eq!(list.to_bytes(), again.to_bytes(), "planning is not repeatable");
    let other = plan(&m, &sp, &PlanOptions { seed: 12, ..opts }, &BTreeMap::new(), &clock).unwrap().0;
    assert_ne!(list.to_bytes(), other.to_bytes());
    let _ = RgbMixer::ID;
}
