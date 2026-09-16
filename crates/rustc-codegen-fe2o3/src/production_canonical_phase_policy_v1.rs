//! Fixed logical limits for codegen's separate canonical materialization and
//! assertion-projection phases, not an optimizer or whole-compiler budget.

pub(crate) const WORK_LIMIT: u64 = 18_014_398_509_481_984;
pub(crate) const STORAGE_LIMIT: usize = 2_147_483_648;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_phase_policy_has_exact_typed_limits() {
        let work: u64 = WORK_LIMIT;
        let storage: usize = STORAGE_LIMIT;
        assert_eq!(work, 1u64 << 54);
        assert_eq!(storage, 1usize << 31);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn canonical_phase_logical_ledger_preserves_exact_limits_and_history() {
        use fe2o3_kernel_ir::{
            CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
            CanonicalKernelIrVerificationResourceErrorV1 as Resource,
            CanonicalKernelIrWorkBudgetV1 as Work,
        };

        // These are logical charges only: no payload allocation or phase run.
        const PREFIX: usize = 7;
        const FLOOR: usize = 23;
        let work_limit = usize::try_from(WORK_LIMIT).unwrap();
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        budget.charge_work(PREFIX).unwrap();
        budget.reserve_storage(FLOOR).unwrap();
        budget.charge_work(work_limit - PREFIX).unwrap();
        budget.reserve_storage(STORAGE_LIMIT - FLOOR).unwrap();
        assert_eq!(budget.work(), work_limit);
        assert_eq!(budget.storage(), STORAGE_LIMIT);
        assert_eq!(budget.peak_storage(), STORAGE_LIMIT);

        let Resource::Work(first_work) = budget.charge_work(1).unwrap_err() else {
            panic!("the first excess logical work unit must be denied");
        };
        assert_eq!(first_work.actual(), work_limit + 1);
        assert_eq!(first_work.limit(), work_limit);
        let Resource::Work(later_work) = budget.charge_work(2).unwrap_err() else {
            panic!("later excess logical work must also be denied");
        };
        assert_eq!(later_work.actual(), work_limit + 2);
        assert_eq!(later_work.limit(), work_limit);

        let Resource::Storage(first_storage) = budget.reserve_storage(1).unwrap_err() else {
            panic!("the first excess logical storage byte must be denied");
        };
        assert_eq!(first_storage.actual(), STORAGE_LIMIT + 1);
        assert_eq!(first_storage.limit(), STORAGE_LIMIT);
        let Resource::Storage(later_storage) = budget.reserve_storage(2).unwrap_err() else {
            panic!("later excess logical storage must also be denied");
        };
        assert_eq!(later_storage.actual(), STORAGE_LIMIT + 2);
        assert_eq!(later_storage.limit(), STORAGE_LIMIT);
        assert_eq!(budget.storage(), STORAGE_LIMIT);
        assert_eq!(budget.work(), work_limit);
        assert_eq!(budget.failed_storage(), Some(STORAGE_LIMIT + 1));

        budget.release_storage(STORAGE_LIMIT - FLOOR).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), STORAGE_LIMIT);
        assert_eq!(budget.failed_storage(), Some(STORAGE_LIMIT + 1));
        assert_eq!(budget.work(), work_limit);
        assert_eq!(work.failed_work(), Some(work_limit + 1));
    }
}
