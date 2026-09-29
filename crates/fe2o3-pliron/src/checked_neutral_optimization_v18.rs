//! Same consuming adoption algorithm, with the nominal V18 independent checker.
use super::super::checked_neutral_optimization_v1::{
    AdoptionProfile, check_and_finish_parts_typed,
};
use super::*;
use crate::fixed_policy_v3::FixedPolicy;
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryV18, CheckedCanonicalKirTransitionV18, check_canonical_kir_transition_v18,
};
use std::convert::Infallible;

/// Actual optimized V18 graph after complete independent scalar/CFG relation
/// checking. The storage table and ordered operation payloads remain exact.
/// This type does not authorize ranked, formal, source, native or publication
/// conclusions; those consumers must independently join this exact output.
///
/// ```compile_fail
/// use fe2o3_pliron::{CheckedNeutralKernelIrOwnerV18, CheckedNeutralKernelIrOwnerPolicy3V1};
/// fn erase(v: CheckedNeutralKernelIrOwnerV18) -> CheckedNeutralKernelIrOwnerPolicy3V1 { v }
/// ```
/// ```compile_fail
/// use fe2o3_pliron::CheckedNeutralKernelIrOwnerV18;
/// fn copy(v: CheckedNeutralKernelIrOwnerV18) { let _ = v.clone(); }
/// ```
pub struct CheckedNeutralKernelIrOwnerV18 {
    parts: Checked,
}

impl KirNeutralOptimizationOutputV18<'_> {
    pub fn try_check_and_finish_v18(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<CheckedNeutralKernelIrOwnerV18, KirCheckedNeutralOptimizationErrorV1> {
        self.try_check_and_finish_with_v18(budget, |_, _| Ok::<_, Infallible>(((), 0)))
            .map(|(owner, (), _)| owner)
    }

    /// The observed receipt must be reserved. This consumes that reservation,
    /// restores the remaining caller floor, and transfers the two returned
    /// receipts. The callback must restore its floor and return a fully paid
    /// owned payload. No temporary checked borrow or captured input can escape.
    /// Use the original budget instance from observation, without moving it.
    /// A changed callback floor rejects custody and leaves its residual storage
    /// untouched, including on unwind. A returned callback error remains the
    /// selected diagnostic; cleanup does not replace it with a floor error.
    ///
    /// ```compile_fail
    /// use fe2o3_pliron::KirNeutralOptimizationOutputV18;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn escape(v: KirNeutralOptimizationOutputV18<'_>, budget: &mut Budget<'_>) {
    ///     let _ = v.try_check_and_finish_with_v18(budget, |checked, _| {
    ///         Ok::<_, ()>((checked.output(), 0))
    ///     });
    /// }
    /// ```
    pub fn try_check_and_finish_with_v18<T: 'static, E: 'static, F>(
        self,
        budget: &mut Budget<'_>,
        source: F,
    ) -> Result<
        (
            CheckedNeutralKernelIrOwnerV18,
            T,
            KirNeutralOwnedOriginStorageV1,
        ),
        KirCheckedNeutralOptimizationErrorV1<E>,
    >
    where
        F: for<'view, 'inventory, 'input, 'output, 'rows, 'work> FnOnce(
            &'view CheckedCanonicalKirTransitionV18<'inventory, 'input, 'output, 'rows>,
            &mut Budget<'work>,
        )
            -> Result<(T, usize), E>,
    {
        let required = self
            .caller_floor
            .checked_add(self.parts.storage.retained_storage());
        if self.ledger != budget.work_ledger_identity_v1()
            || self.slot != std::ptr::from_ref(budget) as usize
            || required.is_none_or(|required| budget.storage() < required)
        {
            // A foreign ledger cannot refund the original observed custody.
            super::super::checked_neutral_optimization_v1::drop_adoption_result(source, true);
            return Err(Resource::Accounting.into());
        }
        check_and_finish_parts_typed(
            self.parts,
            budget,
            source,
            observed_wrapper,
            checked_wrapper,
            FixedPolicy::Checked3,
            AdoptionProfile {
                bytes: Owner::canonical_bytes,
                inventory: |owner, budget| CanonicalKirInventoryV18::derive_v18(owner, budget),
                transition: check_canonical_kir_transition_v18,
                bounded_cleanup: true,
            },
        )
        .map(|(parts, source, receipt)| (CheckedNeutralKernelIrOwnerV18 { parts }, source, receipt))
    }
}

impl CheckedNeutralKernelIrOwnerV18 {
    pub const fn owner(&self) -> &Owner {
        &self.parts.owner
    }
    pub const fn report(&self) -> &PlironOptimizationReportV1 {
        &self.parts.report
    }
    pub const fn bridge(&self) -> &KirBridgeReportV18 {
        &self.parts.bridge
    }
    pub const fn map(&self) -> &KirOptimizationMapPolicy3V18 {
        &self.parts.map
    }
    pub const fn occurrences(&self) -> &KirNeutralOccurrenceRowsV1 {
        &self.parts.occurrences
    }
    pub const fn execution(&self) -> &Policy3ExecutionWitnessV18 {
        &self.parts.extra
    }
    pub fn input_audit_bytes(&self) -> &[u8] {
        &self.parts.input_history
    }
    pub const fn storage(&self) -> KirCheckedNeutralOptimizationStorageV1 {
        self.parts.storage
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

#[cfg(test)]
#[path = "checked_neutral_optimization_v18_hostile_tests.rs"]
mod hostile_tests;
#[cfg(test)]
#[path = "checked_neutral_optimization_v18_resource_tests.rs"]
mod resource_tests;
#[cfg(test)]
#[path = "checked_neutral_optimization_v18_tests.rs"]
mod tests;
