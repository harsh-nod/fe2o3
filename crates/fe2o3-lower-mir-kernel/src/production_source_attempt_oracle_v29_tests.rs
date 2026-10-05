// Independent layout equation for a live attempt surrounding an inner debit.
pub(crate) fn scoped_source_attempt_header_oracle_v29<T, E, F>() -> usize {
    type Args<'a, 'w, F> = (
        &'a ScopedSourceCleanupV29,
        &'a mut ArgumentBudgetV1<'w>,
        usize,
        F,
    );
    type Capture<'a, 'w, F> = (
        &'a mut Option<F>,
        &'a mut ArgumentBudgetV1<'w>,
        &'a Result<(), ArgumentResourceV1>,
        &'a mut Option<usize>,
        &'a usize,
    );
    size_of::<F>()
        + std::mem::align_of::<F>()
        + size_of::<Option<F>>()
        + std::mem::align_of::<Option<F>>()
        + size_of::<Args<'_, '_, F>>()
        + size_of::<Capture<'_, '_, F>>()
        + size_of::<std::panic::AssertUnwindSafe<Capture<'_, '_, F>>>()
        + size_of::<std::thread::Result<Result<T, E>>>()
        + size_of::<Result<T, E>>()
        + size_of::<T>()
        + size_of::<E>()
        + size_of::<Box<dyn std::any::Any + Send>>()
        + size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
        + 8 * size_of::<usize>()
        + size_of::<Option<usize>>()
        + size_of::<bool>()
        + size_of::<Result<usize, ArgumentResourceV1>>()
        + size_of::<Result<(), ArgumentResourceV1>>()
        + source_owned_finish_header_oracle_v26::<T, E>()
}
