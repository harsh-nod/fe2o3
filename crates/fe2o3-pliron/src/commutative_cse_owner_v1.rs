//! One privately imported, actually executed and independently checked successor.
//! No numbered policy, wire witness, source proof, artifact or launch authority.

use crate::{
    KirBridgeErrorV12, KirBridgeOptimizedReceiptV1, KirNeutralOccurrenceRowsV1,
    KirOptimizationMapErrorV12, OperationGraphReplayIdentityV1, PlironOptimizationErrorV1,
};
use fe2o3_kernel_analysis::{
    CanonicalKirCommutativeBitwiseCseErrorV1, CanonicalKirInventoryErrorV1,
    CanonicalKirInventoryV1 as Inventory, check_canonical_kir_commutative_bitwise_cse_v1 as check,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{fmt, mem::size_of};

#[path = "commutative_cse_owner_resources_v1.rs"]
pub(crate) mod resources;

#[path = "owned_commutative_continuation_v1.rs"]
mod owned;
pub use owned::{
    OwnedCommutativeBitwiseContinuationV1, prepare_owned_commutative_bitwise_continuation_v1,
};

/// Exact resource/custody/capture/execution/checker refusal of this closed service.
#[derive(Debug)]
pub enum CommutativeBitwiseOptimizationErrorV1 {
    /// One live canonical budget refused work, storage or accounting.
    Resource(Resource),
    /// Actual import or native-witness extraction failed.
    Bridge(KirBridgeErrorV12),
    /// Actual session transaction or verification failed; candidate is discarded.
    Execution(PlironOptimizationErrorV1),
    /// Complete event/occurrence custody failed; no old map is constructed.
    Capture(KirOptimizationMapErrorV12),
    /// Actual canonical subject inventory was refused.
    Inventory(CanonicalKirInventoryErrorV1),
    /// Independent complete actual-pair relation was refused.
    Relation(CanonicalKirCommutativeBitwiseCseErrorV1),
    /// The one closed raw pass failed without a more specific resource refusal.
    PassRejected,
    /// A private stage unwound. This does not roll back raw mutation.
    Panicked,
}
pub(crate) type Error = CommutativeBitwiseOptimizationErrorV1;
impl From<Resource> for Error {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "checked commutative-bitwise service: {self:?}")
    }
}
impl std::error::Error for Error {}

/// In-process observation of the actual single pass, not a wire claim or policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommutativeBitwiseExecutionV1 {
    pub(crate) before: OperationGraphReplayIdentityV1,
    pub(crate) after: OperationGraphReplayIdentityV1,
    pub(crate) changed: bool,
    pub(crate) input_work: usize,
    pub(crate) output_work: usize,
    pub(crate) dynamic_work: usize,
}
impl CommutativeBitwiseExecutionV1 {
    /// Actual before/after graph identities, including committed epochs.
    pub const fn graphs(
        self,
    ) -> (
        OperationGraphReplayIdentityV1,
        OperationGraphReplayIdentityV1,
    ) {
        (self.before, self.after)
    }
    /// Actual raw pass status, checked against nonzero independently proved deletions.
    pub const fn changed(self) -> bool {
        self.changed
    }
    /// Dynamically charged raw traversal/comparison work, not the opaque envelope.
    pub const fn dynamic_work(self) -> usize {
        self.dynamic_work
    }
}

/// Move-only output and complete rows tied to the actual borrowed input.
/// It is not convertible to a historical checked owner or runtime admission.
///
/// ```compile_fail
/// use fe2o3_pliron::CheckedCommutativeBitwiseOptimizationV1;
/// fn clone_owner(owner: CheckedCommutativeBitwiseOptimizationV1<'_>) { let _ = owner.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_pliron::{CheckedCommutativeBitwiseOptimizationV1, CheckedNeutralKernelIrOwnerV1};
/// fn relabel(owner: CheckedCommutativeBitwiseOptimizationV1<'_>) -> CheckedNeutralKernelIrOwnerV1 { owner }
/// ```
/// ```compile_fail
/// use fe2o3_pliron::{CheckedCommutativeBitwiseOptimizationV1, optimize_checked_commutative_bitwise_cse_v1 as run};
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as Owner, CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// fn detach(input: Owner, budget: &mut Budget<'_>) -> CheckedCommutativeBitwiseOptimizationV1<'static> {
///     run(&input, budget).unwrap()
/// }
/// ```
/// ```
/// use fe2o3_pliron::{CheckedCommutativeBitwiseOptimizationV1, CommutativeBitwiseOptimizationErrorV1, optimize_checked_commutative_bitwise_cse_v1 as run};
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as Owner, CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// fn tied<'a>(input: &'a Owner, budget: &mut Budget<'_>) -> Result<CheckedCommutativeBitwiseOptimizationV1<'a>, CommutativeBitwiseOptimizationErrorV1> {
///     run(input, budget)
/// }
/// ```
pub struct CheckedCommutativeBitwiseOptimizationV1<'input> {
    input: &'input Owner,
    output: Owner,
    bridge: KirBridgeOptimizedReceiptV1,
    rows: KirNeutralOccurrenceRowsV1,
    execution: CommutativeBitwiseExecutionV1,
    proved_pairs: usize,
    retained: usize,
}
impl<'input> CheckedCommutativeBitwiseOptimizationV1<'input> {
    /// Exact caller-owned input, never a reconstructed equivalent module.
    pub const fn input(&self) -> &'input Owner {
        self.input
    }
    /// The actual once-extracted output of this invocation.
    pub const fn owner(&self) -> &Owner {
        &self.output
    }
    /// Complete immutable captured rows, independently checked against both owners.
    pub const fn occurrences(&self) -> &KirNeutralOccurrenceRowsV1 {
        &self.rows
    }
    /// Actual execution, not caller-supplied claims.
    pub const fn execution(&self) -> CommutativeBitwiseExecutionV1 {
        self.execution
    }
    /// Structural receipt from the actual native-witness extraction.
    pub const fn bridge(&self) -> &KirBridgeOptimizedReceiptV1 {
        &self.bridge
    }
    /// Number of omitted definitions independently proved equivalent to retained anchors.
    pub const fn proved_pairs(&self) -> usize {
        self.proved_pairs
    }
    /// Unreserved transfer receipt; reserve before subsequent allocation.
    /// Inherited graph/import/checker envelopes remain logical units, not RSS.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    /// This component grants no source, artifact, protected-admission or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
    /// Independently recheck these same retained subjects and rows in one scope.
    /// The returned count is inert; inventory and checked-view borrows cannot escape.
    pub fn replay(&self, budget: &mut Budget<'_>) -> Result<usize, Error> {
        let count = resources::checked_pair(self.input, &self.output, &self.rows, budget)?;
        if count != self.proved_pairs || self.execution.changed != (count != 0) {
            return Err(Resource::Accounting.into());
        }
        Ok(count)
    }
}
impl fmt::Debug for CheckedCommutativeBitwiseOptimizationV1<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CheckedCommutativeBitwiseOptimizationV1")
            .field("execution", &self.execution)
            .field("proved_pairs", &self.proved_pairs)
            .field("retained", &self.retained)
            .finish_non_exhaustive()
    }
}

/// Runs exactly the closed commutative fixed-integer bitwise raw pass on one
/// genuinely imported native V12 candidate, captures complete actual occurrences,
/// extracts once, and independently checks the new relation. Errors discard the
/// private candidate, never return a partly transformed owner. Input is excluded
/// from the unreserved returned receipt; all owned scratch drops before cleanup.
/// No policy/default route, wire record, user callback or alternate graph is accepted.
pub fn optimize_checked_commutative_bitwise_cse_v1<'input>(
    input: &'input Owner,
    budget: &mut Budget<'_>,
) -> Result<CheckedCommutativeBitwiseOptimizationV1<'input>, Error> {
    #[cfg(test)]
    {
        optimize(input, budget, 0)
    }
    #[cfg(not(test))]
    {
        optimize(input, budget)
    }
}

fn optimize<'input>(
    input: &'input Owner,
    budget: &mut Budget<'_>,
    #[cfg(test)] fault: u8,
) -> Result<CheckedCommutativeBitwiseOptimizationV1<'input>, Error> {
    resources::scoped(budget, |budget, binding| {
        let wrapper = size_of::<CheckedCommutativeBitwiseOptimizationV1<'_>>()
            .checked_sub(size_of::<Owner>())
            .and_then(|n| n.checked_sub(size_of::<KirBridgeOptimizedReceiptV1>()))
            .and_then(|n| n.checked_sub(size_of::<KirNeutralOccurrenceRowsV1>()))
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(wrapper)?;
        let (mut graph, witness) =
            crate::kir_bridge_v1::import_native_neutral_v1(input, budget).map_err(Error::Bridge)?;
        binding.check(budget)?;
        let limits = graph
            .neutral_occurrence_limits_v1(budget)
            .map_err(Error::Capture)?;
        budget.charge_work(limits.work().map_err(Error::Capture)?)?;
        budget.reserve_storage(limits.storage().map_err(Error::Capture)?)?;
        let capture_floor = budget.storage();
        let capture = graph
            .begin_commutative_capture_v1(limits, budget)
            .map_err(Error::Capture)?;
        #[cfg(test)]
        capture.set_fault(fault);
        binding.check(budget)?;
        if capture_floor.checked_add(capture.lineage_storage().map_err(Error::Capture)?)
            != Some(budget.storage())
        {
            return Err(Resource::Accounting.into());
        }
        let execution = graph.execute_commutative_cse_v1(&capture, budget);
        #[cfg(test)]
        if fault == 1 || fault == 2 {
            assert!(
                capture.actual_mutation_seen(),
                "fault must follow independently observed physical mutation"
            );
        }
        let execution = execution?;
        #[cfg(test)]
        if fault >= 3 {
            capture.tamper_lineage(fault).map_err(Error::Capture)?;
        }
        binding.check(budget)?;
        let extraction_floor = budget.storage();
        let (output, bridge, storage) = graph
            .extract_commutative_cse_v1(&witness, budget)
            .map_err(Error::Bridge)?;
        // The private extraction retains this storage; no unreserved gap.
        binding.check(budget)?;
        if extraction_floor.checked_add(storage.retained_storage()) != Some(budget.storage()) {
            return Err(Resource::Accounting.into());
        }
        let roster_storage = limits.storage().map_err(Error::Capture)?;
        budget.reserve_storage(roster_storage)?;
        let roster = capture
            .with_roster_meter(|meter| graph.neutral_live_roster_v1(limits.nodes, meter))
            .map_err(Error::Capture)?;
        let rows_floor = budget.storage();
        let rows = capture
            .finish(&graph.session.context, &roster, output.module(), budget)
            .map_err(Error::Capture)?;
        let rows_storage = rows.retained_storage().map_err(Error::Capture)?;
        binding.check(budget)?;
        if rows_floor.checked_add(rows_storage) != Some(budget.storage()) {
            return Err(Resource::Accounting.into());
        }
        drop(roster);
        budget.release_storage(roster_storage)?;
        let proved_pairs = resources::checked_pair(input, &output, &rows, budget)?;
        binding.check(budget)?;
        if execution.changed != (proved_pairs != 0) {
            return Err(Resource::Accounting.into());
        }
        let retained = storage
            .retained_storage()
            .checked_add(rows_storage)
            .and_then(|n| n.checked_add(wrapper))
            .ok_or(Resource::Arithmetic)?;
        let result = CheckedCommutativeBitwiseOptimizationV1 {
            input,
            output,
            bridge,
            rows,
            execution,
            proved_pairs,
            retained,
        };
        drop(capture);
        drop(witness);
        drop(graph);
        Ok(result)
    })
}

#[cfg(test)]
#[path = "commutative_cse_owner_v1_tests.rs"]
mod tests;
