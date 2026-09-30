//! Guide previews as 8-bit RGB images: target, regions (labelled), flow (line segments over the target) and light,
//! and v1's four-panel guide sheet. For eyes and agents, not for the planner; the host encodes the PNGs.
use crate::compile::Guides;

/// An 8-bit RGB image.
pub struct Rgb8 {
    pub w: usize,
    pub h: usize,
    pub data: Vec<u8>,
}

impl Rgb8 {
    pub fn new(w: usize, h: usize, fill: [u8; 3]) -> Rgb8 {
        Rgb8 { w, h, data: fill.iter().copied().cycle().take(w * h * 3).collect() }
    }

    fn put(&mut self, x: i64, y: i64, c: [u8; 3]) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            let k = (y as usize * self.w + x as usize) * 3;
            self.data[k..k + 3].copy_from_slice(&c);
        }
    }

    fn rect(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, c: [u8; 3]) {
        for y in y0..y1.min(self.h) {
            for x in x0..x1.min(self.w) {
                self.put(x as i64, y as i64, c);
            }
        }
    }

    /// Area resize to `dw` x `dh`.
    pub fn resized(&self, dw: usize, dh: usize) -> Rgb8 {
        let mut out = Rgb8::new(dw, dh, [0; 3]);
        for c in 0..3 {
            let plane: Vec<f32> = self.data.iter().skip(c).step_by(3).map(|v| *v as f32).collect();
            let r = if dw <= self.w { oil_image::resize_area(&plane, self.w, self.h, dw, dh) } else { nearest(&plane, self.w, self.h, dw, dh) };
            for (i, v) in r.iter().enumerate() {
                out.data[i * 3 + c] = (v + 0.5).clamp(0.0, 255.0) as u8;
            }
        }
        out
    }

    fn blit(&mut self, src: &Rgb8, x0: usize, y0: usize) {
        for y in 0..src.h {
            for x in 0..src.w {
                let k = (y * src.w + x) * 3;
                self.put((x0 + x) as i64, (y0 + y) as i64, [src.data[k], src.data[k + 1], src.data[k + 2]]);
            }
        }
    }

    /// Draw text with the 5x7 font at integer `scale`, top-left at (x, y).
    pub fn text(&mut self, x: i64, y: i64, s: &str, scale: usize, c: [u8; 3]) {
        for (n, ch) in s.chars().enumerate() {
            let g = glyph(ch);
            let gx = x + (n * 6 * scale) as i64;
            for (row, bits) in g.iter().enumerate() {
                for col in 0..5 {
                    if bits & (0x10 >> col) != 0 {
                        for sy in 0..scale {
                            for sx in 0..scale {
                                self.put(gx + (col * scale + sx) as i64, y + (row * scale + sy) as i64, c);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Text with a one-pixel halo, readable on any background.
    fn label(&mut self, x: i64, y: i64, s: &str, scale: usize) {
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            self.text(x + dx, y + dy, s, scale, [245, 245, 245]);
        }
        self.text(x, y, s, scale, [0, 0, 0]);
    }
}

fn nearest(src: &[f32], w: usize, h: usize, dw: usize, dh: usize) -> Vec<f32> {
    let mut out = vec![0f32; dw * dh];
    for y in 0..dh {
        let sy = (y * h / dh).min(h - 1);
        for x in 0..dw {
            out[y * dw + x] = src[sy * w + (x * w / dw).min(w - 1)];
        }
    }
    out
}

fn to8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

pub fn target(g: &Guides) -> Rgb8 {
    Rgb8 { w: g.w, h: g.h, data: g.target.iter().flat_map(|p| p.map(to8)).collect() }
}

pub fn light(g: &Guides) -> Rgb8 {
    Rgb8 { w: g.w, h: g.h, data: g.light.iter().flat_map(|v| [to8(*v); 3]).collect() }
}

/// A stable, distinct-enough colour per region id.
pub fn region_color(i: usize) -> [u8; 3] {
    let mut h = (i as u32).wrapping_mul(0x9e37_79b1) ^ 0x5bd1_e995;
    [0, 1, 2].map(|_| {
        h ^= h >> 15;
        h = h.wrapping_mul(0x2c1b_3c6d);
        h ^= h >> 12;
        (60 + (h % 180)) as u8
    })
}

/// Region-coloured map with each name at the centroid of its pixels, drawn at size `w` x `h`.
pub fn regions_at(g: &Guides, w: usize, h: usize, scale: usize) -> Rgb8 {
    let full = Rgb8 { w: g.w, h: g.h, data: g.region_id.iter().flat_map(|id| region_color(*id as usize)).collect() };
    let mut img = if (w, h) == (g.w, g.h) { full } else { full.resized(w, h) };
    let mut sums = vec![(0u64, 0u64, 0u64); g.names.len()];
    for (k, id) in g.region_id.iter().enumerate() {
        let s = &mut sums[*id as usize];
        s.0 += (k % g.w) as u64;
        s.1 += (k / g.w) as u64;
        s.2 += 1;
    }
    for (name, (sx, sy, n)) in g.names.iter().zip(sums) {
        if n > 0 {
            let (cx, cy) = ((sx / n) as usize * w / g.w, (sy / n) as usize * h / g.h);
            let tw = name.chars().count() * 6 * scale;
            img.label(cx as i64 - tw as i64 / 2, cy as i64 - (3 * scale) as i64, name, scale);
        }
    }
    img
}

pub fn regions(g: &Guides) -> Rgb8 {
    regions_at(g, g.w, g.h, (g.w / 300).max(1))
}

/// Short segments along the flow every `max(6, w / 60)` pixels, over the target lightened by half.
pub fn flow(g: &Guides) -> Rgb8 {
    let mut img = Rgb8 { w: g.w, h: g.h, data: g.target.iter().flat_map(|p| p.map(|v| to8(v * 0.5 + 0.5))).collect() };
    let step = (g.w / 60).max(6);
    let len = step as f64 * 0.45;
    for y in (step / 2..g.h).step_by(step) {
        for x in (step / 2..g.w).step_by(step) {
            let f = g.flow[y * g.w + x];
            let (dx, dy) = (f[0] as f64 * len, f[1] as f64 * len);
            let n = (2.0 * len).ceil() as usize * 2;
            for s in 0..=n {
                let t = s as f64 / n as f64 * 2.0 - 1.0;
                img.put((x as f64 + 0.5 + t * dx).floor() as i64, (y as f64 + 0.5 + t * dy).floor() as i64, [40, 40, 160]);
            }
        }
    }
    img
}

/// v1's guide sheet: four panels (target, regions, flow, light) in 360 x 450 cells, titled.
pub fn sheet(g: &Guides) -> Rgb8 {
    let (cw, ch, pad) = (360usize, 450usize, 6usize);
    let s = (cw as f64 / g.w as f64).min(ch as f64 / g.h as f64);
    let (nw, nh) = (((g.w as f64 * s) as usize).max(1), ((g.h as f64 * s) as usize).max(1));
    let panels = [
        ("target", target(g).resized(nw, nh)),
        ("regions", regions_at(g, nw, nh, 1)),
        ("flow", flow(g).resized(nw, nh)),
        ("light", light(g).resized(nw, nh)),
    ];
    let mut out = Rgb8::new(4 * (cw + pad) + pad, ch + 2 * pad, [40; 3]);
    for (i, (title, mut p)) in panels.into_iter().enumerate() {
        p.rect(0, 0, 12 + title.len() * 12, 20, [20; 3]);
        p.text(5, 3, title, 2, [240; 3]);
        out.blit(&p, pad + i * (cw + pad), pad);
    }
    out
}

/// 5x7 glyphs, one row per byte (bit 4 = leftmost column). Lowercase letters, digits and `_-.:/ `; uppercase maps
/// to lowercase and anything else to a box.
fn glyph(c: char) -> [u8; 7] {
    match c.to_ascii_lowercase() {
        'a' => [0x00, 0x00, 0x0e, 0x01, 0x0f, 0x11, 0x0f],
        'b' => [0x10, 0x10, 0x16, 0x19, 0x11, 0x11, 0x1e],
        'c' => [0x00, 0x00, 0x0e, 0x10, 0x10, 0x11, 0x0e],
        'd' => [0x01, 0x01, 0x0d, 0x13, 0x11, 0x11, 0x0f],
        'e' => [0x00, 0x00, 0x0e, 0x11, 0x1f, 0x10, 0x0e],
        'f' => [0x06, 0x09, 0x08, 0x1c, 0x08, 0x08, 0x08],
        'g' => [0x00, 0x0f, 0x11, 0x11, 0x0f, 0x01, 0x0e],
        'h' => [0x10, 0x10, 0x16, 0x19, 0x11, 0x11, 0x11],
        'i' => [0x04, 0x00, 0x0c, 0x04, 0x04, 0x04, 0x0e],
        'j' => [0x02, 0x00, 0x06, 0x02, 0x02, 0x12, 0x0c],
        'k' => [0x10, 0x10, 0x12, 0x14, 0x18, 0x14, 0x12],
        'l' => [0x0c, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e],
        'm' => [0x00, 0x00, 0x1a, 0x15, 0x15, 0x11, 0x11],
        'n' => [0x00, 0x00, 0x16, 0x19, 0x11, 0x11, 0x11],
        'o' => [0x00, 0x00, 0x0e, 0x11, 0x11, 0x11, 0x0e],
        'p' => [0x00, 0x00, 0x1e, 0x11, 0x1e, 0x10, 0x10],
        'q' => [0x00, 0x00, 0x0d, 0x13, 0x0f, 0x01, 0x01],
        'r' => [0x00, 0x00, 0x16, 0x19, 0x10, 0x10, 0x10],
        's' => [0x00, 0x00, 0x0e, 0x10, 0x0e, 0x01, 0x1e],
        't' => [0x08, 0x08, 0x1c, 0x08, 0x08, 0x09, 0x06],
        'u' => [0x00, 0x00, 0x11, 0x11, 0x11, 0x13, 0x0d],
        'v' => [0x00, 0x00, 0x11, 0x11, 0x11, 0x0a, 0x04],
        'w' => [0x00, 0x00, 0x11, 0x11, 0x15, 0x15, 0x0a],
        'x' => [0x00, 0x00, 0x11, 0x0a, 0x04, 0x0a, 0x11],
        'y' => [0x00, 0x00, 0x11, 0x11, 0x0f, 0x01, 0x0e],
        'z' => [0x00, 0x00, 0x1f, 0x02, 0x04, 0x08, 0x1f],
        '0' => [0x0e, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0e],
        '1' => [0x04, 0x0c, 0x04, 0x04, 0x04, 0x04, 0x0e],
        '2' => [0x0e, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1f],
        '3' => [0x1f, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0e],
        '4' => [0x02, 0x06, 0x0a, 0x12, 0x1f, 0x02, 0x02],
        '5' => [0x1f, 0x10, 0x1e, 0x01, 0x01, 0x11, 0x0e],
        '6' => [0x06, 0x08, 0x10, 0x1e, 0x11, 0x11, 0x0e],
        '7' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0e, 0x11, 0x11, 0x0e, 0x11, 0x11, 0x0e],
        '9' => [0x0e, 0x11, 0x11, 0x0f, 0x01, 0x02, 0x0c],
        '_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1f],
        '-' => [0x00, 0x00, 0x00, 0x1f, 0x00, 0x00, 0x00],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0c, 0x0c],
        ':' => [0x00, 0x0c, 0x0c, 0x00, 0x0c, 0x0c, 0x00],
        '/' => [0x00, 0x01, 0x02, 0x04, 0x08, 0x10, 0x00],
        ' ' => [0; 7],
        _ => [0x1f, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1f],
    }
}
