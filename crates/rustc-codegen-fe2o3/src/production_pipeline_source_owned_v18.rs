//! Original prepared source and V18 attachments under one continuing ledger.
//! This entrance grants no ranked, optimizer, native or executable authority.

use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_lower_mir_kernel::{
    ProductionPendingScopedSourceOwnerV29 as Pending, ProductionSemanticKirLimitsV1 as Limits,
    ProductionSourceOwnedViewErrorV18 as ViewError, ProductionSourceOwnedViewV18 as View,
};

impl From<ViewError> for ProductionPipelineError {
    fn from(error: ViewError) -> Self {
        Self::SourceOwnedEntrance(error)
    }
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    // The eventual fixed ranked/final continuation enters here. Until those
    // successors exist, default publication continues to refuse their absence.
    #[allow(
        dead_code,
        reason = "The source-owning ranked and final successors are not activated"
    )]
    pub(crate) fn consume_source_owned_v18<T>(
        self,
        consume: impl for<'scope, 'work> FnOnce(
            &View<'scope>,
            &mut Budget<'work>,
        ) -> Result<T, ViewError>,
    ) -> Result<T, Box<ProductionPipelineError>> {
        self.import_semantic_mir()?
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?
            .consume_source_owned_v18(consume)
    }
}

impl<'tcx> ProductionCompilation<'tcx, SsaSemanticMirStage> {
    fn consume_source_owned_v18<T>(
        self,
        consume: impl for<'scope, 'work> FnOnce(
            &View<'scope>,
            &mut Budget<'work>,
        ) -> Result<T, ViewError>,
    ) -> Result<T, Box<ProductionPipelineError>> {
        self.with_prepared_materialization_budget_v29(|prepared, budget| {
            let PreparedMaterializationV29 {
                materialized,
                ranked_roots,
                bindings,
            } = consume_source_owned_prepared_with_budget_v18(prepared, budget, consume)?;
            // These original descriptor/reference bindings remain alive through
            // the entire callback; later ranked/final stages must consume them.
            drop((ranked_roots, bindings));
            Ok(materialized)
        })
    }
}

fn consume_source_owned_prepared_with_budget_v18<T>(
    prepared: PreparedSsaMaterializationV29,
    budget: &mut Budget<'_>,
    consume: impl for<'scope, 'work> FnOnce(&View<'scope>, &mut Budget<'work>) -> Result<T, ViewError>,
) -> Result<PreparedMaterializationV29<T>, Box<ProductionPipelineError>> {
    consume_source_bound_prepared_with_budget_v18(prepared, budget, |view, _, _, budget| {
        consume(view, budget).map_err(Into::into)
    })
}

// Both the source-view tests and the real ranked continuation consume this
// original prepared owner. No source, graph, launch or binding is reconstructed.
pub(super) fn consume_source_bound_prepared_with_budget_v18<T>(
    prepared: PreparedSsaMaterializationV29,
    budget: &mut Budget<'_>,
    consume: impl for<'scope, 'work> FnOnce(
        &View<'scope>,
        &[crate::production_ranked_projection_v1::ProductionRankedRootInputV1],
        &AuthenticatedProductionBindings,
        &mut Budget<'work>,
    ) -> Result<T, ProductionPipelineError>,
) -> Result<PreparedMaterializationV29<T>, Box<ProductionPipelineError>> {
    consume_source_bound_prepared_inner_v18(prepared, budget, consume).map_err(Box::new)
}

pub(super) fn consume_source_bound_prepared_inner_v18<T>(
    prepared: PreparedSsaMaterializationV29,
    budget: &mut Budget<'_>,
    consume: impl for<'scope, 'work> FnOnce(
        &View<'scope>,
        &[crate::production_ranked_projection_v1::ProductionRankedRootInputV1],
        &AuthenticatedProductionBindings,
        &mut Budget<'work>,
    ) -> Result<T, ProductionPipelineError>,
) -> Result<PreparedMaterializationV29<T>, ProductionPipelineError> {
    consume_prepared_with_bindings_inner_v18(
        prepared,
        budget,
        |_, _| Ok(()),
        |ssa, launch, ranked_roots, bindings, budget| {
            let source = context_handoff_v29::execution_source_v29(&bindings.context_entries, &ssa, budget)?
                .ok_or(ProductionPipelineError::SourceOwnedEntrance(ViewError::Binding(
                    "original source lacks a complete authenticated callable/declaration/event census",
                )))?;
            // No table or source-root emitter runs inside the projection scope.
            // In particular, an inner no-refund disposition cannot be lost at
            // this cross-crate boundary. Owned capture is the only result.
            let prepared = context_handoff_v29::with_projected_execution_source_v29(
                &source,
                budget,
                |input, budget| {
                    Ok(crate::compiler_descriptor::prepare_source_with_kernel_arguments_v18(
                        ssa,
                        launch,
                        input,
                        &bindings.typed_descriptor_roots,
                        Limits::default(),
                        budget,
                    ))
                },
            )
            .map_err(ProductionPipelineError::ContextHandoff)??;
            // Temporary projection vectors have dropped and their exact credits
            // have settled before the shared lowerer cleanup disposition exists.
            prepared.with_source_consumer_v18(budget, |view, budget| {
                consume(view, ranked_roots, bindings, budget)
            })
        },
    )
}

#[cfg(test)]
impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    // An independently hostile original-census control at the real projection
    // boundary, not a second producer or an alternate accepted materializer.
    pub(crate) fn check_source_preparation_refusal_v18(
        self,
    ) -> Result<(), Box<ProductionPipelineError>> {
        self.import_semantic_mir()?
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?
            .with_prepared_materialization_budget_v29(|prepared, budget| {
                consume_prepared_with_budget_v29(prepared, budget, |_, _| Ok(()), |ssa, launch, entries, budget| {
                    let source = context_handoff_v29::execution_source_v29(entries, &ssa, budget)?
                        .expect("actual source has its authenticated complete census");
                    assert!(ssa.occurrence_storage().is_none());
                    let floor = budget.storage();
                    let result = context_handoff_v29::with_projected_execution_source_v29(
                        &source, budget, |input, budget| {
                            assert!(!input.classes.is_empty());
                            let floor = budget.storage();
                            let invalid = fe2o3_lower_mir_kernel::ProductionExecutionSourceInputV29 {
                                classes: &[], ..input
                            };
                            let result = Pending::prepare_source_with_budget_v18(
                                ssa, launch, invalid, Limits::default(), budget,
                            );
                            assert!(matches!(&result, Err(ViewError::Source(
                                fe2o3_lower_mir_kernel::ProductionPendingScopedSourceErrorV29::Source(
                                    fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::Unsupported {
                                        function: 0, block: None, statement: None,
                                        detail: "execution lifecycle differs from its retained source instance",
                                    }
                                )
                            ))), "invalid original census must fail source capture, not emit a module");
                            assert_eq!(budget.storage(), floor, "fresh capture dropped before refund");
                            Ok(result)
                        },
                    ).map_err(ProductionPipelineError::ContextHandoff)?;
                    assert!(matches!(result, Err(ViewError::Source(
                        fe2o3_lower_mir_kernel::ProductionPendingScopedSourceErrorV29::Source(
                            fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::Unsupported {
                                function: 0, block: None, statement: None,
                                detail: "execution lifecycle differs from its retained source instance",
                            }
                        )
                    ))), "projection must not substitute an accounting error");
                    assert_eq!(budget.storage(), floor, "only known projection credits refunded");
                    Ok(())
                }).map(|_| ())
            })
    }
}
