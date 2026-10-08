//! Whole-cohort issue uses the original Context mutation/token path.

use super::*;
use crate::RuntimeGfx942GeneratedCohort3V1;

impl RuntimeContextV1<KfdRuntimeBackendV1> {
    pub(in crate::context) fn progress_gfx942_cohort3_issue_v1<
        P: RuntimeGfx942GeneratedCarrierV1,
    >(
        &mut self,
        prepared: &mut RuntimeGfx942PreparedV1<RuntimeGfx942GeneratedCohort3V1<P>>,
        roster: &GeneratedHostRosterV1,
        hold: &ContextUnpublishedHoldV1,
    ) -> Result<bool, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        self.progress_generated_issue_with_v1(prepared, hold, |context, value, uid, plan| {
            let mut complete = false;
            value
                .with_current_sources_v1(uid, roster, || {
                    complete = context.advance_generated_issue_attempt_v1(plan, roster, hold)?;
                    Ok::<_, RuntimeErrorV1<KfdRuntimeBackendErrorV1>>(())
                })
                .map_err(|_| {
                    RuntimeErrorV1::Validation(RuntimeValidationErrorV1::InvalidBackendDescription)
                })??;
            Ok(complete)
        })
    }
}
