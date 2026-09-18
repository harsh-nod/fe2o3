//! Explicit checked private Store-to-Load continuation, separate from every
//! historical optimizer policy and from final compiler/target authority.

use fe2o3_kernel_analysis::{
    CanonicalKirAppliedStoreForwardingV1, CanonicalKirInventoryErrorV1, CanonicalKirInventoryV1,
    CanonicalKirMemorySsaErrorV1, CanonicalKirMemorySsaV1, CanonicalKirStoreForwardingErrorV1,
    CanonicalKirStoreForwardingPlanV1, CanonicalKirStoreForwardingRowV1,
    CanonicalKirStoreForwardingStorageV1, CheckedCanonicalKirStoreForwardingV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrReplayAdmissionErrorV12,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{fmt, mem::size_of};

#[derive(Debug)]
pub enum CheckedStoreForwardingErrorV1 {
    Resource(Resource),
    Inventory(CanonicalKirInventoryErrorV1),
    MemorySsa(CanonicalKirMemorySsaErrorV1),
    Forwarding(CanonicalKirStoreForwardingErrorV1),
    Admission(CanonicalKernelIrReplayAdmissionErrorV12),
}
impl From<Resource> for CheckedStoreForwardingErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for CheckedStoreForwardingErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "checked store forwarding: {self:?}")
    }
}
impl std::error::Error for CheckedStoreForwardingErrorV1 {}
type Error = CheckedStoreForwardingErrorV1;

/// Owned actual O and consumed rewrite evidence, borrowing the exact input.
/// Neither owner is converted to a Policy3 receipt or source/output theorem.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::CheckedStoreForwardingOutputV1;
/// fn detach<'a>(output: CheckedStoreForwardingOutputV1<'a>)
///     -> CheckedStoreForwardingOutputV1<'static> { output }
/// ```
pub struct CheckedStoreForwardingOutputV1<'input> {
    output: Owner,
    applied: CanonicalKirAppliedStoreForwardingV1<'input>,
    retained: usize,
}
impl<'input> CheckedStoreForwardingOutputV1<'input> {
    pub const fn input(&self) -> &'input Owner {
        self.applied.input()
    }
    pub const fn output(&self) -> &Owner {
        &self.output
    }
    pub fn rows(&self) -> &[CanonicalKirStoreForwardingRowV1] {
        self.applied.rows()
    }
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    pub(crate) fn into_owned_parts(self) -> (Owner, Vec<CanonicalKirStoreForwardingRowV1>, usize) {
        (self.output, self.applied.into_inert_rows(), self.retained)
    }
    /// Independently checks the actual pair again on the supplied same ledger.
    pub fn replay(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<
        (
            CheckedCanonicalKirStoreForwardingV1<'_>,
            CanonicalKirStoreForwardingStorageV1,
        ),
        Error,
    > {
        self.applied
            .check_output(&self.output, budget)
            .map_err(Error::Forwarding)
    }
}

/// Runs a real deterministic rewrite and independent replay, not just a plan.
/// Source storage is retained by the caller. Success transfers this output's
/// `retained_storage()` reservation; all Result exits restore the entry floor.
/// The private candidate copy, both analyses, row capacity, inverse verification
/// and output coexist on the same budget. No rewrite allocates after its paid
/// preflight. This does not append a pass to Policy3 or enable source admission.
pub fn optimize_checked_store_forwarding_v1<'input>(
    input: &'input Owner,
    budget: &mut Budget<'_>,
) -> Result<CheckedStoreForwardingOutputV1<'input>, Error> {
    let floor = budget.storage();
    let result = optimize(input, budget);
    budget.release_storage(
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?,
    )?;
    result
}

fn optimize<'input>(
    input: &'input Owner,
    budget: &mut Budget<'_>,
) -> Result<CheckedStoreForwardingOutputV1<'input>, Error> {
    budget.charge_work(1)?;
    let header = size_of::<CheckedStoreForwardingOutputV1<'_>>()
        .checked_sub(size_of::<Owner>())
        .and_then(|n| n.checked_sub(size_of::<CanonicalKirAppliedStoreForwardingV1<'_>>()))
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(header)?;
    let (inventory, inventory_storage) =
        CanonicalKirInventoryV1::derive(input, budget).map_err(Error::Inventory)?;
    budget.reserve_storage(inventory_storage.retained_storage())?;
    let (memory, memory_storage) =
        CanonicalKirMemorySsaV1::derive(&inventory, Default::default(), budget)
            .map_err(Error::MemorySsa)?;
    budget.reserve_storage(memory_storage.retained_storage())?;
    let (plan, plan_storage) =
        CanonicalKirStoreForwardingPlanV1::derive(&inventory, &memory, budget)
            .map_err(Error::Forwarding)?;
    budget.reserve_storage(plan_storage.retained_storage())?;
    let (mut candidate, candidate_storage) = input
        .copy_module_for_transformation_v12(budget)
        .map_err(Error::Admission)?;
    budget.reserve_storage(candidate_storage.retained_storage())?;
    let applied = plan
        .apply(input, &mut candidate, budget)
        .map_err(Error::Forwarding)?;
    let (output, output_storage) =
        Owner::from_module_ref_with_verification_budget_v12(&candidate, budget)
            .map_err(Error::Admission)?;
    budget.reserve_storage(output_storage.retained_storage())?;
    let (checked, checked_storage) = applied
        .check_output(&output, budget)
        .map_err(Error::Forwarding)?;
    budget.reserve_storage(checked_storage.retained_storage())?;
    drop(checked);
    budget.release_storage(checked_storage.retained_storage())?;
    budget.charge_work(3)?;
    let retained = header
        .checked_add(output_storage.retained_storage())
        .and_then(|n| n.checked_add(applied.retained_storage()))
        .ok_or(Resource::Arithmetic)?;
    drop(candidate);
    budget.release_storage(candidate_storage.retained_storage())?;
    drop(memory);
    budget.release_storage(memory_storage.retained_storage())?;
    drop(inventory);
    budget.release_storage(inventory_storage.retained_storage())?;
    Ok(CheckedStoreForwardingOutputV1 {
        output,
        applied,
        retained,
    })
}

#[cfg(test)]
#[path = "checked_store_forwarding_v1_tests.rs"]
mod tests;
