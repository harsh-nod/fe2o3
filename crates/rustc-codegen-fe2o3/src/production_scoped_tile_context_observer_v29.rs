//! Diagnostic source continuation; never a normal executable compilation result.
use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionPendingScopedSourceOwnerV29 as Pending, ProductionScopedTileCandidateViewV29 as View,
    ProductionScopedTileObservationErrorV29 as ObservationError,
    ProductionScopedTileObservationOrderV29 as ObservationOrder,
};
use std::convert::Infallible;

fn observe_tile_v29(
    entries: &RetainedContextEntriesV29,
    ssa: ProductionSemanticSsaOwnerV1,
    launch: ProductionSourceLaunchRosterV1,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    order: ObservationOrder,
    observe: impl for<'view, 'work> FnOnce(
        View<'view>,
        &mut CanonicalKernelIrVerificationResourceBudgetV1<'work>,
    ) -> Result<(), ProductionPipelineError>,
) -> Result<Infallible, ProductionPipelineError> {
    let owner = pending_source_owner_v29(entries, ssa, launch, budget)?;
    let ledger = budget.work_ledger_identity_v1();
    let adopted = owner.adopted_storage();
    let ready = budget.storage();
    let mut donor = Some(owner);
    let result = catch_unwind(AssertUnwindSafe(|| {
        Pending::with_scalar_candidate_observation_in_order_v29(&mut donor, budget, order, observe)
    }));
    // Entry/frame refusals retain the donor. Once taken, the lowerer owns all
    // candidate cleanup; callback-owned storage is never ours to release.
    let cleanup = if let Some(owner) = donor.take() {
        drop(owner);
        if budget.work_ledger_identity_v1() == ledger && budget.storage() >= ready {
            budget.release_storage(adopted)
        } else {
            Err(Resource::Accounting)
        }
    } else {
        Ok(())
    };
    match result {
        Err(payload) => resume_unwind(payload),
        Ok(result) => {
            cleanup.map_err(|error| ProductionPipelineError::ContextHandoff(error.into()))?;
            match result {
                Ok(()) => Err(ProductionPipelineError::ScopedTileObservationIncomplete),
                Err(ObservationError::Callback(error)) => Err(error),
                Err(ObservationError::Resource(error)) => {
                    Err(ProductionPipelineError::ScopedTileObservation(
                        ObservationError::Resource(error),
                    ))
                }
                Err(ObservationError::Unavailable { phase, reason }) => {
                    Err(ProductionPipelineError::ScopedTileObservation(
                        ObservationError::Unavailable { phase, reason },
                    ))
                }
            }
        }
    }
}

impl<'tcx> super::super::ProductionCompilation<'tcx, super::super::CollectedRustStage<'tcx>> {
    pub(crate) fn observe_scoped_tile_candidate_v29(
        self,
        observe: impl for<'view, 'work> FnOnce(
            View<'view>,
            &mut CanonicalKernelIrVerificationResourceBudgetV1<'work>,
        ) -> Result<(), ProductionPipelineError>,
    ) -> Result<(), Box<ProductionPipelineError>> {
        self.observe_scoped_tile_candidate_in_order_v29(ObservationOrder::Blocked, observe)
    }

    pub(crate) fn observe_scoped_tile_candidate_in_order_v29(
        self,
        order: ObservationOrder,
        observe: impl for<'view, 'work> FnOnce(
            View<'view>,
            &mut CanonicalKernelIrVerificationResourceBudgetV1<'work>,
        ) -> Result<(), ProductionPipelineError>,
    ) -> Result<(), Box<ProductionPipelineError>> {
        let never = self
            .import_semantic_mir()?
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?
            .with_prepared_materialization_budget_v29(|prepared, budget| {
                super::super::consume_prepared_with_budget_v29(
                    prepared,
                    budget,
                    |_, _| Ok(()),
                    |ssa, launch, entries, budget| {
                        observe_tile_v29(entries, ssa, launch, budget, order, observe)
                    },
                )
            })?;
        match never.materialized {}
    }
}
