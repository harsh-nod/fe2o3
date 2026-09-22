use super::{AssertOriginBudgetV1, AssertOriginResourceV1};
use std::{collections::TryReserveError, mem::size_of};

/// One genuine historical value, with no detached or mutable ownership API.
/// The destination header is paid by the containing continuation. The complete
/// new backing is paid separately; inherited logical floors are never refunded.
pub(super) struct OwnedPrefix<T> {
    values: Vec<T>,
}

fn capacity_bytes<T>(capacity: usize) -> Result<usize, AssertOriginResourceV1> {
    capacity
        .checked_mul(size_of::<T>())
        .ok_or(AssertOriginResourceV1::Arithmetic)
}

impl<T> OwnedPrefix<T> {
    pub(super) fn try_new(
        value: T,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<Self, AssertOriginResourceV1> {
        Self::try_new_with_reserve(value, budget, |values| values.try_reserve_exact(1))
    }

    // Closed production allocation; the private seam permits genuine allocator
    // refusal and excess-capacity tests without a global allocator replacement.
    fn try_new_with_reserve(
        value: T,
        budget: &mut AssertOriginBudgetV1<'_>,
        reserve: impl FnOnce(&mut Vec<T>) -> Result<(), TryReserveError>,
    ) -> Result<Self, AssertOriginResourceV1> {
        budget.charge_work(2)?;
        let requested = capacity_bytes::<T>(1)?;
        budget.reserve_storage(requested)?;
        let mut values = Vec::new();
        if reserve(&mut values).is_err() {
            drop(values);
            budget.release_storage(requested)?;
            return Err(AssertOriginResourceV1::Allocation);
        }
        let extent = capacity_bytes::<T>(values.capacity()).and_then(|actual| {
            if values.is_empty() && values.capacity() >= 1 && actual >= requested {
                Ok(actual)
            } else {
                Err(AssertOriginResourceV1::Accounting)
            }
        });
        let actual = match extent {
            Ok(actual) => actual,
            Err(error) => {
                drop(values);
                budget.release_storage(requested)?;
                return Err(error);
            }
        };
        if let Err(error) = budget.reserve_storage(actual - requested) {
            drop(values);
            budget.release_storage(requested)?;
            return Err(error);
        }
        values.push(value);
        Ok(Self { values })
    }

    pub(super) const fn get(&self) -> &T {
        &self.values.as_slice()[0]
    }

    pub(super) fn retained_storage(&self) -> Result<usize, AssertOriginResourceV1> {
        if self.values.len() != 1 {
            return Err(AssertOriginResourceV1::Accounting);
        }
        capacity_bytes::<T>(self.values.capacity())
    }

    #[cfg(test)]
    pub(super) fn test_capacity_v1(&self) -> usize {
        self.values.capacity()
    }
}

#[cfg(test)]
#[path = "production_checked_output_owned_prefix_v1_tests.rs"]
mod production_checked_output_owned_prefix;
