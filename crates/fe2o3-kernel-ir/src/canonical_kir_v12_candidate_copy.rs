//! Allocation-metered candidate copies retain no verification or rewrite authority.

use super::*;

/// Conservative payload of a mutable candidate copied from one verified owner.
/// This counts the Module header and decoder-owned heap payload, not the borrowed
/// source, a second wire buffer, allocator metadata or later mutation allocations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKernelIrCandidateStorageV12 {
    retained: usize,
}

impl CanonicalKernelIrCandidateStorageV12 {
    /// Reserve this transfer before the next allocation while the candidate lives.
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}

impl VerifiedCanonicalKernelIrModuleV12 {
    /// Copies this exact owner's executable through the allocation-metered V12
    /// decoder and checks full equality with the immutable original.
    ///
    /// The result is an ordinary, unverified mutable Module. Mutation needs its
    /// own work/storage accounting and independent semantic-preservation check;
    /// neither the original verification nor this receipt transfers to edits.
    /// Fresh canonical admission is required before a changed candidate can be
    /// used as a verified owner. The original owner remains unchanged.
    ///
    /// The caller keeps the original's reservation live throughout the call.
    /// Every Result exit restores the incoming storage floor, preserving work,
    /// peak and first-denial history. On success reserve the returned candidate
    /// receipt before further allocation, then drop the candidate before release.
    /// Decoder tree payload bounds retain their existing conservative convention.
    ///
    /// ```compile_fail
    /// use fe2o3_kernel_ir::{Module, VerifiedCanonicalKernelIrModuleV12};
    /// fn candidate_is_not_verified(candidate: Module) -> VerifiedCanonicalKernelIrModuleV12 {
    ///     candidate
    /// }
    /// ```
    pub fn copy_module_for_transformation_v12(
        &self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(Module, CanonicalKernelIrCandidateStorageV12), AdmissionError> {
        let floor = budget.storage_checkpoint();
        let result = (|| {
            budget.reserve_storage(std::mem::size_of::<Module>())?;
            let bytes = self.canonical().canonical_bytes();
            let module = crate::wire::decode_module_v12_with_allocation_budget_v1(bytes, budget)
                .map_err(AdmissionError::Decode)?;
            // The complete injective encoding bounds the structural comparison.
            budget.charge_work(bytes.len())?;
            if &module != self.module() {
                return Err(AdmissionError::CanonicalMismatch);
            }
            let retained = budget
                .storage()
                .checked_sub(floor)
                .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Accounting)?;
            Ok((module, CanonicalKernelIrCandidateStorageV12 { retained }))
        })();
        budget.rollback_storage(floor)?;
        result
    }
}

#[cfg(test)]
#[path = "canonical_kir_v12_candidate_copy_tests.rs"]
mod tests;
