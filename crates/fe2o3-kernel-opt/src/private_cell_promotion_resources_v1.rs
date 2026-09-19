use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1;
use std::{
    any::Any,
    panic::{AssertUnwindSafe, catch_unwind},
};

type Payload = Box<dyn Any + Send>;
pub(super) struct Meter<'a, 'w> {
    budget: &'a mut Budget<'w>,
    slot: usize,
    ledger: CanonicalKirWorkIdentity,
    floor: usize,
    live: usize,
    cleanup: bool,
    failed: bool,
    nested_panic: Option<Payload>,
}
type CanonicalKirWorkIdentity = CanonicalKernelIrWorkLedgerIdentityV1;
impl<'w> Meter<'_, 'w> {
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
            Err(Resource::Accounting.into())
        } else if self.nested_panic.is_some() {
            Err(Error::Panicked)
        } else {
            Ok(())
        }
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
        self.work(4)?;
        let requested = count
            .checked_mul(size_of::<T>())
            .ok_or(Resource::Arithmetic)?;
        self.reserve(requested)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| Resource::Allocation)?;
        let bytes = self.capacity::<T>(count, values.capacity())?;
        Ok((values, bytes))
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
    /// Private calls only to inventory/census, candidate copy, fresh admission
    /// and independent replay. They return unreserved receipts and restore
    /// storage normally. No caller callback is accepted.
    /// On unwind their local objects have dropped; still-reserved same-ledger
    /// scratch is adopted for cleanup, without resetting work or peak history.
    pub(super) fn derive<T>(
        &mut self,
        run: impl FnOnce(&mut Budget<'w>) -> Result<T>,
    ) -> Result<T> {
        self.check()?;
        match catch_unwind(AssertUnwindSafe(|| run(self.budget))) {
            Ok(result) => {
                self.check()?;
                result
            }
            Err(payload) => {
                self.nested_panic = Some(payload);
                if self.slot != self.budget as *const Budget<'_> as usize
                    || self.ledger != self.budget.work_ledger_identity_v1()
                    || self.budget.storage()
                        < self
                            .floor
                            .checked_add(self.live)
                            .ok_or(Resource::Arithmetic)?
                {
                    self.cleanup = false;
                    self.failed = true;
                    return Err(Resource::Accounting.into());
                }
                self.live = self
                    .budget
                    .storage()
                    .checked_sub(self.floor)
                    .ok_or(Resource::Accounting)?;
                Err(Error::Panicked)
            }
        }
    }
    #[cfg(test)]
    pub(super) fn budget_for_test(&mut self) -> &mut Budget<'w> {
        self.budget
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
        nested_panic: None,
        budget,
    };
    let mut panics = [None, None];
    let mut result = match catch_unwind(AssertUnwindSafe(|| {
        meter.reserve(size_of::<Meter<'_, '_>>())?;
        run(&mut meter)
    })) {
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
    // Nested and generic result/panic payloads cannot run destructors before
    // the accepted same-ledger reservation has been restored.
    drop(meter);
    drop(panics);
    result
}
