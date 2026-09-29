//! A procedural test sheet: a fixed StrokeList built in code, so it is the same input for every engine version.
//! It exercises every mode and the paths between them (wet-in-wet pick-up at three strengths, wet-on-dry, scumble
//! over texture, smudge across edges, glaze, marble loads, dabs, arcs, a mud stack) and feeds the cross-host check
//! and the golden hashes. It is a determinism fixture, not a visual benchmark (that is the eval harness's sheet).
use oil_kernel::brush::{MODE_GLAZE, MODE_PAINT, MODE_SCUMBLE, MODE_SMUDGE};
use oil_kernel::BrushParams;
use oil_strokes::{Inputs, Layer, Meta, Stroke, StrokeList, NO_REGION};

fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// v1's stroke profile (oilpaint/strokes.py): the brush lands narrow, runs flat and lifts at the end.
fn polyline(xy: &[(f64, f64)], width: f64) -> Vec<[f32; 4]> {
    let mut s = vec![0.0f64];
    for p in xy.windows(2) {
        let (dx, dy) = (p[1].0 - p[0].0, p[1].1 - p[0].1);
        s.push(s.last().unwrap() + (dx * dx + dy * dy).sqrt());
    }
    let total = s.last().unwrap().max(1e-9);
    xy.iter()
        .zip(&s)
        .map(|(&(x, y), &si)| {
            let t = si / total;
            let wf = (0.35 + 0.65 * smoothstep(0.0, 0.12, t)) * (1.0 - 0.5 * smoothstep(0.8, 1.0, t));
            let pr = smoothstep(0.0, 0.05, t) * (1.0 - 0.85 * smoothstep(0.8, 1.0, t));
            [x as f32, y as f32, (width * wf) as f32, pr as f32]
        })
        .collect()
}

fn straight(x0: f64, y0: f64, x1: f64, y1: f64, width: f64) -> Vec<[f32; 4]> {
    let len = ((x1 - x0) * (x1 - x0) + (y1 - y0) * (y1 - y0)).sqrt();
    let n = ((len / (0.35 * width)) as usize + 1).max(3);
    let pts: Vec<(f64, f64)> = (0..n).map(|i| i as f64 / (n - 1) as f64).map(|t| (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t)).collect();
    polyline(&pts, width)
}

fn arc(cx: f64, cy: f64, r: f64, a0: f64, a1: f64, width: f64) -> Vec<[f32; 4]> {
    let n = (((a1 - a0).abs() * r / (0.35 * width)) as usize + 1).max(4);
    let pts: Vec<(f64, f64)> = (0..n)
        .map(|i| a0 + (a1 - a0) * i as f64 / (n - 1) as f64)
        .map(|a| (cx + r * oil_math::cos(a), cy + r * oil_math::sin(a)))
        .collect();
    polyline(&pts, width)
}

fn hex(c: u32) -> [f32; 3] {
    [(c >> 16) & 255, (c >> 8) & 255, c & 255].map(|v| v as f32 / 255.0)
}

struct Builder {
    list: StrokeList,
    layer: u32,
}

impl Builder {
    fn layer(&mut self, name: &str, hblur_sigma: Option<f32>, dry_after: Option<f32>) {
        let n = self.list.strokes.len() as u32;
        if let Some(prev) = self.list.layers.last_mut() {
            prev.end = n;
        }
        self.layer = self.list.layers.len() as u32;
        self.list.layers.push(Layer { start: n, end: n, hblur_sigma, dry_after });
        self.list.meta.layers.push(name.to_string());
    }

    fn stroke(&mut self, pts: Vec<[f32; 4]>, color: u32, color2: Option<u32>, f: impl FnOnce(&mut BrushParams)) {
        let width = pts.iter().fold(0.0f32, |w, p| w.max(p[2]));
        let seed = 1000 + self.list.strokes.len() as u32 * 7;
        let mut brush = BrushParams { seed, nb: ((5.0 + 450.0 * width) as i32).clamp(3, 40), ..BrushParams::default() };
        f(&mut brush);
        self.list.points.extend_from_slice(&pts);
        self.list.offsets.push(self.list.points.len() as u32);
        let color = hex(color);
        self.list.strokes.push(Stroke { layer: self.layer, region: NO_REGION, color, color2: color2.map(hex).unwrap_or(color), streak_amount: 1.0, brush });
        self.list.layers.last_mut().unwrap().end = self.list.strokes.len() as u32;
    }
}

/// The test sheet (aspect 4:5, 94 strokes in 8 layers).
pub fn testsheet() -> StrokeList {
    let mut b = Builder {
        list: StrokeList {
            meta: Meta { generator: "oil-paint testsheet".into(), title: Some("test sheet".into()), plan: None, inputs: Inputs::default(), layers: vec![], regions: vec![] },
            aspect: [4, 5],
            ground: hex(0xe9e1d6),
            layers: vec![],
            offsets: vec![0],
            points: vec![],
            strokes: vec![],
        },
        layer: 0,
    };
    // 1. toned ground: long, thin, translucent strokes
    b.layer("ground", None, None);
    for (i, c) in [0xe4dcd8u32, 0xded7d4, 0xe6ded0, 0xe2d6cc, 0xd8cfc4, 0xe0d4c8].iter().enumerate() {
        let y = 0.08 + 0.2 * i as f64;
        b.stroke(straight(0.02, y, 0.98, y + 0.02, 0.09), *c, None, |p| {
            p.opacity = 0.4;
            p.pickup = 0.3;
            p.hardness = 0.4;
            p.ridge = 0.1;
            p.levee = 0.0;
        });
    }
    // 2. colour blocks: yellow, red, blue, white; marble two-colour loads
    b.layer("blocks", None, None);
    for (i, c) in [0xfeec00u32, 0xff2702, 0x002185, 0xfaf7f0].iter().enumerate() {
        for k in 0..3 {
            let y = 0.10 + 0.06 * k as f64;
            let x0 = 0.05 + 0.23 * i as f64;
            b.stroke(straight(x0, y, x0 + 0.2, y + 0.005, 0.045), *c, None, |_| {});
        }
    }
    for (i, (c, c2)) in [(0x3a67a0u32, 0xf6d09au32), (0xc8503f, 0x352e5e)].iter().enumerate() {
        let y = 0.36 + 0.07 * i as f64;
        b.stroke(straight(0.05, y, 0.9, y + 0.01, 0.05), *c, Some(*c2), |p| p.marble = 0.5);
    }
    // 3. wet-in-wet: blue across wet yellow and red at three pick-ups
    b.layer("wet-in-wet", None, Some(0.2));
    for (i, pk) in [0.1f32, 0.4, 0.8].iter().enumerate() {
        let x = 0.08 + 0.05 * i as f64;
        b.stroke(straight(x, 0.06, x + 0.02, 0.26, 0.03), 0x1446ff, None, |p| p.pickup = *pk);
        let x = 0.30 + 0.05 * i as f64;
        b.stroke(straight(x, 0.06, x + 0.02, 0.26, 0.03), 0xfeec00, None, |p| p.pickup = *pk);
    }
    // 4. wet-on-dry (the layer above dried to 0.2) and a mud stack of eight colours
    b.layer("wet-on-dry", None, None);
    for i in 0..3 {
        let x = 0.55 + 0.05 * i as f64;
        b.stroke(straight(x, 0.06, x + 0.02, 0.26, 0.03), 0x1446ff, None, |p| p.pickup = 0.4);
    }
    for (i, c) in [0x5a8fc0u32, 0xe8944a, 0x2f7590, 0xc9785a, 0x6fa0bb, 0xd98f9a, 0x8a86b8, 0xa98c86].iter().enumerate() {
        let y = 0.52 + 0.004 * i as f64;
        b.stroke(straight(0.1, y, 0.45, y + 0.03, 0.05), *c, None, |p| p.pickup = 0.3);
    }
    // 5. textured impasto, then scumble over it (reads the blurred height)
    b.layer("impasto", None, Some(0.3));
    for i in 0..8 {
        let x = 0.55 + 0.05 * i as f64;
        b.stroke(straight(x, 0.5, x + 0.01, 0.72, 0.035), 0x8d7d9a, None, |p| {
            p.hgain = 2.0;
            p.ridge = 0.8;
            p.levee = 0.5;
        });
    }
    b.layer("scumble", Some(0.01), None);
    for i in 0..5 {
        let y = 0.52 + 0.045 * i as f64;
        b.stroke(straight(0.52, y, 0.97, y + 0.01, 0.04), 0xf0d3a8, None, |p| {
            p.mode = MODE_SCUMBLE;
            p.opacity = 0.35;
            p.body = 0.0;
            p.load = 0.5;
            p.vdry = 0.6;
            p.deplete = 0.0;
            p.dry_thresh = 0.01;
            p.dry_width = 0.12;
            p.pickup = 0.0;
        });
    }
    // 6. smudge across edges, and glaze over light and dark
    b.layer("smudge-glaze", None, None);
    for i in 0..4 {
        let x = 0.1 + 0.08 * i as f64;
        b.stroke(straight(x, 0.34, x + 0.03, 0.48, 0.02), 0xdcd6ea, None, |p| {
            p.mode = MODE_SMUDGE;
            p.opacity = 0.3;
            p.pickup = 0.3;
            p.flatten = 0.2;
            p.body = 0.5;
        });
    }
    for i in 0..4 {
        let x = 0.55 + 0.1 * i as f64;
        b.stroke(straight(x, 0.08, x + 0.02, 0.45, 0.04), 0xa94358, None, |p| {
            p.mode = MODE_GLAZE;
            p.opacity = 0.3;
            p.hgain = 0.0;
        });
    }
    // 7. arcs and dabs
    b.layer("arcs-dabs", None, None);
    for i in 0..6 {
        let r = 0.08 + 0.03 * i as f64;
        b.stroke(arc(0.3, 1.0, r, -2.8, -0.4, 0.02), 0x3f88a0 + 0x0a0a00 * i as u32, None, |p| p.mode = MODE_PAINT);
    }
    for i in 0..30 {
        let (x, y) = (0.58 + 0.06 * (i % 6) as f64, 0.8 + 0.07 * (i / 6) as f64);
        let c = [0xffd870u32, 0xfff2c0, 0xe0a070, 0xf3e9e0, 0x4a4270][i % 5];
        b.stroke(straight(x, y, x + 0.012, y + 0.004, 0.009), c, None, |p| {
            p.blob = 0.3;
            p.levee = 0.3;
            p.hgain = 2.0;
            p.marble = 0.0;
        });
    }
    b.list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn testsheet_is_valid_and_stable() {
        let s = testsheet();
        assert!(s.validate().is_empty(), "{:?}", s.validate());
        assert_eq!(s.strokes.len(), 94);
        assert_eq!(s.layers.len(), 8);
        assert_eq!(testsheet().to_bytes(), s.to_bytes());
    }
}
