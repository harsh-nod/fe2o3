// Keep an uncalled user capture in protected custody across compiler queries
// and constructor unwinding. Moving this guard into a constructor ensures its
// disposal precedes that constructor's scratch refund, even on raw panic.
#[repr(transparent)]
struct SourceCallbackCustodyV29<F>(Option<F>);

impl<F> SourceCallbackCustodyV29<F> {
    fn new(callback: F) -> Self {
        Self(Some(callback))
    }

    fn take(&mut self) -> Option<F> {
        self.0.take()
    }

    fn as_mut(&mut self) -> Option<&mut F> {
        self.0.as_mut()
    }

    fn prepare<T, E>(self, prepare: impl FnOnce() -> Result<T, E>) -> Result<(T, Self), E> {
        let prepared = prepare()?;
        Ok((prepared, self))
    }

    fn finish<T, E>(
        mut self,
        selected: std::thread::Result<Result<T, E>>,
    ) -> std::thread::Result<Result<T, E>> {
        let disposed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            drop(self.take());
        }));
        match (selected, disposed) {
            (Ok(Ok(value)), Err(payload)) => {
                source_reference_discard_v29(value);
                Err(payload)
            }
            (selected, Err(payload)) => {
                source_reference_discard_v29(payload);
                selected
            }
            (selected, Ok(())) => selected,
        }
    }
}

fn source_callback_custody_finish_preflight_v29<T, E>(
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, ArgumentResourceV1> {
    budget.charge_work(SOURCE_REFERENCE_PAYLOAD_ATTEMPTS_V29 + 4)?;
    argument_sum_v1(&[
        argument_product_v1(2, size_of::<std::thread::Result<Result<T, E>>>())?,
        size_of::<Result<T, E>>(),
        size_of::<std::thread::Result<()>>(),
        size_of::<Box<dyn std::any::Any + Send>>(),
        size_of::<&mut SourceCallbackCustodyV29<()>>(),
        size_of::<std::panic::AssertUnwindSafe<&mut SourceCallbackCustodyV29<()>>>(),
        source_reference_cleanup_headers_v29()?,
    ])
}

impl<F> Drop for SourceCallbackCustodyV29<F> {
    fn drop(&mut self) {
        if let Some(callback) = self.0.take() {
            source_reference_discard_v29(callback);
        }
    }
}

// A refusal propagates through one operation; a clear cell authorizes nothing.
struct ScopedSourceCleanupV29 {
    denied: std::cell::Cell<bool>,
    #[cfg(test)]
    fault: std::cell::RefCell<Option<ScopedSourceCleanupFaultV29>>,
    #[cfg(test)]
    fault_storage: std::cell::Cell<Option<usize>>,
    #[cfg(test)]
    fault_skip: std::cell::Cell<usize>,
}

#[cfg(test)]
enum ScopedSourceCleanupFaultV29 {
    Error {
        undercut: bool,
    },
    Panic {
        undercut: bool,
        payload: Box<dyn std::any::Any + Send>,
    },
}

impl ScopedSourceCleanupV29 {
    fn deny_refund(&self) {
        self.denied.set(true);
    }

    fn is_denied(&self) -> bool {
        self.denied.get()
    }

    fn observe_table(
        &self,
        source: &ProductionSemanticSsaOwnerV1,
        layouts: &source_storage_v29::SourceStorageLayoutsV29<'_>,
        budget: &ArgumentBudgetV1<'_>,
    ) {
        if !layouts.permits_table_refund(source, 0, budget) {
            self.deny_refund();
        }
    }

    #[cfg(test)]
    fn source_fault(
        &self,
        floor: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.fault.borrow().is_none() {
            return Ok(());
        }
        if let Some(remaining) = self.fault_skip.get().checked_sub(1) {
            self.fault_skip.set(remaining);
            return Ok(());
        }
        let Some(fault) = self.fault.borrow_mut().take() else {
            return Ok(());
        };
        let original = unsupported(0, None, None, "selected source cleanup error");
        let undercut = match &fault {
            ScopedSourceCleanupFaultV29::Error { undercut }
            | ScopedSourceCleanupFaultV29::Panic { undercut, .. } => *undercut,
        };
        if undercut {
            let target = floor.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
            budget.release_storage(
                budget
                    .storage()
                    .checked_sub(target)
                    .ok_or(ArgumentResourceV1::Accounting)?,
            )?;
        }
        self.fault_storage.set(Some(budget.storage()));
        match fault {
            ScopedSourceCleanupFaultV29::Error { .. } => Err(original),
            ScopedSourceCleanupFaultV29::Panic { payload, .. } => {
                drop(original);
                std::panic::resume_unwind(payload)
            }
        }
    }
}

struct ScopedSourceCleanupBoundaryV29 {
    cleanup: ScopedSourceCleanupV29,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    rollback_floor: usize,
    header: usize,
}

impl ScopedSourceCleanupBoundaryV29 {
    fn new<T, E>(
        rollback_floor: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ArgumentResourceV1> {
        if budget.storage() < rollback_floor {
            return Err(ArgumentResourceV1::Accounting);
        }
        let header = argument_sum_v1(&[
            size_of::<Self>(),
            size_of::<std::thread::Result<Result<T, E>>>(),
        ])?;
        budget.reserve_storage(header)?;
        Ok(Self {
            cleanup: ScopedSourceCleanupV29 {
                denied: std::cell::Cell::new(false),
                #[cfg(test)]
                fault: std::cell::RefCell::new(None),
                #[cfg(test)]
                fault_storage: std::cell::Cell::new(None),
                #[cfg(test)]
                fault_skip: std::cell::Cell::new(0),
            },
            slot: std::ptr::from_ref(budget) as usize,
            ledger: budget.work_ledger_identity_v1(),
            rollback_floor,
            header,
        })
    }

    fn check(&self, budget: &ArgumentBudgetV1<'_>) -> Result<(), ArgumentResourceV1> {
        if self.cleanup.is_denied() {
            return Err(ArgumentResourceV1::Accounting);
        }
        let valid = self.slot == std::ptr::from_ref(budget) as usize
            && self.ledger == budget.work_ledger_identity_v1()
            && self
                .rollback_floor
                .checked_add(self.header)
                .is_some_and(|minimum| budget.storage() >= minimum);
        if !valid {
            self.cleanup.deny_refund();
            return Err(ArgumentResourceV1::Accounting);
        }
        Ok(())
    }

    fn run<'work, T, E: From<ArgumentResourceV1>, F>(
        mut self,
        budget: &mut ArgumentBudgetV1<'work>,
        run: F,
    ) -> Result<T, E>
    where
        F: FnOnce(&ScopedSourceCleanupV29, &mut ArgumentBudgetV1<'work>) -> Result<T, E>,
    {
        let mut run = SourceCallbackCustodyV29::new(run);
        let preparation = (|| {
            let header = argument_sum_v1(&[
                scoped_source_callback_headers_v29::<T, E, F>()?,
                source_owned_finish_preflight_v26::<T, E>(budget)?,
            ])?;
            let total = argument_sum_v1(&[self.header, header])?;
            budget.reserve_storage(header)?;
            self.header = total;
            Ok::<(), ArgumentResourceV1>(())
        })();
        // The owned callback is captured by value and dropped inside this catch,
        // including when prepayment refuses before invocation.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match preparation {
            Ok(()) => {
                run.take().expect("scoped source callback is invoked once")(&self.cleanup, budget)
            }
            Err(error) => {
                source_reference_discard_v29(run.take());
                Err(E::from(error))
            }
        }));
        let retained = budget.storage();
        let checked = self.check(budget);
        let result = match (result, checked) {
            (Ok(Ok(value)), Err(error)) => {
                source_reference_discard_v29(value);
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| Err(E::from(error))))
            }
            (result, _) => result,
        };
        // Destruction and conversion cannot authorize refund of callback-owned
        // allocations or undercut the exact retained post-callback floor.
        let valid = self.check(budget).is_ok() && budget.storage() >= retained;
        if !valid {
            self.cleanup.deny_refund();
        }
        let settlement = if valid {
            budget
                .release_storage(self.header)
                .inspect_err(|_| self.cleanup.deny_refund())
        } else {
            Err(ArgumentResourceV1::Accounting)
        };
        match (result, settlement) {
            (Ok(Ok(value)), Ok(())) => Ok(value),
            (Ok(Ok(value)), Err(error)) => {
                source_reference_discard_v29(value);
                Err(E::from(error))
            }
            (Ok(Err(error)), _) => Err(error),
            (Err(payload), _) => std::panic::resume_unwind(payload),
        }
    }
}

fn scoped_source_callback_headers_v29<T, E, F>() -> Result<usize, ArgumentResourceV1> {
    type Capture<'a, 'work, F> = (
        &'a mut SourceCallbackCustodyV29<F>,
        &'a ScopedSourceCleanupV29,
        &'a mut ArgumentBudgetV1<'work>,
        &'a Result<(), ArgumentResourceV1>,
    );
    argument_sum_v1(&[
        size_of::<F>(),
        std::mem::align_of::<F>(),
        size_of::<SourceCallbackCustodyV29<F>>(),
        std::mem::align_of::<SourceCallbackCustodyV29<F>>(),
        size_of::<Capture<'_, '_, F>>(),
        size_of::<std::panic::AssertUnwindSafe<Capture<'_, '_, F>>>(),
        size_of::<(F, &ScopedSourceCleanupV29, &mut ArgumentBudgetV1<'_>)>(),
        size_of::<T>(),
        size_of::<E>(),
        size_of::<Result<T, E>>(),
        size_of::<Result<(), ArgumentResourceV1>>(),
        size_of::<Result<usize, ArgumentResourceV1>>(),
        size_of::<Box<dyn std::any::Any + Send>>(),
        2 * size_of::<usize>(),
    ])
}

fn with_scoped_source_cleanup_v29<'work, T, E: From<ArgumentResourceV1>>(
    budget: &mut ArgumentBudgetV1<'work>,
    rollback_floor: usize,
    run: impl FnOnce(&ScopedSourceCleanupV29, &mut ArgumentBudgetV1<'work>) -> Result<T, E>,
) -> Result<T, E> {
    let mut run = SourceCallbackCustodyV29::new(run);
    let boundary = match ScopedSourceCleanupBoundaryV29::new::<T, E>(rollback_floor, budget) {
        Ok(boundary) => boundary,
        Err(error) => {
            source_reference_discard_v29(run.take());
            return Err(E::from(error));
        }
    };
    boundary.run(
        budget,
        run.take().expect("scoped source callback is present"),
    )
}

// Compiler scratch transaction only: all retained growth on Err/unwind must
// belong to abandoned construction. Nested visitors must independently enforce
// nonescaping storage and deny this scope's refund on changed custody. Escaping
// callback-owned credit is settled separately by its exact ownership receipt.
fn scoped_source_attempt_v29<'work, T, E: From<ArgumentResourceV1>, F>(
    cleanup: &ScopedSourceCleanupV29,
    budget: &mut ArgumentBudgetV1<'work>,
    floor: usize,
    run: F,
) -> Result<T, E>
where
    F: FnOnce(&mut ArgumentBudgetV1<'work>) -> Result<T, E>,
{
    let mut run = SourceCallbackCustodyV29::new(run);
    if cleanup.is_denied() || budget.storage() < floor {
        cleanup.deny_refund();
        source_reference_discard_v29(run.take());
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let slot = std::ptr::from_ref(budget) as usize;
    let ledger = budget.work_ledger_identity_v1();
    let mut header = 0;
    let mut required = budget.storage();
    let mut rollback = None;
    let preparation = (|| {
        let bytes = scoped_source_attempt_headers_v29::<T, E, F>()?;
        source_owned_finish_preflight_v26::<T, E>(budget)?;
        let minimum = argument_sum_v1(&[required, bytes])?;
        budget.reserve_storage(bytes)?;
        header = bytes;
        required = minimum;
        Ok::<(), ArgumentResourceV1>(())
    })();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match preparation {
        Ok(()) => run.take().expect("scoped source attempt is invoked once")(budget),
        Err(error) => {
            // Freeze this constructor's rollback before arbitrary capture
            // disposal or error conversion. Later growth is not our credit.
            rollback = budget.storage().checked_sub(floor);
            source_reference_discard_v29(run.take());
            Err(E::from(error))
        }
    }));
    let retained = budget.storage();
    let rollback = rollback.or_else(|| retained.checked_sub(floor));
    if slot != std::ptr::from_ref(budget) as usize
        || ledger != budget.work_ledger_identity_v1()
        || budget.storage() < required
    {
        cleanup.deny_refund();
    }
    let result = match result {
        Ok(Ok(value)) if cleanup.is_denied() => {
            source_reference_discard_v29(value);
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                Err(E::from(ArgumentResourceV1::Accounting))
            }))
        }
        result => result,
    };
    if slot != std::ptr::from_ref(budget) as usize
        || ledger != budget.work_ledger_identity_v1()
        || budget.storage() < retained
    {
        cleanup.deny_refund();
    }
    match result {
        Ok(Ok(value)) if !cleanup.is_denied() => match budget.release_storage(header) {
            Ok(()) => Ok(value),
            Err(error) => {
                cleanup.deny_refund();
                source_reference_discard_v29(value);
                Err(E::from(error))
            }
        },
        Ok(Ok(value)) => {
            source_reference_discard_v29(value);
            Err(ArgumentResourceV1::Accounting.into())
        }
        Ok(Err(error)) => {
            if !cleanup.is_denied()
                && rollback.is_none_or(|bytes| budget.release_storage(bytes).is_err())
            {
                cleanup.deny_refund();
            }
            Err(error)
        }
        Err(payload) => {
            if !cleanup.is_denied()
                && rollback.is_none_or(|bytes| budget.release_storage(bytes).is_err())
            {
                cleanup.deny_refund();
            }
            std::panic::resume_unwind(payload)
        }
    }
}

fn scoped_source_attempt_headers_v29<T, E, F>() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a, 'work, F> = (
        &'a ScopedSourceCleanupV29,
        &'a mut ArgumentBudgetV1<'work>,
        usize,
        F,
    );
    type Capture<'a, 'work, F> = (
        &'a mut SourceCallbackCustodyV29<F>,
        &'a mut ArgumentBudgetV1<'work>,
        &'a Result<(), ArgumentResourceV1>,
        &'a mut Option<usize>,
        &'a usize,
    );
    argument_sum_v1(&[
        size_of::<F>(),
        std::mem::align_of::<F>(),
        size_of::<SourceCallbackCustodyV29<F>>(),
        std::mem::align_of::<SourceCallbackCustodyV29<F>>(),
        size_of::<Frame<'_, '_, F>>(),
        size_of::<Capture<'_, '_, F>>(),
        size_of::<std::panic::AssertUnwindSafe<Capture<'_, '_, F>>>(),
        size_of::<std::thread::Result<Result<T, E>>>(),
        size_of::<Result<T, E>>(),
        size_of::<T>(),
        size_of::<E>(),
        size_of::<Box<dyn std::any::Any + Send>>(),
        size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>(),
        // Retain both the exact post-callback floor and frozen rollback extent.
        8 * size_of::<usize>(),
        size_of::<Option<usize>>(),
        size_of::<bool>(),
        size_of::<Result<usize, ArgumentResourceV1>>(),
        size_of::<Result<(), ArgumentResourceV1>>(),
        source_owned_finish_headers_v26::<T, E>()?,
    ])
}

#[cfg(test)]
include!("production_source_attempt_oracle_v29_tests.rs");
