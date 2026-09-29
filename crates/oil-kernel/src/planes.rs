//! Canvas planes. The kernel paints through `Planes`, a borrowed view, so the same code serves an engine-owned
//! `Canvas` and buffers owned by a host (the Python harness shim, later WASM memory and tiled storage).
use oil_mix::{Mixer, State};

/// Borrowed canvas planes, row-major, `w * h` entries each.
pub struct Planes<'a, S> {
    pub w: usize,
    pub h: usize,
    /// Mixer state per pixel (the paint material).
    pub lat: &'a mut [S],
    /// Display colour per pixel, sRGB in [0, 1].
    pub rgb: &'a mut [[f32; 3]],
    /// Paint height.
    pub hgt: &'a mut [f32],
    /// Wetness in [0, 1].
    pub wet: &'a mut [f32],
    /// Summed deposit alpha.
    pub cover: &'a mut [f32],
    /// Relative paint amount (0 = bare ground): deposits mix by amount with the wet part of it.
    pub amount: &'a mut [f32],
    /// Blurred height, read by scumble strokes; empty means zero everywhere.
    pub hblur: &'a [f32],
}

impl<S> Planes<'_, S> {
    pub fn check(&self) -> Result<(), &'static str> {
        let n = self.w.checked_mul(self.h).ok_or("canvas size overflows")?;
        if self.w == 0 || self.h == 0 || self.w > i32::MAX as usize / 2 || self.h > i32::MAX as usize / 2 {
            return Err("canvas dimensions out of range");
        }
        if self.lat.len() != n || self.rgb.len() != n || self.hgt.len() != n || self.wet.len() != n || self.cover.len() != n || self.amount.len() != n {
            return Err("plane length does not match width x height");
        }
        if !self.hblur.is_empty() && self.hblur.len() != n {
            return Err("hblur length does not match width x height");
        }
        Ok(())
    }
}

/// An engine-owned canvas.
pub struct Canvas<M: Mixer> {
    pub w: usize,
    pub h: usize,
    pub lat: Vec<M::State>,
    pub rgb: Vec<[f32; 3]>,
    pub hgt: Vec<f32>,
    pub wet: Vec<f32>,
    pub cover: Vec<f32>,
    pub amount: Vec<f32>,
    pub hblur: Vec<f32>,
}

impl<M: Mixer> Canvas<M> {
    /// A fresh canvas of the ground colour (sRGB), dry and flat.
    pub fn new(m: &M, w: usize, h: usize, ground: [f32; 3]) -> Self {
        let n = w * h;
        let g = m.encode(ground);
        let rgb = m.decode_srgb(&g);
        Canvas { w, h, lat: vec![g; n], rgb: vec![rgb; n], hgt: vec![0.0; n], wet: vec![0.0; n], cover: vec![0.0; n], amount: vec![0.0; n], hblur: Vec::new() }
    }

    pub fn planes(&mut self) -> Planes<'_, M::State> {
        Planes {
            w: self.w,
            h: self.h,
            lat: &mut self.lat,
            rgb: &mut self.rgb,
            hgt: &mut self.hgt,
            wet: &mut self.wet,
            cover: &mut self.cover,
            amount: &mut self.amount,
            hblur: &self.hblur,
        }
    }

    /// Multiply wetness by `factor` (a layer's `dryAfter`).
    pub fn dry(&mut self, factor: f32) {
        for v in &mut self.wet {
            *v *= factor;
        }
    }

    /// Bytes per pixel of this canvas's planes (the memory budget uses this): state, rgb, h, wet, cover, amount,
    /// hblur.
    pub fn bytes_per_pixel() -> usize {
        4 * (<M::State as State>::LEN + 3 + 5)
    }
}
