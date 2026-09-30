//! Complete native mixed reports retain exact source/runtime obligations.
use super::*;
use crate::canonical_private_v1::{
    CanonicalMixedPipelineOutcomeV26, CanonicalMixedPipelineReportV26,
};
use crate::native_conditional_domains_v30::NativeConditionalDomainsV30 as Domains;
use crate::production_analysis::canonical_ranked_checks_v1::private::private_resources::PrivateAnalysisV1;
use fe2o3_kernel_analysis::CheckedCanonicalKirPrivateMemoryV18;
use fe2o3_kernel_ir::CheckedCanonicalConditionalSliceDomainsV26 as Globals;
use fe2o3_kernel_ir::CheckedCanonicalSelectedSliceDomainsV30 as Selected;

struct MixedReportRowV26 {
    outcome: CanonicalMixedPipelineOutcomeV26,
    history: CanonicalRankedPolicyHistoryV1,
}

struct ConditionalOwnerCaptureV30<C> {
    callback: C,
}

fn conditional_accounting_v30(failure: &Failure) -> bool {
    matches!(
        failure,
        Failure::Resource(Resource::Accounting)
            | Failure::ConditionalGlobalsV26(
                fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1::Resource(
                    fe2o3_kernel_ir::FormalGuardedMemoryResourceErrorV1::Accounting
                )
            )
    )
}

/// The exact whole-module mixed native report plus its undischarged premises.
/// Source correspondences, descriptor contracts and physical launch binding
/// must still be joined by the owning compiler before output adoption.
struct PendingConditionalMemoryCoreV30<'s, 'g> {
    owner: &'g VerifiedCanonicalKernelIrModuleV18,
    reports: &'s [Option<MixedReportRowV26>],
    observation: CanonicalRankedPolicyResourceObservationV1,
    guard: &'s Guard,
    obligations: &'s [CanonicalRankedSourceObligationV18],
    globals: Domains<'s>,
    physical: &'s CheckedCanonicalKirPrivateMemoryV18<'s, 's>,
    native: PendingCanonicalGlobalAccessesV18<'s, 'g>,
    retained: usize,
    slot: usize,
    ledger: Ledger,
}

impl<'g> PendingConditionalMemoryCoreV30<'_, 'g> {
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
    pub fn physical_memory(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<&CheckedCanonicalKirPrivateMemoryV18<'_, '_>, Failure> {
        self.check(budget)?;
        Ok(self.physical)
    }
}

macro_rules! pending_conditional_owner_v30 {
    ($name:ident, $accessor:ident, $variant:ident, $domains:ident) => {
        /// Exact native reports and their original checked domain owner remain
        /// inseparable. No source/runtime or final verification is discharged.
        pub struct $name<'s, 'g> {
            core: &'s PendingConditionalMemoryCoreV30<'s, 'g>,
        }
        impl<'g> $name<'_, 'g> {
            /// Refuses lost custody in both native and formal parent scopes.
            pub fn refuse_retained_custody(&self) -> Failure {
                self.core.refuse_retained_custody()
            }
            /// Borrows the exact verified native owner.
            pub fn owner(
                &self,
                budget: &mut Budget<'_>,
            ) -> Result<&'g VerifiedCanonicalKernelIrModuleV18, Failure> {
                self.core.owner(budget)
            }
            /// Borrows the complete unresolved original-source obligation roster.
            pub fn obligations(
                &self,
                budget: &mut Budget<'_>,
            ) -> Result<&[CanonicalRankedSourceObligationV18], Failure> {
                self.core.obligations(budget)
            }
            /// Counts all original functions, including external declarations.
            pub fn function_count(&self, budget: &mut Budget<'_>) -> Result<usize, Failure> {
                self.core.function_count(budget)
            }
            /// Borrows the real nine-stage report; declarations have no report.
            pub fn report(
                &self,
                function: usize,
                budget: &mut Budget<'_>,
            ) -> Result<Option<&CanonicalMixedPipelineReportV26>, Failure> {
                self.core.report(function, budget)
            }
            /// Returns the exact actual native invocation history.
            pub fn history(
                &self,
                function: usize,
                budget: &mut Budget<'_>,
            ) -> Result<Option<CanonicalRankedPolicyHistoryV1>, Failure> {
                self.core.history(function, budget)
            }
            /// Returns descriptive metered observations, not proof authority.
            pub fn observation(
                &self,
                budget: &mut Budget<'_>,
            ) -> Result<CanonicalRankedPolicyResourceObservationV1, Failure> {
                self.core.observation(budget)
            }
            /// Borrows the complete native external-access census.
            pub fn global_accesses(
                &self,
                budget: &mut Budget<'_>,
            ) -> Result<&PendingCanonicalGlobalAccessesV18<'_, 'g>, Failure> {
                self.core.global_accesses(budget)
            }
            /// Borrows the exact same-owner physical private-memory relation.
            pub fn physical_memory(
                &self,
                budget: &mut Budget<'_>,
            ) -> Result<&CheckedCanonicalKirPrivateMemoryV18<'_, '_>, Failure> {
                self.core.physical_memory(budget)
            }
            /// Borrows this owner's exact checked domain family without conversion.
            pub fn $accessor(&self, budget: &mut Budget<'_>) -> Result<&$domains<'_, '_>, Failure> {
                self.core.check(budget)?;
                match self.core.globals {
                    Domains::$variant(domains) => Ok(domains),
                    _ => Err(self.core.guard.exact_graph()),
                }
            }
            /// Always false; all original-source joins remain mandatory.
            pub const fn source_roles_are_complete(&self) -> bool {
                false
            }
            /// Always false; actual runtime allocation premises remain mandatory.
            pub const fn runtime_requirements_are_discharged(&self) -> bool {
                false
            }
            /// Always false; final relation verification remains mandatory.
            pub const fn ranked_verification_is_complete(&self) -> bool {
                false
            }
            /// Always false; no artifact or launch authority is granted here.
            pub const fn grants_artifact_or_launch_authority(&self) -> bool {
                false
            }
        }
    };
}

pending_conditional_owner_v30!(
    PendingCanonicalMixedMemoryPoliciesV26,
    conditional_globals,
    Legacy,
    Globals
);
pending_conditional_owner_v30!(
    PendingCanonicalSelectedMemoryPoliciesV30,
    selected_domains,
    Selected,
    Selected
);

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
        crate::native_conditional_domains_v30::conditional_query_headers_v30()?,
        h::<PrivateAnalysisV1>()?,
        h::<Guard>()?,
        h::<MixedReportRowV26>()?,
        h::<Vec<Option<MixedReportRowV26>>>()?,
        h::<PendingCanonicalMixedMemoryPoliciesV26<'_, '_>>()?,
        h::<PendingCanonicalSelectedMemoryPoliciesV30<'_, '_>>()?,
        h::<PendingConditionalMemoryCoreV30<'_, '_>>()?,
        h::<Domains<'_>>()?,
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
        h::<&Selected<'_, '_>>()?,
        h::<&PendingConditionalMemoryCoreV30<'_, '_>>()?,
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
        h::<(
            &PendingCanonicalSelectedMemoryPoliciesV30<'_, '_>,
            &mut Budget<'_>,
        )>()?,
        h::<(&PendingConditionalMemoryCoreV30<'_, '_>, &mut Budget<'_>)>()?,
        h::<Result<T, CanonicalRankedPolicyChecksErrorV1>>()?,
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
        // The common header measures this complete named capture through the
        // actual adapter closure, including the caller's callback payload.
        let capture = ConditionalOwnerCaptureV30 { callback };
        self.with_conditional_memory_limits_v30(
            physical,
            Domains::Legacy(globals),
            limits,
            budget,
            move |core, budget| {
                let ConditionalOwnerCaptureV30 { callback } = capture;
                callback(&PendingCanonicalMixedMemoryPoliciesV26 { core }, budget)
            },
        )
    }

    /// Executes the same fixed nine native stages while retaining the complete
    /// ordered selected-domain batch. This cannot produce a legacy V26 owner.
    pub fn with_selected_memory_observations_v30<'w, T>(
        &mut self,
        physical: &CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
        domains: &Selected<'_, '_>,
        budget: &mut Budget<'w>,
        callback: impl for<'s> FnOnce(
            &PendingCanonicalSelectedMemoryPoliciesV30<'s, 'g>,
            &mut Budget<'w>,
        ) -> Result<T, Failure>,
    ) -> Result<T, CanonicalRankedPolicyChecksErrorV1> {
        self.with_selected_memory_limits_v30(
            physical,
            domains,
            Limits::production_hard_ceiling(),
            budget,
            callback,
        )
    }

    pub(super) fn with_selected_memory_limits_v30<'w, T>(
        &mut self,
        physical: &CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
        domains: &Selected<'_, '_>,
        limits: Limits,
        budget: &mut Budget<'w>,
        callback: impl for<'s> FnOnce(
            &PendingCanonicalSelectedMemoryPoliciesV30<'s, 'g>,
            &mut Budget<'w>,
        ) -> Result<T, Failure>,
    ) -> Result<T, CanonicalRankedPolicyChecksErrorV1> {
        let capture = ConditionalOwnerCaptureV30 { callback };
        self.with_conditional_memory_limits_v30(
            physical,
            Domains::Selected(domains),
            limits,
            budget,
            move |core, budget| {
                let ConditionalOwnerCaptureV30 { callback } = capture;
                callback(&PendingCanonicalSelectedMemoryPoliciesV30 { core }, budget)
            },
        )
    }

    fn with_conditional_memory_limits_v30<'w, T>(
        &mut self,
        physical: &CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
        globals: Domains<'_>,
        limits: Limits,
        budget: &mut Budget<'w>,
        callback: impl for<'s> FnOnce(
            &PendingConditionalMemoryCoreV30<'s, 'g>,
            &mut Budget<'w>,
        ) -> Result<T, Failure>,
    ) -> Result<T, CanonicalRankedPolicyChecksErrorV1> {
        let mut analysis = PrivateAnalysisV1::new(limits);
        let mut callback = Some(callback);
        let result = protected_retained_v18(budget, self.refund_denied, |budget| {
            let caught = catch_unwind(AssertUnwindSafe(|| {
                self.check(budget)?;
                if !std::ptr::eq(physical.inventory().owner(), self.owner) {
                    return Err(self.guard.exact_graph());
                }
                budget.reserve_storage(mixed_pending_headers_v26::<T>(
                    std::mem::size_of_val(&callback),
                    std::mem::align_of_val(&callback),
                )?)?;
                if !std::ptr::eq(
                    globals
                        .owner(budget)
                        .map_err(Failure::ConditionalGlobalsV26)?,
                    self.owner,
                ) {
                    return Err(self.guard.exact_graph());
                }
                let count = self.owner.module().functions.len();
                let mut reports = reserve_rows::<Option<MixedReportRowV26>>(count, budget)?;
                budget.charge_work(count)?;
                reports.resize_with(count, || None);
                self.graph.visit_conditional_policy_functions_v30(
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
                let view = PendingConditionalMemoryCoreV30 {
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
                    let returned =
                        callback.take().ok_or(Failure::InvocationAccounting)?(&view, budget);
                    match returned {
                        Err(error) if conditional_accounting_v30(&error) => {
                            Err(view.refuse_retained_custody())
                        }
                        other => other,
                    }
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
            let returned = match caught {
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
            };
            match returned {
                Err(error) if conditional_accounting_v30(&error) => {
                    globals.refuse_retained_custody();
                    Err(self.refuse_retained_custody())
                }
                other => other,
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

#[cfg(test)]
mod selected_frame_tests {
    use super::*;

    #[test]
    fn selected_native_pending_owner_and_capture_frames_have_independent_extents() {
        type CoreFields<'a> = (
            &'a VerifiedCanonicalKernelIrModuleV18,
            &'a [Option<MixedReportRowV26>],
            CanonicalRankedPolicyResourceObservationV1,
            &'a Guard,
            &'a [CanonicalRankedSourceObligationV18],
            Domains<'a>,
            &'a CheckedCanonicalKirPrivateMemoryV18<'a, 'a>,
            PendingCanonicalGlobalAccessesV18<'a, 'a>,
            usize,
            usize,
            Ledger,
        );
        assert_eq!(
            size_of::<PendingConditionalMemoryCoreV30<'_, '_>>(),
            size_of::<CoreFields<'_>>()
        );
        assert_eq!(
            size_of::<PendingCanonicalSelectedMemoryPoliciesV30<'_, '_>>(),
            size_of::<&PendingConditionalMemoryCoreV30<'_, '_>>()
        );
        assert_eq!(
            size_of::<PendingCanonicalMixedMemoryPoliciesV26<'_, '_>>(),
            size_of::<&PendingConditionalMemoryCoreV30<'_, '_>>()
        );
        let capture = ConditionalOwnerCaptureV30 {
            callback: [0u8; 257],
        };
        let adapter = move || {
            let ConditionalOwnerCaptureV30 { callback } = capture;
            callback
        };
        let capture_extent = std::mem::size_of_val(&Some(adapter));
        assert_eq!(capture_extent, size_of::<Option<[u8; 257]>>());
        let empty = mixed_pending_headers_v26::<()>(0, 1).unwrap();
        assert_eq!(
            mixed_pending_headers_v26::<()>(capture_extent, 1).unwrap(),
            empty + capture_extent
        );

        fn result_frames<T>() -> usize {
            type Public<T> = Result<T, CanonicalRankedPolicyChecksErrorV1>;
            type Local<T> = Result<T, Failure>;
            type Caught<T> = std::thread::Result<Local<T>>;
            size_of::<Public<T>>()
                + 2 * size_of::<Result<Public<T>, Failure>>()
                + size_of::<Local<T>>()
                + 2 * size_of::<Result<Local<T>, Failure>>()
                + size_of::<Caught<T>>()
                + 2 * size_of::<Result<Caught<T>, Failure>>()
        }
        assert_eq!(
            mixed_pending_headers_v26::<[u8; 257]>(0, 1).unwrap() - empty,
            result_frames::<[u8; 257]>() - result_frames::<()>()
        );
    }
}
