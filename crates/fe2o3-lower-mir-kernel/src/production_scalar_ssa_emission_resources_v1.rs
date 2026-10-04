use super::*;

pub(super) fn table_bytes<T>(capacity: usize) -> Result<usize> {
    capacity
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic.into())
}

pub(super) fn reserve<T>(
    table: &mut Vec<T>,
    required: usize,
    budget: &mut Budget<'_>,
) -> Result<()> {
    if required > MAX_ROWS {
        return Err(Error::Limit);
    }
    if required <= table.capacity() {
        return Ok(());
    }
    let capacity = required.max(table.capacity().saturating_mul(2).min(MAX_ROWS));
    let requested = table_bytes::<T>(capacity)?;
    budget.reserve_storage(requested)?;
    let mut next = Vec::new();
    if next.try_reserve_exact(capacity).is_err() {
        drop(next);
        budget.release_storage(requested)?;
        return Err(Resource::Allocation.into());
    }
    let actual = match table_bytes::<T>(next.capacity()) {
        Ok(actual) => actual,
        Err(error) => {
            drop(next);
            budget.release_storage(requested)?;
            return Err(error);
        }
    };
    if let Err(error) = budget.reserve_storage(actual - requested) {
        drop(next);
        budget.release_storage(requested)?;
        return Err(error.into());
    }
    if let Err(error) = budget.charge_work(table.len()) {
        drop(next);
        budget.release_storage(actual)?;
        return Err(error.into());
    }
    let old_bytes = table_bytes::<T>(table.capacity())?;
    next.append(table);
    let old = std::mem::replace(table, next);
    drop(old);
    budget.release_storage(old_bytes)?;
    Ok(())
}

pub(super) fn append<T>(table: &mut Vec<T>, value: T, budget: &mut Budget<'_>) -> Result<()> {
    budget.charge_work(1)?;
    let len = table.len().checked_add(1).ok_or(Resource::Arithmetic)?;
    reserve(table, len, budget)?;
    table.push(value);
    Ok(())
}

pub(super) fn charge_lookup(len: usize, budget: &mut Budget<'_>) -> Result<()> {
    budget.charge_work(2 + (usize::BITS - len.leading_zeros()) as usize)?;
    Ok(())
}

pub(super) fn copy_name(value: &str, budget: &mut Budget<'_>) -> Result<String> {
    budget.charge_work(value.len())?;
    budget.reserve_storage(value.len())?;
    let mut result = String::new();
    if result.try_reserve_exact(value.len()).is_err() {
        drop(result);
        budget.release_storage(value.len())?;
        return Err(Resource::Allocation.into());
    }
    if let Err(error) = budget.reserve_storage(result.capacity() - value.len()) {
        drop(result);
        budget.release_storage(value.len())?;
        return Err(error.into());
    }
    result.push_str(value);
    Ok(result)
}

pub(super) fn sort_work(len: usize, budget: &mut Budget<'_>) -> Result<()> {
    let units = len
        .checked_mul(2 + (usize::BITS - len.leading_zeros()) as usize)
        .ok_or(Resource::Arithmetic)?;
    budget.charge_work(units)?;
    Ok(())
}

/// Concrete-error scope: user results and both possible panic payloads remain
/// owned until after the original ledger's accepted scratch has been released.
pub(super) fn scoped<T>(
    budget: &mut Budget<'_>,
    run: impl FnOnce(&mut Budget<'_>) -> Result<T>,
) -> Result<T> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = budget as *mut _ as usize;
    let mut panic_payload = None;
    let mut rejected_payload = None;
    let mut result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => Some(result),
        Err(payload) => {
            panic_payload = Some(payload);
            None
        }
    };
    let valid = budget as *mut _ as usize == slot
        && budget.work_ledger_identity_v1() == ledger
        && budget.storage() >= floor;
    if !valid {
        if let Some(value) = result.take()
            && let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(value)))
        {
            rejected_payload = Some(payload);
        }
        // No charge/release on a replaced ledger or undercut original floor.
        drop((panic_payload, rejected_payload));
        return Err(Resource::Accounting.into());
    }
    let released = budget.storage() - floor;
    if let Err(error) = budget.release_storage(released) {
        if let Some(value) = result.take()
            && let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(value)))
        {
            rejected_payload = Some(payload);
        }
        drop((panic_payload, rejected_payload));
        return Err(error.into());
    }
    drop((panic_payload, rejected_payload));
    result.unwrap_or(Err(Error::Panicked))
}
