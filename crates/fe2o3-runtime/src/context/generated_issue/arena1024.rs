//! One original journal writer survives all selected-slot results.

use super::registry4::invalid;
use super::*;
use crate::generated_source::GeneratedProfileV1;
use crate::{RuntimeGfx942GeneratedArena1024V1 as Arena, RuntimeGfx942RegistryCompletionCarrierV1};

impl RuntimeContextV1<KfdRuntimeBackendV1> {
    fn arena_attempt_v1(
        &mut self,
        hold: &ContextUnpublishedHoldV1,
        roster: &GeneratedHostRosterV1,
    ) -> Result<GeneratedShellPlanV1, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        let plan = self.generated_plan_for_hold_v1(hold)?;
        if plan.profile != GeneratedProfileV1::NativeFillArena1024 || plan.count != 1 {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
        }
        self.retained_registry_attempt_v1(hold, roster, plan)
    }

    pub(in crate::context) fn progress_gfx942_arena_round_v1<
        P: RuntimeGfx942RegistryCompletionCarrierV1,
    >(
        &mut self,
        prepared: &mut RuntimeGfx942PreparedV1<Arena<P>>,
        roster: &GeneratedHostRosterV1,
        hold: &ContextUnpublishedHoldV1,
        copied: &mut [bool],
    ) -> Result<usize, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        let scope = self.generated_adoption_scope_for_hold_v1(hold)?;
        let mut transitions = 0;
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.validate_gfx942_prepared_v1(prepared)?;
            let plan = self.arena_attempt_v1(hold, roster)?;
            if copied.len() != fe2o3_kfd::GFX942_NATIVE_FILL_ARENA_SLOTS_V1
                || !self.gfx942_prepared_matches_plan_v1(prepared, &plan)
                || !prepared
                    .value()
                    .validate_sources(plan.binding.backend_device)
                    .map_err(invalid)?
                    .matches(roster)
            {
                return Err(invalid(()));
            }
            self.generated_issues
                .get_mut(&hold.stream())
                .unwrap_or_else(|| std::process::abort())
                .phase = PhaseV1::Unknown;
            // The complete original source roster brackets one finite scan, not
            // each member. Selected native/device checks are still independent.
            for (index, was_copied) in copied.iter_mut().enumerate() {
                if *was_copied {
                    continue;
                }
                if !self
                    .backend
                    .progress_generated_arena_recipe_v1(&plan, roster, index)
                    .map_err(map_backend_error)?
                {
                    continue;
                }
                let mut read = false;
                let expected = prepared
                    .value()
                    .member_original(index)
                    .ok_or_else(|| invalid(()))?;
                let member = prepared.value_mut_v1().members[index]
                    .as_mut()
                    .unwrap_or_else(|| std::process::abort());
                member.with_registry_cycle_completion_view_v1(0, |view| {
                    if read {
                        return Err(invalid(()));
                    }
                    let (source, destinations) = view.into_parts();
                    let actual = source
                        .validate(plan.binding.backend_device)
                        .map_err(invalid)?;
                    if !expected.matches(&actual)
                        || destinations.len() != 1
                        || destinations[0].0 != crate::Gfx942RuntimeBufferAccessV1::WriteOnly
                        || destinations[0].1.capacity() != destinations[0].1.len()
                    {
                        return Err(invalid(()));
                    }
                    source
                        .with_current_source_v1(plan.binding.backend_device, &actual, || {
                            self.backend
                                .read_generated_arena_recipe_v1(
                                    &plan,
                                    roster,
                                    index,
                                    &mut destinations[0].1,
                                )
                                .map_err(map_backend_error)?;
                            source
                                .validate_completed_readback_v1(destinations)
                                .map_err(invalid)
                        })
                        .map_err(invalid)??;
                    read = true;
                    Ok(())
                })?;
                if !read {
                    return Err(invalid(()));
                }
                let actual = prepared.value().members[index]
                    .as_ref()
                    .unwrap_or_else(|| std::process::abort())
                    .source()
                    .validate(plan.binding.backend_device)
                    .map_err(invalid)?;
                if !prepared.value().matches_member(index, &actual) {
                    return Err(invalid(()));
                }
                *was_copied = true;
                transitions += 1;
            }
            prepared.value().revalidate_sources().map_err(invalid)?;
            self.generated_issues
                .get_mut(&hold.stream())
                .unwrap_or_else(|| std::process::abort())
                .phase = PhaseV1::RegistryActive;
            Ok(())
        }));
        self.finish_generated_issue_scoped_v1(hold, scope, result)?;
        Ok(transitions)
    }

    pub(in crate::context) fn close_gfx942_arena_v1<P: RuntimeGfx942RegistryCompletionCarrierV1>(
        &mut self,
        prepared: &RuntimeGfx942PreparedV1<Arena<P>>,
        roster: &GeneratedHostRosterV1,
        hold: &ContextUnpublishedHoldV1,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        let scope = self.generated_adoption_scope_for_hold_v1(hold)?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.validate_gfx942_prepared_v1(prepared)?;
            let plan = self.arena_attempt_v1(hold, roster)?;
            if !self.gfx942_prepared_matches_plan_v1(prepared, &plan)
                || !prepared
                    .value()
                    .validate_sources(plan.binding.backend_device)
                    .map_err(invalid)?
                    .matches(roster)
            {
                return Err(invalid(()));
            }
            self.generated_issues
                .get_mut(&hold.stream())
                .unwrap_or_else(|| std::process::abort())
                .phase = PhaseV1::Unknown;
            self.backend
                .destroy_generated_arena_v1(&plan, roster)
                .map_err(map_backend_error)?;
            prepared.value().revalidate_sources().map_err(invalid)?;
            self.settle_generated_custody_v1(hold.stream(), SubmissionWriterOutcomeV1::Success)?;
            self.retire_generated_shells_v1(hold)?;
            self.generated_issues.remove(&hold.stream());
            self.release_unpublished_hold_v1(hold)?;
            Ok(())
        }));
        self.finish_generated_issue_scoped_v1(hold, scope, result)
    }
}
