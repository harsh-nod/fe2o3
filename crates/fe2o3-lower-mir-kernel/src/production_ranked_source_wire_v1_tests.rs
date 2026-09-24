use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_pliron::ProductionRankedValueIdV1 as Id;

const FLOOR: usize = 37;
const LIMIT: usize = 1_000_000;

fn inputs() -> ([Access; 1], [Effect; 1]) {
    (
        [
            Access::new(0x04030201, Some(5), 6, 7, 8).with_output_extent(Extent::new(
                0,
                Value::Argument(0),
                Value::BlockArgument {
                    block: 11,
                    argument: 12,
                },
                Value::Local(Id::new(13)),
            )),
        ],
        [Effect::new(
            14,
            15,
            16,
            17,
            Origin::GeneratedFromSemanticTerminator,
            [18; 32],
        )],
    )
}

fn encode(access: &[Access], effects: &[Effect]) -> Vec<u8> {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    encode_production_ranked_source_rows_v1(access, effects, &mut budget)
        .unwrap()
        .0
}

fn decode(bytes: &[u8]) -> Result<ProductionRankedSourceRowsV1, E> {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    assert!(budget.reserve_storage(usize::MAX).is_err());
    let result = decode_production_ranked_source_rows_v1(bytes, &mut budget);
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.failed_storage(), Some(usize::MAX));
    result.map(|(owner, _)| owner)
}

#[test]
fn literal_golden_includes_zero_source_argument_and_all_value_tags() {
    let mut golden = vec![
        70, 69, 50, 79, 51, 82, 83, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 2, 3, 4, 1, 5, 0, 0, 0, 6, 0, 0,
        0, 7, 0, 0, 0, 8, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 2, 11, 0, 0, 0, 12, 0, 0, 0, 3,
        13, 0, 0, 0, 1, 0, 0, 0, 14, 0, 0, 0, 15, 0, 0, 0, 16, 0, 0, 0, 17, 0, 0, 0, 1,
    ];
    golden.extend_from_slice(&[18; 32]);
    let (access, effects) = inputs();
    assert_eq!(encode(&access, &effects), golden);
    let rows = decode(&golden).unwrap();
    assert_eq!(rows.access_sources(), access);
    assert_eq!(rows.executable_effect_sources(), effects);
    assert_eq!(
        encode(rows.access_sources(), rows.executable_effect_sources()),
        golden
    );
}

#[test]
fn empty_and_every_extent_position_round_trip_without_normalization() {
    let empty = [
        70, 69, 50, 79, 51, 82, 83, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    assert_eq!(encode(&[], &[]), empty);
    assert!(decode(&empty).unwrap().access_sources().is_empty());
    let values = [
        Value::Argument(u32::MAX),
        Value::BlockArgument {
            block: u32::MAX,
            argument: 0,
        },
        Value::Local(Id::new(u32::MAX)),
    ];
    for statement in [None, Some(0), Some(u32::MAX)] {
        for view in values {
            for extent in values {
                for index in values {
                    let base = Access::new(u32::MAX, statement, 0, u32::MAX, 3);
                    let rows = vec![
                        base.with_output_extent(Extent::new(0, view, extent, index)),
                        base,
                        base,
                    ];
                    let bytes = encode(&rows, &[]);
                    drop(rows);
                    let decoded = decode(&bytes).unwrap();
                    assert_eq!(
                        decoded.access_sources()[0].output_extent(),
                        Some(Extent::new(0, view, extent, index))
                    );
                    assert_eq!(decoded.access_sources()[1..], [base, base]);
                    assert_eq!(encode(decoded.access_sources(), &[]), bytes);
                }
            }
        }
    }
    // Semantic-invalid effects remain inert: replay, not parsing, rejects them.
    let effect = Effect::new(9, 4, 8, 7, Origin::GeneratedFromSemanticTerminator, [0; 32]);
    let distinct = Effect::new(1, 0, 2, 3, Origin::GeneratedFromSemanticTerminator, [4; 32]);
    let bytes = encode(&[], &[effect, distinct]);
    let rows = decode(&bytes).unwrap();
    assert_eq!(rows.executable_effect_sources(), [effect, distinct]);
    assert_eq!(encode(&[], rows.executable_effect_sources()), bytes);
    assert_eq!(
        decode(&encode(&[], &[effect, effect]))
            .unwrap()
            .executable_effect_sources(),
        [effect, effect]
    );
}

#[test]
fn malformed_framing_tags_counts_and_every_truncation_fail_closed() {
    let (access, effects) = inputs();
    let bytes = encode(&access, &effects);
    for end in 0..bytes.len() {
        assert!(decode(&bytes[..end]).is_err(), "prefix {end}");
    }
    for (offset, value) in [
        (0, 0),
        (8, 2),
        (10, 1),
        (20, 2),
        (37, 2),
        (42, 0),
        (47, 4),
        (56, 255),
        (81, 2),
    ] {
        let mut changed = bytes.clone();
        changed[offset] = value;
        assert!(decode(&changed).is_err(), "offset {offset}");
    }
    for offset in [12, 61] {
        for count in [2u32, u32::MAX] {
            let mut changed = bytes.clone();
            changed[offset..offset + 4].copy_from_slice(&count.to_le_bytes());
            assert!(decode(&changed).is_err(), "count at {offset}");
        }
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(matches!(
        decode(&trailing),
        Err(E::Invalid("trailing bytes"))
    ));
    let mut empty = encode(&[], &[]);
    empty[12..16].copy_from_slice(&1024u32.to_le_bytes());
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, size_of::<ProductionRankedSourceRowsV1>());
    assert!(matches!(
        decode_production_ranked_source_rows_v1(&empty, &mut budget),
        Err(E::Invalid("row extent"))
    ));
    assert_eq!(budget.failed_storage(), None);
    assert_eq!(budget.storage(), 0);
    assert!(matches!(
        check_counts(MAX_SOURCE_OPERATIONS, 1),
        Err(E::Invalid("row count"))
    ));
    assert!(matches!(
        check_counts(usize::MAX, 1),
        Err(E::Resource(Resource::Arithmetic))
    ));
}

#[test]
fn exact_independent_work_and_storage_oracles_and_all_one_short_limits() {
    let (access, effects) = inputs();
    let bytes = encode(&access, &effects);
    assert_eq!(bytes.len(), 114);
    // Header 20; access 29 + (u32 5 + value writes 13+22+13);
    // effect count 5; effect 4*u32 5 + tag 2 + digest 33.
    let encode_work = 1 + 2 * (20 + 29 + 5 + 13 + 22 + 13 + 5 + 55) + 114;
    let decode_work = 1 + 20 + 29 + 5 + 7 + 12 + 7 + 5 + 55;
    let retained =
        size_of::<ProductionRankedSourceRowsV1>() + size_of::<Access>() + size_of::<Effect>();
    for encoding in [false, true] {
        let work = if encoding { encode_work } else { decode_work };
        let peak = if encoding { 9 + 114 } else { retained };
        for (work_limit, storage_limit, succeeds) in [
            (work, FLOOR + peak, true),
            (work - 1, FLOOR + peak, false),
            (work, FLOOR + peak - 1, false),
        ] {
            let mut ledger = Work::new(work_limit);
            let mut budget = Budget::new(&mut ledger, storage_limit);
            budget.reserve_storage(FLOOR).unwrap();
            assert!(budget.reserve_storage(usize::MAX).is_err());
            let result = if encoding {
                encode_production_ranked_source_rows_v1(&access, &effects, &mut budget).map(
                    |(bytes, receipt)| {
                        assert_eq!(receipt.retained_storage(), bytes.capacity());
                    },
                )
            } else {
                decode_production_ranked_source_rows_v1(&bytes, &mut budget).map(
                    |(rows, receipt)| {
                        assert_eq!(receipt.retained_storage(), retained);
                        assert_eq!(rows.access.capacity(), 1);
                        assert_eq!(rows.effects.capacity(), 1);
                    },
                )
            };
            assert_eq!(result.is_ok(), succeeds, "encoding={encoding} {result:?}");
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.failed_storage(), Some(usize::MAX));
            if succeeds {
                assert_eq!(budget.work(), work);
                assert_eq!(budget.peak_storage(), FLOOR + peak);
            }
        }
    }
}

#[test]
fn nested_owners_and_unwinds_preserve_the_inherited_floor() {
    let (access, effects) = inputs();
    let bytes = encode(&access, &effects);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let (first, first_storage) =
        decode_production_ranked_source_rows_v1(&bytes, &mut budget).unwrap();
    budget
        .reserve_storage(first_storage.retained_storage())
        .unwrap();
    let (second, second_storage) =
        decode_production_ranked_source_rows_v1(&bytes, &mut budget).unwrap();
    budget
        .reserve_storage(second_storage.retained_storage())
        .unwrap();
    let floor = budget.storage();
    let panic = catch_unwind(AssertUnwindSafe(|| {
        scoped::<()>(&mut budget, |budget| {
            budget.reserve_storage(19)?;
            budget.charge_work(7)?;
            std::panic::panic_any(83u32)
        })
    }))
    .unwrap_err();
    assert_eq!(*panic.downcast::<u32>().unwrap(), 83);
    assert_eq!(budget.storage(), floor);
    assert_eq!(first.access_sources(), second.access_sources());
    drop((first, second));
    budget
        .release_storage(first_storage.retained_storage() + second_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
