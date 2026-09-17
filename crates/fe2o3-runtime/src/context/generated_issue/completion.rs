//! Original readback remains borrowed until native and Context settlement finish.

use super::*;
use crate::{RuntimeGfx942GeneratedCompletionCarrierV1, RuntimeGfx942GeneratedCompletionViewV1};

pub(super) struct NativeSettlementV1 {
    submission: RuntimeSubmissionIdV1,
    backend_submission: u64,
    stream: RuntimeStreamIdV1,
    hold: u64,
}

#[cfg(test)]
pub(super) fn assume_native_settlement_for_context_test_v1(
    context: &RuntimeContextV1<KfdRuntimeBackendV1>,
    hold: &ContextUnpublishedHoldV1,
) -> NativeSettlementV1 {
    let attempt = &context.generated_issues[&hold.stream()];
    NativeSettlementV1 {
        submission: attempt.id,
        backend_submission: attempt.submission.as_ref().unwrap().backend_submission,
        stream: hold.stream(),
        hold: hold.identity(),
    }
}

pub(super) fn require_completion_view_v1<T: RuntimeGfx942GeneratedCompletionCarrierV1>(
    carrier: &mut T,
    complete: impl for<'a> FnOnce(
        RuntimeGfx942GeneratedCompletionViewV1<'a, T::CurrentnessError>,
    ) -> Result<
        NativeSettlementV1,
        RuntimeErrorV1<KfdRuntimeBackendErrorV1>,
    >,
) -> Result<NativeSettlementV1, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
    let mut settled = None;
    carrier.with_completion_view_v1(|view| {
        settled = Some(complete(view)?);
        Ok(())
    })?;
    settled.ok_or_else(|| RuntimeValidationErrorV1::InvalidBackendDescription.into())
}

impl RuntimeContextV1<KfdRuntimeBackendV1> {
    pub(crate) fn complete_gfx942_issue_v1<T: RuntimeGfx942GeneratedCompletionCarrierV1>(
        &mut self,
        prepared: &mut RuntimeGfx942PreparedV1<T>,
        roster: &GeneratedHostRosterV1,
        hold: &ContextUnpublishedHoldV1,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        self.validate_unpublished_hold_v1(hold)?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.validate_gfx942_prepared_v1(prepared)?;
            let plan = self.generated_plan_for_hold_v1(hold)?;
            if !self.gfx942_prepared_matches_plan_v1(prepared, &plan) {
                return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
            }
            let attempt = self
                .generated_issues
                .get(&hold.stream())
                .ok_or(RuntimeValidationErrorV1::InvalidBackendDescription)?;
            if attempt.plan != plan
                || !attempt.roster.matches(roster)
                || attempt.phase != PhaseV1::PhysicallyComplete
            {
                return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
            }
            let token = self.generated_issue_token_v1(hold, &plan)?;
            let id = token.id;
            let backend_submission = token.backend_submission;
            let uid = self.generated_issue_device_uid_v1(plan.binding.backend_device)?;
            self.generated_issues
                .get_mut(&hold.stream())
                .expect("retained attempt")
                .phase = PhaseV1::Unknown;
            let settled = require_completion_view_v1(prepared.value_mut_v1(), |view| {
                let (source, destinations) = view.into_parts();
                source
                    .with_current_source_v1(uid, roster, || {
                        self.backend
                            .read_generated_submission_v1(
                                &plan,
                                backend_submission,
                                roster,
                                destinations,
                            )
                            .map_err(map_backend_error)?;
                        source
                            .validate_completed_readback_v1(destinations)
                            .map_err(|_| {
                                RuntimeErrorV1::Validation(
                                    RuntimeValidationErrorV1::InvalidBackendDescription,
                                )
                            })?;
                        self.backend
                            .retire_generated_data_v1(&plan)
                            .map_err(map_backend_error)?;
                        self.backend
                            .release_submission_v1(backend_submission)
                            .map_err(map_backend_error)?;
                        Ok::<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>>(())
                    })
                    .map_err(|_| {
                        RuntimeErrorV1::Validation(
                            RuntimeValidationErrorV1::InvalidBackendDescription,
                        )
                    })??;
                Ok(NativeSettlementV1 {
                    submission: id,
                    backend_submission,
                    stream: hold.stream(),
                    hold: hold.identity(),
                })
            })?;
            self.settle_completed_gfx942_context_v1(hold, settled)
        }));
        self.finish_generated_issue_v1(hold, result)
    }

    // Called only after required lending, closing currentness and native
    // settlement. This tail releases Context metadata, not native authority.
    pub(super) fn settle_completed_gfx942_context_v1(
        &mut self,
        hold: &ContextUnpublishedHoldV1,
        settled: NativeSettlementV1,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        self.validate_unpublished_hold_v1(hold)?;
        let attempt = self
            .generated_issues
            .get(&hold.stream())
            .ok_or(RuntimeValidationErrorV1::InvalidBackendDescription)?;
        if attempt.phase != PhaseV1::Unknown {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
        }
        let token = self.generated_issue_token_v1(hold, &attempt.plan)?;
        let (id, backend_submission) = (token.id, token.backend_submission);
        if settled.submission != id
            || settled.backend_submission != backend_submission
            || settled.stream != hold.stream()
            || settled.hold != hold.identity()
        {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
        }
        self.retire_generated_shells_v1(hold)?;
        self.require_graph_access(hold.graph_access())?;
        self.release_unpublished_hold_v1(hold)?;
        // This status describes native execution. Typed output stays behind
        // its original gate until the async driver consumes the decoder.
        self.transition_submission_status(id, RuntimeCompletionStatusV1::Succeeded)?;
        self.submissions.remove(&id);
        self.backend_submissions.remove(&backend_submission);
        self.generated_issues.remove(&hold.stream());
        Ok(())
    }
}
