use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1;
use std::panic::{AssertUnwindSafe, catch_unwind};

pub(crate) fn vector<T>(count: usize, budget: &mut Budget<'_>) -> Result<Vec<T>, Resource> {
    budget.charge_work(3)?;
    let bytes = count
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(bytes)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    let extra = values
        .capacity()
        .checked_sub(count)
        .and_then(|n| n.checked_mul(size_of::<T>()))
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(extra)?;
    Ok(values)
}

/// This outer scope owns every reservation above entry: all calls are closed
/// stages, with no caller callback or concurrent budget access. Legacy stages
/// retain their documented logical envelopes; new visible capacities use vector.
pub(super) struct Binding {
    slot: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
}
impl Binding {
    pub(super) fn new(budget: &Budget<'_>) -> Self {
        Self {
            slot: budget as *const Budget<'_> as usize,
            ledger: budget.work_ledger_identity_v1(),
            floor: budget.storage(),
        }
    }
    pub(super) fn check(&self, budget: &Budget<'_>) -> Result<(), Error> {
        if self.slot != budget as *const Budget<'_> as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.floor
        {
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }
}

pub(super) fn scoped<'w, T>(
    budget: &mut Budget<'w>,
    run: impl FnOnce(&mut Budget<'w>, &Binding) -> Result<T, Error>,
) -> Result<T, Error> {
    let binding = Binding::new(budget);
    let mut payloads = [None, None];
    let mut result = match catch_unwind(AssertUnwindSafe(|| run(budget, &binding))) {
        Ok(result) => result,
        Err(payload) => {
            payloads[0] = Some(payload);
            Err(Error::Panicked)
        }
    };
    if binding.check(budget).is_err() {
        let rejected = std::mem::replace(&mut result, Err(Resource::Accounting.into()));
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(rejected))) {
            payloads[1] = Some(payload);
        }
        // Never touch a replaced ledger, another slot or an undercut entry floor.
    } else {
        let owned = budget.storage() - binding.floor;
        if let Err(error) = budget.release_storage(owned) {
            let rejected = std::mem::replace(&mut result, Err(error.into()));
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(rejected))) {
                payloads[1] = Some(payload);
            }
        }
    }
    // Any payload destructor runs only after the valid original-ledger cleanup.
    drop(payloads);
    result
}

pub(super) fn checked_pair(
    input: &Owner,
    output: &Owner,
    rows: &KirNeutralOccurrenceRowsV1,
    budget: &mut Budget<'_>,
) -> Result<usize, Error> {
    scoped(budget, |budget, binding| {
        let before_floor = budget.storage();
        let (before, before_storage) =
            Inventory::derive(input, budget).map_err(Error::Inventory)?;
        binding.check(budget)?;
        if budget.storage() != before_floor {
            return Err(Resource::Accounting.into());
        }
        budget.reserve_storage(before_storage.retained_storage())?;
        let after_floor = budget.storage();
        let (after, after_storage) = Inventory::derive(output, budget).map_err(Error::Inventory)?;
        binding.check(budget)?;
        if budget.storage() != after_floor {
            return Err(Resource::Accounting.into());
        }
        budget.reserve_storage(after_storage.retained_storage())?;
        let count = {
            let checker_floor = budget.storage();
            let (checked, checked_storage) =
                check(&before, &after, rows.candidate(), budget).map_err(Error::Relation)?;
            binding.check(budget)?;
            if budget.storage() != checker_floor {
                return Err(Resource::Accounting.into());
            }
            budget.reserve_storage(checked_storage.retained_storage())?;
            checked.proved_pairs()
        };
        drop(after);
        drop(before);
        Ok(count)
    })
}
