use fe2o3_mir_model::{
    SsaBlockIdV1, SsaBlockInputV1, SsaConstructionInputV1, SsaPlannerErrorV1, SsaPlannerLimitsV1,
    SsaPlannerResourceV1, plan_ssa_v1, plan_ssa_with_limits_v1,
};

fn input() -> SsaConstructionInputV1 {
    SsaConstructionInputV1::new(
        SsaBlockIdV1::new(0),
        1,
        vec![true],
        vec![],
        vec![SsaBlockInputV1::new(vec![], vec![])],
    )
}

fn limits(storage: usize) -> SsaPlannerLimitsV1 {
    let d = SsaPlannerLimitsV1::default();
    SsaPlannerLimitsV1::try_new(
        d.max_variables(),
        d.max_blocks(),
        d.max_edges(),
        d.max_events(),
        d.max_edge_definitions(),
        d.max_output_items(),
        storage,
        d.max_work_units(),
    )
    .unwrap()
}

fn storage_error(required: usize, limit: usize) -> SsaPlannerErrorV1 {
    SsaPlannerErrorV1::ResourceLimitExceeded {
        resource: SsaPlannerResourceV1::StorageWords,
        required,
        limit,
    }
}

#[test]
fn first_dense_matrix_and_reachable_charge_has_an_inclusive_three_word_boundary() {
    // One live-in word, one dense definition word, then ceil(1 bool / 8).
    for limit in [1, 2] {
        assert_eq!(
            plan_ssa_with_limits_v1(&input(), limits(limit)).unwrap_err(),
            storage_error(3, limit)
        );
    }
    // Reaching the next charge is not acceptance of the full plan.
    let next = 3 + std::mem::size_of::<Vec<usize>>().div_ceil(std::mem::size_of::<u64>());
    assert_eq!(
        plan_ssa_with_limits_v1(&input(), limits(3)).unwrap_err(),
        storage_error(next, 3)
    );
}

#[test]
fn same_empty_lit_input_still_requires_its_complete_reported_storage() {
    let input = input();
    let plan = plan_ssa_v1(&input).unwrap();
    let complete = plan.resources().storage_words();
    assert!(complete > 3);
    let at = plan_ssa_with_limits_v1(&input, limits(complete)).unwrap();
    assert_eq!(at, plan);
    at.verify_replay(&input, limits(complete)).unwrap();
    assert_eq!(
        plan_ssa_with_limits_v1(&input, limits(complete - 1)).unwrap_err(),
        storage_error(complete, complete - 1)
    );
}
