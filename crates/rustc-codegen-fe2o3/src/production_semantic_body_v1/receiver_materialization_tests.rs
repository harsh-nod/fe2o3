use super::receiver_materialization_v1::ReceiverLocalV1;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBlockIdentityV1, SemanticFunctionIdentityV1, SemanticLocalIdV1, SemanticLocalIdentityV1,
};

fn function() -> SemanticFunctionIdentityV1 {
    SemanticFunctionIdentityV1::from_sha256([17; 32])
}

fn block() -> SemanticBlockIdentityV1 {
    SemanticBlockIdentityV1::from_sha256([34; 32])
}

fn surrounding_identities() -> ([SemanticLocalIdentityV1; 2], [SemanticLocalIdentityV1; 2]) {
    let mut second_lowest = [0; 32];
    second_lowest[31] = 1;
    let mut second_highest = [u8::MAX; 32];
    second_highest[31] = u8::MAX - 1;
    let lower = [
        SemanticLocalIdentityV1::from_sha256([0; 32]),
        SemanticLocalIdentityV1::from_sha256(second_lowest),
    ];
    let upper = [
        SemanticLocalIdentityV1::from_sha256(second_highest),
        SemanticLocalIdentityV1::from_sha256([u8::MAX; 32]),
    ];
    let receiver = ReceiverLocalV1::derive(function(), block(), []).unwrap();
    assert!(lower[1] < receiver.identity);
    assert!(receiver.identity < upper[0]);
    (lower, upper)
}

fn assert_placement(raw: &[SemanticLocalIdentityV1], expected_rank: u32) {
    let receiver = ReceiverLocalV1::derive(function(), block(), raw.iter().copied()).unwrap();
    assert_eq!(receiver.local, SemanticLocalIdV1::from_index(expected_rank));
    assert_eq!(
        receiver.identity,
        ReceiverLocalV1::derive(function(), block(), [])
            .unwrap()
            .identity,
    );

    // Original IDs are canonical ranks, not positions in the supplied iterator.
    let mut canonical_raw = raw.to_vec();
    canonical_raw.sort_unstable();
    let mut emitted = vec![(receiver.local, receiver.identity)];
    for (index, identity) in canonical_raw.iter().copied().enumerate() {
        let original = u32::try_from(index).unwrap();
        let mapped = receiver
            .remap(SemanticLocalIdV1::from_index(original))
            .unwrap();
        let expected = original + u32::from(original >= expected_rank);
        assert_eq!(mapped, SemanticLocalIdV1::from_index(expected));
        emitted.push((mapped, identity));
    }
    emitted.sort_unstable_by_key(|(local, _)| local.index());
    assert_eq!(emitted.len(), raw.len() + 1);
    for (index, (local, _)) in emitted.iter().enumerate() {
        assert_eq!(local.index(), u32::try_from(index).unwrap());
    }
    assert!(emitted.windows(2).all(|pair| pair[0].1 < pair[1].1));
    assert_eq!(
        emitted
            .iter()
            .filter_map(|(local, identity)| (*local != receiver.local).then_some(*identity))
            .collect::<Vec<_>>(),
        canonical_raw,
    );
}

#[test]
fn receiver_local_precedes_every_raw_local() {
    let (_, upper) = surrounding_identities();
    assert_placement(&upper, 0);
}

#[test]
fn receiver_local_splits_the_canonical_raw_order() {
    let (lower, upper) = surrounding_identities();
    assert_placement(&[upper[1], lower[0], upper[0], lower[1]], 2);
}

#[test]
fn receiver_local_follows_every_raw_local() {
    let (lower, _) = surrounding_identities();
    assert_placement(&lower, u32::try_from(lower.len()).unwrap());
}

#[test]
fn empty_raw_roster_places_the_receiver_at_zero() {
    assert_placement(&[], 0);
}

#[test]
fn receiver_placement_is_invariant_under_raw_identity_permutations() {
    let (lower, upper) = surrounding_identities();
    let [a, b, c] = [lower[0], lower[1], upper[0]];
    let expected = ReceiverLocalV1::derive(function(), block(), [a, b, c]).unwrap();
    for permutation in [
        [a, b, c],
        [a, c, b],
        [b, a, c],
        [b, c, a],
        [c, a, b],
        [c, b, a],
    ] {
        assert_eq!(
            ReceiverLocalV1::derive(function(), block(), permutation).unwrap(),
            expected,
        );
        assert_placement(&permutation, 2);
    }
}

#[test]
fn receiver_identity_collisions_are_rejected_at_every_iterator_position() {
    let receiver = ReceiverLocalV1::derive(function(), block(), []).unwrap();
    let (lower, upper) = surrounding_identities();
    assert_eq!(
        ReceiverLocalV1::derive(function(), block(), [receiver.identity]),
        Err("shared receiver local identity collision"),
    );
    for raw in [
        [receiver.identity, lower[0], upper[0]],
        [lower[0], receiver.identity, upper[0]],
        [lower[0], upper[0], receiver.identity],
    ] {
        assert_eq!(
            ReceiverLocalV1::derive(function(), block(), raw),
            Err("shared receiver local identity collision"),
        );
    }
}

#[test]
fn receiver_remapping_checks_overflow_at_the_shift_boundary() {
    let identity = SemanticLocalIdentityV1::from_sha256([85; 32]);
    let first = ReceiverLocalV1 {
        identity,
        local: SemanticLocalIdV1::from_index(0),
    };
    assert_eq!(
        first.remap(SemanticLocalIdV1::from_index(0)),
        Some(SemanticLocalIdV1::from_index(1)),
    );
    assert_eq!(
        first.remap(SemanticLocalIdV1::from_index(u32::MAX - 1)),
        Some(SemanticLocalIdV1::from_index(u32::MAX)),
    );
    assert_eq!(first.remap(SemanticLocalIdV1::from_index(u32::MAX)), None);

    let last = ReceiverLocalV1 {
        identity,
        local: SemanticLocalIdV1::from_index(u32::MAX),
    };
    for original in [0, u32::MAX - 1] {
        let original = SemanticLocalIdV1::from_index(original);
        assert_eq!(last.remap(original), Some(original));
    }
    assert_eq!(last.remap(SemanticLocalIdV1::from_index(u32::MAX)), None);
}

#[test]
fn receiver_identity_distinguishes_function_and_block_ownership() {
    let original = ReceiverLocalV1::derive(function(), block(), []).unwrap();
    let other_function = ReceiverLocalV1::derive(
        SemanticFunctionIdentityV1::from_sha256([18; 32]),
        block(),
        [],
    )
    .unwrap();
    let other_block = ReceiverLocalV1::derive(
        function(),
        SemanticBlockIdentityV1::from_sha256([35; 32]),
        [],
    )
    .unwrap();
    let swapped = ReceiverLocalV1::derive(
        SemanticFunctionIdentityV1::from_sha256([34; 32]),
        SemanticBlockIdentityV1::from_sha256([17; 32]),
        [],
    )
    .unwrap();
    let placements = [original, other_function, other_block, swapped];
    for (index, placement) in placements.iter().enumerate() {
        assert_eq!(placement.local, SemanticLocalIdV1::from_index(0));
        for previous in &placements[..index] {
            assert_ne!(placement.identity, previous.identity);
        }
    }
    assert_eq!(
        ReceiverLocalV1::derive(function(), block(), []).unwrap(),
        original,
    );
}
