//! The painter: a StrokeList v2, a mixer and a width give canvas planes.
//!
//! Per layer: blur the height plane if the layer asks for it (scumble reads it), paint the strokes in order, then
//! dry. Each stroke's colours are encoded once with the mixer. Its length in brush widths is measured from the cw
//! points, so the bristle pattern does not depend on the canvas size.
#![forbid(unsafe_code)]

pub mod testsheet;

use oil_kernel::brush::{length_in_widths, render_stroke_len};
use oil_kernel::{Canvas, Load, StrokeStats};
use oil_mix::{clamp01, Mixer, State};
use oil_strokes::StrokeList;

/// Streak direction for a colour: half the difference between a lighter/warmer and a darker/cooler variant, times
/// `amount` (v1's `strokes.streak_vector`). Bristles add `t * dz`, t in [-1, 1].
pub fn streak_vector<M: Mixer>(m: &M, z: &M::State, amount: f32) -> M::State {
    let rgb = m.decode_srgb(z);
    let light = [clamp01(rgb[0] * 1.10 + 0.05), clamp01(rgb[1] * 1.10 + 0.035), clamp01(rgb[2] * 1.10)];
    let dark = [clamp01(rgb[0] * 0.88), clamp01(rgb[1] * 0.88 - 0.01), clamp01(rgb[2] * 0.88 + 0.02)];
    let (zl, zd) = (m.encode(light), m.encode(dark));
    let mut dz = M::State::zero();
    let (l, d) = (zl.as_slice(), zd.as_slice());
    for (k, v) in dz.as_mut_slice().iter_mut().enumerate() {
        *v = 0.5 * (l[k] - d[k]) * amount;
    }
    dz
}

/// Totals of a paint run.
#[derive(Clone, Copy, Debug, Default)]
pub struct PaintStats {
    pub strokes: usize,
    pub painted_pixels: i64,
    pub alpha: f64,
}

/// Paint `list` at width `w` with mixer `m`. `after_layer(i, canvas)` is called after each layer (for snapshots,
/// contact sheets and progress).
pub fn paint<M: Mixer>(m: &M, list: &StrokeList, w: u32, mut after_layer: impl FnMut(usize, &Canvas<M>)) -> (Canvas<M>, PaintStats) {
    let (wu, hu) = (w as usize, list.height_for(w) as usize);
    let wf = w as f32;
    let mut cv = Canvas::new(m, wu, hu, list.ground);
    let mut stats = PaintStats::default();
    let mut pts_px: Vec<[f32; 4]> = Vec::new();
    for (li, layer) in list.layers.iter().enumerate() {
        if let Some(sigma) = layer.hblur_sigma {
            cv.hblur = oil_image::blur(&cv.hgt, wu, hu, (sigma as f64 * w as f64).max(1.0));
        }
        for i in layer.start as usize..layer.end as usize {
            let s = &list.strokes[i];
            let pts = list.stroke_points(i);
            pts_px.clear();
            pts_px.extend(pts.iter().map(|p| [p[0] * wf, p[1] * wf, p[2] * wf, p[3]]));
            let zcol = m.encode(s.color);
            let zcol2 = if s.color2 == s.color { zcol } else { m.encode(s.color2) };
            let load = Load { zcol, zcol2, dz: streak_vector(m, &zcol, s.streak_amount) };
            let st: StrokeStats = render_stroke_len(m, &mut cv.planes(), &pts_px, &load, &s.brush, length_in_widths(pts));
            stats.strokes += 1;
            stats.painted_pixels += st.pixels;
            stats.alpha += st.alpha;
        }
        if let Some(d) = layer.dry_after {
            cv.dry(d);
        }
        after_layer(li, &cv);
    }
    (cv, stats)
}

/// Canvas planes that `plane_bytes` serialises.
pub const PLANES: [&str; 5] = ["lat", "rgb", "h", "wet", "cover"];

/// The values of a plane (one of `PLANES`), in row-major order; a mixer state contributes its floats in order.
pub fn plane_values<'a, M: Mixer>(cv: &'a Canvas<M>, plane: &str) -> Box<dyn Iterator<Item = f32> + 'a> {
    match plane {
        "lat" => Box::new(cv.lat.iter().flat_map(|z| z.as_slice().iter().copied())),
        "rgb" => Box::new(cv.rgb.iter().flatten().copied()),
        "h" => Box::new(cv.hgt.iter().copied()),
        "wet" => Box::new(cv.wet.iter().copied()),
        "cover" => Box::new(cv.cover.iter().copied()),
        _ => panic!("unknown plane {plane}"),
    }
}

/// Little-endian bytes of a plane, for hashing (one of `PLANES`). This copies the plane: to hash large canvases,
/// stream `plane_values` instead.
pub fn plane_bytes<M: Mixer>(cv: &Canvas<M>, plane: &str) -> Vec<u8> {
    plane_values(cv, plane).flat_map(|x| x.to_bits().to_le_bytes()).collect()
}