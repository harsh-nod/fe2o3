//! Move-only local continuation, not a fixed policy or source admission owner.
use super::*;
use fe2o3_kernel_analysis::check_canonical_kir_redundant_store_v1;
use fe2o3_kernel_ir::{CanonicalKernelIrReplayStorageV12, VerifiedCanonicalKernelIrIdentityV12};

/// Owns actual J and inert observations without borrowing the input graph.
/// Construction always runs the closed rewrite on the caller's actual input;
/// there is no candidate attachment, raw-parts constructor, or source authority.
/// Replay checks the full actual input/output relation after its cheap typed
/// identity check. Equal bytes in a later owner require that full replay too.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::OwnedRedundantStoreContinuationV1;
/// fn duplicate(value: &OwnedRedundantStoreContinuationV1)
///     -> OwnedRedundantStoreContinuationV1 { value.clone() }
/// ```
/// ```compile_fail,E0616
/// use fe2o3_kernel_opt::OwnedRedundantStoreContinuationV1;
/// fn extract(value: OwnedRedundantStoreContinuationV1) { let _ = value.output; }
/// ```
/// The original owner can be moved or dropped after the owning call. This is
/// only local rewrite custody, not permission to drop a later source prefix:
/// ```
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12, CanonicalKernelIrVerificationResourceBudgetV1};
/// use fe2o3_kernel_opt::{prepare_owned_redundant_store_continuation_v1, OwnedRedundantStoreContinuationV1, CheckedRedundantStoreErrorV1};
/// fn no_self_reference(input: VerifiedCanonicalKernelIrModuleV12, budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>)
///     -> Result<OwnedRedundantStoreContinuationV1, CheckedRedundantStoreErrorV1> {
///     let output = prepare_owned_redundant_store_continuation_v1(&input, budget)?;
///     drop(input);
///     Ok(output)
/// }
/// ```
pub struct OwnedRedundantStoreContinuationV1 {
    output: Owner,
    output_storage: CanonicalKernelIrReplayStorageV12,
    input_identity: VerifiedCanonicalKernelIrIdentityV12,
    rows: Vec<CanonicalKirRedundantStoreRowV1>,
    origins: Vec<CanonicalKirRedundantStoreRetainedOperationV1>,
    retained: usize,
}

impl OwnedRedundantStoreContinuationV1 {
    pub const fn output(&self) -> &Owner {
        &self.output
    }
    pub const fn input_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV12 {
        &self.input_identity
    }
    pub fn rows(&self) -> &[CanonicalKirRedundantStoreRowV1] {
        &self.rows
    }
    pub fn retained_operations(&self) -> &[CanonicalKirRedundantStoreRetainedOperationV1] {
        &self.origins
    }
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Both owners remain separately prepaid. The returned borrowed relation
    /// receipt is unreserved; its identity comparison is never the proof.
    pub fn replay_against<'a>(
        &'a self,
        input: &'a Owner,
        budget: &mut Budget<'_>,
    ) -> Result<
        (
            CheckedCanonicalKirRedundantStoreV1<'a>,
            CanonicalKirRedundantStoreStorageV1,
        ),
        Error,
    > {
        if budget.storage() < self.retained {
            return Err(Resource::Accounting.into());
        }
        budget.charge_work(3)?;
        let header = size_of::<Self>()
            .checked_sub(size_of::<Owner>())
            .ok_or(Resource::Arithmetic)?;
        let rows = capacity_bytes(&self.rows)?;
        let origins = capacity_bytes(&self.origins)?;
        if self
            .output_storage
            .retained_storage()
            .checked_add(header)
            .and_then(|n| n.checked_add(rows))
            .and_then(|n| n.checked_add(origins))
            != Some(self.retained)
        {
            return Err(Resource::Accounting.into());
        }
        if input.canonical().identity() != &self.input_identity {
            return Err(Error::Deletion(
                CanonicalKirRedundantStoreErrorV1::ForeignSubject,
            ));
        }
        check_canonical_kir_redundant_store_v1(
            input,
            &self.output,
            &self.rows,
            &self.origins,
            budget,
        )
        .map_err(Error::Deletion)
    }
}

/// Runs the existing checked rewrite, then privately moves its actual output
/// and detaches only inert observation tables. No extra graph or wire copy is
/// made to remove the input borrow. The input's reservation remains live;
/// success returns the full owned continuation receipt unreserved.
pub fn prepare_owned_redundant_store_continuation_v1(
    input: &Owner,
    budget: &mut Budget<'_>,
) -> Result<OwnedRedundantStoreContinuationV1, Error> {
    scoped(budget, |budget| {
        let borrowed = optimize_checked_redundant_store_v1(input, budget)?;
        budget.reserve_storage(borrowed.retained_storage())?;
        detach(input, borrowed, budget)
    })
}

fn copy_records<T: Copy>(records: &[T], budget: &mut Budget<'_>) -> Result<Vec<T>, Error> {
    let bytes = records
        .len()
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.charge_work(bytes.checked_add(3).ok_or(Resource::Arithmetic)?)?;
    // Vector headers are already included in the prepaid owning wrapper.
    budget.reserve_storage(bytes)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(records.len())
        .map_err(|_| Resource::Allocation)?;
    let actual = result
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(actual.checked_sub(bytes).ok_or(Resource::Accounting)?)?;
    result.extend_from_slice(records);
    Ok(result)
}

fn capacity_bytes<T>(records: &Vec<T>) -> Result<usize, Error> {
    records
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or_else(|| Resource::Arithmetic.into())
}

// This is crate-private implementation, not a user-callable from-parts escape.
// J's typed admission receipt is carried from the original transaction.
fn detach(
    input: &Owner,
    borrowed: CheckedRedundantStoreOutputV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<OwnedRedundantStoreContinuationV1, Error> {
    if budget.storage() < borrowed.retained_storage() {
        return Err(Resource::Accounting.into());
    }
    if !std::ptr::eq(input, borrowed.input()) {
        return Err(Error::Deletion(
            CanonicalKirRedundantStoreErrorV1::ForeignSubject,
        ));
    }
    budget.charge_work(6)?;
    let old_header = borrowed_header_storage()?;
    let old_metadata = old_header
        .checked_add(borrowed.applied.retained_storage())
        .ok_or(Resource::Arithmetic)?;
    if old_metadata.checked_add(borrowed.output_storage.retained_storage())
        != Some(borrowed.retained)
    {
        return Err(Resource::Accounting.into());
    }
    let header = size_of::<OwnedRedundantStoreContinuationV1>()
        .checked_sub(size_of::<Owner>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(header)?;
    let rows = copy_records(borrowed.rows(), budget)?;
    let origins = copy_records(borrowed.retained_operations(), budget)?;
    let rows_bytes = capacity_bytes(&rows)?;
    let origins_bytes = capacity_bytes(&origins)?;
    let retained = borrowed
        .output_storage
        .retained_storage()
        .checked_add(header)
        .and_then(|n| n.checked_add(rows_bytes))
        .and_then(|n| n.checked_add(origins_bytes))
        .ok_or(Resource::Arithmetic)?;
    let CheckedRedundantStoreOutputV1 {
        output,
        output_storage,
        applied,
        ..
    } = borrowed;
    drop(applied);
    // The existing J reservation survives. Only the now-dead old observation
    // owner and its old wrapper header are released after the move.
    budget.release_storage(old_metadata)?;
    let owned = OwnedRedundantStoreContinuationV1 {
        output,
        output_storage,
        input_identity: *input.canonical().identity(),
        rows,
        origins,
        retained,
    };
    let replay_storage = {
        let (_relation, storage) = owned.replay_against(input, budget)?;
        budget.reserve_storage(storage.retained_storage())?;
        storage
    };
    budget.release_storage(replay_storage.retained_storage())?;
    Ok(owned)
}

#[cfg(test)]
#[path = "owned_redundant_store_v1_tests.rs"]
mod tests;
