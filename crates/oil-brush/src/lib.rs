//! Shared brush catalog and geometry; both authored and planned strokes use this implementation.
#![forbid(unsafe_code)]
use oil_kernel::BrushParams;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
pub const CATALOG: &str = include_str!("../catalog.json");
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
    /// Geometric contact profile, not a physical brush-shape claim.
    pub contact: String,
    /// Coherent, seed-stable contact variation. Zero removes authored variation.
    pub variation: f32,
    pub depletion: f32, // total depletion per path length in half-widths
    pub paint: BTreeMap<String, f32>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Catalog {
    pub version: u32,
    pub presets: Vec<Preset>,
}
pub fn catalog() -> Catalog {
    serde_json::from_str(CATALOG).expect("built-in catalog")
}
pub fn unit(v: f32) -> bool {
    (0.0..=1.0).contains(&v)
}
pub fn range(v: f32, lo: f32, hi: f32) -> bool {
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
pub fn apply(b: &mut BrushParams, values: &BTreeMap<String, f32>) -> Result<(), String> {
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
        if self.version != 2 {
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
                || !["flat", "round", "point"].contains(&p.contact.as_str())
                || !range(p.variation, 0., 0.25)
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
pub fn envelope(p: [f32; 3], t: f32) -> f32 {
    let smooth = |x: f32| x * x * (3. - 2. * x);
    if t < 0.2 {
        p[0] + (p[1] - p[0]) * smooth(t / 0.2)
    } else if t > 0.58 {
        p[1] + (p[2] - p[1]) * smooth((t - 0.58) / 0.42)
    } else {
        p[1]
    }
}

pub fn contact_width(p: &Preset, t: f32, seed: u32) -> f32 {
    // A handful of smooth lengthwise changes, not per-point or per-pixel jitter.
    let knot = t * 5.;
    let i = knot.floor() as u32;
    let f = knot - i as f32;
    let f = f * f * (3. - 2. * f);
    let random = |n: u32| (scoped_seed(seed, &["contact", &n.to_string()]) >> 8) as f32 / 16777216.;
    let wobble = 1. + p.variation * (2. * (random(i) + (random(i + 1) - random(i)) * f) - 1.);
    let width = if p.contact == "round" {
        // Rounded footprint with an off-centre loaded belly, not a linear lozenge.
        let peak = 0.43 + 0.1 * random(9);
        let u = if t < peak {
            t / peak
        } else {
            (1. - t) / (1. - peak)
        };
        let belly = (u * (2. - u)).max(0.).sqrt();
        let edge = p.width_profile[0] * (1. - t) + p.width_profile[2] * t;
        edge + (p.width_profile[1] - edge) * belly
    } else {
        envelope(p.width_profile, t)
    };
    (width * wobble).max(0.005)
}

/// Compile a validated preset and centerline using the frozen author-format-2 arithmetic.
pub fn compile_path(
    p: &Preset,
    path: &[[f32; 3]],
    width: f32,
    seed: u32,
) -> Result<(Vec<[f32; 4]>, BrushParams), String> {
    if path.len() < 2
        || !range(width, p.width[0], p.width[2])
        || !path
            .iter()
            .all(|p| p[0].is_finite() && p[1].is_finite() && unit(p[2]))
    {
        return Err("INVALID_BRUSH_PATH".into());
    }
    let mut b = BrushParams {
        mode: p.mode,
        seed,
        ..BrushParams::default()
    };
    apply(&mut b, &p.paint)?;
    let mut points = Vec::new();
    let mut lengths = vec![0f32];
    for pair in path.windows(2) {
        let dx = pair[1][0] - pair[0][0];
        let dy = pair[1][1] - pair[0][1];
        lengths.push(lengths.last().unwrap() + (dx * dx + dy * dy).sqrt());
    }
    let len = *lengths.last().unwrap();
    b.deplete = p.depletion / (2. * len / width).max(1.);
    // Arc-length resampling avoids pointer event density changing the brush envelope.
    let n = ((len / (width * 0.2)).ceil() as usize).clamp(16, 2048);
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
        let a = path[seg];
        let z = path[seg + 1];
        points.push([
            a[0] + (z[0] - a[0]) * f,
            a[1] + (z[1] - a[1]) * f,
            width * contact_width(p, t, b.seed),
            (a[2] + (z[2] - a[2]) * f) * envelope(p.pressure_profile, t),
        ]);
    }

    Ok((points, b))
}
