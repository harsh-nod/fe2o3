//! Original-source custody through actual V18 optimization and final consumers.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as OwnedBudget,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_lower_mir_kernel::{
    ProductionOptimizedSourceCorrespondenceV18 as Optimized,
    ProductionSourceCorrespondenceV18 as Original,
};

type OptimizedOutputV18<T> = (
    fe2o3_pliron::CheckedNeutralKernelIrOwnerV18,
    T,
    fe2o3_pliron::KirNeutralOwnedOriginStorageV1,
);

#[path = "production_pipeline_optimized_policies_v18.rs"]
mod native_policies;

impl<'tcx> ProductionCompilation<'tcx, SsaSemanticMirStage> {
    fn consume_optimized_source_invocations_v18(
        self,
        account: &mut OwnedBudget,
        mut consume: impl FnMut(
            &crate::production_ranked_projection_v1::SourceInvocationProjectionV18<'_, '_>,
            &mut dyn fe2o3_mir_model::SemanticAssertionMeterV1<
                Error = crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1,
            >,
        ) -> Result<usize, crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1>,
    ) -> Result<PreparedMaterializationV29<OptimizedOutputV18<usize>>, ProductionPipelineError> {
        self.consume_optimized_source_owned_v18(account, |original, optimized, _, _, budget| {
            let (count, retained) = crate::production_ranked_projection_v1::with_optimized_source_invocations_v18(
                original, optimized, budget, &mut consume,
            ).map_err(ProductionPipelineError::RankedProjection)?;
            let transferred = retained.checked_add(std::mem::size_of::<usize>())
                .ok_or_else(|| ProductionPipelineError::SourceOwnedEntrance(
                    original.retain_query_resource_error_v18(Resource::Arithmetic)))?;
            // Stage A immediately re-reserves this complete result. No
            // controlled allocation occurs across the existing transfer gap.
            budget.release_storage(retained).map_err(|error|
                ProductionPipelineError::SourceOwnedEntrance(original.retain_query_resource_error_v18(error)))?;
            Ok((count, transferred))
        })
    }

    fn consume_optimized_source_owned_v18<T: 'static, F>(
        self,
        account: &mut OwnedBudget,
        consume: F,
    ) -> Result<PreparedMaterializationV29<OptimizedOutputV18<T>>, ProductionPipelineError>
    where
        F: for<'scope, 'work> FnOnce(
            &Original<'scope>,
            &Optimized<'scope>,
            &[crate::production_ranked_projection_v1::ProductionRankedRootInputV1],
            &AuthenticatedProductionBindings,
            &mut Budget<'work>,
        ) -> Result<(T, usize), ProductionPipelineError>,
    {
        let prepared = self.prepare_standard_materialization_inputs_v18()?;
        consume_optimized_prepared_with_account_v18(prepared, account, consume)
    }
}

#[cfg(test)]
impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    pub(crate) fn inspect_optimized_source_invocations_v18(
        self,
        account: &mut OwnedBudget,
        consume: impl FnMut(
            &crate::production_ranked_projection_v1::SourceInvocationProjectionV18<'_, '_>,
            &mut dyn fe2o3_mir_model::SemanticAssertionMeterV1<
                Error = crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1,
            >,
        ) -> Result<usize, crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1>,
    ) -> Result<OptimizedOutputV18<usize>, ProductionPipelineError> {
        let ssa = self.import_semantic_mir()?.construct_semantic_middle_end()?.construct_semantic_ssa()?;
        let PreparedMaterializationV29 { materialized, ranked_roots, bindings } =
            ssa.consume_optimized_source_invocations_v18(account, consume)?;
        drop((ranked_roots, bindings));
        Ok(materialized)
    }

    pub(crate) fn inspect_optimized_source_v18<T: 'static>(
        self,
        account: &mut OwnedBudget,
        consume: impl for<'scope, 'work> FnOnce(
            &Original<'scope>,
            &Optimized<'scope>,
            &mut Budget<'work>,
        ) -> Result<(T, usize), ProductionPipelineError>,
    ) -> Result<OptimizedOutputV18<T>, ProductionPipelineError> {
        let ssa = self
            .import_semantic_mir()?
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?;
        let PreparedMaterializationV29 {
            materialized,
            ranked_roots,
            bindings,
        } = ssa.consume_optimized_source_owned_v18(
            account,
            |original, optimized, roots, bindings, budget| {
                if roots.is_empty() || roots.len() != bindings.typed_descriptor_roots.len() {
                    return Err(ProductionPipelineError::SourceOwnedEntrance(
                        fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Binding(
                            "optimized test entrance retained root bindings",
                        ),
                    ));
                }
                consume(original, optimized, budget)
            },
        )?;
        drop((ranked_roots, bindings));
        Ok(materialized)
    }
}

/// Scoped terminal work runs before the genuine source, roots and bindings are
/// dropped. Only the checked owned graph and an owned, non-authoritative result
/// can escape. The continuing account is supplied before original source capture.
///
/// Existing prepared descriptor/reference ownership is borrowed without cloning;
/// its inherited admission remains separate from this source/optimizer ledger.
/// This private entrance is not the default or a completed ranked/target route.
fn consume_optimized_prepared_with_account_v18<T: 'static, F>(
    prepared: PreparedSsaMaterializationV29,
    account: &mut OwnedBudget,
    consume: F,
) -> Result<PreparedMaterializationV29<OptimizedOutputV18<T>>, ProductionPipelineError>
where
    F: for<'scope, 'work> FnOnce(
        &Original<'scope>,
        &Optimized<'scope>,
        &[crate::production_ranked_projection_v1::ProductionRankedRootInputV1],
        &AuthenticatedProductionBindings,
        &mut Budget<'work>,
    ) -> Result<(T, usize), ProductionPipelineError>,
{
    let headers = std::cell::Cell::new(0);
    let run = |budget: &mut Budget<'_>| {
        // Header refusal occurs before context authentication or source capture.
        // Errors deliberately retain these credits: an inner source refusal may
        // have denied refunds, and this outer boundary cannot erase that state.
        budget
            .reserve_storage(headers.get())
            .map_err(source_resource_v18)?;
        source_owned_v18::consume_source_bound_prepared_inner_v18(
            prepared,
            budget,
            |source, roots, bindings, budget| {
                source
                    .with_retained_checked_optimization_v18(
                        budget,
                        |original, optimized, budget| {
                            consume(original, optimized, roots, bindings, budget)
                        },
                    )
                    .map_err(source_optimization_error_v18)
            },
        )
    };
    headers.set(optimized_prepared_headers_v18::<T>(std::mem::size_of_val(
        &run,
    ))?);
    // No stack account is reconstructed and no high-water/first-refusal state
    // is copied out or reset. Final production consumers must stay in `consume`.
    account.with_budget(run)
}

fn source_resource_v18(error: Resource) -> ProductionPipelineError {
    ProductionPipelineError::SourceOwnedEntrance(error.into())
}

fn optimized_prepared_headers_v18<T>(
    closure_bytes: usize,
) -> Result<usize, ProductionPipelineError> {
    type Prepared<T> = PreparedMaterializationV29<OptimizedOutputV18<T>>;
    let mut bytes = 0usize;
    for amount in [
        closure_bytes,
        std::mem::size_of::<std::cell::Cell<usize>>(),
        std::mem::size_of::<OwnedBudget>(),
        std::mem::size_of::<PreparedSsaMaterializationV29>(),
        std::mem::size_of::<Prepared<T>>(),
        std::mem::size_of::<Result<Prepared<T>, ProductionPipelineError>>(),
        std::mem::size_of::<ProductionPipelineError>(),
    ] {
        bytes = bytes
            .checked_add(amount)
            .ok_or_else(|| source_resource_v18(Resource::Arithmetic))?;
    }
    Ok(bytes)
}

fn source_optimization_error_v18(
    error: fe2o3_lower_mir_kernel::ProductionSourceOptimizationErrorV18<ProductionPipelineError>,
) -> ProductionPipelineError {
    use fe2o3_lower_mir_kernel::ProductionSourceOptimizationErrorV18 as Source;
    use fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1 as Adoption;
    match error {
        Source::Source(error) => ProductionPipelineError::SourceOwnedEntrance(error),
        Source::Observation(error) => ProductionPipelineError::SourceOptimizationObservation(error),
        Source::Adoption(error) => match error {
            Adoption::Origin(error) => error,
            Adoption::Inventory(error) => {
                ProductionPipelineError::SourceOptimizationAdoption(Adoption::Inventory(error))
            }
            Adoption::Transition(error) => {
                ProductionPipelineError::SourceOptimizationAdoption(Adoption::Transition(error))
            }
            Adoption::Resource(error) => {
                ProductionPipelineError::SourceOptimizationAdoption(Adoption::Resource(error))
            }
            Adoption::OriginAccounting => {
                ProductionPipelineError::SourceOptimizationAdoption(Adoption::OriginAccounting)
            }
            Adoption::Panicked => {
                ProductionPipelineError::SourceOptimizationAdoption(Adoption::Panicked)
            }
        },
    }
}

#[cfg(test)]
#[path = "production_pipeline_optimized_source_resources_v18_tests.rs"]
mod resource_tests;
#[cfg(test)]
#[path = "production_pipeline_optimized_source_v18_tests.rs"]
mod tests;
