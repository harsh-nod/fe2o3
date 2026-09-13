pub(crate) struct ProductionPlironPreloweringOutcomeV1 {
    pub(crate) report: ProductionPlironPreloweringReportV2,
    // Work and peak cover the complete invocation. Retained storage covers
    // the returned report, not the seven exclusive caches dropped at return.
    pub(crate) resource_upper_bound: ProductionAnalysisResourceUpperBoundV1,
}

fn retain_cache_resource_upper_bound_v1(
    analyses: &mut PlironAnalysisManagerV1,
    phase: crate::production_analysis::ProductionAnalysisResourcePhaseV1,
    upper_bound: ProductionAnalysisResourceUpperBoundV1,
) -> Result<(), ProductionAnalysisResourceLimitV1> {
    analyses.admit_retained_resource_upper_bound(phase, upper_bound)
}

// These seven reservations are admitted once by the closed pipeline before
// their corresponding manager cache is prepared. They exclude report owners,
// shared inventory, mixed tensor bounds, and preservation identity/custody.
struct ExclusiveAnalysisCacheReservationsV1 {
    sparse: ProductionAnalysisResourceUpperBoundV1,
    presburger: ProductionAnalysisResourceUpperBoundV1,
    execution_layout: ProductionAnalysisResourceUpperBoundV1,
    invocation_trace: ProductionAnalysisResourceUpperBoundV1,
    provenance: ProductionAnalysisResourceUpperBoundV1,
    simt: ProductionAnalysisResourceUpperBoundV1,
    memory_order: ProductionAnalysisResourceUpperBoundV1,
}

fn finish_exclusive_analysis_caches_v1(
    analyses: PlironAnalysisManagerV1,
    reservations: ExclusiveAnalysisCacheReservationsV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    let complete_invocation = analyses.resource_upper_bound();
    // None of these seven exclusive cache owners crosses this boundary.
    // Independent report copies keep their separate phase reservations.
    drop(analyses);
    let phase = crate::production_analysis::ProductionAnalysisResourcePhaseV1::PipelineVerification;
    let released = [
        reservations.sparse,
        reservations.presburger,
        reservations.execution_layout,
        reservations.invocation_trace,
        reservations.provenance,
        reservations.simt,
        reservations.memory_order,
    ]
    .into_iter()
    .try_fold(0_usize, |total, bound| {
        total
            .checked_add(bound.retained_storage_upper_bound())
            .ok_or(ProductionAnalysisResourceLimitV1 {
                phase,
                resource: "exclusive analysis cache retained storage upper bound",
            })
    })?;
    complete_invocation.checked_then_replace_retained(
        released,
        ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, 0, 0, 0)?,
        phase,
    )
}
