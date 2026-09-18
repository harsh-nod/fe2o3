//! Explicit redundant private-store service. No historical fixed policy, source
//! lifetime admission, proof endpoint or default production schedule is changed.
use fe2o3_kernel_analysis::{
    CanonicalKirAppliedRedundantStoreV1, CanonicalKirInventoryErrorV1, CanonicalKirInventoryV1,
    CanonicalKirMemorySsaErrorV1, CanonicalKirMemorySsaV1, CanonicalKirRedundantStoreErrorV1,
    CanonicalKirRedundantStorePlanV1, CanonicalKirRedundantStoreRetainedOperationV1,
    CanonicalKirRedundantStoreRowV1, CanonicalKirRedundantStoreStorageV1,
    CheckedCanonicalKirRedundantStoreV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrReplayAdmissionErrorV12,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[derive(Debug)]
pub enum CheckedRedundantStoreErrorV1 {
    Resource(Resource),
    Inventory(CanonicalKirInventoryErrorV1),
    MemorySsa(CanonicalKirMemorySsaErrorV1),
    Deletion(CanonicalKirRedundantStoreErrorV1),
    Admission(CanonicalKernelIrReplayAdmissionErrorV12),
    Panicked,
}
type Error = CheckedRedundantStoreErrorV1;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "checked redundant private store: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Actual transformed owner plus consumed mutation evidence borrowing its input.
/// No extraction into a fixed-policy witness or source authority is provided.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::CheckedRedundantStoreOutputV1;
/// fn detach<'a>(output: CheckedRedundantStoreOutputV1<'a>)
///     -> CheckedRedundantStoreOutputV1<'static> { output }
/// ```
pub struct CheckedRedundantStoreOutputV1<'input> {
    output: Owner,
    applied: CanonicalKirAppliedRedundantStoreV1<'input>,
    retained: usize,
}
impl<'input> CheckedRedundantStoreOutputV1<'input> {
    pub const fn input(&self) -> &'input Owner {
        self.applied.input()
    }
    pub const fn output(&self) -> &Owner {
        &self.output
    }
    pub fn rows(&self) -> &[CanonicalKirRedundantStoreRowV1] {
        self.applied.rows()
    }
    pub fn retained_operations(&self) -> &[CanonicalKirRedundantStoreRetainedOperationV1] {
        self.applied.retained_operations()
    }
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    pub fn replay(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<
        (
            CheckedCanonicalKirRedundantStoreV1<'_>,
            CanonicalKirRedundantStoreStorageV1,
        ),
        Error,
    > {
        self.applied
            .check_output(&self.output, budget)
            .map_err(Error::Deletion)
    }
}

/// Actual deterministic rewrite, fresh V12 admission and independent replay.
/// The caller retains its input reservation; success transfers the returned
/// output receipt unreserved. Analyses, rows, origins, candidate and verification
/// scratch coexist on one ledger. Errors/panics discard owned stages before
/// restoring the floor; spent work and peak/failure history are never reset.
pub fn optimize_checked_redundant_store_v1<'input>(
    input: &'input Owner,
    budget: &mut Budget<'_>,
) -> Result<CheckedRedundantStoreOutputV1<'input>, Error> {
    scoped(budget, |budget| optimize(input, budget))
}

fn optimize<'input>(
    input: &'input Owner,
    budget: &mut Budget<'_>,
) -> Result<CheckedRedundantStoreOutputV1<'input>, Error> {
    budget.charge_work(1)?;
    let header = size_of::<CheckedRedundantStoreOutputV1<'_>>()
        .checked_sub(size_of::<Owner>())
        .and_then(|n| n.checked_sub(size_of::<CanonicalKirAppliedRedundantStoreV1<'_>>()))
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(header)?;
    let (inventory, is) =
        CanonicalKirInventoryV1::derive(input, budget).map_err(Error::Inventory)?;
    budget.reserve_storage(is.retained_storage())?;
    let (memory, ms) = CanonicalKirMemorySsaV1::derive(&inventory, Default::default(), budget)
        .map_err(Error::MemorySsa)?;
    budget.reserve_storage(ms.retained_storage())?;
    let (plan, ps) = CanonicalKirRedundantStorePlanV1::derive(&inventory, &memory, budget)
        .map_err(Error::Deletion)?;
    budget.reserve_storage(ps.retained_storage())?;
    let (mut candidate, cs) = input
        .copy_module_for_transformation_v12(budget)
        .map_err(Error::Admission)?;
    budget.reserve_storage(cs.retained_storage())?;
    let applied = plan
        .apply(input, &mut candidate, budget)
        .map_err(Error::Deletion)?;
    let (output, os) = Owner::from_module_ref_with_verification_budget_v12(&candidate, budget)
        .map_err(Error::Admission)?;
    budget.reserve_storage(os.retained_storage())?;
    let (relation, rs) = applied
        .check_output(&output, budget)
        .map_err(Error::Deletion)?;
    budget.reserve_storage(rs.retained_storage())?;
    drop(relation);
    budget.release_storage(rs.retained_storage())?;
    budget.charge_work(3)?;
    let retained = header
        .checked_add(os.retained_storage())
        .and_then(|n| n.checked_add(applied.retained_storage()))
        .ok_or(Resource::Arithmetic)?;
    drop(candidate);
    budget.release_storage(cs.retained_storage())?;
    drop(memory);
    budget.release_storage(ms.retained_storage())?;
    drop(inventory);
    budget.release_storage(is.retained_storage())?;
    Ok(CheckedRedundantStoreOutputV1 {
        output,
        applied,
        retained,
    })
}

fn scoped<'work, T>(
    budget: &mut Budget<'work>,
    run: impl FnOnce(&mut Budget<'work>) -> Result<T, Error>,
) -> Result<T, Error> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(Error::Panicked)
        }
    };
    if ledger != budget.work_ledger_identity_v1() || budget.storage() < floor {
        drop(result);
        return Err(Resource::Accounting.into());
    }
    if let Err(error) = budget.release_storage(budget.storage() - floor) {
        drop(result);
        return Err(error.into());
    }
    result
}

#[cfg(test)]
#[path = "checked_redundant_store_v1_tests.rs"]
mod tests;
