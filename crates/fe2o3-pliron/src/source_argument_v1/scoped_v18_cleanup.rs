use std::mem::size_of;

fn with_argument_scratch_v18<'work, T>(
    cleanup: &CanonicalAnalysisCleanupV1<'_>,
    budget: &mut ArgumentBudgetV1<'work>,
    run: impl FnOnce(&mut ArgumentBudgetV1<'work>) -> Result<T, ProductionSourceArgumentErrorV1>,
) -> Result<T, ProductionSourceArgumentErrorV1> {
    let floor = budget.storage();
    let value = argument_attempt_v18(cleanup, budget, floor, run)?;
    // The run closure owns all scratch. Nested callback attempts have already
    // checked their larger live floors before any enclosing release can occur.
    let release = budget
        .storage()
        .checked_sub(floor)
        .ok_or(ArgumentResourceV1::Accounting)
        .and_then(|amount| budget.release_storage(amount));
    match release {
        Ok(()) => Ok(value),
        Err(error) => {
            cleanup.deny_refund();
            drop(value);
            Err(error.into())
        }
    }
}

fn argument_attempt_v18<'work, T, E: From<ArgumentResourceV1>, F>(
    cleanup: &CanonicalAnalysisCleanupV1<'_>,
    budget: &mut ArgumentBudgetV1<'work>,
    floor: usize,
    run: F,
) -> Result<T, E>
where
    F: FnOnce(&mut ArgumentBudgetV1<'work>) -> Result<T, E>,
{
    if cleanup.refund_denied() || budget.storage() < floor {
        cleanup.deny_refund();
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let slot = std::ptr::from_ref(budget) as usize;
    let ledger = budget.work_ledger_identity_v1();
    let mut header = 0;
    let mut required = budget.storage();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let bytes = argument_attempt_headers_v18::<T, E, F>()?;
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
        Ok(Ok(value)) if !cleanup.refund_denied() => {
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
            if !cleanup.refund_denied() && budget.release_storage(budget.storage() - floor).is_err()
            {
                cleanup.deny_refund();
            }
            Err(error)
        }
        Err(payload) => {
            if !cleanup.refund_denied() && budget.release_storage(budget.storage() - floor).is_err()
            {
                cleanup.deny_refund();
            }
            std::panic::resume_unwind(payload)
        }
    }
}

fn argument_attempt_headers_v18<T, E, F>() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a, 'work, F> = (
        &'a CanonicalAnalysisCleanupV1<'a>,
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
