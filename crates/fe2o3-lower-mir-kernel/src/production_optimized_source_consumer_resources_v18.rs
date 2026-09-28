//! Requested storage for source-native optimized consumers.
use super::*;

pub(super) fn reserve_entry(
    credit: &std::cell::Cell<usize>,
    bytes: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ArgumentResourceV1> {
    let next = credit
        .get()
        .checked_add(bytes)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    budget.reserve_storage(bytes)?;
    credit.set(next);
    Ok(())
}

pub(super) fn entry_custody(
    cleanup: &ScopedSourceCleanupV29,
    budget: &ArgumentBudgetV1<'_>,
    floor: usize,
    accepted: usize,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
) -> Result<(), ArgumentResourceV1> {
    // Nested work must restore its own floor before this boundary. An
    // unexplained residual is not ours to refund, including after unwind.
    if slot != std::ptr::from_ref(budget) as usize
        || ledger != budget.work_ledger_identity_v1()
        || floor.checked_add(accepted) != Some(budget.storage())
    {
        cleanup.deny_refund();
    }
    if cleanup.is_denied() {
        Err(ArgumentResourceV1::Accounting)
    } else {
        Ok(())
    }
}

pub(super) fn optimizer_entry_headers<T, E, F>(_: &F) -> Result<usize, ArgumentResourceV1> {
    optimizer_entry_headers_typed::<
        T,
        E,
        F,
        fe2o3_pliron::CheckedNeutralKernelIrOwnerV18,
        fe2o3_pliron::KirNeutralOptimizationOutputV18<'static>,
    >()
}

pub(super) fn optimizer_entry_headers_typed<T, E, F, C, O>() -> Result<usize, ArgumentResourceV1> {
    type Output<C, T> = (C, T, fe2o3_pliron::KirNeutralOwnedOriginStorageV1);
    type Entry<O, F> = (O, usize, usize, F);
    type Capture<'a, 'work, F> = (
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a mut ArgumentBudgetV1<'work>,
        &'a std::cell::Cell<usize>,
        usize,
        F,
    );
    type EntryResult<O, E, F> =
        Result<Entry<O, F>, SourceConsumerErrorV18<ProductionSourceOptimizationErrorV18<E>>>;
    type Adoption<'a, 'work, O, F> = (
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a mut ArgumentBudgetV1<'work>,
        O,
        F,
    );
    type Adopted<C, T, E> =
        Result<Output<C, T>, fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1<E>>;
    type Settled<C, T, E> =
        Result<Output<C, T>, SourceConsumerErrorV18<ProductionSourceOptimizationErrorV18<E>>>;
    argument_sum_v1(&[
        argument_product_v1(2, size_of::<Capture<'_, '_, F>>())?,
        argument_product_v1(2, std::mem::align_of::<Capture<'_, '_, F>>())?,
        size_of::<std::panic::AssertUnwindSafe<Capture<'_, '_, F>>>(),
        size_of::<Entry<O, F>>(),
        argument_product_v1(2, size_of::<EntryResult<O, E, F>>())?,
        size_of::<std::thread::Result<EntryResult<O, E, F>>>(),
        size_of::<std::panic::AssertUnwindSafe<Entry<O, F>>>(),
        size_of::<std::thread::Result<()>>(),
        size_of::<std::cell::Cell<usize>>(),
        argument_product_v1(3, size_of::<usize>())?,
        size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>(),
        size_of::<SourceOwnedResultV18<()>>(),
        argument_product_v1(4, size_of::<Adoption<'_, '_, O, F>>())?,
        argument_product_v1(4, std::mem::align_of::<Adoption<'_, '_, O, F>>())?,
        size_of::<std::panic::AssertUnwindSafe<Adoption<'_, '_, O, F>>>(),
        size_of::<Adopted<C, T, E>>(),
        size_of::<Settled<C, T, E>>(),
        size_of::<std::thread::Result<Settled<C, T, E>>>(),
        size_of::<Result<Output<C, T>, ProductionSourceOptimizationErrorV18<E>>>(),
        size_of::<
            std::panic::AssertUnwindSafe<
                Result<Output<C, T>, ProductionSourceOptimizationErrorV18<E>>,
            >,
        >(),
        size_of::<ProductionSourceOptimizationErrorV18<E>>(),
        source_reference_cleanup_headers_v29()?,
    ])
}

pub(super) fn retained_entry_headers<T, E, F>(_: &F) -> Result<usize, ArgumentResourceV1> {
    type Capture<'a, 'work, F> = (
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a mut ArgumentBudgetV1<'work>,
        &'a std::cell::Cell<usize>,
        &'a std::cell::Cell<usize>,
        F,
    );
    type Output<T> = (
        fe2o3_pliron::CheckedNeutralKernelIrOwnerV18,
        T,
        fe2o3_pliron::KirNeutralOwnedOriginStorageV1,
    );
    type Settled<T, E> =
        Result<Output<T>, SourceConsumerErrorV18<ProductionSourceOptimizationErrorV18<E>>>;
    argument_sum_v1(&[
        optimizer_transfer_headers::<T, E>(size_of::<Capture<'_, '_, F>>())?,
        size_of::<Capture<'_, '_, F>>(),
        argument_product_v1(2, std::mem::align_of::<Capture<'_, '_, F>>())?,
        size_of::<std::panic::AssertUnwindSafe<Capture<'_, '_, F>>>(),
        size_of::<std::cell::Cell<usize>>(),
        argument_product_v1(2, size_of::<usize>())?,
        size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>(),
        size_of::<SourceOwnedResultV18<()>>(),
        size_of::<Settled<T, E>>(),
        size_of::<std::thread::Result<Settled<T, E>>>(),
        size_of::<std::panic::AssertUnwindSafe<Settled<T, E>>>(),
    ])
}

pub(super) fn analysis_headers<T, E, F>(_: &F) -> Result<usize, ArgumentResourceV1> {
    type EntryCapture<'a, F> = (
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
        F,
    );
    type EntryCatch<'a, 'work, F> = (
        EntryCapture<'a, F>,
        &'a mut ArgumentBudgetV1<'work>,
        &'a std::cell::Cell<usize>,
    );
    type Entry<'a, F> = (
        &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        usize,
        F,
    );
    type EntryResult<'a, F> = Result<Entry<'a, F>, ProductionSourceOwnedViewErrorV18>;
    type Invoke<'a, 'work, F> = (
        F,
        &'a mut ProductionOptimizedSourceAnalysisV18<'a>,
        &'a mut ArgumentBudgetV1<'work>,
    );
    // This boundary pays its own frames on both source trees; it does not rely
    // on the differing generic scoped-attempt helper's transient reservation.
    argument_sum_v1(&[
        argument_product_v1(2, size_of::<EntryCapture<'_, F>>())?,
        argument_product_v1(2, std::mem::align_of::<EntryCapture<'_, F>>())?,
        size_of::<EntryCatch<'_, '_, F>>(),
        size_of::<std::panic::AssertUnwindSafe<EntryCatch<'_, '_, F>>>(),
        size_of::<Entry<'_, F>>(),
        argument_product_v1(2, size_of::<EntryResult<'_, F>>())?,
        size_of::<std::thread::Result<EntryResult<'_, F>>>(),
        size_of::<std::panic::AssertUnwindSafe<Entry<'_, F>>>(),
        size_of::<std::thread::Result<()>>(),
        size_of::<std::cell::Cell<usize>>(),
        argument_product_v1(2, size_of::<usize>())?,
        size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>(),
        size_of::<SourceOwnedResultV18<()>>(),
        size_of::<Invoke<'_, '_, F>>(),
        size_of::<std::panic::AssertUnwindSafe<Invoke<'_, '_, F>>>(),
        size_of::<ProductionOptimizedSourceAnalysisV18<'_>>(),
        size_of::<std::thread::Result<Result<T, E>>>(),
        size_of::<Result<T, E>>(),
        size_of::<std::panic::AssertUnwindSafe<Result<T, E>>>(),
        source_reference_cleanup_headers_v29()?,
    ])
}

pub(super) fn optimizer_transfer_headers<T, E>(
    closure_bytes: usize,
) -> Result<usize, ArgumentResourceV1> {
    type Output<T> = (
        fe2o3_pliron::CheckedNeutralKernelIrOwnerV18,
        T,
        fe2o3_pliron::KirNeutralOwnedOriginStorageV1,
    );
    type Result<T, E> = std::result::Result<Output<T>, ProductionSourceOptimizationErrorV18<E>>;
    argument_sum_v1(&[
        closure_bytes,
        size_of::<std::cell::Cell<usize>>(),
        size_of::<Output<T>>(),
        size_of::<Result<T, E>>(),
        size_of::<std::thread::Result<Result<T, E>>>(),
        size_of::<std::panic::AssertUnwindSafe<Output<T>>>(),
        size_of::<ProductionSourceOptimizationErrorV18<E>>(),
        source_reference_cleanup_headers_v29()?,
    ])
}

pub(super) fn optimizer_transfer_storage(
    checked: usize,
    origin: usize,
) -> Result<usize, ArgumentResourceV1> {
    checked
        .checked_add(origin)
        .ok_or(ArgumentResourceV1::Arithmetic)
}

pub(super) fn reserve_optimizer_transfer(
    checked: usize,
    origin: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ArgumentResourceV1> {
    budget.reserve_storage(optimizer_transfer_storage(checked, origin)?)
}
