use super::{Budget, Resource};
use std::{
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

pub(crate) fn bytes<T>(capacity: usize) -> Result<usize, Resource> {
    capacity
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)
}

pub(crate) fn table<T>(count: usize, budget: &mut Budget<'_>) -> Result<Vec<T>, Resource> {
    budget.charge_work(count)?;
    let requested = bytes::<T>(count)?;
    budget.reserve_storage(requested)?;
    let mut table = Vec::new();
    table
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    let actual = bytes::<T>(table.capacity())?;
    budget.reserve_storage(actual.checked_sub(requested).ok_or(Resource::Accounting)?)?;
    Ok(table)
}

pub(crate) fn name(value: &str, budget: &mut Budget<'_>) -> Result<String, Resource> {
    budget.charge_work(value.len())?;
    budget.reserve_storage(value.len())?;
    let mut name = String::new();
    name.try_reserve_exact(value.len())
        .map_err(|_| Resource::Allocation)?;
    budget.reserve_storage(
        name.capacity()
            .checked_sub(value.len())
            .ok_or(Resource::Accounting)?,
    )?;
    name.push_str(value);
    Ok(name)
}

/// Private constructor scope. Success keeps exactly its returned receipt
/// reserved. Error payloads are the callers' existing diagnostic domain, never
/// graph/storage owners. Generic conversion and panic payload drops happen only
/// after owned candidate drop and valid-ledger cleanup.
pub(crate) fn owned<'work, T, E>(
    budget: &mut Budget<'work>,
    consumed: usize,
    resource: impl Fn(Resource) -> E,
    panicked: impl Fn() -> E,
    run: impl FnOnce(&mut Budget<'work>) -> Result<(T, usize), E>,
) -> Result<T, E> {
    let Some(floor) = budget.storage().checked_sub(consumed) else {
        drop(run);
        return Err(resource(Resource::Accounting));
    };
    let slot = budget as *mut _ as usize;
    let ledger = budget.work_ledger_identity_v1();
    let valid = |budget: &Budget<'_>| {
        budget as *const _ as usize == slot
            && budget.work_ledger_identity_v1() == ledger
            && budget.storage() >= floor
    };
    let mut first = None;
    let mut second = None;
    let mut outcome = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => Some(result),
        Err(payload) => {
            first = Some(payload);
            None
        }
    };
    let accounting = !valid(budget)
        || outcome.as_ref().is_some_and(|result| {
            result
                .as_ref()
                .is_ok_and(|(_, retained)| floor.checked_add(*retained) != Some(budget.storage()))
        });
    if accounting {
        if let Some(result) = outcome.take()
            && let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(result)))
        {
            second = Some(payload);
        }
        if valid(budget) {
            budget
                .release_storage(budget.storage() - floor)
                .map_err(&resource)?;
        }
        drop((first, second));
        return Err(resource(Resource::Accounting));
    }
    match outcome {
        Some(Ok((value, _))) => Ok(value),
        Some(Err(error)) => {
            budget
                .release_storage(budget.storage() - floor)
                .map_err(&resource)?;
            Err(error)
        }
        None => {
            budget
                .release_storage(budget.storage() - floor)
                .map_err(&resource)?;
            drop(first);
            Err(panicked())
        }
    }
}
