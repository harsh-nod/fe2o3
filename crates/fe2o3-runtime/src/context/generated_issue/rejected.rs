//! Only exact lower rejection plus actual original disposal permits local failure.

use super::*;

macro_rules! impl_rejected_generated {
    ($backend:ty) => {
        impl RuntimeContextV1<$backend> {
            pub(in crate::context) fn progress_gfx942_issue_preserving_rejection_v1<
                T: RuntimeGfx942GeneratedCarrierV1,
            >(
                &mut self,
                prepared: &mut RuntimeGfx942PreparedV1<T>,
                roster: &GeneratedHostRosterV1,
                hold: &ContextUnpublishedHoldV1,
            ) -> Result<bool, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
                self.progress_generated_issue_with_v1(
                    prepared,
                    hold,
                    |context, value, uid, plan| {
                        let mut complete = false;
                        value
                            .source()
                            .with_current_source_v1(uid, roster, || {
                                complete = context
                                    .advance_generated_issue_attempt_with_rejection_v1(
                                        plan, roster, hold, true,
                                    )?;
                                Ok::<_, RuntimeErrorV1<KfdRuntimeBackendErrorV1>>(())
                            })
                            .map_err(|_| RuntimeValidationErrorV1::InvalidBackendDescription)??;
                        Ok(complete)
                    },
                )
            }

            pub(in crate::context) fn gfx942_issue_rejected_v1(
                &mut self,
                hold: &ContextUnpublishedHoldV1,
            ) -> Result<bool, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
                self.validate_unpublished_hold_v1(hold)?;
                let plan = self.generated_plan_for_hold_v1(hold)?;
                let attempt = self
                    .generated_issues
                    .get(&hold.stream())
                    .ok_or(RuntimeValidationErrorV1::InvalidBackendDescription)?;
                if attempt.phase != PhaseV1::Active || attempt.plan != plan {
                    return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
                }
                let id = self
                    .generated_issue_token_v1(hold, &plan)?
                    .backend_submission;
                Ok(self.backend.generated_rejected_publication_v1(&plan, id))
            }

            pub(in crate::context) fn settle_gfx942_rejected_v1<
                T: RuntimeGfx942GeneratedCarrierV1,
            >(
                &mut self,
                prepared: &mut RuntimeGfx942PreparedV1<T>,
                roster: &GeneratedHostRosterV1,
                hold: &ContextUnpublishedHoldV1,
            ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
                self.validate_unpublished_hold_v1(hold)?;
                let scope = self.generated_adoption_scope_for_hold_v1(hold)?;
                let result = catch_unwind(AssertUnwindSafe(|| {
                    self.validate_gfx942_prepared_v1(prepared)?;
                    let plan = self.generated_plan_for_hold_v1(hold)?;
                    if !self.gfx942_prepared_matches_plan_v1(prepared, &plan)
                        || !self.gfx942_issue_rejected_v1(hold)?
                        || !self.generated_issues[&hold.stream()].roster.matches(roster)
                    {
                        return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
                    }
                    let token = self.generated_issue_token_v1(hold, &plan)?;
                    let (id, backend_id) = (token.id, token.backend_submission);
                    let unread = self.generated_issue_exclusive_readers_v1(&plan);
                    if !self.journal_result_v1(unread)? {
                        return Err(RuntimeValidationErrorV1::ContextReserved.into());
                    }
                    let uid = self.generated_issue_device_uid_v1(plan.binding.backend_device)?;
                    self.generated_issues
                        .get_mut(&hold.stream())
                        .ok_or(RuntimeValidationErrorV1::InvalidBackendDescription)?
                        .phase = PhaseV1::Unknown;
                    prepared
                        .value()
                        .source()
                        .with_current_source_v1(uid, roster, || {
                            // Rejection alone grants no disposal: the lower abort
                            // independently checks original state and currentness.
                            self.backend
                                .retire_generated_rejected_data_v1(&plan, backend_id)
                                .map_err(map_backend_error)?;
                            self.backend
                                .release_submission_v1(backend_id)
                                .map_err(map_backend_error)?;
                            Ok::<_, RuntimeErrorV1<KfdRuntimeBackendErrorV1>>(())
                        })
                        .map_err(|_| RuntimeValidationErrorV1::InvalidBackendDescription)??;
                    self.settle_stopped_gfx942_context_v1(hold, id, backend_id)
                }));
                self.finish_generated_issue_scoped_v1(hold, scope, result)
            }
        }
    };
}
impl_rejected_generated!(KfdRuntimeBackendV1);
impl_rejected_generated!(KfdMultiDeviceRuntimeBackendV1);
