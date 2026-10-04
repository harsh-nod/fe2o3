//! Private move-only detachment of one actually executed commutative result.
use super::*;
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV12;

/// Actual output, bridge and complete observations, without an input borrow.
/// The input identity is only a fast rejection check; every replay checks the
/// full input/output relation. Source custody belongs to a consuming source owner,
/// not this local component. No candidate/from-parts constructor is exposed.
///
/// ```compile_fail
/// use fe2o3_pliron::OwnedCommutativeBitwiseContinuationV1;
/// fn duplicate(value: OwnedCommutativeBitwiseContinuationV1) { let _ = value.clone(); }
/// ```
/// ```compile_fail,E0616
/// use fe2o3_pliron::OwnedCommutativeBitwiseContinuationV1;
/// fn fabricate(value: OwnedCommutativeBitwiseContinuationV1) { let _ = value.output; }
/// ```
/// ```
/// use fe2o3_pliron::{OwnedCommutativeBitwiseContinuationV1, CommutativeBitwiseOptimizationErrorV1, prepare_owned_commutative_bitwise_continuation_v1};
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as Owner, CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// fn detached(input: Owner, budget: &mut Budget<'_>) -> Result<OwnedCommutativeBitwiseContinuationV1, CommutativeBitwiseOptimizationErrorV1> {
///     let result = prepare_owned_commutative_bitwise_continuation_v1(&input, budget)?;
///     drop(input);
///     Ok(result)
/// }
/// ```
pub struct OwnedCommutativeBitwiseContinuationV1 {
    output: Owner,
    bridge: KirBridgeOptimizedReceiptV1,
    rows: KirNeutralOccurrenceRowsV1,
    execution: CommutativeBitwiseExecutionV1,
    input: VerifiedCanonicalKernelIrIdentityV12,
    proved_pairs: usize,
    retained: usize,
}
impl OwnedCommutativeBitwiseContinuationV1 {
    /// The actual once-extracted output, never a reconstruction.
    pub const fn output(&self) -> &Owner {
        &self.output
    }
    /// Typed identity of the input used by this actual invocation.
    pub const fn input_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV12 {
        &self.input
    }
    /// Complete captured occurrence tables, independently checked before return.
    pub const fn occurrences(&self) -> &KirNeutralOccurrenceRowsV1 {
        &self.rows
    }
    /// Structural bridge from the same actual native extraction.
    pub const fn bridge(&self) -> &KirBridgeOptimizedReceiptV1 {
        &self.bridge
    }
    /// Sealed observation of the actual pass; not a policy or wire claim.
    pub const fn execution(&self) -> CommutativeBitwiseExecutionV1 {
        self.execution
    }
    /// Number of independently proved omitted definitions, including nested pairs.
    pub const fn proved_pairs(&self) -> usize {
        self.proved_pairs
    }
    /// Additional unreserved transfer receipt; the original input is excluded.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    /// No source, artifact, protected admission or launch authority is granted.
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Reuses caller-prepaid inventories for one complete replay. The output
    /// inventory must borrow this exact output; the input identity is checked
    /// before the independent relation. The returned header is unreserved.
    /// Source consumers must derive the input inventory from their retained
    /// actual prefix, not use identity equality as producer custody.
    pub fn check_inventories_v1<'a, 'input, 'output>(
        &'a self,
        input: &'a Inventory<'input>,
        output: &'a Inventory<'output>,
        budget: &mut Budget<'_>,
    ) -> Result<
        (
            fe2o3_kernel_analysis::CheckedCanonicalKirCommutativeBitwiseCseV1<
                'a,
                'input,
                'output,
                'a,
            >,
            fe2o3_kernel_analysis::CanonicalKirCommutativeBitwiseCseStorageV1,
        ),
        Error,
    > {
        if budget.storage() < self.retained {
            return Err(Resource::Accounting.into());
        }
        resources::scoped(budget, |budget, binding| {
            budget.charge_work(4)?;
            if !std::ptr::eq(output.owner(), &self.output)
                || input.owner().canonical().identity() != &self.input
            {
                return Err(Resource::Accounting.into());
            }
            let result =
                check(input, output, self.rows.candidate(), budget).map_err(Error::Relation)?;
            binding.check(budget)?;
            if result.0.proved_pairs() != self.proved_pairs
                || self.execution.changed != (self.proved_pairs != 0)
            {
                return Err(Resource::Accounting.into());
            }
            Ok(result)
        })
    }

    /// Replays the complete independently checked pair against the supplied input.
    /// Equal-byte input owners still require this full check; this API alone does
    /// not establish producer custody. Both owned subjects stay separately prepaid.
    pub fn replay_against(&self, input: &Owner, budget: &mut Budget<'_>) -> Result<usize, Error> {
        if budget.storage() < self.retained {
            return Err(Resource::Accounting.into());
        }
        resources::scoped(budget, |budget, binding| {
            budget.charge_work(3)?;
            if input.canonical().identity() != &self.input {
                return Err(Error::Relation(
                    CanonicalKirCommutativeBitwiseCseErrorV1::Rule(
                        "owning continuation input identity",
                    ),
                ));
            }
            let count = resources::checked_pair(input, &self.output, &self.rows, budget)?;
            binding.check(budget)?;
            if count != self.proved_pairs || self.execution.changed != (count != 0) {
                return Err(Resource::Accounting.into());
            }
            Ok(count)
        })
    }
}

fn wrapper<T>() -> Result<usize, Error> {
    size_of::<T>()
        .checked_sub(size_of::<Owner>())
        .and_then(|n| n.checked_sub(size_of::<KirBridgeOptimizedReceiptV1>()))
        .and_then(|n| n.checked_sub(size_of::<KirNeutralOccurrenceRowsV1>()))
        .ok_or_else(|| Resource::Arithmetic.into())
}

/// Executes the existing closed service once, then privately moves its output,
/// bridge and owned rows. There is no second graph/import/pass or observation
/// copy. Success transfers only the new owner receipt, unreserved; all exits
/// preserve the caller's entry floor and cumulative work on the original ledger.
pub fn prepare_owned_commutative_bitwise_continuation_v1(
    input: &Owner,
    budget: &mut Budget<'_>,
) -> Result<OwnedCommutativeBitwiseContinuationV1, Error> {
    #[cfg(test)]
    {
        prepare(input, budget, 0)
    }
    #[cfg(not(test))]
    {
        prepare(input, budget)
    }
}

pub(super) fn prepare(
    input: &Owner,
    budget: &mut Budget<'_>,
    #[cfg(test)] fault: u8,
) -> Result<OwnedCommutativeBitwiseContinuationV1, Error> {
    resources::scoped(budget, |budget, binding| {
        #[cfg(test)]
        let borrowed = optimize(input, budget, fault)?;
        #[cfg(not(test))]
        let borrowed = optimize_checked_commutative_bitwise_cse_v1(input, budget)?;
        binding.check(budget)?;
        budget.reserve_storage(borrowed.retained_storage())?;
        detach(input, borrowed, budget)
    })
}

// Kept private to this constructor and its hostile component tests. An external
// same-byte result cannot be attached to a different source-owned prefix.
pub(super) fn detach(
    input: &Owner,
    borrowed: CheckedCommutativeBitwiseOptimizationV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<OwnedCommutativeBitwiseContinuationV1, Error> {
    if budget.storage() < borrowed.retained || !std::ptr::eq(input, borrowed.input) {
        return Err(Resource::Accounting.into());
    }
    budget.charge_work(6)?;
    let old_header = wrapper::<CheckedCommutativeBitwiseOptimizationV1<'_>>()?;
    let header = wrapper::<OwnedCommutativeBitwiseContinuationV1>()?;
    let inherited = borrowed
        .retained
        .checked_sub(old_header)
        .ok_or(Resource::Accounting)?;
    let retained = inherited.checked_add(header).ok_or(Resource::Arithmetic)?;
    // Both wrappers coexist until the move. Output/bridge/row headers are
    // already covered by their unchanged aggregate extraction/capture receipt.
    budget.reserve_storage(header)?;
    let CheckedCommutativeBitwiseOptimizationV1 {
        output,
        bridge,
        rows,
        execution,
        proved_pairs,
        ..
    } = borrowed;
    budget.release_storage(old_header)?;
    let owned = OwnedCommutativeBitwiseContinuationV1 {
        output,
        bridge,
        rows,
        execution,
        input: *input.canonical().identity(),
        proved_pairs,
        retained,
    };
    owned.replay_against(input, budget)?;
    Ok(owned)
}
