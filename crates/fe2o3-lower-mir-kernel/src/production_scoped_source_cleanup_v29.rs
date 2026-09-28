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

    fn settle(&self, budget: &mut ArgumentBudgetV1<'_>) -> Result<(), ArgumentResourceV1> {
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
        budget.release_storage(self.header).inspect_err(|_| {
            self.cleanup.deny_refund();
        })
    }

    fn run<'work, T, E: From<ArgumentResourceV1>, F>(
        mut self,
        budget: &mut ArgumentBudgetV1<'work>,
        run: F,
    ) -> Result<T, E>
    where
        F: FnOnce(&ScopedSourceCleanupV29, &mut ArgumentBudgetV1<'work>) -> Result<T, E>,
    {
        let preparation = (|| {
            let header = scoped_source_callback_headers_v29::<T, E, F>()?;
            let total = argument_sum_v1(&[self.header, header])?;
            budget.reserve_storage(header)?;
            self.header = total;
            Ok::<(), ArgumentResourceV1>(())
        })();
        // The owned callback is captured by value and dropped inside this catch,
        // including when prepayment refuses before invocation.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            preparation.map_err(E::from)?;
            run(&self.cleanup, budget)
        }));
        let settlement = self.settle(budget);
        match result {
            Ok(Ok(value)) => match settlement {
                Ok(()) => Ok(value),
                Err(error) => {
                    drop(value);
                    Err(error.into())
                }
            },
            Ok(Err(error)) => Err(error),
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}

fn scoped_source_callback_headers_v29<T, E, F>() -> Result<usize, ArgumentResourceV1> {
    type Capture<'a, 'work, F> = (
        F,
        &'a ScopedSourceCleanupBoundaryV29,
        &'a mut ArgumentBudgetV1<'work>,
        Result<(), ArgumentResourceV1>,
    );
    argument_sum_v1(&[
        size_of::<F>(),
        std::mem::align_of::<F>(),
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
    let boundary = ScopedSourceCleanupBoundaryV29::new::<T, E>(rollback_floor, budget)?;
    boundary.run(budget, run)
}

fn scoped_source_attempt_v29<'work, T, E: From<ArgumentResourceV1>, F>(
    cleanup: &ScopedSourceCleanupV29,
    budget: &mut ArgumentBudgetV1<'work>,
    floor: usize,
    run: F,
) -> Result<T, E>
where
    F: FnOnce(&mut ArgumentBudgetV1<'work>) -> Result<T, E>,
{
    if cleanup.is_denied() || budget.storage() < floor {
        cleanup.deny_refund();
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let slot = std::ptr::from_ref(budget) as usize;
    let ledger = budget.work_ledger_identity_v1();
    let mut header = 0;
    let mut required = budget.storage();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let bytes = scoped_source_attempt_headers_v29::<T, E, F>()?;
        let minimum = argument_sum_v1(&[required, bytes])?;
        budget.reserve_storage(bytes)?;
        header = bytes;
        required = minimum;
        run(budget)
    }));
    if slot != std::ptr::from_ref(budget) as usize
        || ledger != budget.work_ledger_identity_v1()
        || budget.storage() < required
    {
        cleanup.deny_refund();
    }
    match result {
        Ok(Ok(value)) if !cleanup.is_denied() => {
            budget
                .release_storage(header)
                .inspect_err(|_| cleanup.deny_refund())?;
            Ok(value)
        }
        Ok(Ok(value)) => {
            drop(value);
            Err(ArgumentResourceV1::Accounting.into())
        }
        Ok(Err(error)) => {
            if !cleanup.is_denied() && budget.release_storage(budget.storage() - floor).is_err() {
                cleanup.deny_refund();
            }
            Err(error)
        }
        Err(payload) => {
            if !cleanup.is_denied() && budget.release_storage(budget.storage() - floor).is_err() {
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
        F,
        &'a mut ArgumentBudgetV1<'work>,
        &'a mut usize,
        &'a mut usize,
    );
    argument_sum_v1(&[
        size_of::<F>(),
        std::mem::align_of::<F>(),
        size_of::<Frame<'_, '_, F>>(),
        size_of::<Capture<'_, '_, F>>(),
        size_of::<std::panic::AssertUnwindSafe<Capture<'_, '_, F>>>(),
        size_of::<std::thread::Result<Result<T, E>>>(),
        size_of::<Result<T, E>>(),
        size_of::<T>(),
        size_of::<E>(),
        size_of::<Box<dyn std::any::Any + Send>>(),
        size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>(),
        // The seventh slot is the caller's post-header retained/scratch floor.
        7 * size_of::<usize>(),
        size_of::<bool>(),
        size_of::<Result<usize, ArgumentResourceV1>>(),
        size_of::<Result<(), ArgumentResourceV1>>(),
    ])
}
