//! Fixed Policy9 worklist closure and independent actual-V18 adoption.
use super::checked_neutral_optimization_v1::{
    AdoptionProfile, CheckedParts, ObservedParts, check_and_finish_parts_typed,
};
use super::storage_v18::KirNeutralOptimizationErrorV18;
use crate::{
    IntegerWorklistExecutionWitnessV18, KirBridgeReportV18, KirCheckedNeutralOptimizationErrorV1,
    KirCheckedNeutralOptimizationStorageV1, KirNeutralOccurrenceRowsV1,
    KirNeutralOptimizationStorageV1, KirNeutralOwnedOriginStorageV1,
    KirOptimizationMapIntegerWorklistV18, PlironOptimizationReportV1, fixed_policy_v3::FixedPolicy,
};
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryV18, CheckedCanonicalKirTransitionV18, check_canonical_kir_transition_v18,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, StorageLayoutLimitsV1,
    VerifiedCanonicalKernelIrModuleV18 as Owner,
};
use std::{convert::Infallible, mem::size_of};

type Observed<'a> = ObservedParts<
    'a,
    KirOptimizationMapIntegerWorklistV18,
    IntegerWorklistExecutionWitnessV18,
    Owner,
    KirBridgeReportV18,
>;
type Checked = CheckedParts<
    KirOptimizationMapIntegerWorklistV18,
    IntegerWorklistExecutionWitnessV18,
    Owner,
    KirBridgeReportV18,
>;

/// Actual worklist-closure/DCE observation, distinct from historical Policy6.
/// It grants no source, memory, or publication rights.
pub struct KirNeutralOptimizationOutputIntegerWorklistV18<'input> {
    parts: Observed<'input>,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    slot: usize,
    caller_floor: usize,
}

/// Move-only actual V18 successor after independent occurrence-rule checking.
/// It is not a Policy3, Policy6 or V12 owner and does not authorize final code generation.
///
/// ```compile_fail
/// use fe2o3_pliron::{CheckedNeutralKernelIrOwnerIntegerWorklistV18, CheckedNeutralKernelIrOwnerIntegerContinuationV18};
/// fn erase(v: CheckedNeutralKernelIrOwnerIntegerWorklistV18) -> CheckedNeutralKernelIrOwnerIntegerContinuationV18 { v }
/// ```
pub struct CheckedNeutralKernelIrOwnerIntegerWorklistV18 {
    parts: Checked,
}

fn wrapper<T>() -> Option<usize> {
    [
        size_of::<Owner>(),
        size_of::<PlironOptimizationReportV1>(),
        size_of::<KirBridgeReportV18>(),
        size_of::<KirOptimizationMapIntegerWorklistV18>(),
        size_of::<KirNeutralOccurrenceRowsV1>(),
    ]
    .into_iter()
    .try_fold(size_of::<T>(), usize::checked_sub)
}
fn observed_wrapper() -> Option<usize> {
    wrapper::<KirNeutralOptimizationOutputIntegerWorklistV18<'_>>()
}
fn checked_wrapper() -> Option<usize> {
    wrapper::<CheckedNeutralKernelIrOwnerIntegerWorklistV18>()
}

/// Executes only the fixed def-use integer-neutral worklist and DCE on
/// the actual V18 graph. No V12 re-encoding, pass substitution or fallback occurs.
/// Reserve the returned receipt before any subsequent controlled allocation.
pub fn optimize_neutral_kernel_ir_integer_worklist_v18<'input>(
    input: &'input Owner,
    layouts: StorageLayoutLimitsV1,
    budget: &mut Budget<'_>,
) -> Result<KirNeutralOptimizationOutputIntegerWorklistV18<'input>, KirNeutralOptimizationErrorV18>
{
    let ledger = budget.work_ledger_identity_v1();
    let slot = std::ptr::from_ref(budget) as usize;
    let caller_floor = budget.storage();
    let wrapper = observed_wrapper().ok_or(Resource::Arithmetic)?;
    let crate::kir_bridge_v1::ExecutedV18Parts {
        owner,
        report,
        bridge,
        map,
        occurrences,
        execution,
        retained,
    } = crate::kir_bridge_v1::optimize_integer_worklist_v18_graph(input, layouts, wrapper, budget)?;
    Ok(KirNeutralOptimizationOutputIntegerWorklistV18 {
        ledger,
        slot,
        caller_floor,
        parts: ObservedParts {
            input,
            owner,
            report,
            bridge,
            map,
            occurrences,
            extra: execution,
            storage: KirNeutralOptimizationStorageV1 { retained },
        },
    })
}

impl<'input> KirNeutralOptimizationOutputIntegerWorklistV18<'input> {
    pub const fn input(&self) -> &'input Owner {
        self.parts.input
    }
    pub const fn owner(&self) -> &Owner {
        &self.parts.owner
    }
    pub const fn report(&self) -> &PlironOptimizationReportV1 {
        &self.parts.report
    }
    pub const fn map(&self) -> &KirOptimizationMapIntegerWorklistV18 {
        &self.parts.map
    }
    pub const fn occurrences(&self) -> &KirNeutralOccurrenceRowsV1 {
        &self.parts.occurrences
    }
    pub const fn execution(&self) -> &IntegerWorklistExecutionWitnessV18 {
        &self.parts.extra
    }
    pub const fn storage(&self) -> KirNeutralOptimizationStorageV1 {
        self.parts.storage
    }

    pub fn try_check_and_finish_v18(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<CheckedNeutralKernelIrOwnerIntegerWorklistV18, KirCheckedNeutralOptimizationErrorV1>
    {
        self.try_check_and_finish_with_v18(budget, |_, _| Ok::<_, Infallible>(((), 0)))
            .map(|(owner, (), _)| owner)
    }

    /// Consuming same-ledger adoption with bounded cleanup and original error
    /// selection. Temporary input/output borrows cannot escape the owned result.
    pub fn try_check_and_finish_with_v18<T: 'static, E: 'static, F>(
        self,
        budget: &mut Budget<'_>,
        source: F,
    ) -> Result<
        (
            CheckedNeutralKernelIrOwnerIntegerWorklistV18,
            T,
            KirNeutralOwnedOriginStorageV1,
        ),
        KirCheckedNeutralOptimizationErrorV1<E>,
    >
    where
        F: for<'view, 'inventory, 'src, 'output, 'rows, 'work> FnOnce(
            &'view CheckedCanonicalKirTransitionV18<'inventory, 'src, 'output, 'rows>,
            &mut Budget<'work>,
        )
            -> Result<(T, usize), E>,
    {
        let required = self
            .caller_floor
            .checked_add(self.parts.storage.retained_storage())
            .ok_or(Resource::Accounting)?;
        if self.ledger != budget.work_ledger_identity_v1()
            || self.slot != std::ptr::from_ref(budget) as usize
            || budget.storage() < required
        {
            return Err(Resource::Accounting.into());
        }
        check_and_finish_parts_typed(
            self.parts,
            budget,
            source,
            observed_wrapper,
            checked_wrapper,
            FixedPolicy::IntegerWorklist9,
            AdoptionProfile {
                bytes: Owner::canonical_bytes,
                inventory: |owner, budget| CanonicalKirInventoryV18::derive_v18(owner, budget),
                transition: check_canonical_kir_transition_v18,
                bounded_cleanup: true,
            },
        )
        .map(|(parts, source, receipt)| {
            (
                CheckedNeutralKernelIrOwnerIntegerWorklistV18 { parts },
                source,
                receipt,
            )
        })
    }
}

impl CheckedNeutralKernelIrOwnerIntegerWorklistV18 {
    pub const fn owner(&self) -> &Owner {
        &self.parts.owner
    }
    pub const fn report(&self) -> &PlironOptimizationReportV1 {
        &self.parts.report
    }
    pub const fn bridge(&self) -> &KirBridgeReportV18 {
        &self.parts.bridge
    }
    pub const fn map(&self) -> &KirOptimizationMapIntegerWorklistV18 {
        &self.parts.map
    }
    pub const fn occurrences(&self) -> &KirNeutralOccurrenceRowsV1 {
        &self.parts.occurrences
    }
    pub const fn execution(&self) -> &IntegerWorklistExecutionWitnessV18 {
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
#[path = "neutral_integer_worklist_v18_tests.rs"]
mod tests;
