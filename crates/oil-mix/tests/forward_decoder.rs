use oil_mix::{
    palette::{
        synthetic_four, AmountBasis, ForwardDecoder, PaletteLut, PaletteMetadataN, PaletteMixer,
        PaletteMixerN, PaletteN,
    },
    palette_forward::PaletteForwardN,
    Mixer,
};

fn palette<const N: usize, const B: usize>() -> PaletteN<N, B> {
    let names: [String; N] = std::array::from_fn(|i| format!("paint-{i}"));
    let k = std::array::from_fn(|b| {
        std::array::from_fn(|i| match b % 11 {
            0 => 0.,
            1 => 1e100,
            _ => 0.005 + ((b * 13 + i * 17) % 67) as f64 / 20.,
        })
    });
    let s = std::array::from_fn(|b| {
        std::array::from_fn(|i| {
            if b % 11 == 1 {
                1e-100
            } else {
                0.1 + ((b * 7 + i * 11) % 53) as f64 / 30.
            }
        })
    });
    PaletteN::from_optics_with_pair_correction(
        PaletteMetadataN {
            id: "forward-test",
            paint_names: std::array::from_fn(|i| names[i].as_str()),
            amount_basis: AmountBasis::Relative,
            provenance: "Synthetic extreme/control-bound tests",
        },
        k,
        s,
        vec![[0.8, -0.8, 0.8, -0.8]; N * (N - 1) / 2],
    )
    .unwrap()
}
fn check<const N: usize, const B: usize>() {
    let p = palette::<N, B>();
    let expected_default = if N == 1 {
        ForwardDecoder::Reference
    } else {
        ForwardDecoder::ExpLutV1
    };
    let direct = PaletteMixerN::direct(p.clone()).unwrap();
    let imported = PaletteMixerN::<N, false, B>::from_palette_bytes(&p.to_bytes()).unwrap();
    for mixer in [&direct, &imported] {
        assert_eq!(mixer.forward_decoder(), expected_default);
        assert_eq!(
            mixer.forward_auxiliary_bytes(),
            if N == 1 { 0 } else { B * 32 + 513 * 8 }
        );
        let explicit = PaletteForwardN::new(&p, expected_default);
        let state = mixer.recipe([1.; N]).unwrap();
        let expected = ::ochrell::conversion::gamut_map(
            explicit.decode_linear(state.map(|v| v as f64)).unwrap(),
        );
        assert_eq!(
            mixer.decode_linear_rgb(&state).map(f32::to_bits),
            expected.map(|v| (v as f32).to_bits())
        );
    }
    let legacy = PaletteMixerN::<N, false, B>::from_bytes(&p.to_bytes(), &[]).unwrap();
    assert_eq!(legacy.forward_decoder(), ForwardDecoder::Reference);
    assert_eq!(legacy.forward_auxiliary_bytes(), 0);
    let mut probes = Vec::new();
    for i in 0..N {
        let mut c = [0.; N];
        c[i] = 1.;
        probes.push(c);
    }
    for i in 0..N {
        for j in i + 1..N {
            for t in [1e-12, 0.1, 0.5, 0.9, 1. - 1e-12] {
                let mut c = [0.; N];
                c[i] = 1. - t;
                c[j] = t;
                probes.push(c);
            }
        }
    }
    for n in 0..64 {
        probes.push(std::array::from_fn(|i| ((n * 17 + i * 7) % 29 + 1) as f64));
    }
    for method in [ForwardDecoder::AlgebraicV1, ForwardDecoder::ExpLutV1] {
        let decoder = PaletteForwardN::new(&p, method);
        for (index, c) in probes.iter().enumerate() {
            let expected = p.recipe(*c).unwrap().reflectance();
            let actual = decoder.reflectance(*c).unwrap();
            assert!(actual
                .iter()
                .all(|v| v.is_finite() && (0. ..=1.).contains(v)));
            if index < N {
                assert_eq!(actual.map(f64::to_bits), expected.map(f64::to_bits));
            }
            let bound = if method == ForwardDecoder::AlgebraicV1 {
                2e-12
            } else {
                1e-5
            };
            assert!(actual
                .iter()
                .zip(expected)
                .all(|(a, b)| (a - b).abs() <= bound));
            let raw = decoder.decode_linear(*c).unwrap();
            let reference = p.recipe(*c).unwrap().decode_linear();
            assert!(raw.iter().zip(reference).all(|(a, b)| (a - b).abs()
                <= if method == ForwardDecoder::AlgebraicV1 {
                    2e-12
                } else {
                    2e-5
                }));
        }
        assert!(decoder.reflectance([0.; N]).is_err());
        assert!(decoder.reflectance([f64::NAN; N]).is_err());
        assert!(decoder.reflectance([-1.; N]).is_err());
    }
}
#[test]
fn extremes_boundaries_and_full_control_bounds_across_counts_and_grids() {
    check::<1, 31>();
    check::<4, 31>();
    check::<8, 31>();
    check::<10, 31>();
    check::<16, 31>();
    check::<1, 81>();
    check::<4, 81>();
    check::<8, 81>();
    check::<10, 81>();
    check::<16, 81>();
}
#[test]
fn decoder_switch_clears_reports_and_preserves_reference_authoring() {
    let p = palette::<8, 31>();
    let m = PaletteMixerN::direct(p.clone())
        .unwrap()
        .with_forward_decoder(ForwardDecoder::Reference)
        .unwrap();
    let target = [0.3, 0.4, 0.5];
    let original = m.match_target(target).unwrap();
    let state = m.recipe([1., 2., 3., 4., 5., 6., 7., 8.]).unwrap();
    let authored = m.authoring_srgb(&state);
    assert_eq!(m.target_cache_stats().entries, 1);
    let m = m.with_forward_decoder(ForwardDecoder::ExpLutV1).unwrap();
    assert_eq!(m.target_cache_stats().entries, 0);
    assert_eq!(m.palette().to_bytes(), p.to_bytes());
    let found = m.match_target(target).unwrap();
    assert_eq!(
        found.recipe.map(f32::to_bits),
        original.recipe.map(f32::to_bits)
    );
    assert_eq!(found.achieved_srgb, m.decode_srgb(&found.recipe));
    assert_eq!(
        m.authoring_srgb(&state).map(f32::to_bits),
        authored.map(f32::to_bits)
    );
    assert_eq!(m.forward_auxiliary_bytes(), 31 * 32 + 513 * 8);
    let reference = m.with_forward_decoder(ForwardDecoder::Reference).unwrap();
    assert_eq!(reference.target_cache_stats().entries, 0);
    assert_eq!(
        reference.match_target(target).unwrap().achieved_srgb,
        original.achieved_srgb
    );
    let prepared = PaletteMixer::new(
        PaletteLut::build(synthetic_four(), 17)
            .unwrap()
            .into_owned(),
    )
    .unwrap();
    assert_eq!(prepared.forward_auxiliary_bytes(), 0);
    assert!(prepared
        .with_forward_decoder(ForwardDecoder::ExpLutV1)
        .is_err());
    let plain = PaletteMixerN::direct(synthetic_four().clone()).unwrap();
    assert_eq!(plain.forward_decoder(), ForwardDecoder::Reference);
    assert_eq!(plain.forward_auxiliary_bytes(), 0);
}
