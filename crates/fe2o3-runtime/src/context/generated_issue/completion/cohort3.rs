//! All three original destinations stay borrowed through whole-batch settlement.

use super::*;
use crate::RuntimeGfx942GeneratedCohort3V1;

fn require_cohort3_views_v1<P: RuntimeGfx942GeneratedCompletionCarrierV1>(
    value: &mut RuntimeGfx942GeneratedCohort3V1<P>,
    complete: impl for<'a> FnOnce(
        [RuntimeGfx942GeneratedCompletionViewV1<'a, P::CurrentnessError>; 3],
    ) -> Result<
        NativeSettlementV1,
        RuntimeErrorV1<KfdRuntimeBackendErrorV1>,
    >,
) -> Result<NativeSettlementV1, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
    let [a, b, c] = value.members.each_mut();
    require_completion_view_v1(a, |a| {
        require_completion_view_v1(b, |b| {
            require_completion_view_v1(c, |c| complete([a, b, c]))
        })
    })
}

impl RuntimeContextV1<KfdRuntimeBackendV1> {
    pub(in crate::context) fn complete_gfx942_cohort3_issue_v1<
        P: RuntimeGfx942GeneratedCompletionCarrierV1,
    >(
        &mut self,
        prepared: &mut RuntimeGfx942PreparedV1<RuntimeGfx942GeneratedCohort3V1<P>>,
        roster: &GeneratedHostRosterV1,
        hold: &ContextUnpublishedHoldV1,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        self.validate_unpublished_hold_v1(hold)?;
        let scope = self.generated_adoption_scope_for_hold_v1(hold)?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.validate_gfx942_prepared_v1(prepared)?;
            let plan = self.generated_plan_for_hold_v1(hold)?;
            if !self.gfx942_prepared_matches_plan_v1(prepared, &plan)
                || plan.profile != crate::generated_source::GeneratedProfileV1::NativeFillCohort3
            {
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
            let expected_writer = attempt.expected_writer;
            let expected_reader = attempt.expected_reader;
            let unread = self.generated_issue_exclusive_readers_v1(&plan);
            if !self.journal_result_v1(unread)? {
                return Err(RuntimeValidationErrorV1::ContextReserved.into());
            }
            let uid = self.generated_issue_device_uid_v1(plan.binding.backend_device)?;
            self.generated_issues
                .get_mut(&hold.stream())
                .expect("retained attempt")
                .phase = PhaseV1::Unknown;
            let settled = require_cohort3_views_v1(prepared.value_mut_v1(), |views| {
                let [(a, ad), (b, bd), (c, cd)] = views.map(|view| view.into_parts());
                // Each source validates its own original output, while the backend
                // requires the exact ordered aggregate roster and one real receipt.
                let ar = a.validate(uid).map_err(invalid_source)?;
                let br = b.validate(uid).map_err(invalid_source)?;
                let cr = c.validate(uid).map_err(invalid_source)?;
                let actual = crate::generated_source::combine_original_rosters([
                    ar.clone(),
                    br.clone(),
                    cr.clone(),
                ])
                .map_err(invalid_source)?;
                if !actual.matches(roster) {
                    return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
                }
                a.with_current_source_v1(uid, &ar, || {
                    b.with_current_source_v1(uid, &br, || {
                        c.with_current_source_v1(uid, &cr, || {
                            self.backend
                                .read_generated_cohort3_submission_v1(
                                    &plan,
                                    backend_submission,
                                    roster,
                                    [&mut *ad, &mut *bd, &mut *cd],
                                )
                                .map_err(map_backend_error)?;
                            a.validate_completed_readback_v1(ad)
                                .map_err(invalid_source)?;
                            b.validate_completed_readback_v1(bd)
                                .map_err(invalid_source)?;
                            c.validate_completed_readback_v1(cd)
                                .map_err(invalid_source)?;
                            self.backend
                                .retire_generated_data_v1(&plan)
                                .map_err(map_backend_error)?;
                            self.backend
                                .release_submission_v1(backend_submission)
                                .map_err(map_backend_error)
                        })
                        .map_err(invalid_source)?
                    })
                    .map_err(invalid_source)?
                })
                .map_err(invalid_source)??;
                Ok(NativeSettlementV1 {
                    submission: id,
                    backend_submission,
                    stream: hold.stream(),
                    hold: hold.identity(),
                    expected_writer,
                    expected_reader,
                })
            })?;
            self.settle_completed_gfx942_context_v1(hold, settled)
        }));
        self.finish_generated_issue_scoped_v1(hold, scope, result)
    }
}

fn invalid_source<E>(_: E) -> RuntimeErrorV1<KfdRuntimeBackendErrorV1> {
    RuntimeValidationErrorV1::InvalidBackendDescription.into()
}
