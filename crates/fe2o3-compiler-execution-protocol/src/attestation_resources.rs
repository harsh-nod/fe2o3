//! Fixed, prepaid protocol operations. These units bound logical admission,
//! not generated instructions, allocator behavior, process RSS or stack size.
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};

pub(crate) const ENTRY_WORK: usize = 8;
pub(crate) const KEY_VALIDATION_WORK: usize = 4096;
pub(crate) const SIGN_WORK: usize = 65536;
pub(crate) const STRICT_VERIFY_WORK: usize = 131072;

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
    nested_fixed(budget, input_floor, work, storage, |_| operation())
}

/// Nested native decoders share the ledger; only this crate's fixed adapters
/// supply the callback. Returned owners transfer their reservation to callers.
pub(crate) fn nested_fixed<'work, T, E: From<Resource>>(
    budget: &mut Budget<'work>,
    input_floor: usize,
    work: usize,
    storage: usize,
    operation: impl FnOnce(&mut Budget<'work>) -> Result<T, E>,
) -> Result<T, E> {
    budget.with_prepaid_scope(input_floor, ENTRY_WORK, work, storage, operation)
}

pub(crate) fn fixed_input_floor(bytes: &[u8], expected: usize) -> usize {
    if bytes.len() == expected { expected } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[test]
    fn nested_scope_preserves_work_floor_peak_and_denial_history() {
        for mode in 0..3 {
            let mut work = Work::new(100);
            let mut budget = Budget::new(&mut work, 100);
            assert!(budget.reserve_storage(101).is_err());
            budget.reserve_storage(19).unwrap();
            let result = catch_unwind(AssertUnwindSafe(|| {
                nested_fixed::<(), Resource>(&mut budget, 19, 20, 20, |budget| {
                    nested_fixed(budget, 39, 30, 10, |budget| {
                        budget.reserve_storage(7)?;
                        match mode {
                            0 => Ok(()),
                            1 => Err(Resource::Allocation),
                            _ => panic!("nested scope"),
                        }
                    })
                })
            }));
            match mode {
                0 => assert!(result.unwrap().is_ok()),
                1 => assert!(matches!(result.unwrap(), Err(Resource::Allocation))),
                _ => assert!(result.is_err()),
            }
            assert_eq!(budget.storage(), 19);
            assert_eq!(budget.work(), 50);
            assert_eq!(budget.peak_storage(), 56);
            assert_eq!(budget.failed_storage(), Some(101));
        }
    }

    #[test]
    fn nested_scope_never_releases_a_replacement_ledger() {
        for mode in 0..3 {
            let mut work = Work::new(100);
            let mut foreign_work = Work::new(100);
            let mut budget = Budget::new(&mut work, 100);
            let mut foreign = Budget::new(&mut foreign_work, 100);
            budget.reserve_storage(19).unwrap();
            foreign.reserve_storage(71).unwrap();
            let result = catch_unwind(AssertUnwindSafe(|| {
                nested_fixed::<(), Resource>(&mut budget, 19, 20, 20, |budget| {
                    std::mem::swap(budget, &mut foreign);
                    match mode {
                        0 => Ok(()),
                        1 => Err(Resource::Allocation),
                        _ => panic!("replaced ledger"),
                    }
                })
            }));
            if mode == 2 {
                assert!(result.is_err());
            } else {
                assert!(matches!(result.unwrap(), Err(Resource::Accounting)));
            }
            assert_eq!(budget.storage(), 71);
            assert_eq!(budget.work(), 0);
            assert_eq!(foreign.storage(), 39);
            assert_eq!(foreign.work(), 20);
        }
    }

    #[test]
    fn nested_scope_rejects_retiring_the_callers_input_floor() {
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 100);
        budget.reserve_storage(19).unwrap();
        let result = nested_fixed::<(), Resource>(&mut budget, 19, 20, 20, |budget| {
            budget.release_storage(39)
        });
        assert!(matches!(result, Err(Resource::Accounting)));
        assert_eq!(budget.storage(), 0);
        assert_eq!(budget.work(), 20);
    }

    #[test]
    fn nested_scope_protects_its_own_frame_before_accepting_callback_results() {
        for success in [false, true] {
            let mut work = Work::new(100);
            let mut budget = Budget::new(&mut work, 100);
            budget.reserve_storage(19).unwrap();
            let result = nested_fixed::<(), Resource>(&mut budget, 19, 20, 20, |budget| {
                budget.release_storage(1)?;
                if success {
                    Ok(())
                } else {
                    Err(Resource::Allocation)
                }
            });
            assert!(matches!(result, Err(Resource::Accounting)));
            assert_eq!(budget.storage(), 19);
            assert_eq!(budget.work(), 20);
            assert_eq!(budget.peak_storage(), 39);
        }
    }

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
