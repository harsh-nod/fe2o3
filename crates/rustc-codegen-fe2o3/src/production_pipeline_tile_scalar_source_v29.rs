//! Non-default, pre-verification consuming source connection.
//! A successful callback receives transport, not global-read/collective/launch safety.
use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionCheckedTileScalarTransportV29 as Checked,
    ProductionPendingScopedSourceOwnerV29 as Pending, ProductionSemanticKirLimitsV1,
    ProductionTileScalarOrderV29 as Order, ProductionTileScalarTransportErrorV29 as TransportError,
    ProductionTileScalarTransportOwnerV29 as Owner,
};
type Budget<'w> = CanonicalKernelIrVerificationResourceBudgetV1<'w>;
type TransportResult<T> = Result<T, TransportError>;

#[derive(Debug)]
pub(crate) enum ProductionTileScalarPipelineErrorV29 {
    Source(Box<ProductionPipelineError>),
    Transport(TransportError),
}
impl std::fmt::Display for ProductionTileScalarPipelineErrorV29 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(error) => error.fmt(formatter),
            Self::Transport(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for ProductionTileScalarPipelineErrorV29 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error.as_ref()),
            Self::Transport(error) => Some(error),
        }
    }
}
fn drain<T>(value: T) {
    let mut caught = catch_unwind(AssertUnwindSafe(|| drop(value)));
    while let Err(payload) = caught {
        caught = catch_unwind(AssertUnwindSafe(|| drop(payload)));
    }
}
fn consume_pending<'w, T>(
    pending: Pending,
    order: Order,
    budget: &mut Budget<'w>,
    consume: impl for<'scope> FnOnce(&Checked<'scope>, &mut Budget<'w>) -> TransportResult<T>,
) -> TransportResult<T> {
    let floor = budget
        .storage()
        .checked_sub(pending.adopted_storage())
        .ok_or(Resource::Accounting)?;
    let ledger = budget.work_ledger_identity_v1();
    let slot = std::ptr::from_ref(budget) as usize;
    let mut donor = Some(pending);
    let result = (|| {
        let headers = std::mem::size_of::<std::thread::Result<TransportResult<T>>>()
            .checked_add(std::mem::size_of::<std::thread::Result<()>>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(headers)?;
        let caught = catch_unwind(AssertUnwindSafe(|| {
            let owner = Owner::try_from_pending_with_budget_v29(&mut donor, order, budget)?;
            let result = owner.with_checked_transport_v29(budget, consume);
            drop(owner);
            result
        }));
        match caught {
            Ok(result) => result,
            Err(payload) => {
                drain(payload);
                Err(TransportError::Panicked)
            }
        }
    })();
    // A denied construction can restore donor custody; it must be dropped before refund.
    drop(donor);
    if slot != std::ptr::from_ref(budget) as usize
        || ledger != budget.work_ledger_identity_v1()
        || budget.storage() < floor
    {
        drain(result);
        return Err(Resource::Accounting.into());
    }
    budget.release_storage(
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?,
    )?;
    result
}
fn consume_source<'w, T>(
    entries: &RetainedContextEntriesV29,
    ssa: ProductionSemanticSsaOwnerV1,
    launch: ProductionSourceLaunchRosterV1,
    budget: &mut Budget<'w>,
    order: Order,
    consume: impl for<'scope> FnOnce(&Checked<'scope>, &mut Budget<'w>) -> TransportResult<T>,
) -> Result<TransportResult<T>, ProductionPipelineError> {
    let source = execution_source_v29(entries, &ssa, budget)?.ok_or(
        ProductionPipelineError::ContextHandoff(ProductionContextRootErrorV29::RootCensus),
    )?;
    let pending = with_projected_execution_source_v29(&source, budget, |input, budget| {
        Ok(Pending::try_materialize_with_budget(
            ssa,
            launch,
            input,
            ProductionSemanticKirLimitsV1::default(),
            budget,
        ))
    })
    .map_err(ProductionPipelineError::ContextHandoff)?
    .map_err(ProductionPipelineError::PendingScopedSource)?;
    Ok(consume_pending(pending, order, budget, consume))
}

impl<'tcx> super::super::ProductionCompilation<'tcx, super::super::CollectedRustStage<'tcx>> {
    /// Fixed callers must choose their own source/target policy. This is not a user-selectable
    /// production optimizer, and successful transport does not complete compilation.
    pub(crate) fn consume_checked_tile_scalar_source_v29<T>(
        self,
        order: Order,
        consume: impl for<'scope, 'w> FnOnce(&Checked<'scope>, &mut Budget<'w>) -> TransportResult<T>,
    ) -> Result<T, ProductionTileScalarPipelineErrorV29> {
        use ProductionTileScalarPipelineErrorV29 as E;
        let prepared = self
            .import_semantic_mir()
            .map_err(|error| E::Source(Box::new(error)))?
            .construct_semantic_middle_end()
            .map_err(|error| E::Source(Box::new(error)))?
            .construct_semantic_ssa()
            .map_err(|error| E::Source(Box::new(error)))?
            .with_prepared_materialization_budget_v29(|prepared, budget| {
                super::super::consume_prepared_with_budget_v29(
                    prepared,
                    budget,
                    |_, _| Ok(()),
                    |ssa, launch, entries, budget| {
                        consume_source(entries, ssa, launch, budget, order, consume)
                    },
                )
            })
            .map_err(E::Source)?;
        prepared.materialized.map_err(E::Transport)
    }
}
