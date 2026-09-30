//! Resolved styles: the engine's neutral defaults, then the scene's preset, then the region's style, then the
//! layer's style keys (spec/SCENEPLAN_V1.md, "Styles"). Colours are resolved to sRGB once, with the active mixer.
use oil_mix::Mixer;
use oil_scene::spec::{ColorSpec, Layer, Mode, Style};

/// Every style key with a concrete value.
#[derive(Clone, Debug, PartialEq)]
pub struct St {
    pub colors: Vec<[f32; 3]>,
    pub flecks: Vec<([f32; 3], f64)>,
    pub width: [f64; 2],
    pub length: [f64; 2],
    pub curvature: f64,
    pub align: f64,
    pub opacity: [f64; 2],
    pub pickup: f64,
    pub load: f64,
    pub deplete: f64,
    pub vdry: f64,
    pub hgain: f64,
    pub flatten: f64,
    pub streak: f64,
    pub streak_mix: f64,
    pub body: f64,
    pub hardness: f64,
    pub grain: f64,
    pub nb_per_cw: f64,
    pub nb_base: f64,
    pub release: f64,
    pub dropout: f64,
    pub ragged: f64,
    pub ridge: f64,
    pub levee: f64,
    pub furrow: f64,
    pub blob: f64,
    pub stiff: f64,
    pub marble: f64,
    pub load2: Option<[f32; 3]>,
    pub splay: f64,
    pub snap: f64,
    pub jitter: [f64; 2],
    pub warmth: f64,
    pub warm_color: [f32; 3],
    pub priority: i32,
    pub reverse_p: f64,
    pub spill: f64,
    pub stop_at_edge: f64,
    pub min_aspect: f64,
    pub end_width: f64,
    pub end_pressure: f64,
    pub hgain_jitter: f64,
    pub size_by_y: Option<[f64; 4]>,
    pub opacity_by_light: f64,
    pub l_floor: f64,
    pub size_by_detail: f64,
    pub length_skew: f64,
    pub dab_share: f64,
    pub end_variation: f64,
    pub flick: f64,
    pub relief_by_value: f64,
    pub mode: Option<Mode>,
}

impl St {
    /// The engine's neutral defaults: v1's planner defaults, without Storm Light's taste (`lFloor` 20 moved to the
    /// impressionist preset; `splay` 1.0 read as pencil lines).
    pub fn defaults() -> St {
        St {
            colors: Vec::new(),
            flecks: Vec::new(),
            width: [0.015, 0.03],
            length: [0.04, 0.10],
            curvature: 0.3,
            align: 0.85,
            opacity: [0.85, 1.0],
            pickup: 0.12,
            load: 1.0,
            deplete: 0.02,
            vdry: 0.25,
            hgain: 1.0,
            flatten: 0.6,
            streak: 0.25,
            streak_mix: 0.8,
            body: 0.9,
            hardness: 0.75,
            grain: 0.10,
            nb_per_cw: 450.0,
            nb_base: 5.0,
            release: 0.3,
            dropout: 0.02,
            ragged: 0.5,
            ridge: 0.5,
            levee: 0.35,
            furrow: 0.15,
            blob: 0.3,
            stiff: 0.25,
            marble: 0.0,
            load2: None,
            splay: 0.35,
            snap: 0.85,
            jitter: [5.0, 4.0],
            warmth: 0.0,
            warm_color: [246.0 / 255.0, 208.0 / 255.0, 154.0 / 255.0],
            priority: 0,
            reverse_p: 0.0,
            spill: 0.15,
            stop_at_edge: 0.9,
            min_aspect: 2.5,
            end_width: 0.5,
            end_pressure: 0.15,
            hgain_jitter: 0.25,
            size_by_y: None,
            opacity_by_light: 0.0,
            l_floor: 0.0,
            size_by_detail: 0.0,
            length_skew: 0.0,
            dab_share: 0.0,
            end_variation: 0.0,
            flick: 0.0,
            relief_by_value: 0.0,
            mode: None,
        }
    }
}

fn rgb<M: Mixer>(m: &M, c: &ColorSpec) -> [f32; 3] {
    // validated before planning
    oil_scene::color::resolve(c, m, "").unwrap_or([0.5, 0.5, 0.5])
}

/// Copy each `Some` field of `s` into `st`.
macro_rules! copy_some {
    ($st:ident, $s:ident; $($f:ident),*) => { $(if let Some(v) = $s.$f { $st.$f = v; })* };
}

/// Overlay the style keys of a `Style` or a `Layer` (they share them) onto a resolved style.
macro_rules! overlay_fn {
    ($name:ident, $t:ty) => {
        pub fn $name<M: Mixer>(st: &mut St, s: &$t, m: &M) {
            copy_some!(st, s; width, length, curvature, align, opacity, pickup, load, deplete, vdry, hgain, flatten, streak, streak_mix, body,
                  hardness, grain, nb_per_cw, nb_base, release, dropout, ragged, ridge, levee, furrow, blob, stiff, marble, splay,
                  snap, jitter, warmth, priority, reverse_p, spill, stop_at_edge, min_aspect, end_width, end_pressure, hgain_jitter,
                  opacity_by_light, l_floor, size_by_detail, length_skew, dab_share, end_variation, flick, relief_by_value);
            if let Some(v) = s.size_by_y {
                st.size_by_y = Some(v);
            }
            if let Some(v) = s.mode {
                st.mode = Some(v);
            }
            if let Some(v) = &s.colors {
                st.colors = v.iter().map(|c| rgb(m, c)).collect();
            }
            if let Some(v) = &s.flecks {
                st.flecks = v.iter().map(|(c, p)| (rgb(m, c), *p)).collect();
            }
            if let Some(v) = &s.load2 {
                st.load2 = Some(rgb(m, v));
            }
            if let Some(v) = &s.warm_color {
                st.warm_color = rgb(m, v);
            }
        }
    };
}

overlay_fn!(overlay_style, Style);
overlay_fn!(overlay_layer, Layer);

/// The style of one region in one layer.
pub fn resolve<M: Mixer>(m: &M, preset: Option<&Style>, region: Option<&Style>, layer: &Layer) -> St {
    let mut st = St::defaults();
    if let Some(p) = preset {
        overlay_style(&mut st, p, m);
    }
    if let Some(r) = region {
        overlay_style(&mut st, r, m);
    }
    overlay_layer(&mut st, layer, m);
    st
}

/// The brush mode of a stroke: the region's (or layer override's) `mode`, else the layer's, else paint.
pub fn mode_of(st: &St, layer: &Layer) -> i32 {
    match st.mode.or(layer.mode).unwrap_or(Mode::Paint) {
        Mode::Paint => oil_kernel::brush::MODE_PAINT,
        Mode::Scumble => oil_kernel::brush::MODE_SCUMBLE,
        Mode::Smudge => oil_kernel::brush::MODE_SMUDGE,
        Mode::Glaze => oil_kernel::brush::MODE_GLAZE,
    }
}
