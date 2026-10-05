//! Storage-aware fixed integer-neutral execution and independent adoption.
use super::checked_neutral_optimization_v1::{
    AdoptionProfile, CheckedParts, ObservedParts, check_and_finish_parts_typed,
};
use super::storage_v18::KirNeutralOptimizationErrorV18;
use crate::{
    IntegerContinuationExecutionWitnessV18, KirBridgeReportV18,
    KirCheckedNeutralOptimizationErrorV1, KirCheckedNeutralOptimizationStorageV1,
    KirNeutralOccurrenceRowsV1, KirNeutralOptimizationStorageV1, KirNeutralOwnedOriginStorageV1,
    KirOptimizationMapIntegerContinuationV18, PlironOptimizationReportV1,
    fixed_policy_v3::FixedPolicy,
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
    KirOptimizationMapIntegerContinuationV18,
    IntegerContinuationExecutionWitnessV18,
    Owner,
    KirBridgeReportV18,
>;
type Checked = CheckedParts<
    KirOptimizationMapIntegerContinuationV18,
    IntegerContinuationExecutionWitnessV18,
    Owner,
    KirBridgeReportV18,
>;

/// Actual two-pass observation. It grants no source, memory, or publication rights.
pub struct KirNeutralOptimizationOutputIntegerContinuationV18<'input> {
    parts: Observed<'input>,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    slot: usize,
    caller_floor: usize,
}

/// Move-only actual V18 successor after independent occurrence-rule checking.
/// It is not a Policy3 or V12 owner and does not authorize final code generation.
///
/// ```compile_fail
/// use fe2o3_pliron::{CheckedNeutralKernelIrOwnerIntegerContinuationV18, CheckedNeutralKernelIrOwnerV18};
/// fn erase(v: CheckedNeutralKernelIrOwnerIntegerContinuationV18) -> CheckedNeutralKernelIrOwnerV18 { v }
/// ```
pub struct CheckedNeutralKernelIrOwnerIntegerContinuationV18 {
    parts: Checked,
}

fn wrapper<T>() -> Option<usize> {
    [
        size_of::<Owner>(),
        size_of::<PlironOptimizationReportV1>(),
        size_of::<KirBridgeReportV18>(),
        size_of::<KirOptimizationMapIntegerContinuationV18>(),
        size_of::<KirNeutralOccurrenceRowsV1>(),
    ]
    .into_iter()
    .try_fold(size_of::<T>(), usize::checked_sub)
}
fn observed_wrapper() -> Option<usize> {
    wrapper::<KirNeutralOptimizationOutputIntegerContinuationV18<'_>>()
}
fn checked_wrapper() -> Option<usize> {
    wrapper::<CheckedNeutralKernelIrOwnerIntegerContinuationV18>()
}

/// Executes only the fixed existing integer-neutral and DCE implementations on
/// the actual V18 graph. No V12 re-encoding, pass substitution or fallback occurs.
/// Reserve the returned receipt before any subsequent controlled allocation.
pub fn optimize_neutral_kernel_ir_integer_continuation_v18<'input>(
    input: &'input Owner,
    layouts: StorageLayoutLimitsV1,
    budget: &mut Budget<'_>,
) -> Result<
    KirNeutralOptimizationOutputIntegerContinuationV18<'input>,
    KirNeutralOptimizationErrorV18,
> {
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
    } = crate::kir_bridge_v1::optimize_integer_v18_graph(input, layouts, wrapper, budget)?;
    Ok(KirNeutralOptimizationOutputIntegerContinuationV18 {
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

impl<'input> KirNeutralOptimizationOutputIntegerContinuationV18<'input> {
    pub const fn input(&self) -> &'input Owner {
        self.parts.input
    }
    pub const fn owner(&self) -> &Owner {
        &self.parts.owner
    }
    pub const fn report(&self) -> &PlironOptimizationReportV1 {
        &self.parts.report
    }
    pub const fn map(&self) -> &KirOptimizationMapIntegerContinuationV18 {
        &self.parts.map
    }
    pub const fn occurrences(&self) -> &KirNeutralOccurrenceRowsV1 {
        &self.parts.occurrences
    }
    pub const fn execution(&self) -> &IntegerContinuationExecutionWitnessV18 {
        &self.parts.extra
    }
    pub const fn storage(&self) -> KirNeutralOptimizationStorageV1 {
        self.parts.storage
    }

    pub fn try_check_and_finish_v18(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<
        CheckedNeutralKernelIrOwnerIntegerContinuationV18,
        KirCheckedNeutralOptimizationErrorV1,
    > {
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
            CheckedNeutralKernelIrOwnerIntegerContinuationV18,
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
            .checked_add(self.parts.storage.retained_storage());
        if self.ledger != budget.work_ledger_identity_v1()
            || self.slot != std::ptr::from_ref(budget) as usize
            || required.is_none_or(|required| budget.storage() < required)
        {
            // An uncalled capture can panic during destruction as well.
            super::checked_neutral_optimization_v1::drop_adoption_result(source, true);
            return Err(Resource::Accounting.into());
        }
        check_and_finish_parts_typed(
            self.parts,
            budget,
            source,
            observed_wrapper,
            checked_wrapper,
            FixedPolicy::Integer6,
            AdoptionProfile {
                bytes: Owner::canonical_bytes,
                inventory: |owner, budget| CanonicalKirInventoryV18::derive_v18(owner, budget),
                transition: check_canonical_kir_transition_v18,
                bounded_cleanup: true,
            },
        )
        .map(|(parts, source, receipt)| {
            (
                CheckedNeutralKernelIrOwnerIntegerContinuationV18 { parts },
                source,
                receipt,
            )
        })
    }
}

impl CheckedNeutralKernelIrOwnerIntegerContinuationV18 {
    pub const fn owner(&self) -> &Owner {
        &self.parts.owner
    }
    pub const fn report(&self) -> &PlironOptimizationReportV1 {
        &self.parts.report
    }
    pub const fn bridge(&self) -> &KirBridgeReportV18 {
        &self.parts.bridge
    }
    pub const fn map(&self) -> &KirOptimizationMapIntegerContinuationV18 {
        &self.parts.map
    }
    pub const fn occurrences(&self) -> &KirNeutralOccurrenceRowsV1 {
        &self.parts.occurrences
    }
    pub const fn execution(&self) -> &IntegerContinuationExecutionWitnessV18 {
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
