//! Explicit physical-memory profile; original census-based entry stays closed.
use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryErrorV1 as InventoryError,
    CanonicalKirPrivateMemoryErrorV1 as MemoryError,
    CanonicalKirPrivateMemoryLimitsV1 as MemoryLimits, check_canonical_kir_private_memory_v1,
};

impl<'i, 'g> CanonicalPrivateGraphFactsV1<'i, 'g> {
    fn derive_physical(
        inventory: &'i Inventory<'g>,
        terminals: &'i traps::CanonicalTrapPairsGraphFactsV1<'i, 'g>,
        limits: MemoryLimits,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Failure> {
        if !terminals.belongs_to(inventory) {
            return Err(Failure::ExactGraph);
        }
        budget.reserve_storage(size_of::<Self>())?;
        let (proof, receipt) = check_canonical_kir_private_memory_v1(inventory, limits, budget)
            .map_err(memory_error)?;
        budget.reserve_storage(receipt.retained_storage())?;
        Self::finish_profile(
            inventory,
            PrivateMemoryEvidenceV1::Physical(proof),
            Some(terminals),
            budget,
        )
    }
}

fn memory_error(error: MemoryError) -> Failure {
    match error {
        MemoryError::Resource(error) | MemoryError::Inventory(InventoryError::Resource(error)) => {
            Failure::Resource(error)
        }
        MemoryError::Inventory(_) => Failure::ExactGraph,
        MemoryError::Unsupported { .. } => {
            refuse(CanonicalPrivateRequirementV1::CompleteCells, None)
        }
        MemoryError::Panicked => Failure::Panicked,
    }
}

/// Exact physical proof plus a fresh full-nine/trap view on the same owner.
/// This is neither source/lifetime equivalence nor artifact/launch authority.
///
/// ```compile_fail
/// use fe2o3_pliron::CheckedCanonicalPrivateMemoryPoliciesV1;
/// fn forge() { let _ = CheckedCanonicalPrivateMemoryPoliciesV1 {}; }
/// ```
/// ```compile_fail
/// use fe2o3_pliron::CheckedCanonicalPrivateMemoryPoliciesV1;
/// fn duplicate(value: CheckedCanonicalPrivateMemoryPoliciesV1<'_, '_>) {
///     let _ = value.clone();
/// }
/// ```
pub struct CheckedCanonicalPrivateMemoryPoliciesV1<'s, 'g> {
    inner: traps::CheckedCanonicalTrapPoliciesV1<'s, 'g>,
    physical: &'s PhysicalMemory<'s, 'g>,
}

impl<'s, 'g> CheckedCanonicalPrivateMemoryPoliciesV1<'s, 'g> {
    pub fn owner(&self, budget: &mut Budget<'_>) -> Result<&'g Owner, Failure> {
        self.inner.owner(budget)
    }

    /// Borrow the already checked physical proof, never a writable attachment.
    pub fn physical_memory(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<&PhysicalMemory<'s, 'g>, Failure> {
        self.inner.owner(budget)?;
        Ok(self.physical)
    }

    /// Shape and all nine actual policy reports; no source predicate truth.
    pub fn trap_policies(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<&traps::CheckedCanonicalTrapPoliciesV1<'s, 'g>, Failure> {
        self.inner.owner(budget)?;
        Ok(&self.inner)
    }

    pub const fn pending_obligations(&self) -> Obligations {
        self.inner.pending_obligations()
    }

    pub const fn ranked_verification_is_complete(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// Derives complete physical memory, terminal shape and call closure from the
/// actual checked inventory, then runs the shared nine producers on every
/// definition. The old ordinary/private entries do not select this profile.
/// Prepay escaping callback storage separately and restore the exact callback
/// floor. Native epoch, full schema and custody are checked before return.
///
/// ```compile_fail
/// use fe2o3_pliron::{CheckedCanonicalPrivateMemoryPoliciesV1,
///     with_canonical_private_memory_policy_checks_v1};
/// use fe2o3_kernel_analysis::{CheckedCanonicalRankedViewV1, CanonicalKirPrivateMemoryLimitsV1};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn escape<'a>(checked: &mut CheckedCanonicalRankedViewV1<'_, '_, '_, '_>,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>)
///     -> &'a CheckedCanonicalPrivateMemoryPoliciesV1<'a, 'a> {
///     with_canonical_private_memory_policy_checks_v1(
///         checked, CanonicalKirPrivateMemoryLimitsV1 { max_cells: 8 },
///         budget, |view, _| Ok(view)).unwrap()
/// }
/// ```
pub fn with_canonical_private_memory_policy_checks_v1<'w, T>(
    checked: &mut CheckedCanonicalRankedViewV1<'_, '_, '_, '_>,
    memory_limits: MemoryLimits,
    budget: &mut Budget<'w>,
    callback: impl for<'s, 'g> FnOnce(
        &CheckedCanonicalPrivateMemoryPoliciesV1<'s, 'g>,
        &mut Budget<'w>,
    ) -> Result<T, Failure>,
) -> Result<T, CanonicalRankedPolicyChecksErrorV1> {
    with_memory_checks(
        checked,
        memory_limits,
        budget,
        Limits::production_hard_ceiling(),
        callback,
    )
}

fn with_memory_checks<'w, T>(
    checked: &mut CheckedCanonicalRankedViewV1<'_, '_, '_, '_>,
    memory_limits: MemoryLimits,
    budget: &mut Budget<'w>,
    analysis_limits: Limits,
    callback: impl for<'s, 'g> FnOnce(
        &CheckedCanonicalPrivateMemoryPoliciesV1<'s, 'g>,
        &mut Budget<'w>,
    ) -> Result<T, Failure>,
) -> Result<T, CanonicalRankedPolicyChecksErrorV1> {
    let mut analysis = private_resources::PrivateAnalysisV1::new(analysis_limits);
    let result = protected(budget, |budget| {
        // This foundation query must precede every nested header reservation.
        let inventory = checked.inventory(budget)?;
        let owner = inventory.owner();
        budget.reserve_storage(checked_add(
            checked_add(
                size_of::<private_resources::PrivateAnalysisV1>(),
                size_of::<Guard>(),
            )?,
            checked_add(
                size_of::<CheckedCanonicalPrivateMemoryPoliciesV1<'_, '_>>(),
                checked_add(
                    size_of::<std::thread::Result<Result<T, Failure>>>(),
                    drain_header(),
                )?,
            )?,
        )?)?;
        let terminals = traps::CanonicalTrapPairsGraphFactsV1::derive(inventory, budget)?;
        let facts = CanonicalPrivateGraphFactsV1::derive_physical(
            inventory,
            &terminals,
            memory_limits,
            budget,
        )?;
        let mut projection = NativeCanonicalPrivateProjectionV1::import(&facts, budget)?;
        let mut reports =
            reserve_rows::<PrivateReportRowV1>(terminals.definitions().len(), budget)?;
        for &coordinate in terminals.definitions() {
            budget.charge_work(1)?;
            let ordinal = coordinate.0 as usize;
            let outcome =
                projection.with_function(ordinal, budget, |input| analysis.invoke(input))??;
            let history = analysis.last.ok_or(Failure::InvocationAccounting)?;
            if history.function() != ordinal {
                return Err(Failure::InvocationAccounting);
            }
            reports.push(PrivateReportRowV1 { outcome, history });
            projection.check_epoch()?;
        }
        facts.require_completed(reports.len(), budget)?;
        projection.check(budget)?;
        let guard = Guard::new(budget);
        let inner = CheckedCanonicalPrivatePoliciesV1 {
            owner,
            reports: &reports,
            guard: &guard,
            observation: analysis.observation(),
        };
        let PrivateMemoryEvidenceV1::Physical(physical) = &facts.cells else {
            return Err(Failure::ExactGraph);
        };
        let view = CheckedCanonicalPrivateMemoryPoliciesV1 {
            inner: traps::CheckedCanonicalTrapPoliciesV1::new(inner, &terminals),
            physical,
        };
        let result = guard.callback(budget, |budget| callback(&view, budget));
        if let Err(error) = projection.check_epoch() {
            resources::discard(result);
            return Err(error);
        }
        drop(projection);
        drop(reports);
        drop(facts);
        drop(terminals);
        analysis.release_reports()?;
        result
    });
    result.map_err(|failure| CanonicalRankedPolicyChecksErrorV1 {
        failure,
        observation: analysis.observation(),
        last_invocation: analysis.last,
    })
}

#[cfg(test)]
#[path = "canonical_private_memory_resources_v1_tests.rs"]
mod resource_tests;
#[cfg(test)]
#[path = "canonical_private_memory_v1_tests.rs"]
mod tests;
