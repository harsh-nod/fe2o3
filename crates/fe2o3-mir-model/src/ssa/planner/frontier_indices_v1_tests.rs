use super::*;

#[path = "frontier_indices_v1_tests/oracle.rs"]
pub(crate) mod oracle;

fn fixture(entry: u32, edges: &[&[u32]]) -> SsaConstructionInputV1 {
    SsaConstructionInputV1::new(
        SsaBlockIdV1::new(entry),
        0,
        vec![],
        vec![],
        edges
            .iter()
            .map(|targets| {
                SsaBlockInputV1::new(
                    vec![],
                    targets
                        .iter()
                        .enumerate()
                        .map(|(i, &target)| {
                            SsaEdgeInputV1::new(
                                SsaEdgeRoleV1::new((i + 1) as u16),
                                SsaBlockIdV1::new(target),
                                vec![],
                            )
                        })
                        .collect(),
                )
            })
            .collect(),
    )
}

fn compare_frontiers(input: &SsaConstructionInputV1) -> oracle::Oracle {
    let expected = oracle::inspect(input);
    let mut planner = Planner::new(input, SsaPlannerLimitsV1::default()).unwrap();
    planner.compute_reachability_and_order().unwrap();
    planner.collect_facts().unwrap();
    planner.solve_liveness().unwrap();
    let parents = planner.compute_immediate_dominators().unwrap();
    assert_eq!(parents, expected.parents);
    let before = planner.storage_words;
    let frontiers = planner.compute_dominance_frontiers(&parents).unwrap();
    let expected_words = expected
        .capacities
        .iter()
        .map(|c| (c * size_of::<SsaBlockIdV1>()).div_ceil(8))
        .sum::<usize>();
    assert_eq!(planner.storage_words - before, expected_words);
    for (block, row) in frontiers.iter().enumerate() {
        assert_eq!(
            row.iter().map(|id| id.get()).collect::<Vec<_>>(),
            expected.frontiers[block]
        );
        assert_eq!(row.capacity(), expected.capacities[block]);
    }
    expected
}

#[test]
fn block_id_layout_and_order_preserve_full_u32_domain() {
    assert_eq!(size_of::<SsaBlockIdV1>(), size_of::<u32>());
    let ids = [u32::MAX, 0, 65_536, u32::MAX - 1, 1, 65_535];
    let mut wide = ids.map(|id| id as usize);
    let mut compact = ids.map(SsaBlockIdV1::new);
    wide.sort_unstable();
    compact.sort_unstable();
    assert_eq!(compact.map(|id| id.get() as usize), wide);
}

#[test]
fn independent_frontiers_cover_entry_cycles_irreducible_and_unreachable_blocks() {
    for input in [
        fixture(0, &[&[]]),
        fixture(0, &[&[0]]),
        fixture(0, &[&[1], &[0]]),
        fixture(0, &[&[1, 2], &[2, 3], &[1, 3], &[]]),
        fixture(2, &[&[0, 1, 2], &[1], &[3, 4], &[5], &[5], &[2]]),
        fixture(0, &[&[1, 1, 2], &[3], &[3], &[1, 3]]),
    ] {
        compare_frontiers(&input);
        let plan = plan_ssa_v1(&input).unwrap();
        plan.verify_replay(&input, SsaPlannerLimitsV1::default())
            .unwrap();
    }
    let result = compare_frontiers(&fixture(0, &[&[1], &[0]]));
    assert_eq!(result.frontiers, vec![vec![0], vec![0]]);
    assert_eq!(result.capacities, vec![2, 1]);
}

#[test]
fn bounded_random_graphs_match_independent_dominator_sets() {
    let mut state = 0x794e_f960_592b_28a3_u64;
    for blocks in [1, 2, 5, 9, 17] {
        for _ in 0..32 {
            let mut rows = vec![Vec::new(); blocks];
            for row in &mut rows {
                for _ in 0..4 {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    if state % 5 != 0 {
                        row.push((state as usize % blocks) as u32);
                    }
                }
            }
            let edges = rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
            compare_frontiers(&fixture((state as usize % blocks) as u32, &edges));
        }
    }
}

#[test]
fn capacity_charge_retains_duplicates_and_odd_word_rounding() {
    for copies in [0, 1, 2, 3, 4, 7, 8, 31, 32, 65] {
        let targets = vec![1; copies];
        let input = fixture(0, &[&targets, &[]]);
        let expected = compare_frontiers(&input);
        assert!(expected.frontiers.iter().all(Vec::is_empty));
        assert_eq!(expected.capacities, vec![copies, 0]);
        assert_eq!(
            oracle::saving(&input),
            (copies * size_of::<usize>()).div_ceil(8) - copies.div_ceil(2)
        );
    }
}

#[test]
fn parallel_normal_unwind_definitions_remain_distinct() {
    let variable = SsaVariableIdV1::new(0);
    let input = SsaConstructionInputV1::new(
        SsaBlockIdV1::new(0),
        1,
        vec![true],
        vec![],
        vec![
            SsaBlockInputV1::new(
                vec![],
                vec![
                    SsaEdgeInputV1::new(
                        SsaEdgeRoleV1::new(1),
                        SsaBlockIdV1::new(1),
                        vec![variable],
                    ),
                    SsaEdgeInputV1::new(
                        SsaEdgeRoleV1::new(2),
                        SsaBlockIdV1::new(1),
                        vec![variable],
                    ),
                ],
            ),
            SsaBlockInputV1::new(vec![SsaEventV1::Use(variable)], vec![]),
        ],
    );
    let plan = plan_ssa_v1(&input).unwrap();
    assert_eq!(
        plan.merge_variables(SsaBlockIdV1::new(1)).unwrap(),
        &[variable]
    );
    let normal = SsaEdgeIdV1::new(input.entry(), 0);
    let unwind = SsaEdgeIdV1::new(input.entry(), 1);
    assert_ne!(plan.edge_definitions(normal), plan.edge_definitions(unwind));
    assert_ne!(plan.edge_arguments(normal), plan.edge_arguments(unwind));
    let mut bad = input.clone();
    bad.blocks[0].edges[1].definitions.clear();
    assert!(
        matches!(plan_ssa_v1(&bad), Err(SsaPlannerErrorV1::UndefinedAtEdge { edge, .. }) if edge == unwind)
    );
    assert!(
        plan.verify_replay(&bad, SsaPlannerLimitsV1::default())
            .is_err()
    );
}

#[test]
fn exact_storage_and_work_boundaries_still_reject_one_below() {
    let input = fixture(0, &[&[1, 1, 2], &[3], &[3], &[0, 3]]);
    let plan = plan_ssa_v1(&input).unwrap();
    let limits = SsaPlannerLimitsV1::default();
    let at = SsaPlannerLimitsV1 {
        max_storage_words: plan.resources().storage_words(),
        max_work_units: plan.resources().work_units(),
        ..limits
    };
    assert_eq!(plan_ssa_with_limits_v1(&input, at).unwrap(), plan);
    assert!(matches!(
        plan_ssa_with_limits_v1(
            &input,
            SsaPlannerLimitsV1 {
                max_storage_words: at.max_storage_words - 1,
                ..at
            }
        ),
        Err(SsaPlannerErrorV1::ResourceLimitExceeded {
            resource: SsaPlannerResourceV1::StorageWords,
            ..
        })
    ));
    assert!(matches!(
        plan_ssa_with_limits_v1(
            &input,
            SsaPlannerLimitsV1 {
                max_work_units: at.max_work_units - 1,
                ..at
            }
        ),
        Err(SsaPlannerErrorV1::ResourceLimitExceeded {
            resource: SsaPlannerResourceV1::WorkUnits,
            ..
        })
    ));
}
