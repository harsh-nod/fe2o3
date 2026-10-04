#[test]
fn preservation_replacement_real_pipeline_replays_at_its_exact_receipt() {
    let context = &mut setup();
    let (function, _) = valid_tensor_function(context, "replacement_exact_receipt");
    let first = require_production_pliron_checks_before_lowering_with_resource_limits_v1(
        context,
        &function,
        ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
    )
    .unwrap();
    let bound = first.resource_upper_bound;
    let replay = require_production_pliron_checks_before_lowering_with_resource_limits_v1(
        context,
        &function,
        ProductionAnalysisResourceLimitsV1::new(
            bound.work_upper_bound(),
            bound.peak_storage_upper_bound(),
        ),
    )
    .expect("a completed invocation's exact receipt must admit identical replay");
    assert_eq!(replay.resource_upper_bound, bound);
    assert_eq!(replay.report, first.report);
    assert!(replay.report.is_clean());
    assert!(replay.report.preservation().is_exact_identity());
    assert_eq!(replay.report.preservation().certificates().len(), 9);
    assert!(
        replay
            .report
            .preservation()
            .exactly_matches_retained_output(first.report.preservation())
    );
}

#[test]
fn preservation_replacement_real_pipeline_rejects_one_under_without_graph_mutation() {
    let context = &mut setup();
    let (function, _) = valid_tensor_function(context, "replacement_one_under");
    let first = require_production_pliron_checks_before_lowering_with_resource_limits_v1(
        context,
        &function,
        ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
    )
    .unwrap();
    let bound = first.resource_upper_bound;
    for (work, peak, expected_resource) in [
        (
            bound.work_upper_bound() - 1,
            bound.peak_storage_upper_bound(),
            "work upper bound",
        ),
        (
            bound.work_upper_bound(),
            bound.peak_storage_upper_bound() - 1,
            "peak storage upper bound",
        ),
    ] {
        let error = require_production_pliron_checks_before_lowering_with_resource_limits_v1(
            context,
            &function,
            ProductionAnalysisResourceLimitsV1::new(work, peak),
        )
        .err()
        .expect("one-under receipt must not admit the full invocation");
        let resource = match error {
            ProductionPlironPreloweringErrorV2::ResourceLimit { resource, .. }
            | ProductionPlironPreloweringErrorV2::Preservation(
                PlironPassPreservationErrorV1::ResourceLimit { resource },
            )
            | ProductionPlironPreloweringErrorV2::ReportValidation(
                ProductionAnalysisReportValidationErrorV1::ResourceLimit { resource, .. },
            ) => resource,
            error => panic!("unexpected nonresource failure: {error:?}"),
        };
        assert_eq!(resource, expected_resource);
    }
    let replay = require_production_pliron_checks_before_lowering_with_resource_limits_v1(
        context,
        &function,
        ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
    )
    .unwrap();
    assert_eq!(replay.report, first.report);
    assert_eq!(replay.resource_upper_bound, bound);
}
