//! Common journal writer retained until actual destruction; no scalar submission token.

use super::*;
use crate::generated_source::{
    GeneratedContractsV1, GeneratedProfileV1, GeneratedSourceIdentityV1,
};
use crate::{
    RuntimeGfx942GeneratedResidentRegistryV1 as Registry, RuntimeGfx942RegistryCompletionCarrierV1,
};

impl RuntimeContextV1<KfdRuntimeBackendV1> {
    fn registry4_attempt_v1(
        &mut self,
        hold: &ContextUnpublishedHoldV1,
        roster: &GeneratedHostRosterV1,
    ) -> Result<GeneratedShellPlanV1, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        let plan = self.generated_plan_for_hold_v1(hold)?;
        if !matches!(
            plan.profile,
            GeneratedProfileV1::NativeFillRegistry4
                | GeneratedProfileV1::NativeFillRegistry4Repeat2
                | GeneratedProfileV1::NativeFillRegistry16
        ) {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
        }
        if !self.generated_issues.contains_key(&hold.stream()) {
            self.begin_generated_issue_v1(hold, plan, roster)?;
            self.generated_issues
                .get_mut(&hold.stream())
                .unwrap_or_else(|| std::process::abort())
                .phase = PhaseV1::RegistryActive;
        }
        let attempt = self
            .generated_issues
            .get(&hold.stream())
            .unwrap_or_else(|| std::process::abort());
        if attempt.phase != PhaseV1::RegistryActive
            || attempt.plan != plan
            || !attempt.roster.matches(roster)
            || attempt.submission.is_some()
        {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
        }
        let (id, domain) = attempt.writer_binding_v1();
        self.validate_generated_writer_v1(id, domain, attempt.expected_writer, &plan, roster)
            .map_err(invalid)?;
        self.validate_generated_readers_v1(id, domain, attempt.expected_reader, &plan, roster)
            .map_err(invalid)?;
        Ok(plan)
    }

    pub(in crate::context) fn progress_gfx942_registry4_v1<
        P: RuntimeGfx942RegistryCompletionCarrierV1,
        const N: usize,
    >(
        &mut self,
        prepared: &mut RuntimeGfx942PreparedV1<Registry<P, N>>,
        roster: &GeneratedHostRosterV1,
        hold: &ContextUnpublishedHoldV1,
        member: usize,
        cycle: u8,
    ) -> Result<bool, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        let scope = self.generated_adoption_scope_for_hold_v1(hold)?;
        let mut complete = false;
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.validate_gfx942_prepared_v1(prepared)?;
            let plan = self.registry4_attempt_v1(hold, roster)?;
            if member >= N
                || !self.gfx942_prepared_matches_plan_v1(prepared, &plan)
                || !self.backend.validate_registry4_cycle_v1(&plan, cycle)
            {
                return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
            }
            self.generated_issues
                .get_mut(&hold.stream())
                .unwrap_or_else(|| std::process::abort())
                .phase = PhaseV1::Unknown;
            prepared
                .value()
                .with_current_sources_v1(plan.binding.backend_device, roster, || {
                    complete = self
                        .backend
                        .progress_generated_registry4_recipe_v1(&plan, roster, member)
                        .map_err(map_backend_error)?;
                    Ok::<_, RuntimeErrorV1<KfdRuntimeBackendErrorV1>>(())
                })
                .map_err(invalid)??;
            self.generated_issues
                .get_mut(&hold.stream())
                .unwrap_or_else(|| std::process::abort())
                .phase = PhaseV1::RegistryActive;
            Ok(())
        }));
        self.finish_generated_issue_scoped_v1(hold, scope, result)?;
        Ok(complete)
    }

    pub(in crate::context) fn copy_gfx942_registry4_result_v1<
        P: RuntimeGfx942RegistryCompletionCarrierV1,
        const N: usize,
    >(
        &mut self,
        prepared: &mut RuntimeGfx942PreparedV1<Registry<P, N>>,
        roster: &GeneratedHostRosterV1,
        hold: &ContextUnpublishedHoldV1,
        member: usize,
        cycle: u8,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        let scope = self.generated_adoption_scope_for_hold_v1(hold)?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.validate_gfx942_prepared_v1(prepared)?;
            let plan = self.registry4_attempt_v1(hold, roster)?;
            if member >= N
                || !self.backend.validate_registry4_cycle_v1(&plan, cycle)
                || !self.gfx942_prepared_matches_plan_v1(prepared, &plan)
                || !prepared
                    .value()
                    .validate_sources(plan.binding.backend_device)
                    .map_err(invalid)?
                    .matches(roster)
            {
                return Err(invalid(()));
            }
            let identities: &[_] = match &roster.source_identity {
                GeneratedSourceIdentityV1::Registry4(identities)
                | GeneratedSourceIdentityV1::Registry4Repeat2(identities) => identities,
                GeneratedSourceIdentityV1::Registry16(identities) => identities,
                _ => return Err(invalid(())),
            };
            let contracts: &[_] = match &roster.dispatch_contract_sha256 {
                GeneratedContractsV1::Registry4(contracts)
                | GeneratedContractsV1::Registry4Repeat2(contracts) => contracts,
                GeneratedContractsV1::Registry16(contracts) => contracts,
                _ => return Err(invalid(())),
            };
            self.generated_issues
                .get_mut(&hold.stream())
                .unwrap_or_else(|| std::process::abort())
                .phase = PhaseV1::Unknown;
            let mut copied = false;
            prepared.value_mut_v1().members[member].with_registry_cycle_completion_view_v1(
                cycle,
                |view| {
                    if copied {
                        return Err(invalid(()));
                    }
                    let (source, destinations) = view.into_parts();
                    let original = source
                        .validate(plan.binding.backend_device)
                        .map_err(invalid)?;
                    if !original
                        .source_identity
                        .matches(&GeneratedSourceIdentityV1::Singleton(
                            std::sync::Arc::clone(&identities[member]),
                        ))
                        || original.dispatch_contract_sha256
                            != GeneratedContractsV1::Singleton(contracts[member])
                        || destinations.len() != 1
                        || destinations[0].0 != crate::Gfx942RuntimeBufferAccessV1::WriteOnly
                        || destinations[0].1.capacity() != destinations[0].1.len()
                    {
                        return Err(invalid(()));
                    }
                    source
                        .with_current_source_v1(plan.binding.backend_device, &original, || {
                            self.backend
                                .read_generated_registry4_recipe_v1(
                                    &plan,
                                    roster,
                                    member,
                                    &mut destinations[0].1,
                                )
                                .map_err(map_backend_error)?;
                            source
                                .validate_completed_readback_v1(destinations)
                                .map_err(invalid)
                        })
                        .map_err(invalid)??;
                    copied = true;
                    Ok(())
                },
            )?;
            if !copied {
                return Err(invalid(()));
            }
            for original in &prepared.value().members {
                original.source().revalidate().map_err(invalid)?;
            }
            self.generated_issues
                .get_mut(&hold.stream())
                .unwrap_or_else(|| std::process::abort())
                .phase = PhaseV1::RegistryActive;
            Ok(())
        }));
        self.finish_generated_issue_scoped_v1(hold, scope, result)
    }

    pub(in crate::context) fn rearm_gfx942_registry4_v1<
        P: RuntimeGfx942RegistryCompletionCarrierV1,
        const N: usize,
    >(
        &mut self,
        prepared: &RuntimeGfx942PreparedV1<Registry<P, N>>,
        roster: &GeneratedHostRosterV1,
        hold: &ContextUnpublishedHoldV1,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        let scope = self.generated_adoption_scope_for_hold_v1(hold)?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.validate_gfx942_prepared_v1(prepared)?;
            let plan = self.registry4_attempt_v1(hold, roster)?;
            if plan.profile != GeneratedProfileV1::NativeFillRegistry4Repeat2
                || !self.gfx942_prepared_matches_plan_v1(prepared, &plan)
            {
                return Err(invalid(()));
            }
            self.generated_issues
                .get_mut(&hold.stream())
                .unwrap_or_else(|| std::process::abort())
                .phase = PhaseV1::Unknown;
            prepared
                .value()
                .with_current_sources_v1(plan.binding.backend_device, roster, || {
                    self.backend
                        .rearm_generated_registry4_v1(&plan, roster)
                        .map_err(map_backend_error)
                })
                .map_err(invalid)??;
            // The same writer, shell, native debit and hold remain outstanding.
            self.generated_issues
                .get_mut(&hold.stream())
                .unwrap_or_else(|| std::process::abort())
                .phase = PhaseV1::RegistryActive;
            Ok(())
        }));
        self.finish_generated_issue_scoped_v1(hold, scope, result)
    }

    pub(in crate::context) fn close_gfx942_registry4_v1<
        P: RuntimeGfx942RegistryCompletionCarrierV1,
        const N: usize,
    >(
        &mut self,
        prepared: &RuntimeGfx942PreparedV1<Registry<P, N>>,
        roster: &GeneratedHostRosterV1,
        hold: &ContextUnpublishedHoldV1,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        let scope = self.generated_adoption_scope_for_hold_v1(hold)?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.validate_gfx942_prepared_v1(prepared)?;
            let plan = self.registry4_attempt_v1(hold, roster)?;
            if !self.gfx942_prepared_matches_plan_v1(prepared, &plan) {
                return Err(invalid(()));
            }
            self.generated_issues
                .get_mut(&hold.stream())
                .unwrap_or_else(|| std::process::abort())
                .phase = PhaseV1::Unknown;
            prepared
                .value()
                .with_current_sources_v1(plan.binding.backend_device, roster, || {
                    self.backend
                        .destroy_generated_registry4_v1(&plan, roster)
                        .map_err(map_backend_error)
                })
                .map_err(invalid)??;
            // Common native destruction, not a copied result, permits this tail.
            // No ordinary RuntimeSubmission or per-member version receipt is minted.
            self.settle_generated_custody_v1(hold.stream(), SubmissionWriterOutcomeV1::Success)?;
            self.retire_generated_shells_v1(hold)?;
            self.generated_issues.remove(&hold.stream());
            self.release_unpublished_hold_v1(hold)?;
            Ok(())
        }));
        self.finish_generated_issue_scoped_v1(hold, scope, result)
    }
}

fn invalid<E>(_: E) -> RuntimeErrorV1<KfdRuntimeBackendErrorV1> {
    RuntimeValidationErrorV1::InvalidBackendDescription.into()
}
