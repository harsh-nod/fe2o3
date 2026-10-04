//! Original conditional TARGET account. SOURCE retains its separate account.

use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
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

/// One synchronous phase; neither the account nor its borrow can escape.
/// Host-only compilation does not construct a device account.
pub(crate) fn with_device_phase<T>(
    has_device_roots: bool,
    run: impl FnOnce(Option<&mut Budget<'_>>) -> T,
) -> Result<T, Resource> {
    if has_device_roots {
        let mut account = new()?;
        Ok(account.with_budget(|budget| run(Some(budget))))
    } else {
        Ok(run(None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::Cell,
        panic::{AssertUnwindSafe, catch_unwind},
    };

    #[test]
    fn device_phase_calls_once_and_preserves_original_account_until_owner_drop() {
        use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as Identity;
        struct Owner<'a, 'w> {
            budget: &'a Budget<'w>,
            identity: Identity,
            dropped: &'a Cell<usize>,
        }
        impl Drop for Owner<'_, '_> {
            fn drop(&mut self) {
                assert!(self.budget.work_ledger_identity_v1() == self.identity);
                assert_eq!(self.budget.work(), 19);
                assert_eq!(self.budget.storage(), 23);
                assert_eq!(
                    self.budget.failed_storage(),
                    Some(self.budget.storage_limit() + 23)
                );
                self.dropped.set(self.dropped.get() + 1);
            }
        }
        for fault in ["success", "error", "unwind"] {
            let called = Cell::new(0);
            let dropped = Cell::new(0);
            let result = catch_unwind(AssertUnwindSafe(|| {
                with_device_phase(true, |budget| {
                    assert_eq!(called.replace(1), 0);
                    let budget = budget.expect("device account");
                    budget.charge_work(19).unwrap();
                    budget.reserve_storage(23).unwrap();
                    assert!(budget.reserve_storage(budget.storage_limit()).is_err());
                    let _owner = Owner {
                        identity: budget.work_ledger_identity_v1(),
                        budget,
                        dropped: &dropped,
                    };
                    match fault {
                        "success" => Ok(7),
                        "error" => Err("phase refused"),
                        _ => panic!("phase unwind"),
                    }
                })
            }));
            assert_eq!(called.get(), 1);
            assert_eq!(dropped.get(), 1);
            match fault {
                "success" => assert_eq!(result.unwrap().unwrap(), Ok(7)),
                "error" => assert_eq!(result.unwrap().unwrap(), Err("phase refused")),
                _ => assert_eq!(
                    result.unwrap_err().downcast_ref::<&str>(),
                    Some(&"phase unwind")
                ),
            }
        }
    }

    #[test]
    fn host_only_phase_runs_once_without_a_device_account() {
        let called = Cell::new(0);
        assert_eq!(
            with_device_phase(false, |budget| {
                assert!(budget.is_none());
                assert_eq!(called.replace(1), 0);
                7
            })
            .unwrap(),
            7
        );
        assert_eq!(called.get(), 1);
    }

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
