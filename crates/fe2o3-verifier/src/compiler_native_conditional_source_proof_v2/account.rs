use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

pub(super) fn transfer<R>(
    budget: &mut Budget<'_>,
    run: impl FnOnce(&mut Budget<'_>) -> Result<(R, NativeConditionalSourceStorageV2), E>,
) -> Result<(R, NativeConditionalSourceStorageV2), E> {
    budget.charge_work(8)?;
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let address = budget as *const Budget<'_> as usize;
    let result = catch_unwind(AssertUnwindSafe(|| run(budget)));
    let intact = budget.work_ledger_identity_v1() == ledger
        && budget as *const Budget<'_> as usize == address
        && budget.storage() >= floor;
    if !intact {
        return match result {
            Ok(result) => {
                drop(result);
                Err(Resource::Accounting.into())
            }
            Err(payload) => resume_unwind(payload),
        };
    }
    let release = budget.storage() - floor;
    match result {
        Ok(Ok((owner, storage))) => {
            if release < storage.retained_storage() {
                drop(owner);
                return Err(Resource::Accounting.into());
            }
            // Explicit unreserved transfer is the sole success refund while
            // custody remains live. The next caller must reserve the receipt.
            if let Err(error) = budget.release_storage(release) {
                drop(owner);
                return Err(error.into());
            }
            Ok((owner, storage))
        }
        Ok(Err(error)) => {
            if error.refund_safe() {
                budget.release_storage(release)?;
            }
            Err(error)
        }
        Err(payload) => {
            // An inner scope may have observed a damaged floor before unwinding.
            // Its panic carries no accounting verdict. Owners are already dead,
            // but retain terminal charges rather than laundering that damage.
            resume_unwind(payload)
        }
    }
}

pub(super) fn vector<T>(count: usize, budget: &mut Budget<'_>) -> Result<(Vec<T>, usize), E> {
    budget.charge_work(3)?;
    let requested = count
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(requested)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    let actual = values
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(actual.checked_sub(requested).ok_or(Resource::Accounting)?)?;
    Ok((values, actual))
}

pub(super) fn text(value: &str, budget: &mut Budget<'_>) -> Result<String, E> {
    budget.charge_work(value.len().checked_add(1).ok_or(Resource::Arithmetic)?)?;
    budget.reserve_storage(value.len())?;
    let mut result = String::new();
    result
        .try_reserve_exact(value.len())
        .map_err(|_| Resource::Allocation)?;
    budget.reserve_storage(
        result
            .capacity()
            .checked_sub(value.len())
            .ok_or(Resource::Accounting)?,
    )?;
    result.push_str(value);
    Ok(result)
}
