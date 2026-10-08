//! Cancellation gate over the original DATA and optional submission, never polling.

use super::*;

macro_rules! impl_unpublished_generated {
    ($backend:ty) => {
        impl RuntimeContextV1<$backend> {
            pub(in crate::context) fn gfx942_adoption_unpublished_v1(
                &mut self,
                hold: &ContextUnpublishedHoldV1,
            ) -> Result<bool, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
                self.validate_unpublished_hold_v1(hold)?;
                // Preserve the original journal-corruption quarantine. This
                // validation may terminalize custody but never polls or disposes.
                let plan = self.generated_plan_for_hold_v1(hold)?;
                let submission = match self.generated_issues.get(&hold.stream()) {
                    None => None,
                    Some(attempt) => {
                        if attempt.plan != plan || attempt.phase != PhaseV1::Active {
                            return Ok(false);
                        }
                        Some(
                            self.generated_issue_token_v1(hold, &plan)?
                                .backend_submission,
                        )
                    }
                };
                Ok(self
                    .backend
                    .generated_data_unpublished_v1(&plan, submission))
            }

            pub(in crate::context) fn retire_gfx942_unpublished_v1(
                &mut self,
                hold: &ContextUnpublishedHoldV1,
            ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
                // Exclusive Context custody prevents publication between this
                // gate and retirement. It never observes/recycles a packet to
                // make it eligible: Recycled is rejected just like Published.
                if !self.gfx942_adoption_unpublished_v1(hold)? {
                    return Err(RuntimeValidationErrorV1::ContextReserved.into());
                }
                if self.generated_issues.contains_key(&hold.stream()) {
                    if !self.retire_gfx942_issued_v1(hold)? {
                        return Err(RuntimeValidationErrorV1::ContextReserved.into());
                    }
                    Ok(())
                } else {
                    // Before first ISSUE there is no Context submission token.
                    // Use the existing original pre-issue DATA retirement path.
                    self.retire_gfx942_adoption_v1(hold)
                }
            }
        }
    };
}

impl_unpublished_generated!(KfdRuntimeBackendV1);
impl_unpublished_generated!(KfdMultiDeviceRuntimeBackendV1);
