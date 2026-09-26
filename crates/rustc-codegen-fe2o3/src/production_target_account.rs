//! Original conditional TARGET account. SOURCE retains its separate account.

use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};

pub(crate) fn new() -> Result<Account, Resource> {
    let limit = usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
        .map_err(|_| Resource::Arithmetic)?;
    Ok(Account::new(
        Work::new(limit),
        fe2o3_compiler_lineage::MAX_NATIVE_CONDITIONAL_STORAGE_V1,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn early_target_account_preserves_prefix_and_denials_across_moves() {
        let mut account = new().unwrap();
        assert_eq!(account.work_limit(), 1usize << 54);
        assert_eq!(account.storage_limit(), 256 * 1024 * 1024);
        account.with_budget(|budget| {
            budget.charge_work(19).unwrap();
            budget.reserve_storage(23).unwrap();
            assert!(budget.charge_work(1usize << 54).is_err());
            assert!(budget.reserve_storage(256 * 1024 * 1024).is_err());
        });
        // A move preserves accounting, not address tokens from a finished borrow.
        let mut moved = Box::new(account);
        moved.with_budget(|budget| {
            assert_eq!(budget.work(), 19);
            assert_eq!(budget.storage(), 23);
            assert_eq!(budget.failed_work(), Some((1usize << 54) + 19));
            assert_eq!(budget.failed_storage(), Some(256 * 1024 * 1024 + 23));
            budget.charge_work(7).unwrap();
            budget.reserve_storage(11).unwrap();
        });
        assert_eq!(moved.work(), 26);
        assert_eq!(moved.storage(), 34);
        assert_eq!(moved.failed_work(), Some((1usize << 54) + 19));
        assert_eq!(moved.failed_storage(), Some(256 * 1024 * 1024 + 23));
    }
}
