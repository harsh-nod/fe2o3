//! Requested storage for source-native optimized consumers.
use super::*;

pub(super) fn optimizer_entry_headers<T, E>() -> Result<usize, ArgumentResourceV1> {
    type Output<T> = (
        fe2o3_pliron::CheckedNeutralKernelIrOwnerV18,
        T,
        fe2o3_pliron::KirNeutralOwnedOriginStorageV1,
    );
    argument_sum_v1(&[
        size_of::<Result<Output<T>, ProductionSourceOptimizationErrorV18<E>>>(),
        size_of::<
            std::panic::AssertUnwindSafe<
                Result<Output<T>, ProductionSourceOptimizationErrorV18<E>>,
            >,
        >(),
        size_of::<ProductionSourceOptimizationErrorV18<E>>(),
        source_reference_cleanup_headers_v29()?,
    ])
}

pub(super) fn analysis_headers<T, E>() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
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
