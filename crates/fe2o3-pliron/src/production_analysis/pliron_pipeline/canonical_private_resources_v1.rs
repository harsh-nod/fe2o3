//! Additional analysis-domain logical units; these are not KIR bytes or RSS.
use super::*;
type Bound = ProductionAnalysisResourceUpperBoundV1;
type Limit = ProductionAnalysisResourceLimitV1;
type Phase = ProductionAnalysisResourcePhaseV1;

// Nine inline Option slots: 9 * (eleven coverage fields + one tag).
// Identity labels have two fields. Pending: eleven fields + tag.
// Input/manager/identity/next and setup bound: eight fields. Total: 128.
// The ordinary validation session is accounted by its existing separate bound.
pub(super) fn setup() -> Result<Bound, Limit> {
    Bound::checked_phase(Phase::ReportValidation, 128, 128, 0)
}

pub(super) fn stage(
    input: &NativeCanonicalPrivateAdmissionV1<'_>,
    phase: Phase,
) -> Result<Bound, Limit> {
    let work = input
        .stage_lookup_work()
        .and_then(|work| work.checked_add(32))
        .ok_or(Limit {
            phase,
            resource: "private stage coverage work",
        })?;
    // The prepared slot and final slot were prepaid in setup; six counters are
    // scratch, not a second report or an allocation-proportional cell table.
    Bound::checked_phase(phase, work, 0, 6)
}

pub(super) fn record() -> Result<Bound, Limit> {
    Bound::checked_phase(Phase::ReportValidation, 32, 0, 4)
}

pub(super) fn finish() -> Result<Bound, Limit> {
    Bound::checked_phase(Phase::ReportValidation, 9 * 12 + 8, 0, 4)
}

pub(super) fn admit(
    analyses: &mut PlironAnalysisManagerV1,
    phase: Phase,
    bound: Bound,
    observer: PipelineObservationV1<'_, '_, '_>,
) -> Result<(), PipelineErrorV1> {
    let error = |error| observed_pipeline_resource_error_v1(observer, error);
    let limits = analyses.remaining_resource_limits(phase).map_err(error)?;
    invocation_receipt_v1::require_observed_v1(limits, phase, Ok(bound), observer)
        .map_err(error)?;
    analyses
        .admit_retained_resource_upper_bound(phase, bound)
        .map_err(error)
}
