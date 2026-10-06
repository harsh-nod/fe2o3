//! One explicit refusal policy for the original-account native continuation.
use fe2o3_kernel_ir::{
    CANONICAL_PHASE_STORAGE_LIMIT_V1, CANONICAL_PHASE_WORK_LIMIT_V1,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};

/// Finite funding allowance for one complete source/F, Worker, finalization,
/// publication and readiness continuation. It reuses the existing production
/// canonical-phase values, shared once across all roots and replay operations.
///
/// This is a versioned resource policy, NOT a derived worst-case quote or promise
/// that every format-admissible program fits. Exhaustion is a terminal refusal;
/// callers must not retry with a fresh account, replenish the ledger, skip checks,
/// or fall back to another compiler route. The narrower existing operation caps,
/// Worker limits and protected-proof timeout remain independently enforced.
///
/// The allowance is inert: it creates no budget, reservation, runtime custody,
/// semantic evidence, load authority, or default-pipeline selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeConditionalContinuationAllowanceV1 {
    work: usize,
    storage: usize,
}
impl NativeConditionalContinuationAllowanceV1 {
    /// Selects the fixed V1 policy, refusing hosts that cannot represent its work.
    pub fn production_v1() -> Result<Self, Resource> {
        Ok(Self {
            work: usize::try_from(CANONICAL_PHASE_WORK_LIMIT_V1)
                .map_err(|_| Resource::Arithmetic)?,
            storage: CANONICAL_PHASE_STORAGE_LIMIT_V1,
        })
    }

    /// One aggregate continuation work allowance, never a per-operation reset.
    pub const fn work(self) -> usize {
        self.work
    }
    /// One aggregate logical storage allowance beyond separately funded prelude
    /// custody. This is not a 2-GiB component window or allocator/RSS guarantee.
    pub const fn storage(self) -> usize {
        self.storage
    }

    /// Combines known prelude funding with this allowance BEFORE constructing
    /// the original request account. These totals are request ceilings, not
    /// additional work/storage credits granted when entering a later operation.
    /// Existing unused prelude allowance remains part of that one total account.
    pub fn compose_startup(
        self,
        prelude_work: usize,
        prelude_storage: usize,
    ) -> Result<(usize, usize), Resource> {
        Ok((
            prelude_work
                .checked_add(self.work)
                .ok_or(Resource::Arithmetic)?,
            prelude_storage
                .checked_add(self.storage)
                .ok_or(Resource::Arithmetic)?,
        ))
    }
}

#[cfg(all(test, target_pointer_width = "64"))]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };

    #[test]
    fn continuation_allowance_is_fixed_checked_and_not_a_component_quote() {
        let plan = NativeConditionalContinuationAllowanceV1::production_v1().unwrap();
        assert_eq!(plan.work(), 1usize << 54);
        assert_eq!(plan.storage(), 1usize << 31);
        assert_eq!(
            plan.compose_startup(17, 31).unwrap(),
            (17 + plan.work(), 31 + plan.storage())
        );
        assert_eq!(
            plan.compose_startup(usize::MAX, 0),
            Err(Resource::Arithmetic)
        );
        assert_eq!(
            plan.compose_startup(0, usize::MAX),
            Err(Resource::Arithmetic)
        );
    }

    #[test]
    fn continuation_exact_exhaustion_keeps_original_work_storage_and_first_denials() {
        let plan = NativeConditionalContinuationAllowanceV1::production_v1().unwrap();
        let (work, storage) = plan.compose_startup(17, 31).unwrap();
        let mut account = Owned::new(Work::new(work), storage);
        account.with_budget(|b| {
            b.charge_work(17).unwrap();
            b.reserve_storage(31).unwrap();
            let ledger = b.work_ledger_identity_v1();
            let owner = b.storage_account_identity_v1();
            let address = b as *const Budget<'_> as usize;
            b.charge_work(plan.work()).unwrap();
            b.reserve_storage(plan.storage()).unwrap();
            assert_eq!((b.work(), b.storage()), (work, storage));
            let Resource::Work(first_work) = b.charge_work(1).unwrap_err() else {
                panic!("work ceiling");
            };
            let Resource::Storage(first_storage) = b.reserve_storage(1).unwrap_err() else {
                panic!("storage ceiling");
            };
            assert_eq!((first_work.actual(), first_work.limit()), (work + 1, work));
            assert_eq!(
                (first_storage.actual(), first_storage.limit()),
                (storage + 1, storage)
            );
            assert!(b.charge_work(2).is_err());
            assert!(b.reserve_storage(2).is_err());
            b.release_storage(plan.storage()).unwrap();
            assert_eq!(
                (b.storage(), b.work(), b.peak_storage()),
                (31, work, storage)
            );
            assert_eq!(b.failed_work(), Some(work + 1));
            assert_eq!(b.failed_storage(), Some(storage + 1));
            assert!(b.work_ledger_identity_v1() == ledger);
            assert_eq!(b.storage_account_identity_v1(), owner);
            assert_eq!(b as *const Budget<'_> as usize, address);
        });
    }

    #[test]
    fn continuation_funding_does_not_widen_existing_local_windows_or_refund_failure() {
        const LOCAL: usize = fe2o3_kernel_opt::MAX_REFINED_FORWARDING_HISTORY_STORAGE_V1;
        const FLOOR: usize = 31 + Budget::STORAGE_WINDOW_SCRATCH_V1;
        let plan = NativeConditionalContinuationAllowanceV1::production_v1().unwrap();
        let (work, storage) = plan.compose_startup(17, FLOOR).unwrap();
        let mut account = Owned::new(Work::new(work), storage);
        account.with_budget(|b| {
            b.charge_work(17).unwrap();
            b.reserve_storage(FLOOR).unwrap();
            let slot = b as *const Budget<'_> as usize;
            let ledger = b.work_ledger_identity_v1();
            let owner = b.storage_account_identity_v1();
            let result: Result<(), Resource> = b.with_additional_storage_window_v1(LOCAL, |b| {
                assert_eq!(b as *const Budget<'_> as usize, slot);
                b.reserve_storage(LOCAL)?;
                b.reserve_storage(1)
            });
            let Resource::Storage(denied) = result.unwrap_err() else {
                panic!("local ceiling");
            };
            assert_eq!(
                (denied.actual(), denied.limit()),
                (FLOOR + LOCAL + 1, FLOOR + LOCAL)
            );
            assert_eq!(b.storage(), FLOOR + LOCAL);
            assert_eq!(b.work(), 17 + Budget::STORAGE_WINDOW_WORK_V1);
            assert_eq!(b.storage_limit(), storage);
            assert!(b.work_ledger_identity_v1() == ledger);
            assert_eq!(b.storage_account_identity_v1(), owner);
            assert_eq!(b as *const Budget<'_> as usize, slot);
            b.release_storage(LOCAL).unwrap();
            assert_eq!(b.storage(), FLOOR);
            assert_eq!(b.failed_storage(), Some(FLOOR + LOCAL + 1));
        });
    }
}
