use super::{GuardLedger, ResourceError};
use crate::verification_index_v1::{
    verification_bounded_sort_by_v1, verification_ceil_log2_v1, verification_find_last_by_v1,
};
use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1,
};
use std::{cmp::Ordering, mem::size_of};

pub(in crate::formal_memory_obligations) trait GuardMeter {
    fn charge(&mut self, work: usize) -> Result<(), ResourceError>;
    fn storage(&mut self, bytes: usize) -> Result<(), ResourceError>;
    fn reserve<T>(&mut self, rows: &mut Vec<T>, count: usize) -> Result<(), ResourceError>;
    fn push<T>(&mut self, rows: &mut Vec<T>, value: T) -> Result<(), ResourceError> {
        self.charge(1)?;
        if rows.len() == rows.capacity() {
            let count = rows
                .capacity()
                .max(1)
                .checked_mul(2)
                .ok_or(ResourceError::Arithmetic)?;
            self.reserve(rows, count)?;
        }
        rows.push(value);
        Ok(())
    }
    fn sort<T>(
        &mut self,
        rows: &mut [T],
        width: usize,
        compare: impl FnMut(&T, &T) -> Ordering,
    ) -> Result<(), ResourceError>;
    fn find_width<T>(
        &mut self,
        rows: &[T],
        width: usize,
        compare: impl FnMut(&T) -> Ordering,
    ) -> Result<Option<usize>, ResourceError>;
    fn find<T>(
        &mut self,
        rows: &[T],
        compare: impl FnMut(&T) -> Ordering,
    ) -> Result<Option<usize>, ResourceError> {
        self.find_width(rows, 1, compare)
    }
}

impl GuardMeter for GuardLedger {
    fn charge(&mut self, work: usize) -> Result<(), ResourceError> {
        GuardLedger::charge(self, work)
    }
    fn storage(&mut self, bytes: usize) -> Result<(), ResourceError> {
        GuardLedger::storage(self, bytes)
    }
    fn reserve<T>(&mut self, rows: &mut Vec<T>, count: usize) -> Result<(), ResourceError> {
        GuardLedger::reserve(self, rows, count)
    }
    fn sort<T>(
        &mut self,
        rows: &mut [T],
        width: usize,
        compare: impl FnMut(&T, &T) -> Ordering,
    ) -> Result<(), ResourceError> {
        GuardLedger::sort(self, rows, width, compare)
    }
    fn find_width<T>(
        &mut self,
        rows: &[T],
        width: usize,
        compare: impl FnMut(&T) -> Ordering,
    ) -> Result<Option<usize>, ResourceError> {
        verification_find_last_by_v1(rows, width, &mut Budget::new(&mut self.work, 0), compare)
            .map_err(Into::into)
    }
}

pub(super) struct LiveGuardMeter<'b, 'w> {
    pub(super) budget: &'b mut Budget<'w>,
    work: CanonicalKernelIrWorkBudgetV1,
    bytes: usize,
    storage_limit: usize,
    records: usize,
    record_limit: usize,
}

impl<'b, 'w> LiveGuardMeter<'b, 'w> {
    pub(super) fn new(
        budget: &'b mut Budget<'w>,
        work: usize,
        storage: usize,
        records: usize,
    ) -> Self {
        Self {
            budget,
            work: CanonicalKernelIrWorkBudgetV1::new(work),
            bytes: 0,
            storage_limit: storage,
            records: 0,
            record_limit: records,
        }
    }

    fn local_storage(&mut self, bytes: usize) -> Result<(), ResourceError> {
        let actual = self
            .bytes
            .checked_add(bytes)
            .ok_or(ResourceError::Arithmetic)?;
        if actual > self.storage_limit {
            return Err(ResourceError::Storage {
                actual,
                limit: self.storage_limit,
            });
        }
        self.bytes = actual;
        Ok(())
    }

    fn local_records(&mut self, count: usize) -> Result<(), ResourceError> {
        let actual = self
            .records
            .checked_add(count)
            .ok_or(ResourceError::Arithmetic)?;
        if actual > self.record_limit {
            return Err(ResourceError::Storage {
                actual,
                limit: self.record_limit,
            });
        }
        self.records = actual;
        Ok(())
    }
}

impl GuardMeter for LiveGuardMeter<'_, '_> {
    fn charge(&mut self, work: usize) -> Result<(), ResourceError> {
        self.work.charge_work(work).map_err(ResourceError::Work)?;
        self.budget.charge_work(work).map_err(Into::into)
    }
    fn storage(&mut self, bytes: usize) -> Result<(), ResourceError> {
        self.local_storage(bytes)?;
        self.budget.reserve_storage(bytes).map_err(Into::into)
    }
    fn reserve<T>(&mut self, rows: &mut Vec<T>, count: usize) -> Result<(), ResourceError> {
        self.charge(2)?;
        if rows.capacity() >= count {
            return Ok(());
        }
        self.charge(rows.len())?;
        self.local_records(count)?;
        let old = rows
            .capacity()
            .checked_mul(size_of::<T>())
            .ok_or(ResourceError::Arithmetic)?;
        self.storage(
            count
                .checked_mul(size_of::<T>())
                .ok_or(ResourceError::Arithmetic)?,
        )?;
        rows.try_reserve_exact(
            count
                .checked_sub(rows.len())
                .ok_or(ResourceError::Accounting)?,
        )
        .map_err(|_| ResourceError::Allocation)?;
        let excess = rows
            .capacity()
            .checked_sub(count)
            .ok_or(ResourceError::Accounting)?;
        self.local_records(excess)?;
        self.storage(
            excess
                .checked_mul(size_of::<T>())
                .ok_or(ResourceError::Arithmetic)?,
        )?;
        // The allocator has retired the previous backing before this refund.
        self.budget.release_storage(old).map_err(Into::into)
    }
    fn sort<T>(
        &mut self,
        rows: &mut [T],
        width: usize,
        compare: impl FnMut(&T, &T) -> Ordering,
    ) -> Result<(), ResourceError> {
        let work = rows
            .len()
            .checked_mul(verification_ceil_log2_v1(rows.len()))
            .and_then(|n| n.checked_mul(4))
            .and_then(|n| n.checked_mul(width.max(1)))
            .ok_or(ResourceError::Arithmetic)?;
        self.work.charge_work(work).map_err(ResourceError::Work)?;
        verification_bounded_sort_by_v1(rows, width, self.budget, compare).map_err(Into::into)
    }
    fn find_width<T>(
        &mut self,
        rows: &[T],
        width: usize,
        compare: impl FnMut(&T) -> Ordering,
    ) -> Result<Option<usize>, ResourceError> {
        // This local conservative cap is independent of the external helper's
        // actual comparison-by-comparison accepted prefix.
        let work =
            verification_ceil_log2_v1(rows.len().checked_add(1).ok_or(ResourceError::Arithmetic)?)
                .checked_add(1)
                .and_then(|n| n.checked_mul(width.max(1)))
                .and_then(|n| n.checked_add(1))
                .ok_or(ResourceError::Arithmetic)?;
        self.work.charge_work(work).map_err(ResourceError::Work)?;
        verification_find_last_by_v1(rows, width, self.budget, compare).map_err(Into::into)
    }
}
