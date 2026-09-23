//! Fixed, prepaid protocol operations. These units bound logical admission,
//! not generated instructions, allocator behavior, process RSS or stack size.
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

pub(crate) const ENTRY_WORK: usize = 8;
pub(crate) const KEY_VALIDATION_WORK: usize = 4096;

/// Additional retained storage returned unreserved by a native operation.
/// Keep consumed inputs' reservations and reserve this delta before retaining
/// the result. Use the owner's `retained_storage()` for its eventual release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerExecutionAttestationStorageV2(pub(crate) usize);
impl CompilerExecutionAttestationStorageV2 {
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

// The callback cannot access or replace the ledger. Nested fixed codecs must
// be included in the caller's quota, not run with a new/unlimited budget.
pub(crate) fn fixed<T, E: From<Resource>>(
    budget: &mut Budget<'_>,
    input_floor: usize,
    work: usize,
    storage: usize,
    operation: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    budget.charge_work(ENTRY_WORK)?;
    if budget.storage() < input_floor {
        return Err(Resource::Accounting.into());
    }
    budget.charge_work(work.checked_sub(ENTRY_WORK).ok_or(Resource::Arithmetic)?)?;
    budget.reserve_storage(storage)?;
    let result = catch_unwind(AssertUnwindSafe(operation));
    budget.release_storage(storage)?;
    match result {
        Ok(result) => result,
        Err(panic) => resume_unwind(panic),
    }
}

pub(crate) fn fixed_input_floor(bytes: &[u8], expected: usize) -> usize {
    if bytes.len() == expected { expected } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    #[test]
    fn unwind_restores_floor_and_preserves_work_and_peak() {
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 100);
        budget.reserve_storage(19).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            fixed::<(), Resource>(&mut budget, 19, 50, 70, || panic!("fixed codec"))
        }));
        assert!(result.is_err());
        assert_eq!(budget.storage(), 19);
        assert_eq!(budget.peak_storage(), 89);
        assert_eq!(budget.work(), 50);
    }
}
