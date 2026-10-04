fn prepare_exclusive_release_test_caches(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
    census: ProductionAnalysisInputCensusV1,
) -> ExclusiveAnalysisCacheReservationsV1 {
    use crate::production_analysis::ProductionAnalysisResourcePhaseV1 as Phase;
    let limits = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
    let sparse =
        preflight_sparse_index_resource_upper_bound_v1(context, function, census, limits).unwrap();
    retain_cache_resource_upper_bound_v1(analyses, Phase::SparseIndex, sparse).unwrap();
    analyses.prepare_sparse_indices(context, function);
    let presburger =
        preflight_presburger_resource_upper_bound_v1(analyses.sparse_indices().ok(), limits)
            .unwrap();
    retain_cache_resource_upper_bound_v1(analyses, Phase::Presburger, presburger).unwrap();
    analyses.prepare_presburger(context, function);
    let execution_layout =
        preflight_execution_layout_resource_upper_bound_v1(census, limits).unwrap();
    retain_cache_resource_upper_bound_v1(analyses, Phase::LaunchContract, execution_layout)
        .unwrap();
    analyses.prepare_execution_layout(context, function);
    let trace = preflight_invocation_trace_resource_upper_bound_v1(
        context,
        analyses.function_inventory().ok(),
        census,
        analyses.sparse_indices().ok(),
        analyses.execution_layout().unwrap(),
        limits,
    )
    .unwrap();
    let invocation_trace = trace.attempt_upper_bound();
    retain_cache_resource_upper_bound_v1(analyses, Phase::InvocationTrace, invocation_trace)
        .unwrap();
    analyses.prepare_exact_trace(context, function);
    let trace_admission = trace.exact_admission(analyses.exact_trace()).unwrap();
    let provenance = preflight_provenance_alias_resource_upper_bound_v1(census, limits).unwrap();
    retain_cache_resource_upper_bound_v1(analyses, Phase::ProvenanceAlias, provenance).unwrap();
    analyses.prepare_provenance_alias(context, function);
    let simt = preflight_simt_protocol_resource_upper_bound_v1(trace_admission, limits).unwrap();
    retain_cache_resource_upper_bound_v1(analyses, Phase::SimtProtocol, simt).unwrap();
    analyses.prepare_simt_protocol(context, function);
    let memory_order =
        preflight_memory_order_attempt_resource_upper_bound_v1(census, trace_admission, limits)
            .unwrap()
            .upper_bound();
    retain_cache_resource_upper_bound_v1(analyses, Phase::MemoryOrder, memory_order).unwrap();
    analyses.prepare_memory_order(context, function);
    ExclusiveAnalysisCacheReservationsV1 {
        sparse,
        presburger,
        execution_layout,
        invocation_trace,
        provenance,
        simt,
        memory_order,
    }
}

fn exclusive_release_test_sum(reservations: &ExclusiveAnalysisCacheReservationsV1) -> usize {
    reservations.sparse.retained_storage_upper_bound()
        + reservations.presburger.retained_storage_upper_bound()
        + reservations.execution_layout.retained_storage_upper_bound()
        + reservations.invocation_trace.retained_storage_upper_bound()
        + reservations.provenance.retained_storage_upper_bound()
        + reservations.simt.retained_storage_upper_bound()
        + reservations.memory_order.retained_storage_upper_bound()
}

#[test]
fn pipeline_cache_release_preserves_real_report_shared_inventory_and_exact_peak() {
    use crate::production_analysis::ProductionAnalysisResourcePhaseV1 as Phase;
    use std::sync::Arc;
    let context = &mut setup();
    let (function, _) = valid_tensor_function(context, "cache_release_report");
    let limits = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
    let report = require_production_pliron_checks_before_lowering_with_resource_limits_v1(
        context, &function, limits,
    )
    .unwrap();
    let expected_report = report.report.clone();
    let preservation = begin_production_pliron_pass_contract_session_with_resource_limits_v1(
        LivePlironStructuralIdentityProviderV1::new(context, &function),
        limits,
    )
    .unwrap();
    let census = preservation.input_census_v1();
    let setup_bound = preservation.initial_identity_resource_upper_bound_v1();
    let mut analyses = PlironAnalysisManagerV1::new_with_resource_contract(
        &function,
        census,
        setup_bound,
        preservation
            .lineage_identity_resource_upper_bound_v1()
            .retained_storage_upper_bound(),
        limits,
    )
    .unwrap();
    // A real returned report is independently live while the new caches run.
    analyses
        .admit_retained_resource_upper_bound(
            Phase::PipelineVerification,
            report.resource_upper_bound,
        )
        .unwrap();
    let reservations =
        prepare_exclusive_release_test_caches(context, &function, &mut analyses, census);
    assert_eq!(analyses.cached_entries(), 8);
    assert!(!analyses.exact_trace().unwrap().is_empty());
    let inventory = analyses.function_inventory_handle().unwrap();
    let weak_inventory = Arc::downgrade(&inventory);
    assert_eq!(Arc::strong_count(&inventory), 2);
    let before = analyses.resource_upper_bound();
    let released = exclusive_release_test_sum(&reservations);
    let after = finish_exclusive_analysis_caches_v1(analyses, reservations).unwrap();
    assert!(released > 0);
    assert_eq!(after.work_upper_bound(), before.work_upper_bound());
    assert_eq!(
        after.peak_storage_upper_bound(),
        before.peak_storage_upper_bound()
    );
    assert_eq!(
        after.retained_storage_upper_bound(),
        before.retained_storage_upper_bound() - released
    );
    // Inventory and identity/report owners are not among the seven releases.
    let inventory_storage = census.blocks * 2 + census.operations;
    assert_eq!(
        after.retained_storage_upper_bound(),
        setup_bound.retained_storage_upper_bound()
            + inventory_storage
            + report.resource_upper_bound.retained_storage_upper_bound()
    );
    assert_eq!(Arc::strong_count(&inventory), 1);
    assert!(weak_inventory.upgrade().is_some());
    assert_eq!(report.report, expected_report);
    assert!(report.report.is_clean());
    assert_eq!(report.report.preservation().certificates().len(), 9);
    let exact = ProductionAnalysisResourceLimitsV1::new(
        before.work_upper_bound(),
        before.peak_storage_upper_bound(),
    );
    assert_eq!(exact.require(Phase::PipelineVerification, after), Ok(after));
    for (work, storage, resource) in [
        (
            before.work_upper_bound() - 1,
            before.peak_storage_upper_bound(),
            "work upper bound",
        ),
        (
            before.work_upper_bound(),
            before.peak_storage_upper_bound() - 1,
            "peak storage upper bound",
        ),
    ] {
        assert_eq!(
            ProductionAnalysisResourceLimitsV1::new(work, storage)
                .require(Phase::PipelineVerification, after),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: Phase::PipelineVerification,
                resource
            })
        );
    }
    drop(inventory);
    assert!(weak_inventory.upgrade().is_none());
}

#[test]
fn pipeline_cache_release_keeps_failed_attempt_reservations_until_manager_drop() {
    let context = &mut setup();
    let function = FuncOp::new(
        context,
        "failed_trace_release".try_into().unwrap(),
        FunctionType::get(context, vec![], vec![]),
    );
    let entry = function.get_entry_block(context);
    dialect_kernel::InvocationIndexOp::new(context, 0, 0)
        .get_operation()
        .insert_at_back(entry, context);
    ReturnOp::new(context)
        .get_operation()
        .insert_at_back(entry, context);
    let limits = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
    let preservation = begin_production_pliron_pass_contract_session_with_resource_limits_v1(
        LivePlironStructuralIdentityProviderV1::new(context, &function),
        limits,
    )
    .unwrap();
    let census = preservation.input_census_v1();
    let mut analyses = PlironAnalysisManagerV1::new_with_resource_contract(
        &function,
        census,
        preservation.initial_identity_resource_upper_bound_v1(),
        preservation
            .lineage_identity_resource_upper_bound_v1()
            .retained_storage_upper_bound(),
        limits,
    )
    .unwrap();
    let reservations =
        prepare_exclusive_release_test_caches(context, &function, &mut analyses, census);
    assert!(analyses.exact_trace().is_err());
    assert_eq!(analyses.cached_entries(), 8);
    let before = analyses.resource_upper_bound();
    let released = exclusive_release_test_sum(&reservations);
    let after = finish_exclusive_analysis_caches_v1(analyses, reservations).unwrap();
    assert_eq!(
        after.retained_storage_upper_bound(),
        before.retained_storage_upper_bound() - released
    );
    assert_eq!(after.work_upper_bound(), before.work_upper_bound());
    assert_eq!(
        after.peak_storage_upper_bound(),
        before.peak_storage_upper_bound()
    );
}
