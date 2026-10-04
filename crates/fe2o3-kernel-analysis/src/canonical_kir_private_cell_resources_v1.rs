use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1;
use std::panic::{AssertUnwindSafe, catch_unwind};

pub(super) struct Meter<'a, 'w> {
    budget: &'a mut Budget<'w>,
    slot: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    live: usize,
    cleanup: bool,
    failed: bool,
}
impl<'w> Meter<'_, 'w> {
    #[cfg(test)]
    pub(super) fn budget_for_test(&mut self) -> &mut Budget<'w> {
        self.budget
    }
    fn check(&mut self) -> Result<()> {
        let expected = self.floor.checked_add(self.live);
        if self.slot != self.budget as *const Budget<'_> as usize
            || self.ledger != self.budget.work_ledger_identity_v1()
            || expected.is_none_or(|expected| self.budget.storage() < expected)
        {
            self.cleanup = false;
            self.failed = true;
        } else if expected != Some(self.budget.storage()) {
            self.failed = true;
        }
        if self.failed {
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }
    pub(super) fn work(&mut self, count: usize) -> Result<()> {
        self.check()?;
        self.budget.charge_work(count)?;
        Ok(())
    }
    pub(super) fn reserve(&mut self, bytes: usize) -> Result<()> {
        self.check()?;
        let next = self.live.checked_add(bytes).ok_or(Resource::Arithmetic)?;
        self.budget.reserve_storage(bytes)?;
        self.live = next;
        Ok(())
    }
    pub(super) fn release(&mut self, bytes: usize) -> Result<()> {
        self.check()?;
        let next = self.live.checked_sub(bytes).ok_or(Resource::Accounting)?;
        self.budget.release_storage(bytes)?;
        self.live = next;
        Ok(())
    }
    pub(super) fn table<T>(&mut self, count: usize) -> Result<(Vec<T>, usize)> {
        self.work(5)?;
        let requested = count
            .checked_mul(size_of::<T>())
            .ok_or(Resource::Arithmetic)?;
        self.reserve(requested)?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(count)
            .map_err(|_| Resource::Allocation)?;
        let actual = self.capacity::<T>(count, rows.capacity())?;
        Ok((rows, actual))
    }
    pub(super) fn capacity<T>(&mut self, requested: usize, actual: usize) -> Result<usize> {
        let requested = requested
            .checked_mul(size_of::<T>())
            .ok_or(Resource::Arithmetic)?;
        let actual = actual
            .checked_mul(size_of::<T>())
            .ok_or(Resource::Arithmetic)?;
        self.reserve(actual.checked_sub(requested).ok_or(Resource::Accounting)?)?;
        Ok(actual)
    }
    pub(super) fn push<T>(&mut self, rows: &mut Vec<T>, row: T) -> Result<()> {
        self.work(1)?;
        if rows.len() == rows.capacity() {
            return Err(Resource::Accounting.into());
        }
        rows.push(row);
        Ok(())
    }
    /// Only the existing scoped CFG is called here. It returns the exact floor;
    /// callback-owned output tables have already been prepaid by this meter.
    pub(super) fn cfg<T>(&mut self, run: impl FnOnce(&mut Budget<'_>) -> Result<T>) -> Result<T> {
        self.check()?;
        let result = run(self.budget);
        self.check()?;
        result
    }
}

pub(super) fn scoped<'w, T>(
    budget: &mut Budget<'w>,
    run: impl FnOnce(&mut Meter<'_, 'w>) -> Result<T>,
) -> Result<T> {
    let mut meter = Meter {
        slot: budget as *const Budget<'_> as usize,
        ledger: budget.work_ledger_identity_v1(),
        floor: budget.storage(),
        live: 0,
        cleanup: true,
        failed: false,
        budget,
    };
    let mut panics = [None, None];
    let mut result = match catch_unwind(AssertUnwindSafe(|| run(&mut meter))) {
        Ok(result) => result,
        Err(payload) => {
            panics[0] = Some(payload);
            Err(Error::Panicked)
        }
    };
    if let Err(error) = meter.check() {
        let rejected = std::mem::replace(&mut result, Err(error));
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(rejected))) {
            panics[1] = Some(payload);
        }
    }
    if meter.cleanup
        && let Err(error) = meter.budget.release_storage(meter.live)
    {
        let rejected = std::mem::replace(&mut result, Err(error.into()));
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(rejected))) {
            panics[1] = Some(payload);
        }
    }
    drop(panics);
    result
}
