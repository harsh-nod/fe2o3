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

fn retain_cache_with_observation_v1(
    analyses: &mut PlironAnalysisManagerV1,
    phase: ProductionAnalysisResourcePhaseV1,
    bound: ProductionAnalysisResourceUpperBoundV1,
    observer: PipelineObservationV1<'_, '_, '_>,
) -> Result<(), ProductionAnalysisResourceLimitV1> {
    invocation_receipt_v1::observe_resource_preflight_v1(observer, |observer| {
        if let Some(observer) = observer {
            observer.require(analyses.remaining_resource_limits(phase)?, phase, Ok(bound))?;
        }
        retain_cache_resource_upper_bound_v1(analyses, phase, bound)
    })
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

#[cfg(test)]
fn finish_exclusive_analysis_caches_v1(
    analyses: PlironAnalysisManagerV1,
    reservations: ExclusiveAnalysisCacheReservationsV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    finish_exclusive_analysis_caches_with_observation_v1(analyses, reservations, None)
}

fn finish_exclusive_analysis_caches_with_observation_v1(
    analyses: PlironAnalysisManagerV1,
    reservations: ExclusiveAnalysisCacheReservationsV1,
    receipt: Option<&mut invocation_receipt_v1::InvocationReceiptV1>,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    let complete_invocation = analyses.resource_upper_bound();
    // None of these seven exclusive cache owners crosses this boundary.
    // Independent report copies keep their separate phase reservations.
    let Some(receipt) = receipt else {
        drop(analyses);
        return settle_exclusive_cache_reservations_v1(complete_invocation, reservations)
            .map(|(_, returned)| returned);
    };
    let phase_kind = ProductionAnalysisResourcePhaseV1::PipelineVerification;
    let phase = receipt.phase(phase_kind, 0)?;
    let (released, returned) =
        invocation_receipt_v1::observe_resource_preflight_v1(Some(&phase.observer(&Ok)), |_| {
            settle_exclusive_cache_reservations_v1(complete_invocation, reservations)
        })?;
    drop(phase);
    receipt.drop_owner(phase_kind, analyses, released)?;
    Ok(returned)
}

fn settle_exclusive_cache_reservations_v1(
    complete_invocation: ProductionAnalysisResourceUpperBoundV1,
    reservations: ExclusiveAnalysisCacheReservationsV1,
) -> Result<(usize, ProductionAnalysisResourceUpperBoundV1), ProductionAnalysisResourceLimitV1> {
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
    let returned = complete_invocation.checked_then_replace_retained(
        released,
        ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, 0, 0, 0)?,
        phase,
    )?;
    Ok((released, returned))
}
