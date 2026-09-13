#[allow(clippy::result_large_err)]
fn run_and_record_production_analysis_stage_v1<T, E>(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
    preservation: &mut PlironPassContractSessionV1<LivePlironStructuralIdentityProviderV1<'_>>,
    validation: &mut ProductionAnalysisReportValidationSessionV1<'_>,
    stage: ProductionAnalysisStageV1,
    execute: impl FnOnce(&mut PlironAnalysisManagerV1) -> Result<T, E>,
) -> Result<Result<T, E>, ProductionPlironPreloweringErrorV2>
where
    T: SealedProductionAnalysisReportV1,
{
    run_and_record_production_analysis_invocation_v1(
        context,
        function,
        analyses,
        preservation,
        validation,
        stage,
        |preservation, pass, limits, analyses| {
            preservation
                .run_contiguous_pass_with_resource_limits_v1(pass, limits, || execute(analyses))
        },
    )
}

#[allow(clippy::result_large_err)]
fn run_and_record_scoped_semantic_refinement_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
    preservation: &mut PlironPassContractSessionV1<LivePlironStructuralIdentityProviderV1<'_>>,
    validation: &mut ProductionAnalysisReportValidationSessionV1<'_>,
    producing_phase_upper_bound: ProductionAnalysisResourceUpperBoundV1,
) -> Result<
    Result<PlironSemanticRefinementReportV1, Box<PlironSemanticRefinementCheckErrorV1>>,
    ProductionPlironPreloweringErrorV2,
> {
    run_and_record_production_analysis_invocation_v1(
        context,
        function,
        analyses,
        preservation,
        validation,
        ProductionAnalysisStageV1 {
            pass: KernelCheckPassKindV1::SemanticRefinement,
            producing_phase:
                crate::production_analysis::ProductionAnalysisResourcePhaseV1::SemanticRefinement,
            producing_phase_upper_bound,
        },
        |preservation, _pass, limits, analyses| {
            preservation.run_scoped_semantic_refinement_with_resource_limits_v1(limits, |input| {
                require_pliron_semantic_refinement_with_scoped_input_v1(input, analyses)
                    .map(|result| result.map_err(Box::new))
            })
        },
    )
}

#[allow(clippy::result_large_err)]
fn run_and_record_scoped_barrier_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
    preservation: &mut PlironPassContractSessionV1<LivePlironStructuralIdentityProviderV1<'_>>,
    validation: &mut ProductionAnalysisReportValidationSessionV1<'_>,
    producing_phase_upper_bound: ProductionAnalysisResourceUpperBoundV1,
    progress_admitted: bool,
) -> Result<
    Result<PlironBarrierReportV1, PlironBarrierCheckErrorV1>,
    ProductionPlironPreloweringErrorV2,
> {
    run_and_record_production_analysis_invocation_v1(
        context,
        function,
        analyses,
        preservation,
        validation,
        ProductionAnalysisStageV1 {
            pass: KernelCheckPassKindV1::BarrierConvergence,
            producing_phase:
                crate::production_analysis::ProductionAnalysisResourcePhaseV1::BarrierConvergence,
            producing_phase_upper_bound,
        },
        |preservation, _pass, limits, analyses| {
            preservation.run_scoped_barrier_with_resource_limits_v1(limits, |input| {
                require_pliron_barrier_with_scoped_input_v1(input, analyses, progress_admitted)
            })
        },
    )
}
