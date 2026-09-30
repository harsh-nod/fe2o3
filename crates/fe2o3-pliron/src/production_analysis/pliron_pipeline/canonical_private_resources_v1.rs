//! Additional analysis-domain logical units; these are not KIR bytes or RSS.
use super::*;
type Bound = ProductionAnalysisResourceUpperBoundV1;
type Limit = ProductionAnalysisResourceLimitV1;
type Phase = ProductionAnalysisResourcePhaseV1;

// Nine inline Option slots and one pending slot retain two global counters
// independently of the five private counters: 128 + 10 * 2 + profile = 149.
// Global rows are not reclassified as private effects in any stage.
// The ordinary validation session is accounted by its existing separate bound.
pub(super) fn setup() -> Result<Bound, Limit> {
    // Ten coverage slots plus the session retain their versioned CFG domain.
    // Four further units cover selecting/rejoining the fixed profile/domain.
    Bound::checked_phase(Phase::ReportValidation, 149 + 11 + 4, 149 + 11, 0)
}

pub(super) fn stage(input: &impl NativePrivateInputV1, phase: Phase) -> Result<Bound, Limit> {
    let work = input
        .stage_lookup_work()
        .and_then(|work| work.checked_add(128 + 20))
        .ok_or(Limit {
            phase,
            resource: "private stage coverage work",
        })?;
    // The fixed work also covers both epoch checks and comparison with at most
    // nine preceding seven-counter rows. Nine census counters are scratch;
    // pending/final coverage slots were prepaid in setup. Twenty more work
    // units cover all prior-domain comparisons, two currentness queries, and
    // installing/replaying the new domain field.
    Bound::checked_phase(phase, work, 0, 9)
}

pub(super) fn record() -> Result<Bound, Limit> {
    Bound::checked_phase(Phase::ReportValidation, 48 + 4, 0, 4)
}

pub(super) fn finish() -> Result<Bound, Limit> {
    // Check all profile/count joins and copy each complete coverage row into
    // the final report. This also covers the separate missing-slot census.
    // Per row: 13 transferred fields + 7 baseline counters + 4 global counters
    // + 6 identity/control fields + loop/tags. The CFG domain adds four
    // copy/compare units per row and four session/currentness units.
    Bound::checked_phase(Phase::ReportValidation, 9 * (40 + 4) + 64 + 4, 0, 4)
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
