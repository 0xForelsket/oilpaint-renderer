use oil_mix::{
    palette::{synthetic_four, AmountBasis, PaletteMetadataN, PaletteN},
    Mixer,
};
use oil_paint::{plane_bytes, PLANES};
use oil_palette::{PaletteJob, PaletteJobN, PaletteMixerN, RecipeLoadN};
use sha2::{Digest, Sha256};

fn mixer<const N: usize>() -> PaletteMixerN<N> {
    let names: [String; N] = std::array::from_fn(|i| format!("paint-{i}"));
    let base = synthetic_four();
    let palette = PaletteN::from_optics(
        PaletteMetadataN {
            id: "renderer-many-paint-test",
            paint_names: std::array::from_fn(|i| names[i].as_str()),
            amount_basis: AmountBasis::Relative,
            provenance: "Synthetic test optics; no measured Old Holland data",
        },
        std::array::from_fn(|b| {
            std::array::from_fn(|i| {
                base.absorption()[b][i % 4] * (1. + 0.4 * (i / 4) as f64) + 0.004 * i as f64
            })
        }),
        std::array::from_fn(|b| {
            std::array::from_fn(|i| base.scattering()[b][i % 4] * (1. + 0.1 * i as f64))
        }),
    )
    .unwrap();
    PaletteMixerN::direct(palette).unwrap()
}

fn job<const N: usize>(mutate_rgb: bool) -> PaletteJobN<N> {
    let m = mixer::<N>();
    let mut geometry = oil_paint::testsheet::testsheet();
    let ground = m.paint("paint-3").unwrap();
    let main = m.recipe(std::array::from_fn(|i| (i + 1) as f64)).unwrap();
    let secondary = m.paint(&format!("paint-{}", N - 1)).unwrap();
    let mut dz = [0.; N];
    dz[N - 1] = 0.02;
    dz[0] = -0.02;
    let loads = vec![
        RecipeLoadN {
            main,
            secondary,
            dz
        };
        geometry.strokes.len()
    ];
    if mutate_rgb {
        geometry.ground = [0.; 3];
        for stroke in &mut geometry.strokes {
            stroke.color = [0.; 3];
            stroke.color2 = [1.; 3];
        }
    }
    PaletteJobN::new(m, geometry, ground, loads).unwrap()
}

fn replay<const N: usize>() {
    let original = job::<N>(false);
    let bytes = original.to_bytes().unwrap();
    assert_eq!(&bytes[..4], b"OPJ2");
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize,
        N
    );
    let restored = PaletteJobN::<N>::from_bytes(&bytes).unwrap();
    assert_eq!(bytes, restored.to_bytes().unwrap());
    assert_eq!(original.loads(), restored.loads());
    let (a, stats) = original.paint(64, |_, _| {}).unwrap();
    let (b, _) = restored.paint(64, |_, _| {}).unwrap();
    let (c, _) = job::<N>(true).paint(64, |_, _| {}).unwrap();
    let (final_only, final_stats) = restored.paint_final(64).unwrap();
    assert_eq!(stats.strokes, final_stats.strokes);
    assert_eq!(stats.painted_pixels, final_stats.painted_pixels);
    assert_eq!(stats.alpha.to_bits(), final_stats.alpha.to_bits());
    assert!(stats.painted_pixels > 0);
    for plane in PLANES {
        assert_eq!(plane_bytes(&a, plane), plane_bytes(&b, plane), "{plane}");
        assert_eq!(
            plane_bytes(&a, plane),
            plane_bytes(&final_only, plane),
            "deferred {plane}"
        );
        assert_eq!(
            plane_bytes(&a, plane),
            plane_bytes(&c, plane),
            "RGB fields rematched: {plane}"
        );
    }
    assert_eq!(a.hblur, b.hblur);
    assert_eq!(a.hblur, final_only.hblur);
    assert!(a
        .lat
        .iter()
        .all(|v| v.iter().all(|x| x.is_finite() && *x >= 0.)
            && (v.iter().sum::<f32>() - 1.).abs() < 1e-4));
    assert!(a.lat.iter().any(|v| v[N - 1] > 0.));
    assert_eq!(
        oil_kernel::Canvas::<PaletteMixerN<N>>::bytes_per_pixel(),
        28 + 4 * N
    );
    let mut state = original.mixer().recipe([1.; N]).unwrap();
    let mut direction = [0.; N];
    direction[0] = -0.5;
    direction[N - 1] = 0.5;
    for i in 0..1000 {
        original
            .mixer()
            .streak(&mut state, &direction, if i % 2 == 0 { 40. } else { -40. });
        assert!(original.mixer().is_valid(&state));
        assert!((state.iter().sum::<f32>() - 1.).abs() < 1e-5);
    }
}

#[test]
fn eight_ten_and_sixteen_paint_canvases_preserve_all_recipe_slots_and_replay() {
    replay::<8>();
    replay::<10>();
    replay::<16>();
}

fn checksum(bytes: &mut [u8]) {
    let end = bytes.len() - 32;
    let hash = Sha256::digest(&bytes[..end]);
    bytes[end..].copy_from_slice(&hash);
}

#[test]
fn replay_rejects_wrong_count_decoder_and_corrupt_last_slot() {
    let original = job::<8>(false).to_bytes().unwrap();
    assert!(PaletteJob::from_bytes(&original).is_err());
    assert!(PaletteJobN::<10>::from_bytes(&original).is_err());
    assert!(PaletteJobN::<8, true>::from_bytes(&original).is_err());
    let mut positions = vec![4, 8];
    let mut pos = 12;
    for _ in 0..3 {
        let n = u32::from_le_bytes(original[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4 + n;
    }
    positions.push(pos + 7 * 4); // Eighth ground component.
    positions.push(original.len() - 32 - 4); // Last streak component of the last stroke.
    for offset in positions {
        let mut bad = original.clone();
        bad[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        checksum(&mut bad);
        assert!(
            PaletteJobN::<8>::from_bytes(&bad).is_err(),
            "offset {offset}"
        );
    }
    let mut bad = original.clone();
    bad[40] ^= 1;
    assert!(PaletteJobN::<8>::from_bytes(&bad).is_err());
    assert!(PaletteJobN::<8>::from_bytes(&original[..original.len() - 1]).is_err());
}

#[test]
fn direct_decode_and_rgb_import_report_the_actual_selected_palette() {
    let m = mixer::<16>();
    let recipe = m.recipe([1.; 16]).unwrap();
    let expected = m
        .palette()
        .recipe(recipe.map(|v| v as f64))
        .unwrap()
        .decode_linear();
    let actual = m.decode_linear_rgb(&recipe);
    assert!(actual
        .iter()
        .zip(expected)
        .all(|(a, b)| (*a as f64 - b).abs() < 1e-6));
    let mut geometry = oil_paint::testsheet::testsheet();
    // Exercise every existing stroke's match/import path, including streaks.
    geometry.ground = [0.; 3];
    let (job, report) = PaletteJobN::<16>::from_rgb(m, geometry).unwrap();
    assert!(report.ground.error_ok100 > 0.);
    assert_eq!(report.strokes.len(), job.loads().len());
    for (load, found) in job.loads().iter().zip(report.strokes) {
        assert_eq!(load.main, found[0].recipe);
        assert_eq!(job.mixer().decode_srgb(&load.main), found[0].achieved_srgb);
    }
    assert!(PaletteJobN::<16>::from_bytes(&job.to_bytes().unwrap()).is_ok());
}
