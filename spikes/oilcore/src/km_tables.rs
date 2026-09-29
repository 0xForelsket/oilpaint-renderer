//! Generated from mixers/open_mixers.py: Kubelka-Munk mixer on spectral.js 3.0 data (MIT, (c) Ronald van Wijnen),
//! 38 samples grouped into 12 bands.  latent = (Y*KS_1..12, Y, residual rgb) -> LAT = 16.
pub const KM_N: usize = 12;
pub const CMF: [[f32; 12]; 3] = [[5.171289667e-03, 1.069502011e-01, 6.414256990e-02, 4.751546774e-03, 5.182798952e-02, 1.683999598e-01, 2.596453726e-01, 2.065394223e-01, 6.945862621e-02, 1.176675688e-02, 1.346078934e-03, 1.642516581e-04], [1.438075997e-04, 7.794704288e-03, 3.182664514e-02, 1.061512530e-01, 2.524898350e-01, 2.788898945e-01, 1.961246580e-01, 9.514862299e-02, 2.659285255e-02, 4.292329773e-03, 4.860937188e-04, 5.931428313e-05], [2.460935339e-02, 5.402945280e-01, 4.152073860e-01, 9.217949957e-02, 1.406075992e-02, 1.421938534e-03, 3.100014292e-04, 4.853415157e-05, 1.584012580e-06, 0.000000000e+00, 0.000000000e+00, 0.000000000e+00]];
pub const XYZ_RGB: [[f32; 3]; 3] = [[3.240969896e+00, -1.537383199e+00, -4.986107647e-01], [-9.692436457e-01, 1.875967503e+00, 4.155505821e-02], [5.563008040e-02, -2.039769590e-01, 1.056971550e+00]];
#[inline(always)]
fn compand(x: f32) -> f32 { let x = if x < 0.0 { 0.0 } else if x > 1.0 { 1.0 } else { x }; if x > 0.0031308 { 1.055 * x.powf(1.0 / 2.4) - 0.055 } else { x * 12.92 } }
#[inline(always)]
pub fn km_latent_to_rgb(l: &[f32], rgb: &mut [f32]) {
    let y = if l[KM_N] > 1e-6 { l[KM_N] } else { 1e-6 };
    let (mut x0, mut x1, mut x2) = (0f32, 0f32, 0f32);
    for g in 0..KM_N {
        let mut ks = l[g] / y; if ks < 0.0 { ks = 0.0; }
        let r = 1.0 + ks - (ks * ks + 2.0 * ks).sqrt();
        x0 += CMF[0][g] * r; x1 += CMF[1][g] * r; x2 += CMF[2][g] * r;
    }
    for c in 0..3 {
        let lin = XYZ_RGB[c][0] * x0 + XYZ_RGB[c][1] * x1 + XYZ_RGB[c][2] * x2;
        let v = compand(lin) + l[KM_N + 1 + c];
        rgb[c] = if v < 0.0 { 0.0 } else if v > 1.0 { 1.0 } else { v };
    }
}
