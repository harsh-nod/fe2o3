use super::*;

fn fixture(statements: &[usize]) -> ExecutionEventOriginsV1 {
    let mut offsets = ExecutionEventOriginsV1::zeroed(statements.len() + 1).unwrap();
    for (index, length) in statements.iter().enumerate() {
        offsets[index + 1] = offsets[index] + *length as u32 + 1;
    }
    let ends = ExecutionEventOriginsV1::zeroed(*offsets.last().unwrap() as usize).unwrap();
    ExecutionEventOriginsV1 {
        offsets,
        ends,
        ..ExecutionEventOriginsV1::default()
    }
}

fn digest(value: &ExecutionEventOriginsV1) -> [u8; 32] {
    let mut digest = Sha256::new();
    value.hash_into(&mut digest);
    digest.finalize().into()
}

fn complete(value: &mut ExecutionEventOriginsV1) {
    for block in 0..value.offsets.len() - 1 {
        let count = (value.offsets[block + 1] - value.offsets[block]) as usize;
        for site in 0..count {
            value.record(
                block as u32,
                if site + 1 == count {
                    None
                } else {
                    Some(site as u32)
                },
                site..site + 1,
            );
        }
    }
}

#[test]
fn retained_source_storage_exact_spans_empty_sites_and_u32_endpoints() {
    let mut origins = fixture(&[4, 0]);
    origins.record(0, Some(0), 0..0);
    origins.record(0, Some(1), 0..3);
    origins.record(0, Some(2), 3..3);
    origins.record(0, Some(3), 3..4);
    origins.record(0, None, 4..4);
    origins.record(1, None, 0..u32::MAX as usize);
    origins.resources().unwrap();
    assert_eq!(origins.block_ends(0), Some([0, 3, 3, 4, 4].as_slice()));
    for event in 0..3 {
        assert_eq!(origins.statement(0, event), Some(Some(1)));
    }
    assert_eq!(origins.statement(0, 3), Some(Some(3)));
    assert_eq!(origins.statement(0, 4), None);
    assert_eq!(origins.statement(1, u32::MAX - 1), Some(None));
    assert_eq!(origins.statement(1, u32::MAX), None);
    assert_eq!(origins.statement(u32::MAX, 0), None);
}

#[test]
fn retained_source_storage_incomplete_duplicate_gap_reordered_and_max_tag_reject() {
    for mutation in 0..9 {
        let mut origins = fixture(&[1]);
        match mutation {
            0 => {}
            1 => origins.record(0, Some(0), 0..1),
            2 => origins.record(0, None, 0..1),
            3 => origins.record(0, Some(u32::MAX), 0..1),
            4 => origins.record(u32::MAX, Some(0), 0..1),
            5 => origins.record(0, Some(0), 1..2),
            6 => {
                origins.record(0, Some(0), 0..2);
                origins.record(0, None, 2..1);
            }
            7 => {
                origins.record(0, Some(0), 0..2);
                origins.record(0, Some(0), 2..3);
            }
            8 => {
                complete(&mut origins);
                origins.record(0, None, 2..3);
            }
            _ => unreachable!(),
        }
        assert_eq!(
            origins.resources(),
            Err(ProductionSemanticSsaErrorV1::ReplayMismatch),
            "{mutation}"
        );
    }
    if let Some(too_large) = (u32::MAX as usize).checked_add(1) {
        let mut origins = fixture(&[0]);
        origins.record(0, None, 0..too_large);
        assert_eq!(
            origins.resources(),
            Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
        );
    }
}

#[test]
fn retained_source_storage_no_growth_retained_replay_peak_is_exactly_reserved() {
    for counts in [vec![], vec![0], vec![1, 4, 0, 17], vec![6; 4272]] {
        let mut origins = fixture(&counts);
        complete(&mut origins);
        let reserved = origins.resources().unwrap();
        let word = std::mem::size_of::<usize>();
        let arrays = |value: &ExecutionEventOriginsV1| {
            (value.offsets.capacity() * 4).div_ceil(word)
                + (value.ends.capacity() * 4).div_ceil(word)
        };
        let mut replay = fixture(&counts);
        let capacities = (replay.offsets.capacity(), replay.ends.capacity());
        assert_eq!(
            reserved.storage_words,
            arrays(&origins)
                + arrays(&replay)
                + 2 * std::mem::size_of::<ExecutionEventOriginsV1>().div_ceil(word)
        );
        complete(&mut replay);
        assert_eq!(
            capacities,
            (replay.offsets.capacity(), replay.ends.capacity())
        );
        assert_eq!(origins, replay);
        assert_eq!(reserved, replay.resources().unwrap());
        assert_eq!(
            reserved.work_units,
            12 * (counts.len() + counts.iter().sum::<usize>() + counts.len() + 1)
        );
    }
}

#[test]
fn retained_source_storage_same_length_offsets_endpoints_and_cursors_are_hashed() {
    let mut original = fixture(&[2, 2]);
    complete(&mut original);
    let hash = digest(&original);
    for mutation in 0..6 {
        let mut changed = original.clone();
        match mutation {
            0 => changed.offsets[1] += 1,
            1 => changed.ends[1] = changed.ends[0],
            2 => changed.ends[2] += 1,
            3 => changed.next -= 1,
            4 => changed.block -= 1,
            5 => changed.invalid = true,
            _ => unreachable!(),
        }
        assert_ne!(digest(&changed), hash, "{mutation}");
        assert_ne!(changed, original);
    }
    assert_eq!(
        ExecutionEventOriginsV1::reservation(usize::MAX, 1),
        Err(ProductionSemanticSsaErrorV1::ResourceOverflow)
    );
    assert_eq!(
        ExecutionEventOriginsV1::reservation(1, usize::MAX),
        Err(ProductionSemanticSsaErrorV1::ResourceOverflow)
    );
}

#[test]
fn retained_value_origins_same_length_endpoint_and_site_substitutions_reject_replay() {
    for expanded in [false, true] {
        for mutation in 0..3 {
            let mut owner = frame_initialization::tests::owner(false);
            let origins = if expanded {
                &mut owner.execution.plans[0].1.event_origins
            } else {
                &mut owner.source_plans[0].event_origins
            };
            let sizes = (origins.offsets.len(), origins.ends.len());
            let before = digest(origins);
            match mutation {
                0 => origins.ends[0] ^= 1,
                1 => origins.offsets[1] ^= 1,
                _ => origins.next -= 1,
            }
            assert_eq!(sizes, (origins.offsets.len(), origins.ends.len()));
            assert_ne!(before, digest(origins));
            assert_eq!(
                owner.verify_replay(),
                Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
            );
        }
    }
}
