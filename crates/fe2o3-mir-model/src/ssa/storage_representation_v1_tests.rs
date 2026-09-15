use super::*;
#[path = "storage_representation_v1_tests/golden.rs"]
mod golden;
use golden::compare;
use std::num::NonZeroU32;

fn variable(index: usize) -> SsaVariableIdV1 {
    SsaVariableIdV1::new(index as u32)
}

fn edge(role: u16, target: usize, definitions: &[usize]) -> SsaEdgeInputV1 {
    SsaEdgeInputV1::new(
        SsaEdgeRoleV1::new(role),
        SsaBlockIdV1::new(target as u32),
        definitions.iter().copied().map(variable).collect(),
    )
}

fn limits(storage: usize, work: usize) -> SsaPlannerLimitsV1 {
    let d = SsaPlannerLimitsV1::default();
    SsaPlannerLimitsV1::try_new(
        d.max_variables(),
        d.max_blocks(),
        d.max_edges(),
        d.max_events(),
        d.max_edge_definitions(),
        d.max_output_items(),
        storage,
        work,
    )
    .unwrap()
}

#[test]
fn differential_cycles_joins_kills_and_parallel_edges() {
    let mut state = 0x781246ab339d627fu64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut case = 0;
    let mut accepted = 0;
    let mut rejected = 0;
    for variables in [1, 2, 63, 64, 65, 129] {
        for blocks in [1, 2, 5, 17] {
            for trial in 0..16 {
                let mut sources = Vec::new();
                for block in 0..blocks {
                    let mut events = Vec::new();
                    for _ in 0..5 {
                        let variable = variable(next() as usize % variables);
                        match next() % 4 {
                            0 => events.push(SsaEventV1::Use(variable)),
                            1 => events.push(SsaEventV1::Define(variable)),
                            _ if trial % 2 == 0 => events
                                .extend([SsaEventV1::Kill(variable), SsaEventV1::Define(variable)]),
                            _ => events.push(SsaEventV1::Kill(variable)),
                        }
                    }
                    sources.push(SsaBlockInputV1::new(
                        events,
                        vec![
                            edge(1, (block + 1) % blocks, &[]),
                            edge(2, next() as usize % blocks, &[next() as usize % variables]),
                            edge(3, next() as usize % blocks, &[]),
                        ],
                    ));
                }
                let input = SsaConstructionInputV1::new(
                    SsaBlockIdV1::new(0),
                    variables as u32,
                    (0..variables).map(|id| id % 5 != 3).collect(),
                    (0..variables).map(variable).collect(),
                    sources,
                );
                if compare(case, &input).is_some() {
                    accepted += 1
                } else {
                    rejected += 1
                }
                case += 1;
            }
        }
    }
    assert_eq!(case, 384);
    assert!(accepted > 0 && rejected > 0);
}

fn merge_fixture() -> SsaConstructionInputV1 {
    let variables = 257;
    SsaConstructionInputV1::new(
        SsaBlockIdV1::new(0),
        variables,
        vec![true; variables as usize],
        (0..variables as usize).map(variable).collect(),
        vec![
            SsaBlockInputV1::new(vec![], vec![edge(1, 1, &[]), edge(2, 2, &[])]),
            SsaBlockInputV1::new(
                (0..variables as usize)
                    .map(|id| SsaEventV1::Define(variable(id)))
                    .collect(),
                vec![edge(1, 3, &[])],
            ),
            SsaBlockInputV1::new(vec![], vec![edge(1, 3, &[])]),
            SsaBlockInputV1::new(
                (0..variables as usize)
                    .map(|id| SsaEventV1::Use(variable(id)))
                    .collect(),
                vec![],
            ),
        ],
    )
}

#[test]
fn exact_storage_and_work_limits_remain_enforced() {
    let input = merge_fixture();
    let plan = compare(394, &input).unwrap();
    assert_eq!(
        plan.merge_variables(SsaBlockIdV1::new(3)).unwrap().len(),
        257
    );
    let storage = golden::expected_storage(394, &input);
    let work = golden::expected_work(394);
    assert_eq!(
        plan_ssa_with_limits_v1(&input, limits(storage, work)).unwrap(),
        plan
    );
    assert!(matches!(
        plan_ssa_with_limits_v1(&input, limits(storage - 1, work)),
        Err(SsaPlannerErrorV1::ResourceLimitExceeded {
            resource: SsaPlannerResourceV1::StorageWords,
            ..
        })
    ));
    let (old_required, old_limit) = golden::old_storage_failure();
    // The immutable trailer predates these exact physical representation savings.
    assert_eq!(
        old_limit,
        storage + 3 + golden::journal_saving(394, &input) + golden::frontier_saving(&input)
            + golden::incoming_edge_saving(&input)
    );
    assert!(old_required > old_limit);
    let d = SsaPlannerLimitsV1::default();
    let error =
        plan_ssa_with_limits_v1(&input, limits(d.max_storage_words(), work - 1)).unwrap_err();
    golden::assert_extra(0, &error);
}

#[test]
fn sparse_promotability_preserves_high_local_indices_and_empty_case() {
    for (case, count) in [0, 1, 63, 64, 65, HARD_MAX_SSA_VARIABLES_V1]
        .into_iter()
        .enumerate()
    {
        let promoted = (0..count)
            .filter(|id| *id < 2 || id % 1024 == 0 || id + 1 == count)
            .collect::<Vec<_>>();
        let input = SsaConstructionInputV1::new(
            SsaBlockIdV1::new(0),
            count as u32,
            (0..count)
                .map(|id| promoted.binary_search(&id).is_ok())
                .collect(),
            promoted.iter().copied().map(variable).collect(),
            vec![SsaBlockInputV1::new(
                promoted
                    .iter()
                    .copied()
                    .map(|id| SsaEventV1::Use(variable(id)))
                    .collect(),
                vec![],
            )],
        );
        compare(384 + case, &input).unwrap();
    }
}

#[test]
fn edge_definition_cannot_initialize_unwind_or_resurrect_killed_value() {
    for entry_defined in [false, true] {
        for kill in [false, true] {
            let mut use_events = vec![SsaEventV1::Use(variable(0))];
            if kill {
                use_events.insert(0, SsaEventV1::Kill(variable(0)));
            }
            let input = SsaConstructionInputV1::new(
                SsaBlockIdV1::new(0),
                2,
                vec![true, true],
                if entry_defined {
                    vec![variable(0)]
                } else {
                    vec![]
                },
                vec![
                    SsaBlockInputV1::new(vec![], vec![edge(1, 1, &[0]), edge(2, 2, &[])]),
                    SsaBlockInputV1::new(vec![SsaEventV1::Use(variable(0))], vec![]),
                    SsaBlockInputV1::new(use_events, vec![]),
                    SsaBlockInputV1::new(vec![SsaEventV1::Use(variable(1))], vec![]),
                ],
            );
            let case = 390 + 2 * usize::from(entry_defined) + usize::from(kill);
            assert_eq!(compare(case, &input).is_some(), entry_defined && !kill);
        }
    }
}

#[test]
fn changed_input_still_invalidates_replay_identity() {
    let input = merge_fixture();
    let plan = compare(394, &input).unwrap();
    let mut blocks = input.blocks().to_vec();
    blocks[0] = SsaBlockInputV1::new(vec![], vec![edge(4, 1, &[]), edge(2, 2, &[])]);
    let substituted = SsaConstructionInputV1::new(
        input.entry(),
        input.variable_count(),
        input.promotable().to_vec(),
        input.entry_definitions().to_vec(),
        blocks,
    );
    compare(395, &substituted).unwrap();
    let error = plan
        .verify_replay(&substituted, SsaPlannerLimitsV1::default())
        .unwrap_err();
    assert!(matches!(error, SsaPlannerErrorV1::ReplayMismatch { .. }));
    golden::assert_extra(32, &error);
}

#[test]
fn removed_allocations_have_smaller_physical_layouts() {
    assert_eq!(std::mem::size_of::<Option<NonZeroU32>>(), 4);
    assert!(std::mem::size_of::<Option<NonZeroU32>>() < std::mem::size_of::<Option<usize>>());
    assert_eq!(
        golden::old_plan_size() - std::mem::size_of::<SsaConstructionPlanV1>(),
        std::mem::size_of::<Vec<Vec<SsaVariableIdV1>>>()
    );
    let input = merge_fixture();
    let before = golden::old_storage(394);
    let after = compare(394, &input).unwrap();
    println!(
        "synthetic diamond V=257 B=4: planner words {} -> {}, unchanged work {}",
        before,
        after.resources().storage_words(),
        after.resources().work_units()
    );
}
