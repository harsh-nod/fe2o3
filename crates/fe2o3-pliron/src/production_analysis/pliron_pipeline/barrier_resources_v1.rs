#[allow(clippy::result_large_err)]
fn preflight_barrier_stage_v1(
    context: &Context,
    analyses: &mut PlironAnalysisManagerV1,
    input_census: &ProductionAnalysisInputCensusV1,
    trace_admission: Option<crate::production_analysis::pliron_invocation_trace::ProductionInvocationTraceResourceAdmissionV1>,
    pipeline_protocol_upper_bound: ProductionAnalysisResourceUpperBoundV1,
) -> Result<(ProductionAnalysisResourceUpperBoundV1, bool), ProductionPlironPreloweringErrorV2> {
    let barrier_progress_needed = admit_barrier_progress_probe_v1(context, analyses, input_census)
        .map_err(resource_upper_bound_error_v1)?;
    let barrier_progress_upper_bound = if barrier_progress_needed {
        Some(
            preflight_scoped_progress_resource_upper_bound_v1(
                *input_census,
                remaining_resource_limits_v1(
                    analyses,
                    crate::production_analysis::ProductionAnalysisResourcePhaseV1::Progress,
                )
                .map_err(resource_upper_bound_error_v1)?,
            )
            .map_err(resource_upper_bound_error_v1)?,
        )
    } else {
        None
    };
    let barrier_local_upper_bound = preflight_barrier_convergence_resource_upper_bound_v1(
        *input_census,
        trace_admission,
        remaining_resource_limits_v1(
            analyses,
            crate::production_analysis::ProductionAnalysisResourcePhaseV1::BarrierConvergence,
        )
        .map_err(resource_upper_bound_error_v1)?,
    )
    .map_err(resource_upper_bound_error_v1)?;
    let barrier_upper_bound = compose_barrier_dependencies_v1(
        barrier_local_upper_bound,
        pipeline_protocol_upper_bound,
        barrier_progress_upper_bound,
    )
    .map_err(resource_upper_bound_error_v1)?;
    Ok((barrier_upper_bound, barrier_progress_needed))
}
