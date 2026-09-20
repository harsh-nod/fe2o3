use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1,
};
use std::{
    any::Any,
    marker::PhantomData,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

type Payload = Box<dyn Any + Send>;
pub(super) trait ScopeError: From<Resource> {
    fn panicked() -> Self;
}
pub(super) struct Meter<'a, 'w, E> {
    budget: &'a mut Budget<'w>,
    slot: usize,
    ledger: CanonicalKirWorkIdentity,
    floor: usize,
    live: usize,
    cleanup: bool,
    failed: bool,
    nested_panic: Option<Payload>,
    error: PhantomData<fn() -> E>,
}
type CanonicalKirWorkIdentity = CanonicalKernelIrWorkLedgerIdentityV1;
impl<'w, E: ScopeError> Meter<'_, 'w, E> {
    fn check(&mut self) -> Result<(), E> {
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
            Err(E::panicked())
        } else {
            Ok(())
        }
    }
    pub(super) fn work(&mut self, count: usize) -> Result<(), E> {
        self.check()?;
        self.budget.charge_work(count)?;
        Ok(())
    }
    pub(super) fn reserve(&mut self, bytes: usize) -> Result<(), E> {
        self.check()?;
        let next = self.live.checked_add(bytes).ok_or(Resource::Arithmetic)?;
        self.budget.reserve_storage(bytes)?;
        self.live = next;
        Ok(())
    }
    pub(super) fn release(&mut self, bytes: usize) -> Result<(), E> {
        self.check()?;
        let next = self.live.checked_sub(bytes).ok_or(Resource::Accounting)?;
        self.budget.release_storage(bytes)?;
        self.live = next;
        Ok(())
    }
    pub(super) fn table<T>(&mut self, count: usize) -> Result<(Vec<T>, usize), E> {
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
    pub(super) fn capacity<T>(&mut self, requested: usize, actual: usize) -> Result<usize, E> {
        let requested = requested
            .checked_mul(size_of::<T>())
            .ok_or(Resource::Arithmetic)?;
        let actual = actual
            .checked_mul(size_of::<T>())
            .ok_or(Resource::Arithmetic)?;
        self.reserve(actual.checked_sub(requested).ok_or(Resource::Accounting)?)?;
        Ok(actual)
    }
    pub(super) fn push<T>(&mut self, rows: &mut Vec<T>, row: T) -> Result<(), E> {
        self.work(1)?;
        if rows.len() == rows.capacity() {
            return Err(Resource::Accounting.into());
        }
        rows.push(row);
        Ok(())
    }
    /// Private calls only to inventory/census derives, which return unreserved
    /// receipts and restore storage normally. No caller callback is accepted.
    /// On unwind their local objects have dropped; still-reserved same-ledger
    /// scratch is adopted for cleanup, without resetting work or peak history.
    pub(super) fn derive<T>(
        &mut self,
        run: impl FnOnce(&mut Budget<'w>) -> Result<T, E>,
    ) -> Result<T, E> {
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
                Err(E::panicked())
            }
        }
    }
    #[cfg(test)]
    pub(super) fn budget_for_test(&mut self) -> &mut Budget<'w> {
        self.budget
    }
}

pub(super) fn scoped<'w, T, E: ScopeError>(
    budget: &mut Budget<'w>,
    run: impl FnOnce(&mut Meter<'_, 'w, E>) -> Result<T, E>,
) -> Result<T, E> {
    let mut meter = Meter {
        slot: budget as *const Budget<'_> as usize,
        ledger: budget.work_ledger_identity_v1(),
        floor: budget.storage(),
        live: 0,
        cleanup: true,
        failed: false,
        nested_panic: None,
        error: PhantomData,
        budget,
    };
    let mut panics = [None, None];
    let mut result = match catch_unwind(AssertUnwindSafe(|| {
        meter.reserve(size_of::<Meter<'_, '_, E>>())?;
        run(&mut meter)
    })) {
        Ok(result) => result,
        Err(payload) => {
            panics[0] = Some(payload);
            Err(E::panicked())
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

#[cfg(test)]
mod tests {
    use super::{Budget, CanonicalKirWorkIdentity, Meter, Payload, Resource, ScopeError, scoped};
    use crate::{
        CanonicalKirLoopPreheadersErrorV1 as LoopError,
        CanonicalKirPrivateCellPromotionErrorV1 as CellError,
    };
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    use std::{
        fmt::Debug,
        mem::{align_of, size_of},
        panic::panic_any,
    };

    // The former header's exact field types/order, with no resource algorithm.
    struct PriorMeterLayout<'a, 'w> {
        _budget: &'a mut Budget<'w>,
        _slot: usize,
        _ledger: CanonicalKirWorkIdentity,
        _floor: usize,
        _live: usize,
        _cleanup: bool,
        _failed: bool,
        _nested_panic: Option<Payload>,
    }

    #[test]
    fn typed_meter_preserves_prior_header_size_and_alignment() {
        assert_eq!(
            size_of::<Meter<'_, '_, CellError>>(),
            size_of::<PriorMeterLayout<'_, '_>>()
        );
        assert_eq!(
            size_of::<Meter<'_, '_, LoopError>>(),
            size_of::<PriorMeterLayout<'_, '_>>()
        );
        assert_eq!(
            align_of::<Meter<'_, '_, CellError>>(),
            align_of::<PriorMeterLayout<'_, '_>>()
        );
        assert_eq!(
            align_of::<Meter<'_, '_, LoopError>>(),
            align_of::<PriorMeterLayout<'_, '_>>()
        );
    }

    fn check_mapping<E: ScopeError + Debug>(classify: fn(E) -> Option<Resource>) {
        let floor = 37;
        let header = size_of::<PriorMeterLayout<'_, '_>>();
        let limit = floor + header + 7;
        for mode in 0..5 {
            let mut work = Work::new(5);
            {
                let mut budget = Budget::new(&mut work, limit);
                budget.reserve_storage(floor).unwrap();
                let result: Result<(), E> = scoped(&mut budget, |meter| {
                    meter.work(3)?;
                    match mode {
                        0 => meter.work(3),
                        1 => meter.reserve(8),
                        2 => panic_any("typed scope unwind"),
                        3 => meter.derive(|budget| {
                            budget.reserve_storage(7)?;
                            panic_any("typed nested unwind");
                        }),
                        _ => {
                            meter.budget_for_test().reserve_storage(1)?;
                            Ok(())
                        }
                    }
                });
                match (mode, classify(result.unwrap_err())) {
                    (0, Some(Resource::Work(error))) => {
                        assert_eq!((error.actual(), error.limit()), (6, 5));
                    }
                    (1, Some(Resource::Storage(error))) => {
                        assert_eq!((error.actual(), error.limit()), (limit + 1, limit));
                    }
                    (2 | 3, None) | (4, Some(Resource::Accounting)) => {}
                    actual => panic!("wrong typed scope error: {actual:?}"),
                }
                assert_eq!(budget.storage(), floor + usize::from(mode == 4));
                assert_eq!(budget.work(), 3);
                assert_eq!(budget.failed_storage(), (mode == 1).then_some(limit + 1));
                assert_eq!(
                    budget.peak_storage(),
                    floor
                        + header
                        + match mode {
                            3 => 7,
                            4 => 1,
                            _ => 0,
                        }
                );
            }
            assert_eq!(work.work(), 3);
            assert_eq!(work.failed_work(), (mode == 0).then_some(6));
        }
    }

    #[test]
    fn both_caller_errors_preserve_exact_scope_denials_and_history() {
        check_mapping::<CellError>(|error| match error {
            CellError::Resource(resource) => Some(resource),
            CellError::Panicked => None,
            other => panic!("unexpected private-cell error: {other:?}"),
        });
        check_mapping::<LoopError>(|error| match error {
            LoopError::Resource(resource) => Some(resource),
            LoopError::Panicked => None,
            other => panic!("unexpected preheader error: {other:?}"),
        });
    }
}
