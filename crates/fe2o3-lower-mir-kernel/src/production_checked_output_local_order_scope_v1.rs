use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1;
use std::panic::{AssertUnwindSafe, catch_unwind};

pub(super) struct Binding {
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    slot: usize,
    floor: usize,
}
impl Binding {
    pub(super) fn check(&self, budget: &AssertOriginBudgetV1<'_>) -> CResult<()> {
        if self.ledger != budget.work_ledger_identity_v1()
            || self.slot != budget as *const AssertOriginBudgetV1<'_> as usize
            || budget.storage() < self.floor
        {
            return Err(AssertOriginResourceV1::Accounting.into());
        }
        Ok(())
    }
}

// Only closed internal phases use this callback. New scope-owned allocations
// above entry are retired once; inherited and foreign reservations are untouched.
pub(super) fn scoped<'w, T>(
    required: usize,
    budget: &mut AssertOriginBudgetV1<'w>,
    run: impl FnOnce(&mut AssertOriginBudgetV1<'w>, &Binding) -> CResult<T>,
) -> CResult<T> {
    if budget.storage() < required {
        return Err(AssertOriginResourceV1::Accounting.into());
    }
    let binding = Binding {
        ledger: budget.work_ledger_identity_v1(),
        slot: budget as *const AssertOriginBudgetV1<'w> as usize,
        floor: budget.storage(),
    };
    let mut payloads = [None, None];
    let mut result = match catch_unwind(AssertUnwindSafe(|| run(budget, &binding))) {
        Ok(result) => result,
        Err(payload) => {
            payloads[0] = Some(payload);
            Err(CError::Panicked)
        }
    };
    if binding.check(budget).is_err() {
        let rejected =
            std::mem::replace(&mut result, Err(AssertOriginResourceV1::Accounting.into()));
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(rejected))) {
            payloads[1] = Some(payload);
        }
    } else {
        let accepted = budget.storage() - binding.floor;
        if let Err(error) = budget.release_storage(accepted) {
            let rejected = std::mem::replace(&mut result, Err(error.into()));
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(rejected))) {
                payloads[1] = Some(payload);
            }
        }
    }
    drop(payloads);
    result
}
