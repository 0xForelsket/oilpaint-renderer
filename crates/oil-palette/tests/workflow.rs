use oil_mix::{
    palette::{synthetic_four, AmountBasis, Palette, PaletteLut, PaletteMetadata},
    Mixer,
};
use oil_paint::{plane_bytes, PLANES};
use oil_palette::{PaletteJob, PaletteMixer, RecipeLoad};
use sha2::{Digest, Sha256};

fn mixer(custom: bool) -> PaletteMixer {
    let builtin = synthetic_four();
    let mut k = *builtin.absorption();
    let mut s = *builtin.scattering();
    for i in 0..81 {
        k[i][3] *= 2.0;
        s[i][3] *= 2.0;
    }
    let changed = Palette::from_optics(
        PaletteMetadata {
            id: "test-strong-white",
            paint_names: builtin.paint_names(),
            amount_basis: AmountBasis::Relative,
            provenance: "Synthetic integration test",
        },
        k,
        s,
    )
    .unwrap();
    PaletteMixer::new(
        PaletteLut::build(if custom { &changed } else { builtin }, 17)
            .unwrap()
            .into_owned(),
    )
    .unwrap()
}
fn explicit(custom: bool, mutate_rgb: bool) -> PaletteJob {
    let m = mixer(custom);
    let mut list = oil_paint::testsheet::testsheet();
    let main = m.recipe([1.0, 0.0, 0.3, 0.5]).unwrap();
    let secondary = m.paint("blue").unwrap();
    let loads = vec![
        RecipeLoad {
            main,
            secondary,
            dz: [0.02, 0.0, -0.02, 0.0]
        };
        list.strokes.len()
    ];
    if mutate_rgb {
        list.ground = [0.0; 3];
        for s in &mut list.strokes {
            s.color = [0.0; 3];
            s.color2 = [1.0; 3];
            s.streak_amount = 0.0;
        }
    }
    let ground = m.paint("white").unwrap();
    PaletteJob::new(m, list, ground, loads).unwrap()
}
#[test]
fn saved_recipes_replay_all_canvas_planes_exactly() {
    let job = explicit(false, false);
    let bytes = job.to_bytes().unwrap();
    let restored = PaletteJob::from_bytes(&bytes).unwrap();
    assert_eq!(bytes, restored.to_bytes().unwrap());
    assert_eq!(job.loads(), restored.loads());
    let (original, stats) = job.paint(192, |_, _| {}).unwrap();
    let (replay, again) = restored.paint(192, |_, _| {}).unwrap();
    assert_eq!(stats.strokes, job.loads().len());
    assert!(stats.painted_pixels > 0);
    assert_eq!(stats.alpha.to_bits(), again.alpha.to_bits());
    for plane in PLANES {
        assert_eq!(
            plane_bytes(&original, plane),
            plane_bytes(&replay, plane),
            "{plane}"
        );
    }
    assert_eq!(original.hblur, replay.hblur);
    assert!(original.lat.iter().all(|z| job.mixer().is_valid(z)));
    assert!(original
        .lat
        .iter()
        .all(|z| (z.iter().sum::<f32>() - 1.0).abs() < 1e-4));
    assert_eq!(oil_kernel::Canvas::<PaletteMixer>::bytes_per_pixel(), 44);
    assert!(job.paint(0, |_, _| {}).is_err());
}
#[test]
fn explicit_painting_never_rematches_rgb_and_custom_optics_change_tints() {
    let a = explicit(false, false).paint(96, |_, _| {}).unwrap().0;
    let b = explicit(false, true).paint(96, |_, _| {}).unwrap().0;
    let custom = explicit(true, false).paint(96, |_, _| {}).unwrap().0;
    for plane in PLANES {
        assert_eq!(plane_bytes(&a, plane), plane_bytes(&b, plane));
    }
    assert_ne!(plane_bytes(&a, "rgb"), plane_bytes(&custom, "rgb"));
    // Every recipe and transport decision is the same; only optical decoding changes.
    assert_eq!(plane_bytes(&a, "lat"), plane_bytes(&custom, "lat"));
}
#[test]
fn rgb_import_exposes_gamut_error_and_saves_streaks() {
    let mut list = oil_paint::testsheet::testsheet();
    list.ground = [0.0; 3];
    let (job, report) = PaletteJob::from_rgb(mixer(false), list).unwrap();
    assert!(report.ground.error_ok100 > 40.0);
    assert_eq!(report.strokes.len(), job.loads().len());
    assert!(job.loads().iter().any(|l| l.dz != [0.0; 4]));
    assert_eq!(job.ground(), report.ground.recipe);
    for (load, matched) in job.loads().iter().zip(report.strokes) {
        assert_eq!(load.main, matched[0].recipe);
        assert_eq!(load.secondary, matched[1].recipe);
    }
    assert!(PaletteJob::from_bytes(&job.to_bytes().unwrap()).is_ok());
}
fn checksum(bytes: &mut [u8]) {
    let end = bytes.len() - 32;
    let hash = Sha256::digest(&bytes[..end]);
    bytes[end..].copy_from_slice(&hash);
}
#[test]
fn replay_rejects_corruption_lengths_wrong_engine_and_bad_materials() {
    let job = explicit(false, false);
    let original = job.to_bytes().unwrap();
    for n in [0, 67, 100, original.len() - 1] {
        assert!(PaletteJob::from_bytes(&original[..n]).is_err());
    }
    let mut bytes = original.clone();
    bytes[80] ^= 1;
    assert!(PaletteJob::from_bytes(&bytes).is_err());
    bytes = original.clone();
    bytes[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    checksum(&mut bytes);
    assert!(PaletteJob::from_bytes(&bytes).is_err());
    let mut pos = 4;
    let mut sections = Vec::new();
    for _ in 0..3 {
        let n = u32::from_le_bytes(original[pos..pos + 4].try_into().unwrap()) as usize;
        sections.push((pos + 4, n));
        pos += 4 + n;
    }
    bytes = original.clone();
    bytes[pos..pos + 4].copy_from_slice(&f32::NAN.to_le_bytes());
    checksum(&mut bytes);
    assert!(PaletteJob::from_bytes(&bytes).is_err());
    bytes = original.clone();
    bytes[pos + 16..pos + 20].copy_from_slice(&u32::MAX.to_le_bytes());
    checksum(&mut bytes);
    assert!(PaletteJob::from_bytes(&bytes).is_err());
    // Re-checksummed wrong palette identity in the LUT still fails its own binding.
    let other = mixer(true);
    assert!(
        PaletteMixer::from_bytes(&other.palette().to_bytes(), &job.mixer().table().to_bytes())
            .is_err()
    );
    let geometry = job.geometry().to_bytes_with_version("9.9.9");
    assert_eq!(geometry.len(), sections[2].1);
    bytes = original.clone();
    bytes[sections[2].0..sections[2].0 + sections[2].1].copy_from_slice(&geometry);
    checksum(&mut bytes);
    let error = match PaletteJob::from_bytes(&bytes) {
        Ok(_) => panic!("wrong engine accepted"),
        Err(e) => e,
    };
    assert!(error.to_string().contains("ENGINE_VERSION_MISMATCH"));
}
