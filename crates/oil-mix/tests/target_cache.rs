use oil_mix::palette::{
    synthetic_four, AmountBasis, Palette, PaletteLut, PaletteMetadata, PaletteMixer, PaletteMixerN,
    TargetMatchN,
};
use std::sync::Arc;

fn direct(stronger_white: bool) -> PaletteMixerN<4> {
    let p = synthetic_four();
    let factor = if stronger_white { 3. } else { 1. };
    let mut k = *p.absorption();
    let mut s = *p.scattering();
    for band in 0..81 {
        k[band][3] *= factor;
        s[band][3] *= factor;
    }
    let p = Palette::from_optics(
        PaletteMetadata {
            id: "cache-test",
            paint_names: p.paint_names(),
            amount_basis: AmountBasis::Relative,
            provenance: "synthetic cache test; deliberately shared text ID",
        },
        k,
        s,
    )
    .unwrap();
    PaletteMixerN::direct(p).unwrap()
}

fn bits<const N: usize>(x: TargetMatchN<N>) -> Vec<u64> {
    x.recipe
        .iter()
        .chain(x.achieved_srgb.iter())
        .map(|v| v.to_bits() as u64)
        .chain([
            x.error_ok100.to_bits(),
            x.reference_error_ok100.to_bits(),
            x.evaluations as u64,
        ])
        .collect()
}

#[test]
fn exact_targets_preserve_every_report_bit_and_reject_invalid_inputs() {
    let m = direct(false);
    let reference = direct(false).with_target_cache_capacity(0);
    let target = [0.3, 0.5, 0.7];
    let first = m.match_target(target).unwrap();
    assert_eq!(bits(first), bits(reference.match_target(target).unwrap()));
    assert_eq!(bits(first), bits(m.match_target(target).unwrap()));
    let mut adjacent = target;
    adjacent[0] = f32::from_bits(target[0].to_bits() + 1);
    assert_eq!(
        bits(m.match_target(adjacent).unwrap()),
        bits(reference.match_target(adjacent).unwrap())
    );
    let stats = m.target_cache_stats();
    assert_eq!((stats.hits, stats.misses, stats.entries), (1, 2, 2));
    for target in [
        [f32::NAN, 0., 0.],
        [f32::INFINITY, 0., 0.],
        [-0.1, 0., 0.],
        [1.01, 0., 0.],
    ] {
        assert!(m.match_target(target).is_err());
    }
    assert_eq!(m.target_cache_stats(), stats);
    assert_eq!(reference.target_cache_stats().entries, 0);
}

#[test]
fn caches_are_isolated_by_model_decoder_and_reload() {
    let a = direct(false);
    let b = direct(true);
    let input = [0.7, 0.5, 0.2];
    let saved = a.palette().to_bytes();
    a.match_target(input).unwrap();
    let found = b.match_target(input).unwrap();
    assert_eq!(b.target_cache_stats().misses, 1);
    assert_eq!(b.target_cache_stats().hits, 0);
    assert_eq!(
        bits(found),
        bits(
            direct(true)
                .with_target_cache_capacity(0)
                .match_target(input)
                .unwrap()
        )
    );
    assert_ne!(saved, b.palette().to_bytes());
    assert_eq!(saved, a.palette().to_bytes());
    let restored = PaletteMixerN::<4>::from_palette_bytes(&saved).unwrap();
    assert_eq!(restored.target_cache_stats().entries, 0);
    assert_eq!(
        bits(restored.match_target(input).unwrap()),
        bits(a.match_target(input).unwrap())
    );
    let prepared = PaletteMixer::new(
        PaletteLut::build(synthetic_four(), 17)
            .unwrap()
            .into_owned(),
    )
    .unwrap();
    let before = prepared.table().to_bytes();
    let x = prepared.match_target(input).unwrap();
    assert_eq!(bits(x), bits(prepared.match_target(input).unwrap()));
    assert_eq!(prepared.target_cache_stats().misses, 1);
    assert_eq!(before, prepared.table().to_bytes());
}

#[test]
fn bounded_fifo_eviction_and_clear_never_change_results() {
    let mut m = direct(false).with_target_cache_capacity(2);
    let a = [0.2, 0.4, 0.7];
    let b = [0.4, 0.6, 0.2];
    let c = [0.8, 0.3, 0.4];
    let original = m.match_target(a).unwrap();
    m.match_target(b).unwrap();
    m.match_target(a).unwrap();
    m.match_target(c).unwrap();
    let stats = m.target_cache_stats();
    assert_eq!(
        (stats.hits, stats.misses, stats.entries, stats.evictions),
        (1, 3, 2, 1)
    );
    assert_eq!(bits(original), bits(m.match_target(a).unwrap()));
    assert_eq!(m.target_cache_stats().misses, 4);
    m.clear_target_cache();
    let stats = m.target_cache_stats();
    assert_eq!(
        (
            stats.hits,
            stats.misses,
            stats.entries,
            stats.evictions,
            stats.capacity
        ),
        (0, 0, 0, 0, 2)
    );
    assert_eq!(bits(original), bits(m.match_target(a).unwrap()));
}

#[test]
fn shared_mixer_remains_send_sync_with_concurrent_hits_and_misses() {
    let m = Arc::new(direct(false));
    let target = [0.1, 0.4, 0.7];
    let expected = bits(m.match_target(target).unwrap());
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let m = Arc::clone(&m);
            let expected = expected.clone();
            scope.spawn(move || {
                for _ in 0..16 {
                    assert_eq!(expected, bits(m.match_target(target).unwrap()));
                }
                let another = [0.5, 0.4, 0.3];
                assert_eq!(
                    bits(m.match_target(another).unwrap()),
                    bits(m.match_target(another).unwrap())
                );
            });
        }
    });
    let stats = m.target_cache_stats();
    assert_eq!(stats.entries, 2);
    assert_eq!(stats.hits + stats.misses, 73);
}
