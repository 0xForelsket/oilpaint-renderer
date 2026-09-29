//! Exposed-paint approximation, independent of bristle geometry and relighting.
//! Amounts are relative, not measured volume. Pickup copies paint; no conservation
//! solver or layer stack is claimed. Mode 1 deliberately loses material history;
//! mode 2 is padded sRGB for a matched-transport diagnostic control.
use ochrell::{Color, FastPigmentMixer, Latent};
pub const N: usize = 85;

pub fn pack(z: Latent, out: &mut [f32]) {
    out[..41].copy_from_slice(z.absorption());
    out[41..82].copy_from_slice(z.scattering());
    out[82..85].copy_from_slice(&z.residual());
}

pub fn unpack(z: &[f32]) -> Result<Latent, ochrell::MixError> {
    Latent::try_from_parts(
        z[..41].try_into().unwrap(),
        z[41..82].try_into().unwrap(),
        z[82..85].try_into().unwrap(),
    )
}

pub fn encode(rgb: [f32; 3], out: &mut [f32], mode: i32) -> Result<(), ochrell::MixError> {
    let c = Color::srgb(rgb[0], rgb[1], rgb[2])?;
    if mode == 2 {
        out.fill(0.);
        out[..3].copy_from_slice(&rgb);
    } else {
        pack(FastPigmentMixer.encode(c), out);
    }
    Ok(())
}

pub fn decode(z: &[f32], mode: i32) -> [f32; 3] {
    #[cfg(feature = "mixbox_comparison")]
    if mode == 3 {
        let mut rgb = [0.; 3];
        crate::kernel::mixbox_latent_to_rgb(&z[..7], &mut rgb);
        return rgb;
    }
    if mode == 2 {
        return [z[0].clamp(0., 1.), z[1].clamp(0., 1.), z[2].clamp(0., 1.)];
    }
    // Imported buffers are validated in Python; convex kernel operations preserve
    // validity. A failure is a programming error, not silent RGB reconstruction.
    FastPigmentMixer
        .decode(unpack(z).expect("invalid Ochrell optical state"))
        .channels()
}

pub fn roundtrip(z: &mut [f32], mode: i32) {
    if mode == 1 {
        let rgb = decode(z, mode);
        encode(rgb, z, mode).unwrap();
    }
}

/// Limit a signed streak along its original direction before it can make K/S
/// negative. This retains the existing artistic feature without invalid optics.
pub fn streak(base: &mut [f32], delta: &[f32], amplitude: f32, mode: i32) {
    let mut scale = 1.0_f32;
    if mode <= 1 {
        for k in 0..82 {
            let d = amplitude * delta[k];
            let floor = if k < 41 { 0. } else { 1e-10 };
            if d < 0. {
                scale = scale.min(((base[k] - floor) / -d).max(0.));
            }
        }
    }
    for k in 0..N {
        base[k] += amplitude * scale * delta[k];
    }
    // The limiting subtraction can round just below its boundary in f32.
    if mode <= 1 {
        for v in &mut base[..41] {
            *v = v.max(0.);
        }
        for v in &mut base[41..82] {
            *v = v.max(1e-10);
        }
    }
    if mode == 2 {
        for v in &mut base[..3] {
            *v = v.clamp(0., 1.);
        }
    }
}

pub fn composite(old: &mut [f32], new: [f32; 3], coverage: f32) {
    for ch in 0..3 {
        let a = ochrell::conversion::srgb_to_linear(old[ch] as f64);
        let b = ochrell::conversion::srgb_to_linear(new[ch] as f64);
        old[ch] = ochrell::conversion::linear_to_srgb(a + coverage as f64 * (b - a)) as f32;
    }
}

pub fn deposit(
    z: &mut [f32],
    rgb: &mut [f32],
    amount: &mut f32,
    source: &[f32],
    incoming: f32,
    wet: f32,
    coverage: f32,
    glaze: bool,
    mode: i32,
) {
    if glaze {
        composite(rgb, decode(source, mode), coverage);
        return;
    }
    if incoming <= 0. {
        return;
    }
    let accessible = *amount * wet.clamp(0., 1.);
    let total = accessible + incoming;
    let t = incoming / total;
    for k in 0..N {
        z[k] = (1. - t) * z[k] + t * source[k];
    }
    *amount = total;
    roundtrip(z, mode);
    composite(rgb, decode(z, mode), coverage);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state(rgb: [f32; 3]) -> [f32; N] {
        let mut z = [0.; N];
        encode(rgb, &mut z, 0).unwrap();
        z
    }
    #[test]
    fn mixture_uses_amount_and_coverage_only_composites() {
        let red = state([0.9, 0.1, 0.2]);
        let blue = state([0.1, 0.2, 0.9]);
        let mut outputs = Vec::new();
        for a in [0.1, 0.9] {
            let mut z = red;
            let mut amount = 3.;
            let mut rgb = [1.; 3];
            deposit(&mut z, &mut rgb, &mut amount, &blue, 1., 1., a, false, 0);
            assert_eq!(amount, 4.);
            for i in 0..N {
                assert!((z[i] - (0.75 * red[i] + 0.25 * blue[i])).abs() < 1e-6);
            }
            outputs.push((z, rgb));
        }
        assert_eq!(outputs[0].0, outputs[1].0);
        assert_ne!(outputs[0].1, outputs[1].1);
    }
    #[test]
    fn ground_and_dry_paint_are_not_pigment_dilutants() {
        let red = state([0.9, 0.1, 0.2]);
        let blue = state([0.1, 0.2, 0.9]);
        for (mut amount, wet) in [(0., 1.), (100., 0.)] {
            let mut z = red;
            let mut rgb = [1.; 3];
            deposit(&mut z, &mut rgb, &mut amount, &blue, 1., wet, 0.5, false, 0);
            assert_eq!(z, blue);
            assert_eq!(amount, 1.);
        }
    }
    #[test]
    fn glaze_changes_only_display() {
        let mut z = state([0.9, 0.1, 0.2]);
        let before = z;
        let source = state([0.1, 0.2, 0.9]);
        let mut amount = 2.;
        let mut rgb = [1.; 3];
        deposit(&mut z, &mut rgb, &mut amount, &source, 1., 1., 0.2, true, 0);
        assert_eq!(z, before);
        assert_eq!(amount, 2.);
        assert_ne!(rgb, [1.; 3]);
    }
}
