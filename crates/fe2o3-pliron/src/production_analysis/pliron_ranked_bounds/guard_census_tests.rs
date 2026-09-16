#[test]
fn sparse_guard_census_admits_large_cfg_without_raising_work_cap() {
    let census = ProductionAnalysisInputCensusV1 {
        blocks: 840,
        operations: 8_000,
        operands: 10_000,
        results: 4_000,
        successors: 1_600,
        attributes: 2_000,
        memory_bounds_guard_candidates: 160,
        ..ProductionAnalysisInputCensusV1::default()
    };
    let limits = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
    let bound = preflight_ranked_bounds_resource_upper_bound_v1(census, limits)
        .expect("sparse actual guards fit the unchanged work budget");
    assert_eq!(MAX_RANKED_BOUNDS_WORK_UNITS, 8_388_608);
    assert_eq!(
        preflight_ranked_bounds_resource_upper_bound_v1(
            census,
            ProductionAnalysisResourceLimitsV1::new(
                bound.work_upper_bound(),
                bound.peak_storage_upper_bound(),
            ),
        ),
        Ok(bound),
    );
    assert!(
        preflight_ranked_bounds_resource_upper_bound_v1(
            census,
            ProductionAnalysisResourceLimitsV1::new(
                bound.work_upper_bound() - 1,
                bound.peak_storage_upper_bound(),
            ),
        )
        .is_err()
    );
    let dense = ProductionAnalysisInputCensusV1 {
        memory_bounds_guard_candidates: census.blocks,
        ..census
    };
    assert_eq!(
        preflight_ranked_bounds_resource_upper_bound_v1(dense, limits)
            .unwrap_err()
            .resource,
        "memory-bounds work hard limit",
    );
}

#[test]
fn bounds_guard_candidates_monotonically_increase_the_envelope() {
    let limits = ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX);
    let mut previous = None;
    for guards in [0, 1, 63, 64, 65, 128] {
        let bound = preflight_ranked_bounds_resource_upper_bound_v1(
            ProductionAnalysisInputCensusV1 {
                blocks: 128,
                operations: 512,
                operands: 512,
                successors: 256,
                memory_bounds_guard_candidates: guards,
                ..ProductionAnalysisInputCensusV1::default()
            },
            limits,
        )
        .unwrap();
        if let Some((work, storage)) = previous {
            assert!(bound.work_upper_bound() >= work);
            assert!(bound.peak_storage_upper_bound() >= storage);
        }
        previous = Some((bound.work_upper_bound(), bound.peak_storage_upper_bound()));
    }
}

#[test]
fn guard_census_does_not_remove_runtime_budget_enforcement() {
    let mut budget = RankedBoundsBudget {
        work_units: MAX_RANKED_BOUNDS_WORK_UNITS,
        ..RankedBoundsBudget::default()
    };
    assert!(
        matches!(budget.work(1), Err(RankedBoundsFindingV1::ResourceLimitExceeded {
        resource: "analysis work unit", limit: MAX_RANKED_BOUNDS_WORK_UNITS, actual,
    }) if actual == MAX_RANKED_BOUNDS_WORK_UNITS + 1)
    );
}
