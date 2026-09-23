//! Owned result of the existing closed scheduler, without an input borrow.
use super::*;

/// Actual scheduled output and independently checked transition. The input
/// identity is not producer custody: a consuming source owner must retain its
/// actual prefix and privately create this tail from that prefix's output.
/// No source, artifact, protected-admission, or launch authority is granted.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::OwnedU32LocalOrderContinuationV1;
/// fn duplicate(value: OwnedU32LocalOrderContinuationV1) { let _ = value.clone(); }
/// ```
/// ```compile_fail,E0616
/// use fe2o3_kernel_opt::OwnedU32LocalOrderContinuationV1;
/// fn fabricate(value: OwnedU32LocalOrderContinuationV1) { let _ = value.output; }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::OwnedU32LocalOrderContinuationV1;
/// fn mutate(value: &mut OwnedU32LocalOrderContinuationV1) {
///     value.output().module().functions.clear();
/// }
/// ```
pub struct OwnedU32LocalOrderContinuationV1 {
    output: Owner,
    receipt: InertCanonicalKirTransitionReceiptV1,
    region: U32LocalOrderRegionV1,
    preference: U32LocalOrderPreferenceV1,
    retained: usize,
}

impl OwnedU32LocalOrderContinuationV1 {
    pub const fn output(&self) -> &Owner {
        &self.output
    }
    pub const fn input_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV12 {
        &self.region.expected_input
    }
    pub const fn transition_receipt(&self) -> &InertCanonicalKirTransitionReceiptV1 {
        &self.receipt
    }
    pub const fn region(&self) -> U32LocalOrderRegionV1 {
        self.region
    }
    pub const fn preference(&self) -> U32LocalOrderPreferenceV1 {
        self.preference
    }
    /// Additional unreserved transfer receipt, excluding the original input.
    /// Reserve it before further controlled work while retaining that input.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Recompute the supplied input's canonical identity under this ledger,
    /// then check the full inverse permutation and the independent transition.
    /// Equal-byte inputs still require the complete check; equality does not
    /// establish source producer custody. The actual input and this tail must
    /// remain separately prepaid. All Result exits restore the entry floor.
    pub fn replay(&self, input: &Owner, budget: &mut Budget<'_>) -> Result<()> {
        if budget.storage() < self.retained {
            return Err(Resource::Accounting.into());
        }
        scoped(budget, |budget| {
            budget.charge_work(3)?;
            if input.canonical().identity() != self.input_identity() {
                return Err(Error::InputIdentity);
            }
            check_input(input, self.input_identity(), budget)?;
            replay_pair(
                input,
                &self.output,
                &self.receipt,
                self.region,
                self.preference,
                budget,
            )
        })
    }
}

/// Executes the existing scheduler once and privately moves its actual output
/// and receipt into a sealed owner. No graph transformation engine, input clone,
/// caller-provided output, detached receipt admission, or policy conversion is
/// introduced. Both wrapper headers are charged during the move. The returned
/// retained-storage receipt is unreserved; all Result exits restore the entry
/// floor and preserve cumulative work, peak, and failure history.
pub fn prepare_owned_u32_local_order_continuation_v1(
    input: &Owner,
    region: U32LocalOrderRegionV1,
    preference: U32LocalOrderPreferenceV1,
    budget: &mut Budget<'_>,
) -> Result<OwnedU32LocalOrderContinuationV1> {
    scoped(budget, |budget| {
        let borrowed = schedule_checked_u32_local_order_v1(input, region, preference, budget)?;
        budget.reserve_storage(borrowed.retained_storage())?;
        detach(input, borrowed, budget)
    })
}

fn wrapper<T>() -> Result<usize> {
    size_of::<T>()
        .checked_sub(size_of::<Owner>())
        .and_then(|size| size.checked_sub(size_of::<InertCanonicalKirTransitionReceiptV1>()))
        .ok_or_else(|| Resource::Arithmetic.into())
}

// Only the producer above and hostile component tests can call this. Source
// consumers cannot attach an equal-byte foreign result to a retained prefix.
fn detach(
    input: &Owner,
    borrowed: CheckedU32LocalOrderOutputV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<OwnedU32LocalOrderContinuationV1> {
    budget.charge_work(6)?;
    if budget.storage() < borrowed.retained || !std::ptr::eq(input, borrowed.input) {
        return Err(Resource::Accounting.into());
    }
    let previous_header = wrapper::<CheckedU32LocalOrderOutputV1<'_>>()?;
    let header = wrapper::<OwnedU32LocalOrderContinuationV1>()?;
    let inherited = borrowed
        .retained
        .checked_sub(previous_header)
        .ok_or(Resource::Accounting)?;
    let retained = inherited.checked_add(header).ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(header)?;
    let CheckedU32LocalOrderOutputV1 {
        output,
        receipt,
        region,
        preference,
        ..
    } = borrowed;
    budget.release_storage(previous_header)?;
    let owned = OwnedU32LocalOrderContinuationV1 {
        output,
        receipt,
        region,
        preference,
        retained,
    };
    owned.replay(input, budget)?;
    Ok(owned)
}

// Fresh bounded encode/inverse-decode/verification/equality/hash of the actual
// immutable input. This transient canonical owner is dropped before release;
// no second input graph is retained by the continuation.
fn check_input(
    input: &Owner,
    expected: &VerifiedCanonicalKernelIrIdentityV12,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let (canonical, storage) =
        fe2o3_kernel_ir::VerifiedCanonicalKernelIrV12::from_module_ref_with_verification_budget_v12(
            input.module(),
            budget,
        )
        .map_err(Error::Admission)?;
    budget.reserve_storage(storage.retained_storage())?;
    budget.charge_work(
        canonical
            .canonical_bytes()
            .len()
            .checked_add(input.canonical().canonical_bytes().len())
            .and_then(|bytes| bytes.checked_add(2))
            .ok_or(Resource::Arithmetic)?,
    )?;
    let exact = canonical.identity() == expected
        && canonical.canonical_bytes() == input.canonical().canonical_bytes();
    drop(canonical);
    budget.release_storage(storage.retained_storage())?;
    if !exact {
        return Err(Error::InputIdentity);
    }
    Ok(())
}

#[cfg(test)]
#[path = "checked_u32_local_order_owned_v1_tests.rs"]
mod tests;
