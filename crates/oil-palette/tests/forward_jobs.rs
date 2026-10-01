use oil_mix::{
    palette::{AmountBasis, PaletteMetadataN, PaletteN},
    Mixer,
};
use oil_palette::{ForwardDecoder, PaletteJobN, PaletteMixerN, RecipeLoadN};
use sha2::{Digest, Sha256};

fn mixer<const N: usize>() -> PaletteMixerN<N, false, 31> {
    let names: [String; N] = std::array::from_fn(|i| format!("paint-{i}"));
    let p = PaletteN::from_optics_with_pair_correction(
        PaletteMetadataN {
            id: "forward-job",
            paint_names: std::array::from_fn(|i| names[i].as_str()),
            amount_basis: AmountBasis::Relative,
            provenance: "Synthetic forward replay test",
        },
        std::array::from_fn(|b| {
            std::array::from_fn(|i| 0.01 + ((b * 7 + i * 13) % 41) as f64 / 12.)
        }),
        std::array::from_fn(|b| std::array::from_fn(|i| 0.1 + ((b + i * 11) % 23) as f64 / 15.)),
        vec![[0.8, -0.4, 0.6, -0.8]; N * (N - 1) / 2],
    )
    .unwrap();
    PaletteMixerN::direct(p).unwrap()
}
fn job<const N: usize>() -> PaletteJobN<N, false, 31> {
    let m = mixer::<N>();
    let g = oil_paint::testsheet::testsheet();
    let ground = m.recipe([1.; N]).unwrap();
    let main = m.recipe(std::array::from_fn(|i| (i + 1) as f64)).unwrap();
    let secondary = m.paint(&format!("paint-{}", N - 1)).unwrap();
    let mut dz = [0.; N];
    if N > 1 {
        dz[0] = -0.02;
        dz[N - 1] = 0.02;
    }
    let loads = vec![
        RecipeLoadN {
            main,
            secondary,
            dz
        };
        g.strokes.len()
    ];
    PaletteJobN::new(m, g, ground, loads).unwrap()
}
fn roundtrip<const N: usize>() {
    let new_job = job::<N>();
    let expected_default = if N == 1 {
        ForwardDecoder::Reference
    } else {
        ForwardDecoder::ExpLutV1
    };
    assert_eq!(new_job.mixer().forward_decoder(), expected_default);
    let new_bytes = new_job.to_bytes().unwrap();
    assert_eq!(
        u32::from_le_bytes(new_bytes[8..12].try_into().unwrap()),
        expected_default.tag()
    );
    let new_restored = PaletteJobN::<N, false, 31>::from_bytes(&new_bytes).unwrap();
    assert_eq!(new_restored.mixer().forward_decoder(), expected_default);
    assert_eq!(new_restored.to_bytes().unwrap(), new_bytes);
    let new_rgb = oil_paint::plane_bytes(&new_job.paint_final(64).unwrap().0, "rgb");
    assert_eq!(
        new_rgb,
        oil_paint::plane_bytes(&new_restored.paint_final(64).unwrap().0, "rgb")
    );
    let reference = new_job
        .with_forward_decoder(ForwardDecoder::Reference)
        .unwrap();
    let bytes = reference.to_bytes().unwrap();
    let (base, stats) = reference.paint_final(64).unwrap();
    assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 0);
    let old_restored = PaletteJobN::<N, false, 31>::from_bytes(&bytes).unwrap();
    assert_eq!(
        old_restored.mixer().forward_decoder(),
        ForwardDecoder::Reference
    );
    assert_eq!(old_restored.to_bytes().unwrap(), bytes);
    assert_eq!(
        oil_paint::plane_bytes(&base, "rgb"),
        oil_paint::plane_bytes(&old_restored.paint_final(64).unwrap().0, "rgb")
    );
    for method in [ForwardDecoder::AlgebraicV1, ForwardDecoder::ExpLutV1] {
        let candidate = PaletteJobN::<N, false, 31>::from_bytes(&bytes)
            .unwrap()
            .with_forward_decoder(method)
            .unwrap();
        assert_eq!(candidate.ground(), reference.ground());
        assert_eq!(candidate.loads(), reference.loads());
        let saved = candidate.to_bytes().unwrap();
        assert_eq!(
            u32::from_le_bytes(saved[8..12].try_into().unwrap()),
            method.tag()
        );
        assert_eq!(&saved[12..saved.len() - 32], &bytes[12..bytes.len() - 32]);
        let restored = PaletteJobN::<N, false, 31>::from_bytes(&saved).unwrap();
        assert_eq!(restored.mixer().forward_decoder(), method);
        assert_eq!(restored.to_bytes().unwrap(), saved);
        let (a, s) = candidate.paint_final(64).unwrap();
        let b = restored.paint_final(64).unwrap().0;
        let c = restored.paint(64, |_, _| {}).unwrap().0;
        assert_eq!(
            (stats.strokes, stats.painted_pixels, stats.alpha.to_bits()),
            (s.strokes, s.painted_pixels, s.alpha.to_bits())
        );
        for plane in oil_paint::PLANES {
            let data = oil_paint::plane_bytes(&a, plane);
            assert_eq!(data, oil_paint::plane_bytes(&b, plane));
            assert_eq!(data, oil_paint::plane_bytes(&c, plane));
            if plane != "rgb" {
                assert_eq!(data, oil_paint::plane_bytes(&base, plane));
            }
        }
        assert_eq!(base.hblur, a.hblur);
        assert_eq!(a.hblur, b.hblur);
        assert!(a
            .rgb
            .iter()
            .flatten()
            .zip(base.rgb.iter().flatten())
            .all(|(a, b)| (*a - *b).abs() <= 1e-4));
        assert_eq!(
            restored
                .with_forward_decoder(ForwardDecoder::Reference)
                .unwrap()
                .to_bytes()
                .unwrap(),
            bytes
        );
    }
}
#[test]
fn selected_decoder_roundtrips_without_changing_materials_for_one_to_sixteen() {
    roundtrip::<1>();
    roundtrip::<4>();
    roundtrip::<8>();
    roundtrip::<10>();
    roundtrip::<16>();
}
#[test]
fn authoring_recipes_streaks_and_reference_search_are_decoder_independent() {
    let g = oil_paint::testsheet::testsheet();
    let (reference, rr) = PaletteJobN::from_rgb(
        mixer::<8>()
            .with_forward_decoder(ForwardDecoder::Reference)
            .unwrap(),
        g.clone(),
    )
    .unwrap();
    let (preferred, _) = PaletteJobN::from_rgb(mixer::<8>(), g.clone()).unwrap();
    assert_eq!(
        preferred.mixer().forward_decoder(),
        ForwardDecoder::ExpLutV1
    );
    assert_eq!(preferred.loads(), reference.loads());
    assert_eq!(preferred.ground(), reference.ground());
    for method in [ForwardDecoder::AlgebraicV1, ForwardDecoder::ExpLutV1] {
        let (candidate, cr) = PaletteJobN::from_rgb(
            mixer::<8>().with_forward_decoder(method).unwrap(),
            g.clone(),
        )
        .unwrap();
        assert_eq!(candidate.loads(), reference.loads());
        assert_eq!(candidate.ground(), reference.ground());
        for (a, b) in rr.strokes.iter().flatten().zip(cr.strokes.iter().flatten()) {
            assert_eq!(a.recipe.map(f32::to_bits), b.recipe.map(f32::to_bits));
            assert_eq!(
                a.reference_error_ok100.to_bits(),
                b.reference_error_ok100.to_bits()
            );
            assert_eq!(a.evaluations, b.evaluations);
            assert_eq!(b.achieved_srgb, candidate.mixer().decode_srgb(&b.recipe));
        }
    }
}
#[test]
fn rechecksummed_unknown_decoder_is_rejected() {
    let mut bytes = job::<8>().to_bytes().unwrap();
    bytes[8..12].copy_from_slice(&3u32.to_le_bytes());
    let end = bytes.len() - 32;
    let hash = Sha256::digest(&bytes[..end]);
    bytes[end..].copy_from_slice(&hash);
    assert!(PaletteJobN::<8, false, 31>::from_bytes(&bytes).is_err());
}
