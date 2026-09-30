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
#[cfg(test)]
mod tests {
    use super::*;
    use oil_kernel::BrushParams;
    use oil_mix::RgbMixer;

    /// Mean red channel in 20 px windows along a blue drag through a patch of wet red, with the drag sampled every
    /// `step` pixels.
    fn red_along_drag(step: f32) -> Vec<f64> {
        let m = RgbMixer;
        let (w, h) = (320usize, 120usize);
        let mut cv = Canvas::new(&m, w, h, [0.95, 0.93, 0.9]);
        let line = |x0: f32, x1: f32, width: f32, step: f32| -> Vec<[f32; 4]> {
            let n = ((x1 - x0) / step).round() as usize;
            (0..=n).map(|i| [x0 + (x1 - x0) * i as f32 / n as f32, 60.0, width, 1.0]).collect()
        };
        let red = Load { zcol: [0.8, 0.1, 0.1], zcol2: [0.8, 0.1, 0.1], dz: [0.0; 3] };
        let blue = Load { zcol: [0.1, 0.2, 0.8], zcol2: [0.1, 0.2, 0.8], dz: [0.0; 3] };
        let bp = BrushParams { pickup: 0.4, release: 0.3, seed: 7, ..BrushParams::default() };
        render_stroke_len(&m, &mut cv.planes(), &line(60.0, 140.0, 40.0, 10.0), &red, &bp, 2.0);
        render_stroke_len(&m, &mut cv.planes(), &line(20.0, 300.0, 24.0, step), &blue, &BrushParams { seed: 9, ..bp }, 11.7);
        (0..14)
            .map(|b| {
                let (mut s, mut n) = (0.0f64, 0.0);
                for y in 55..65 {
                    for x in 20 + b * 20..40 + b * 20 {
                        s += cv.rgb[y * w + x][0] as f64;
                        n += 1.0;
                    }
                }
                s / n
            })
            .collect()
    }

    /// Pick-up and release rates are per distance: the red a brush drags in from wet paint and the length of the
    /// smear it leaves after it must not depend on how densely the stroke is sampled. With per-segment rates (engine
    /// 2.0.0-dev.2) the smear after the patch was about half as long at 3 px spacing as at 12 px (summed difference
    /// 0.54); per distance it is 0.10, the rest coming from other per-segment details.
    #[test]
    fn pick_up_does_not_depend_on_point_spacing() {
        let (sparse, dense) = (red_along_drag(12.0), red_along_drag(3.0));
        let diff: f64 = sparse.iter().zip(&dense).map(|(a, b)| (a - b).abs()).sum();
        assert!(diff < 0.2, "12 px vs 3 px sampling differ by {diff:.3}: {sparse:.3?} vs {dense:.3?}");
    }
}
