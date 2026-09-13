fn cache_release_verified_session(
    limits: crate::production_analysis::ProductionAnalysisResourceLimitsV1,
) -> (
    ProductionPlironSessionV1,
    ProductionStageHandleV1<KernelChecksVerifiedGraphStageV1>,
    ProductionRootHandleV1<KernelChecksVerifiedGraphStageV1>,
) {
    let mut session = ProductionPlironSessionV1::new_with_analysis_resource_limits_v1(
        ProductionSessionLimitsV1::default(),
        [
            dialect_gpu::dialect_registration().unwrap(),
            dialect_kernel::dialect_registration().unwrap(),
        ],
        limits,
    )
    .unwrap();
    let (stage, root) = construct_ranked(&mut session, "cache_release_replay");
    let (stage, root) = session
        .verify_production_ranked_kernel_pipeline(stage, root)
        .unwrap();
    (session, stage, root)
}

fn cache_release_replay_limits(
    run: crate::production_analysis::ProductionAnalysisResourceUpperBoundV1,
) -> crate::production_analysis::ProductionAnalysisResourceLimitsV1 {
    // The first returned report overlaps the full second invocation. The
    // comparison remains the existing two-report dynamic plus inline bound.
    let work = 2 * run.work_upper_bound()
        + 2 * run.retained_storage_upper_bound()
        + 2 * std::mem::size_of::<crate::ProductionPlironPreloweringReportV2>()
        + 1;
    let peak = run.retained_storage_upper_bound() + run.peak_storage_upper_bound();
    crate::production_analysis::ProductionAnalysisResourceLimitsV1::new(work, peak)
}

#[test]
fn pipeline_cache_release_actual_two_run_replay_retains_exact_report_and_envelope() {
    let (baseline, stage, _) = cache_release_verified_session(
        crate::production_analysis::ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
    );
    let run = baseline.constructed_roots[&stage.identity]
        .production_analysis_resource_upper_bound
        .unwrap();
    let exact = cache_release_replay_limits(run);
    drop(baseline);
    let (session, stage, root) = cache_release_verified_session(exact);
    let record = &session.constructed_roots[&stage.identity];
    assert_eq!(record.production_analysis_resource_upper_bound, Some(run));
    let expected_report = record.production_pipeline_report.as_ref().unwrap().clone();
    let prepared = session.prepare_ranked_lowering(stage, root).unwrap();
    assert_eq!(prepared.production_pipeline_report(), &expected_report);
    assert!(prepared.production_pipeline_report().is_clean());
    assert_eq!(
        prepared
            .production_pipeline_report()
            .preservation()
            .certificates()
            .len(),
        9
    );
    assert_eq!(
        prepared.production_analysis_work_upper_bound_v1(),
        exact.max_work()
    );
    assert_eq!(
        prepared.production_analysis_peak_storage_upper_bound_v1(),
        exact.max_peak_storage()
    );
    assert_eq!(
        prepared.production_analysis_retained_storage_upper_bound_v1(),
        2 * run.retained_storage_upper_bound()
    );
}

#[test]
fn pipeline_cache_release_actual_two_run_replay_rejects_one_under_limits() {
    use crate::production_analysis::ProductionAnalysisResourceLimitsV1 as Limits;
    let (baseline, stage, _) = cache_release_verified_session(Limits::production_hard_ceiling());
    let run = baseline.constructed_roots[&stage.identity]
        .production_analysis_resource_upper_bound
        .unwrap();
    let exact = cache_release_replay_limits(run);
    drop(baseline);
    for (limits, resource) in [
        (
            Limits::new(exact.max_work() - 1, exact.max_peak_storage()),
            "work upper bound",
        ),
        (
            Limits::new(exact.max_work(), exact.max_peak_storage() - 1),
            "peak storage upper bound",
        ),
    ] {
        let (session, stage, root) = cache_release_verified_session(limits);
        assert_eq!(
            session.constructed_roots[&stage.identity].production_analysis_resource_upper_bound,
            Some(run)
        );
        let error = session.prepare_ranked_lowering(stage, root).unwrap_err();
        assert!(
            matches!(error, ProductionSessionErrorV1::AnalysisResourceLimit {
            resource: actual, ..
        } if actual == resource),
            "{error:?}"
        );
    }
}
