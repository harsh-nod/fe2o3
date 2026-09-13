use crate::production_analysis::pliron_pass_contract::{
    PlironPassPreservationErrorV1, ScopedVerifiedProgressInputV1,
};

pub(crate) fn require_pliron_barrier_with_scoped_input_v1(
    input: ScopedVerifiedProgressInputV1<'_, true>,
    analyses: &mut PlironAnalysisManagerV1,
    progress_admitted: bool,
) -> Result<Result<PlironBarrierReportV1, PlironBarrierCheckErrorV1>, PlironPassPreservationErrorV1>
{
    let (context, function) = input.endpoints()?;
    let report = run_barrier_with_progress_v1(context, function, analyses, || {
        if !progress_admitted {
            return Err(PlironPassPreservationErrorV1::InvalidSessionState {
                detail: "barrier progress was not resource-admitted",
            });
        }
        crate::production_analysis::pliron_progress::run_pliron_progress_with_scoped_input_v1(input)
            .map(|progress| progress.report)
    })?;
    Ok(if report.is_clean() {
        Ok(report)
    } else {
        Err(PlironBarrierCheckErrorV1 { report })
    })
}

#[cfg(test)]
pub(crate) fn run_pliron_barrier_convergence_check_with_analyses_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
) -> PlironBarrierReportV1 {
    run_barrier_with_progress_v1(context, function, analyses, || {
        Ok(
            crate::production_analysis::pliron_progress::run_pliron_progress_check_v1(
                context, function,
            ),
        )
    })
    .expect("the standalone test path does not create a scoped preservation error")
}
