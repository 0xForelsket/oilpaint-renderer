//! Colour specs: `#rrggbb`, tube names, sRGB triples and two-colour mixes made with the active mixer.
use crate::spec::ColorSpec;
use oil_errors::Error;
use oil_mix::{Mixer, State};

/// The tube catalogue (v1's `mix.TUBES`): Mixbox's published pigment swatches plus a few painter's tubes, as 8-bit
/// sRGB.
pub const TUBES: [(&str, [u8; 3]); 22] = [
    ("lead_white", [250, 247, 240]),
    ("titanium_white", [255, 255, 255]),
    ("cadmium_yellow", [254, 236, 0]),
    ("hansa_yellow", [252, 211, 0]),
    ("cadmium_orange", [255, 105, 0]),
    ("cadmium_red", [255, 39, 2]),
    ("vermilion", [227, 66, 52]),
    ("madder", [160, 32, 60]),
    ("quinacridone_magenta", [128, 2, 46]),
    ("cobalt_violet", [78, 0, 66]),
    ("cobalt_violet_light", [146, 89, 163]),
    ("ultramarine", [25, 0, 89]),
    ("cobalt_blue", [0, 33, 133]),
    ("phthalo_blue", [13, 27, 68]),
    ("cerulean", [42, 120, 190]),
    ("phthalo_green", [0, 60, 50]),
    ("viridian", [64, 130, 109]),
    ("emerald", [60, 180, 110]),
    ("permanent_green", [7, 109, 22]),
    ("sap_green", [107, 148, 4]),
    ("yellow_ochre", [204, 153, 51]),
    ("burnt_sienna", [138, 54, 15]),
];

fn byte(v: u8) -> f32 {
    (v as f64 / 255.0) as f32
}

/// A named colour (`#rrggbb` or a tube) as sRGB, or an `UNKNOWN_COLOR` error with a did-you-mean fix.
pub fn named(s: &str, path: &str) -> Result<[f32; 3], Error> {
    if let Some(hex) = s.strip_prefix('#') {
        let ok = hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit());
        if !ok {
            return Err(Error::new("UNKNOWN_COLOR", format!("{s} is not a #rrggbb colour"))
                .path(path)
                .got(s)
                .expected("#rrggbb (6 hex digits)"));
        }
        let c = |i: usize| byte(u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0));
        return Ok([c(0), c(2), c(4)]);
    }
    if let Some((_, rgb)) = TUBES.iter().find(|(n, _)| *n == s) {
        return Ok(rgb.map(byte));
    }
    let mut e = Error::new("UNKNOWN_COLOR", format!("{s} is neither #rrggbb nor a tube name")).path(path).got(s);
    e = match oil_errors::suggest(s, TUBES.iter().map(|(n, _)| *n)) {
        Some(t) => e.fix(format!("did you mean \"{t}\"?")),
        None => e.expected(format!("#rrggbb or one of: {}", TUBES.map(|(n, _)| n).join(", "))),
    };
    Err(e)
}

/// Check a colour spec without resolving mixes (validation runs before a mixer is chosen).
pub fn check(c: &ColorSpec, path: &str) -> Vec<Error> {
    let mut errs = Vec::new();
    match c {
        ColorSpec::Named(s) => errs.extend(named(s, path).err()),
        ColorSpec::Mix(a, b, t) => {
            errs.extend(named(a, &format!("{path}/0")).err());
            errs.extend(named(b, &format!("{path}/1")).err());
            if !(0.0..=1.0).contains(t) {
                errs.push(Error::new("RANGE", "a mix ratio must be in [0, 1]").path(format!("{path}/2")).got(t).expected("[0, 1]"));
            }
        }
        ColorSpec::Rgb(v) => {
            for (i, x) in v.iter().enumerate() {
                if !(0.0..=1.0).contains(x) {
                    let mut e = Error::new("RANGE", "sRGB channels are in [0, 1]").path(format!("{path}/{i}")).got(x).expected("[0, 1]");
                    if v.iter().all(|x| (0.0..=255.0).contains(x)) {
                        e = e.fix(format!("8-bit values? use [{:.4}, {:.4}, {:.4}] or \"#{:02x}{:02x}{:02x}\"",
                            v[0] / 255.0, v[1] / 255.0, v[2] / 255.0, v[0] as u8, v[1] as u8, v[2] as u8));
                    }
                    errs.push(e);
                    break;
                }
            }
        }
    }
    errs
}

/// Resolve a colour spec to sRGB. Mixes are made with `mixer`: both colours are encoded, their states lerped by t,
/// and the result decoded (v1's `mix_rgb`).
pub fn resolve<M: Mixer>(c: &ColorSpec, mixer: &M, path: &str) -> Result<[f32; 3], Error> {
    match c {
        ColorSpec::Named(s) => named(s, path),
        ColorSpec::Rgb(v) => Ok(v.map(|x| x as f32)),
        ColorSpec::Mix(a, b, t) => {
            let za = mixer.encode(named(a, &format!("{path}/0"))?);
            let zb = mixer.encode(named(b, &format!("{path}/1"))?);
            let t = *t as f32;
            let mut z = M::State::zero();
            for ((o, x), y) in z.as_mut_slice().iter_mut().zip(za.as_slice()).zip(zb.as_slice()) {
                *o = (1.0 - t) * x + t * y;
            }
            Ok(mixer.decode_srgb(&z))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_hex_and_mixes() {
        assert_eq!(named("#ff0080", "/c").unwrap(), [1.0, 0.0, byte(0x80)]);
        assert_eq!(named("cobalt_blue", "/c").unwrap(), [0.0, byte(33), byte(133)]);
        let e = named("cobalt_bleu", "/c").unwrap_err();
        assert_eq!(e.code, "UNKNOWN_COLOR");
        assert!(e.fix.unwrap().contains("cobalt_blue"));
        assert_eq!(named("#12345", "/c").unwrap_err().code, "UNKNOWN_COLOR");
        let m = ColorSpec::Mix("#000000".into(), "#ffffff".into(), 0.5);
        let c = resolve(&m, &oil_mix::RgbMixer, "/c").unwrap();
        assert_eq!(c, [0.5, 0.5, 0.5]);
        let bad = ColorSpec::Rgb([200.0, 10.0, 0.0]);
        assert!(check(&bad, "/c")[0].fix.as_ref().unwrap().contains("#c80a00"));
    }
}
