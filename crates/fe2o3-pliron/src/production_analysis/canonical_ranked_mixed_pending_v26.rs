//! Complete native mixed reports retain exact source/runtime obligations.
use super::*;
use crate::canonical_private_v1::{
    CanonicalMixedPipelineOutcomeV26, CanonicalMixedPipelineReportV26,
};
use crate::production_analysis::canonical_ranked_checks_v1::private::private_resources::PrivateAnalysisV1;
use fe2o3_kernel_analysis::CheckedCanonicalKirPrivateMemoryV18;
use fe2o3_kernel_ir::CheckedCanonicalConditionalSliceDomainsV26 as Globals;

struct MixedReportRowV26 {
    outcome: CanonicalMixedPipelineOutcomeV26,
    history: CanonicalRankedPolicyHistoryV1,
}

/// The exact whole-module mixed native report plus its undischarged premises.
/// Source correspondences, descriptor contracts and physical launch binding
/// must still be joined by the owning compiler before output adoption.
pub struct PendingCanonicalMixedMemoryPoliciesV26<'s, 'g> {
    owner: &'g VerifiedCanonicalKernelIrModuleV18,
    reports: &'s [Option<MixedReportRowV26>],
    observation: CanonicalRankedPolicyResourceObservationV1,
    guard: &'s Guard,
    obligations: &'s [CanonicalRankedSourceObligationV18],
    globals: &'s Globals<'s, 's>,
    physical: &'s CheckedCanonicalKirPrivateMemoryV18<'s, 's>,
    native: PendingCanonicalGlobalAccessesV18<'s, 'g>,
    retained: usize,
    slot: usize,
    ledger: Ledger,
}

impl<'g> PendingCanonicalMixedMemoryPoliciesV26<'_, 'g> {
    fn check(&self, budget: &mut Budget<'_>) -> Result<(), Failure> {
        if self.slot != std::ptr::from_ref(&*budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.retained
        {
            return Err(self.refuse_retained_custody());
        }
        if let Err(error) = self.guard.query(budget) {
            if matches!(error, Failure::Resource(Resource::Accounting)) {
                self.refuse_retained_custody();
            }
            return Err(error);
        }
        self.native.check_owner(self.owner, budget)?;
        if !std::ptr::eq(
            self.globals
                .owner(budget)
                .map_err(Failure::ConditionalGlobalsV26)?,
            self.owner,
        ) {
            return Err(self.guard.exact_graph());
        }
        Ok(())
    }
    /// A composing source scope may only report loss of retained custody.
    /// Refusal reaches both native and formal ancestors, irrespective of their
    /// nesting order; the first native refusal remains the selected error.
    pub fn refuse_retained_custody(&self) -> Failure {
        let failure = self.native.refuse_retained_custody();
        self.globals.refuse_retained_custody();
        failure
    }
    pub fn owner(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<&'g VerifiedCanonicalKernelIrModuleV18, Failure> {
        self.check(budget)?;
        Ok(self.owner)
    }
    pub fn obligations(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<&[CanonicalRankedSourceObligationV18], Failure> {
        self.check(budget)?;
        Ok(self.obligations)
    }
    pub fn function_count(&self, budget: &mut Budget<'_>) -> Result<usize, Failure> {
        self.check(budget)?;
        Ok(self.reports.len())
    }
    pub fn report(
        &self,
        function: usize,
        budget: &mut Budget<'_>,
    ) -> Result<Option<&CanonicalMixedPipelineReportV26>, Failure> {
        self.check(budget)?;
        self.reports
            .get(function)
            .map(|row| row.as_ref().map(|row| &row.outcome.report))
            .ok_or_else(|| self.guard.invalid(function))
    }
    pub fn history(
        &self,
        function: usize,
        budget: &mut Budget<'_>,
    ) -> Result<Option<CanonicalRankedPolicyHistoryV1>, Failure> {
        self.check(budget)?;
        self.reports
            .get(function)
            .map(|row| row.as_ref().map(|row| row.history))
            .ok_or_else(|| self.guard.invalid(function))
    }
    pub fn observation(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<CanonicalRankedPolicyResourceObservationV1, Failure> {
        self.check(budget)?;
        Ok(self.observation)
    }
    /// The same native graph remains available for exact original-source joins.
    pub fn global_accesses(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<&PendingCanonicalGlobalAccessesV18<'_, 'g>, Failure> {
        self.check(budget)?;
        Ok(&self.native)
    }
    pub fn conditional_globals(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<&Globals<'_, '_>, Failure> {
        self.check(budget)?;
        Ok(self.globals)
    }
    pub fn physical_memory(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<&CheckedCanonicalKirPrivateMemoryV18<'_, '_>, Failure> {
        self.check(budget)?;
        Ok(self.physical)
    }
    pub const fn source_roles_are_complete(&self) -> bool {
        false
    }
    pub const fn runtime_requirements_are_discharged(&self) -> bool {
        false
    }
    pub const fn ranked_verification_is_complete(&self) -> bool {
        false
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn mixed_pending_headers_v26<T>(capture: usize, alignment: usize) -> Result<usize, Failure> {
    fn h<T>() -> Result<usize, Failure> {
        checked_add(
            size_of::<T>(),
            size_of::<Result<T, Failure>>()
                .checked_mul(2)
                .ok_or(Resource::Arithmetic)?,
        )
    }
    let mut bytes = checked_add(
        capture,
        alignment.checked_mul(2).ok_or(Resource::Arithmetic)?,
    )?;
    for n in [
        h::<PrivateAnalysisV1>()?,
        h::<Guard>()?,
        h::<MixedReportRowV26>()?,
        h::<Vec<Option<MixedReportRowV26>>>()?,
        h::<PendingCanonicalMixedMemoryPoliciesV26<'_, '_>>()?,
        h::<PendingCanonicalGlobalAccessesV18<'_, '_>>()?,
        h::<CanonicalMixedPipelineOutcomeV26>()?,
        h::<CanonicalRankedPolicyHistoryV1>()?,
        h::<CanonicalRankedPolicyResourceObservationV1>()?,
        h::<Option<&mut Option<MixedReportRowV26>>>()?,
        h::<&mut Option<MixedReportRowV26>>()?,
        h::<&[Option<MixedReportRowV26>]>()?,
        h::<Option<&MixedReportRowV26>>()?,
        h::<std::iter::Enumerate<std::slice::Iter<'_, Option<MixedReportRowV26>>>>()?,
        h::<(usize, &Option<MixedReportRowV26>)>()?,
        h::<fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1>()?,
        h::<
            Option<(
                fe2o3_kernel_ir::ExplicitLaunchExtent,
                fe2o3_kernel_ir::FormalIndexWidth,
                usize,
                usize,
            )>,
        >()?,
        h::<Option<[usize; 2]>>()?,
        h::<std::ops::Range<usize>>()?,
        h::<Option<&CanonicalMixedPipelineReportV26>>()?,
        h::<usize>()?,
        h::<bool>()?,
        h::<(Ledger, usize, usize)>()?,
        h::<&Globals<'_, '_>>()?,
        h::<fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1>()?,
        h::<&VerifiedCanonicalKernelIrModuleV18>()?,
        h::<&CheckedCanonicalKirPrivateMemoryV18<'_, '_>>()?,
        h::<&Guard>()?,
        h::<&[CanonicalRankedSourceObligationV18]>()?,
        h::<&mut Budget<'_>>()?,
        h::<(&mut PrivateAnalysisV1, &mut [Option<MixedReportRowV26>])>()?,
        h::<(
            &PendingCanonicalMixedMemoryPoliciesV26<'_, '_>,
            &mut Budget<'_>,
        )>()?,
        h::<Result<T, Failure>>()?,
        h::<std::thread::Result<Result<T, Failure>>>()?,
        h::<Option<CanonicalRankedPolicyHistoryV1>>()?,
        drain_header(),
    ] {
        bytes = checked_add(bytes, n)?;
    }
    Ok(bytes)
}

impl<'g> PendingCanonicalRankedSourceRolesV18<'_, 'g> {
    /// Runs the fixed nine stages with distinct private/global effect coverage
    /// on every actual definition. The callback retains all source/runtime
    /// premises; report cleanliness cannot discharge them.
    pub fn with_mixed_memory_observations_v26<'w, T>(
        &mut self,
        physical: &CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
        globals: &Globals<'_, '_>,
        budget: &mut Budget<'w>,
        callback: impl for<'s> FnOnce(
            &PendingCanonicalMixedMemoryPoliciesV26<'s, 'g>,
            &mut Budget<'w>,
        ) -> Result<T, Failure>,
    ) -> Result<T, CanonicalRankedPolicyChecksErrorV1> {
        self.with_mixed_memory_limits_v26(
            physical,
            globals,
            Limits::production_hard_ceiling(),
            budget,
            callback,
        )
    }

    pub(super) fn with_mixed_memory_limits_v26<'w, T>(
        &mut self,
        physical: &CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
        globals: &Globals<'_, '_>,
        limits: Limits,
        budget: &mut Budget<'w>,
        callback: impl for<'s> FnOnce(
            &PendingCanonicalMixedMemoryPoliciesV26<'s, 'g>,
            &mut Budget<'w>,
        ) -> Result<T, Failure>,
    ) -> Result<T, CanonicalRankedPolicyChecksErrorV1> {
        let mut analysis = PrivateAnalysisV1::new(limits);
        let mut callback = Some(callback);
        let result = protected_retained_v18(budget, self.refund_denied, |budget| {
            let caught = catch_unwind(AssertUnwindSafe(|| {
                self.check(budget)?;
                if !std::ptr::eq(physical.inventory().owner(), self.owner)
                    || !std::ptr::eq(
                        globals
                            .owner(budget)
                            .map_err(Failure::ConditionalGlobalsV26)?,
                        self.owner,
                    )
                {
                    return Err(self.guard.exact_graph());
                }
                budget.reserve_storage(mixed_pending_headers_v26::<T>(
                    std::mem::size_of_val(&callback),
                    std::mem::align_of_val(&callback),
                )?)?;
                let count = self.owner.module().functions.len();
                let mut reports = reserve_rows::<Option<MixedReportRowV26>>(count, budget)?;
                budget.charge_work(count)?;
                reports.resize_with(count, || None);
                self.graph.visit_mixed_policy_functions_v26(
                    physical,
                    globals,
                    self.epoch,
                    self.layouts,
                    Some(self.structural),
                    budget,
                    |ordinal, input| {
                        let slot = reports.get_mut(ordinal).ok_or(Failure::ExactGraph)?;
                        if slot.is_some() {
                            return Err(Failure::ExactGraph);
                        }
                        let outcome = analysis.invoke_mixed_v26(input)?;
                        let history = analysis.last.ok_or(Failure::InvocationAccounting)?;
                        if history.function() != ordinal {
                            return Err(Failure::InvocationAccounting);
                        }
                        *slot = Some(MixedReportRowV26 { outcome, history });
                        Ok(())
                    },
                )?;
                budget.charge_work(count)?;
                if reports
                    .iter()
                    .zip(&self.owner.module().functions)
                    .any(|(report, function)| report.is_some() != function.body.is_some())
                {
                    return Err(Failure::ExactGraph);
                }
                for (ordinal, row) in reports.iter().enumerate() {
                    budget.charge_work(4)?;
                    let Some(row) = row else {
                        continue;
                    };
                    let coordinate = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                        u32::try_from(ordinal).map_err(|_| Resource::Arithmetic)?,
                    );
                    let (_, _, reads, writes) = globals
                        .function_conditions(coordinate, budget)
                        .map_err(Failure::ConditionalGlobalsV26)?
                        .ok_or(Failure::ExactGraph)?;
                    for stage in 0..9 {
                        budget.charge_work(4)?;
                        if row.outcome.report.global_access_counts(stage) != Some([reads, writes]) {
                            return Err(Failure::ExactGraph);
                        }
                    }
                }
                let guard = Guard::new(budget);
                let native = PendingCanonicalGlobalAccessesV18::after_mixed_census_v26(
                    self.owner,
                    self.graph,
                    self.epoch,
                    &guard,
                    self.refund_denied,
                );
                let view = PendingCanonicalMixedMemoryPoliciesV26 {
                    owner: self.owner,
                    reports: &reports,
                    observation: analysis.observation(),
                    guard: &guard,
                    obligations: self.obligations,
                    globals,
                    physical,
                    native,
                    retained: budget.storage(),
                    slot: std::ptr::from_ref(&*budget) as usize,
                    ledger: budget.work_ledger_identity_v1(),
                };
                let mut returned = guard.callback(budget, |budget| {
                    callback.take().ok_or(Failure::InvocationAccounting)?(&view, budget)
                });
                // Validate parent custody before a rejected callback result drops
                // and before any backing is destroyed or its credit refunded.
                let postcheck = view.check(budget).and_then(|()| self.guard.check(budget));
                if let Err(error) = postcheck {
                    resources::discard(std::mem::replace(&mut returned, Err(error)));
                }
                drop(view);
                drop(reports);
                if let Err(error) = analysis.release_reports() {
                    resources::discard(std::mem::replace(&mut returned, Err(error)));
                }
                returned
            }));
            resources::discard(callback.take());
            match caught {
                Ok(result) => result,
                Err(payload) => {
                    resources::discard(payload);
                    Err(self
                        .guard
                        .check(budget)
                        .err()
                        .or_else(|| {
                            globals
                                .owner(budget)
                                .err()
                                .map(Failure::ConditionalGlobalsV26)
                        })
                        .unwrap_or(Failure::Panicked))
                }
            }
        });
        resources::discard(callback.take());
        result.map_err(|failure| {
            // Keep the child's selected refusal ahead of the parent's later
            // exact-floor postcheck, including a caller that ignores this Err.
            let failure = match failure {
                Failure::Resource(error) => self.guard.resource(error),
                Failure::InvalidQuery { function } => self.guard.invalid(function),
                Failure::ExactGraph => self.guard.exact_graph(),
                Failure::Mutation => self.guard.mutation(),
                other => other,
            };
            CanonicalRankedPolicyChecksErrorV1 {
                failure,
                observation: analysis.observation(),
                last_invocation: analysis.last,
            }
        })
    }
}
