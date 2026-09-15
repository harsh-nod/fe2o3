use super::*;

fn edge(target: u32, ordinal: u16) -> SsaEdgeInputV1 {
    SsaEdgeInputV1::new(
        SsaEdgeRoleV1::new(ordinal + 1),
        SsaBlockIdV1::new(target),
        vec![SsaVariableIdV1::new(0)],
    )
}

fn split_fixture() -> SsaConstructionInputV1 {
    SsaConstructionInputV1::new(
        SsaBlockIdV1::new(3),
        1,
        vec![true],
        vec![],
        vec![
            SsaBlockInputV1::new(vec![SsaEventV1::Use(SsaVariableIdV1::new(0))], vec![]),
            SsaBlockInputV1::new(vec![SsaEventV1::Use(SsaVariableIdV1::new(0))], vec![]),
            SsaBlockInputV1::new(vec![], vec![edge(0, 0)]),
            SsaBlockInputV1::new(vec![], vec![edge(0, 0), edge(1, 1)]),
        ],
    )
}

#[test]
fn absent_slot_is_distinct_from_zero_source_and_ordinal() {
    assert_eq!(size_of::<IncomingEdge>(), size_of::<u64>());
    assert_eq!(size_of::<Option<IncomingEdge>>(), size_of::<u64>());
    let slots = [None, Some(IncomingEdge::new(0, 0).unwrap())];
    assert_ne!(slots[0], slots[1]);
    assert_eq!(slots[1].unwrap().indices(), (0, 0));
    if usize::BITS == 64 {
        assert_eq!(size_of::<Option<(usize, usize)>>(), 24);
        assert_eq!(size_of_val(&slots), 16);
    }
}

#[test]
fn coordinates_round_trip_without_source_or_ordinal_aliases() {
    let sources = [
        0,
        1,
        255,
        65_535,
        HARD_MAX_SSA_BLOCKS_V1 - 1,
        u32::MAX as usize,
    ];
    let ordinals = [0, 1, 255, HARD_MAX_SSA_EDGES_V1 - 1, u32::MAX as usize - 1];
    let mut pairs = Vec::new();
    for source in sources {
        for ordinal in ordinals {
            let packed = IncomingEdge::new(source, ordinal).unwrap();
            assert_eq!(packed.indices(), (source, ordinal));
            for &(previous, coordinates) in &pairs {
                assert_ne!(packed, previous);
                assert_ne!((source, ordinal), coordinates);
            }
            pairs.push((packed, (source, ordinal)));
        }
    }
    assert_eq!(
        IncomingEdge::new(0, u32::MAX as usize).unwrap().indices(),
        (0, u32::MAX as usize)
    );
}

#[test]
fn invalid_coordinates_fail_checked_before_any_alias() {
    assert_eq!(
        IncomingEdge::new(u32::MAX as usize, u32::MAX as usize),
        Err(SsaPlannerErrorV1::IdentityOverflow)
    );
    if let Some(wide) = (u32::MAX as usize).checked_add(1) {
        assert_eq!(
            IncomingEdge::new(wide, 0),
            Err(SsaPlannerErrorV1::IdentityOverflow)
        );
        assert_eq!(
            IncomingEdge::new(0, wide),
            Err(SsaPlannerErrorV1::IdentityOverflow)
        );
    }
}

#[test]
fn unique_edges_retain_exact_definitions_not_unreachable_predecessors() {
    let input = split_fixture();
    let plan = plan_ssa_v1(&input).unwrap();
    assert!(!plan.is_reachable(SsaBlockIdV1::new(2)));
    for ordinal in 0..2 {
        let target = SsaBlockIdV1::new(ordinal);
        let edge = SsaEdgeIdV1::new(input.entry(), ordinal);
        let definition = plan.edge_definitions(edge).unwrap()[0];
        assert_eq!(
            plan.resolved_event(target, 0),
            Some(&SsaResolvedEventV1::Use {
                variable: definition.variable(),
                value: definition.value(),
            })
        );
        assert!(plan.merge_variables(target).unwrap().is_empty());
    }
    assert_ne!(
        plan.edge_definitions(SsaEdgeIdV1::new(input.entry(), 0)),
        plan.edge_definitions(SsaEdgeIdV1::new(input.entry(), 1))
    );
    for ordinal in 0..2 {
        let mut bad = input.clone();
        bad.blocks[3].edges[ordinal].definitions.clear();
        assert!(matches!(
            plan_ssa_v1(&bad),
            Err(SsaPlannerErrorV1::UndefinedAtUse { .. })
        ));
        assert!(
            plan.verify_replay(&bad, SsaPlannerLimitsV1::default())
                .is_err()
        );
    }
    let mut substituted = input.clone();
    substituted.blocks[3].edges.swap(0, 1);
    assert!(matches!(
        plan.verify_replay(&substituted, SsaPlannerLimitsV1::default()),
        Err(SsaPlannerErrorV1::ReplayMismatch { .. })
    ));
}

#[test]
fn parallel_edges_and_entry_backedges_cannot_supply_missing_values() {
    let variable = SsaVariableIdV1::new(0);
    let mut input = split_fixture();
    input.blocks[3].edges[1].target = SsaBlockIdV1::new(0);
    let plan = plan_ssa_v1(&input).unwrap();
    assert_eq!(
        plan.merge_variables(SsaBlockIdV1::new(0)),
        Some(&[variable][..])
    );
    input.blocks[3].edges[1].definitions.clear();
    assert!(matches!(
        plan_ssa_v1(&input),
        Err(SsaPlannerErrorV1::UndefinedAtEdge { edge, .. })
            if edge == SsaEdgeIdV1::new(input.entry(), 1)
    ));
    let input = SsaConstructionInputV1::new(
        SsaBlockIdV1::new(0),
        1,
        vec![true],
        vec![],
        vec![SsaBlockInputV1::new(
            vec![SsaEventV1::Use(variable)],
            vec![edge(0, 0)],
        )],
    );
    assert_eq!(
        plan_ssa_v1(&input),
        Err(SsaPlannerErrorV1::UndefinedAtEntry { variable })
    );
}

#[test]
fn cyclic_unique_edges_retain_exact_storage_and_work_limits() {
    let mut input = split_fixture();
    input.blocks[0].edges.push(edge(3, 0));
    let plan = plan_ssa_v1(&input).unwrap();
    let limits = SsaPlannerLimitsV1 {
        max_storage_words: plan.resources().storage_words(),
        max_work_units: plan.resources().work_units(),
        ..SsaPlannerLimitsV1::default()
    };
    assert_eq!(plan_ssa_with_limits_v1(&input, limits).unwrap(), plan);
    for (storage, work, resource) in [
        (
            limits.max_storage_words - 1,
            limits.max_work_units,
            SsaPlannerResourceV1::StorageWords,
        ),
        (
            limits.max_storage_words,
            limits.max_work_units - 1,
            SsaPlannerResourceV1::WorkUnits,
        ),
    ] {
        assert!(matches!(
            plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1 {
                max_storage_words: storage, max_work_units: work, ..limits
            }),
            Err(SsaPlannerErrorV1::ResourceLimitExceeded { resource: got, .. }) if got == resource
        ));
    }
    assert_eq!(HARD_MAX_SSA_STORAGE_WORDS_V1, 2_097_152);
}
