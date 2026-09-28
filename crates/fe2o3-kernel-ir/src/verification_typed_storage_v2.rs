//! Private typed-vector accounting shared by CFG and source-index prerequisites.
//! These byte units never reinterpret legacy logical-row storage conventions.
use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Error,
};

pub(crate) fn vector_bytes_v2<T>(values: &Vec<T>) -> Result<usize, Error> {
    values
        .capacity()
        .checked_mul(std::mem::size_of::<T>())
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Vec<T>>()))
        .ok_or(Error::Arithmetic)
}

pub(crate) fn admit_capacity_v2<T>(
    values: &Vec<T>,
    requested: usize,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    let excess = values
        .capacity()
        .checked_sub(requested)
        .and_then(|excess| excess.checked_mul(std::mem::size_of::<T>()))
        .ok_or(Error::Arithmetic)?;
    budget.reserve_storage(excess)
}

pub(crate) fn allocate_vector_v2<T>(
    capacity: usize,
    budget: &mut Budget<'_>,
) -> Result<Vec<T>, Error> {
    let bytes = capacity
        .checked_mul(std::mem::size_of::<T>())
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Vec<T>>()))
        .ok_or(Error::Arithmetic)?;
    budget.charge_work(1)?;
    budget.reserve_storage(bytes)?;
    let mut values = Vec::new();
    if values.try_reserve_exact(capacity).is_err() {
        drop(values);
        budget.release_storage(bytes)?;
        return Err(Error::Allocation);
    }
    if let Err(error) = admit_capacity_v2(&values, capacity, budget) {
        drop(values);
        budget.release_storage(bytes)?;
        return Err(error);
    }
    Ok(values)
}

pub(crate) fn prior_denial_v2(budget: &Budget<'_>) -> Result<(), Error> {
    if let Some(actual) = budget.failed_work() {
        return Err(Error::Work(crate::CanonicalKernelIrWorkLimitV1::new(
            actual,
            budget.work_limit_v1(),
        )));
    }
    if let Some(actual) = budget.failed_storage() {
        return Err(Error::Storage(
            crate::CanonicalKernelIrVerificationStorageLimitV1::new(actual, budget.storage_limit()),
        ));
    }
    Ok(())
}
