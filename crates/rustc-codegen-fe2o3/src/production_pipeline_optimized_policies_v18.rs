//! Additive source-owned observation of the actual output's fixed policy.
//! This private continuation grants no source-complete/ranked/target owner and
//! does not select or activate a default production route. Source-role recipes
//! remain mandatory before memory, execution, and other closed policy gates.
use super::*;
use fe2o3_kernel_analysis::{
    CanonicalRankedMetadataV18, CanonicalRankedViewErrorV1 as ViewError,
    build_canonical_ranked_candidate_v18, with_checked_canonical_ranked_view_v18,
};
use fe2o3_lower_mir_kernel::ProductionSourceNativeLifecycleDiagnosticV18 as NativeDiagnostic;
use fe2o3_lower_mir_kernel::ProductionSourceNativeLifecycleErrorV18 as NativeError;
type DiagnosticCell = std::cell::Cell<Option<NativeDiagnostic>>;

#[cfg(test)]
#[path = "production_pipeline_native_lifecycle_probe_v18_tests.rs"]
mod lifecycle_probe;

/// Diagnostic result only: no source-complete, final-ranked, or launch owner.
#[derive(Debug)]
pub(crate) struct OptimizedSourcePolicyObservationV18 {
    pub(crate) invocations: usize,
    pub(crate) currentness_roots: usize,
    pub(crate) defined_functions: usize,
    pub(crate) declarations: usize,
}

impl<'tcx> ProductionCompilation<'tcx, SsaSemanticMirStage> {
    fn consume_optimized_source_policies_v18(
        self,
        account: &mut OwnedBudget,
    ) -> Result<
        PreparedMaterializationV29<OptimizedOutputV18<OptimizedSourcePolicyObservationV18>>,
        ProductionPipelineError,
    > {
        let envelopes = native_lifecycle_envelopes()?;
        account
            .with_budget(|budget| budget.reserve_storage(envelopes))
            .map_err(source_resource_v18)?;
        let diagnostic = DiagnosticCell::new(None);
        let result = self.consume_optimized_source_owned_v18(
            account,
            |original, optimized, ranked_roots, bindings, budget| {
                let (invocations, retained) =
                    crate::production_ranked_projection_v1::with_optimized_source_invocations_v18(
                        original,
                        optimized,
                        budget,
                        |invocation, meter| {
                            // Runs the shared original invocation solver, preserving
                            // inactive disposition and exact original caller operands.
                            invocation.check_effect_rows_v18(meter)?;
                            Ok(0)
                        },
                    )
                    .map_err(ProductionPipelineError::RankedProjection)?;
                if retained != 0 {
                    return Err(retain_resource(original, Resource::Accounting));
                }
                // This is the existing final-output solver, not an input report
                // rebound to the optimizer result. Its first refusal is terminal.
                let currentness_roots =
                    original.check_optimized_source_currentness_v18(optimized, budget)?;
                check_actual_execution_input(original, bindings, budget)?;
                let observation = consume_actual_native_policy(
                    original,
                    optimized,
                    budget,
                    invocations,
                    currentness_roots,
                    &diagnostic,
                    ranked_roots,
                    &bindings.reference_effect_bindings,
                )?;
                Ok((
                    observation,
                    std::mem::size_of::<OptimizedSourcePolicyObservationV18>(),
                ))
            },
        );
        // Only a completed authentic source scope permits settling unused fixed
        // envelopes. Errors retain them conservatively, including owned native
        // diagnostics; their dynamic reports retain the native analysis receipt.
        if result.is_ok() {
            account
                .with_budget(|budget| budget.release_storage(envelopes))
                .map_err(source_resource_v18)?;
        }
        result.map_err(|error| attach_native_diagnostic(error, diagnostic.get()))
    }
}

fn native_lifecycle_envelopes() -> Result<usize, ProductionPipelineError> {
    let mut total = 0usize;
    for bytes in [
        std::mem::size_of::<NativeError>(),
        std::mem::size_of::<Result<(), NativeError>>(),
        std::mem::size_of::<Result<Result<(), NativeError>, ViewError>>(),
        std::mem::size_of::<Result<OptimizedSourcePolicyObservationV18, ProductionPipelineError>>(),
        std::mem::size_of::<OptimizedSourcePolicyObservationV18>(),
        std::mem::size_of::<std::cell::Cell<(usize, usize)>>(),
        2 * std::mem::size_of::<DiagnosticCell>(),
        std::mem::size_of::<Result<NativeDiagnostic, NativeError>>(),
    ] {
        total = total
            .checked_add(bytes)
            .ok_or_else(|| source_resource_v18(Resource::Arithmetic))?;
    }
    #[cfg(test)]
    for bytes in [
        std::mem::size_of::<lifecycle_probe::Observation>() * 2,
        std::mem::size_of::<
            Result<
                lifecycle_probe::Observation,
                fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18,
            >,
        >() * 2,
        std::mem::size_of::<std::cell::Cell<Option<lifecycle_probe::Observation>>>(),
        std::mem::size_of::<Result<(), fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18>>(),
    ] {
        total = total
            .checked_add(bytes)
            .ok_or_else(|| source_resource_v18(Resource::Arithmetic))?;
    }
    Ok(total)
}

fn attach_native_diagnostic(
    error: ProductionPipelineError,
    diagnostic: Option<NativeDiagnostic>,
) -> ProductionPipelineError {
    match (error, diagnostic) {
        (ProductionPipelineError::SourceOwnedEntrance(source), Some(diagnostic)) => {
            ProductionPipelineError::SourceNativeLifecycle(Box::new(
                NativeError::SourceAfterNative { source, diagnostic },
            ))
        }
        (error, _) => error,
    }
}

fn check_actual_execution_recipes(
    original: &Original<'_>,
    optimized: &Optimized<'_>,
    bindings: &AuthenticatedProductionBindings,
    budget: &mut Budget<'_>,
) -> Result<usize, ProductionPipelineError> {
    check_actual_execution_input(original, bindings, budget)?;
    let count = std::cell::Cell::new(0);
    optimized.with_execution_recipes_v18(budget, |recipes, budget| {
        count.set(recipes.len(budget)?);
        Ok::<_, fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18>(())
    })?;
    original.check_query_v18(budget)?;
    budget
        .release_storage(std::mem::size_of::<
            Result<(), fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18>,
        >())
        .map_err(|error| retain_resource(original, error))?;
    Ok(count.get())
}

fn check_actual_execution_input(
    original: &Original<'_>,
    bindings: &AuthenticatedProductionBindings,
    budget: &mut Budget<'_>,
) -> Result<(), ProductionPipelineError> {
    original.check_query_v18(budget)?;
    let source = original.source(budget)?;
    let ssa = source.source_ssa(budget)?;
    source.check_original_source(ssa, budget)?;
    let receipt =
        context_handoff_v29::execution_source_v29(&bindings.context_entries, ssa, budget)?.ok_or(
            ProductionPipelineError::SourceOwnedEntrance(
                fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Binding(
                    "original source lacks authenticated execution recipe input",
                ),
            ),
        )?;
    // Comparison only: the projection cannot see the lowerer's sticky cleanup
    // denial. It must settle before any authoritative scoped consumer runs.
    let compared = context_handoff_v29::with_projected_execution_source_v29(
        &receipt,
        budget,
        |input, budget| Ok(source.check_execution_input_v18(input, budget)),
    )
    .map_err(ProductionPipelineError::ContextHandoff)?;
    original.check_query_v18(budget)?;
    compared?;
    Ok(())
}

#[cfg(test)]
impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    pub(crate) fn inspect_optimized_source_policies_observed_v18(
        self,
        account: &mut OwnedBudget,
    ) -> (
        Result<OptimizedOutputV18<OptimizedSourcePolicyObservationV18>, ProductionPipelineError>,
        Option<lifecycle_probe::Observation>,
    ) {
        lifecycle_probe::reset(None);
        let result = self.inspect_optimized_source_policies_v18(account);
        (result, lifecycle_probe::take())
    }
    pub(crate) fn inspect_optimized_source_policies_resource_cut_v18(
        self,
        account: &mut OwnedBudget,
        storage_short: bool,
    ) -> (
        Result<OptimizedOutputV18<OptimizedSourcePolicyObservationV18>, ProductionPipelineError>,
        Option<lifecycle_probe::Observation>,
    ) {
        lifecycle_probe::reset(Some(storage_short));
        let result = self.inspect_optimized_source_policies_v18(account);
        (result, lifecycle_probe::take())
    }
    pub(crate) fn inspect_optimized_execution_recipes_v18(
        self,
        account: &mut OwnedBudget,
    ) -> Result<OptimizedOutputV18<usize>, ProductionPipelineError> {
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
            |original, optimized, _, bindings, budget| {
                let count = check_actual_execution_recipes(original, optimized, bindings, budget)?;
                Ok((count, std::mem::size_of::<usize>()))
            },
        )?;
        drop((ranked_roots, bindings));
        Ok(materialized)
    }

    pub(crate) fn inspect_optimized_source_policies_v18(
        self,
        account: &mut OwnedBudget,
    ) -> Result<OptimizedOutputV18<OptimizedSourcePolicyObservationV18>, ProductionPipelineError>
    {
        let ssa = self
            .import_semantic_mir()?
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?;
        let PreparedMaterializationV29 {
            materialized,
            ranked_roots,
            bindings,
        } = ssa.consume_optimized_source_policies_v18(account)?;
        drop((ranked_roots, bindings));
        Ok(materialized)
    }
}

fn retain_resource(original: &Original<'_>, error: Resource) -> ProductionPipelineError {
    ProductionPipelineError::SourceOwnedEntrance(original.retain_query_resource_error_v18(error))
}

fn view_error(original: &Original<'_>, error: ViewError) -> ProductionPipelineError {
    match error {
        ViewError::Resource(error) => retain_resource(original, error),
        other => ProductionPipelineError::SourceOptimizedRankedView(other),
    }
}

pub(super) fn reserve_policy_error_payload(budget: &mut Budget<'_>) -> Result<(), Resource> {
    // Keep the shared pipeline error small while prepaying its possible owned
    // diagnostic before a policy failure can exhaust the continuing ledger.
    budget.reserve_storage(std::mem::size_of::<
        fe2o3_pliron::CanonicalRankedPolicyChecksErrorV1,
    >())
}

fn consume_actual_native_policy(
    original: &Original<'_>,
    optimized: &Optimized<'_>,
    budget: &mut Budget<'_>,
    invocations: usize,
    currentness_roots: usize,
    diagnostic: &DiagnosticCell,
    ranked_roots: &[crate::production_ranked_projection_v1::ProductionRankedRootInputV1],
    reference_bindings: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
) -> Result<OptimizedSourcePolicyObservationV18, ProductionPipelineError> {
    original.check_query_v18(budget)?;
    let layouts = original
        .source(budget)?
        .limits(budget)?
        .storage_layout_limits();
    let output = optimized.output_inventory(budget)?;
    #[cfg(test)]
    lifecycle_probe::record(original, optimized, output, budget)?;
    // Empty metadata carries no source authority. The genuine original recipe
    // below must consume the complete structural role census independently.
    let metadata = CanonicalRankedMetadataV18::new(output.owner(), &[]);
    let metadata_storage = metadata
        .storage_extent(budget)
        .map_err(|error| view_error(original, error))?;
    budget
        .reserve_storage(metadata_storage)
        .map_err(|error| retain_resource(original, error))?;
    let (candidate, candidate_storage) =
        build_canonical_ranked_candidate_v18(output, &metadata, budget)
            .map_err(|error| view_error(original, error))?;
    budget
        .reserve_storage(candidate_storage.retained_storage())
        .map_err(|error| retain_resource(original, error))?;
    // The only graph emitted below is the actual checked output owner. Source
    // blocks are never replayed as replacement topology or an alternate graph.
    let counts = std::cell::Cell::new((0usize, 0usize));
    let result = with_checked_canonical_ranked_view_v18(
        output,
        &metadata,
        &candidate,
        budget,
        |checked, budget| {
            Ok::<_, ViewError>(optimized.with_lifecycle_native_policies_v18(
                checked,
                layouts,
                budget,
                |policies, budget| {
                    crate::production_ranked_projection_v1::with_source_native_ranked_roots_v18(
                        original,
                        optimized,
                        policies,
                        ranked_roots,
                        reference_bindings,
                        budget,
                        |roots, budget| {
                            for root in 0..roots.root_count(budget)? {
                                let _ = roots.root(root, budget)?;
                            }
                            diagnostic.set(Some(policies.diagnostic(budget)?));
                            #[cfg(test)]
                            if let Some(storage_short) = lifecycle_probe::native_entry() {
                                // Query the exact actual report before cutting this same
                                // invocation; no separate unmodified graph is emitted.
                                let count = policies.function_count(budget)?;
                                assert!(count > 0);
                                assert!((0..count).any(|ordinal| {
                                    policies.report(ordinal, budget).unwrap().is_some()
                                }));
                                if storage_short {
                                    let limit =
                                        crate::production_canonical_phase_policy_v1::STORAGE_LIMIT;
                                    let error = budget
                                        .reserve_storage(limit - budget.storage() + 1)
                                        .unwrap_err();
                                    return Err(NativeError::Source(
                                        original.retain_query_resource_error_v18(error),
                                    ));
                                }
                                let limit = usize::try_from(
                                    crate::production_canonical_phase_policy_v1::WORK_LIMIT,
                                )
                                .unwrap();
                                budget.charge_work(limit - budget.work()).unwrap();
                            }
                            let mut defined_functions = 0usize;
                            let mut declarations = 0usize;
                            for ordinal in 0..policies.function_count(budget)? {
                                if policies.report(ordinal, budget)?.is_some() {
                                    defined_functions =
                                        defined_functions.checked_add(1).ok_or_else(|| {
                                            NativeError::Source(
                                                original.retain_query_resource_error_v18(
                                                    Resource::Arithmetic,
                                                ),
                                            )
                                        })?;
                                } else {
                                    declarations =
                                        declarations.checked_add(1).ok_or_else(|| {
                                            NativeError::Source(
                                                original.retain_query_resource_error_v18(
                                                    Resource::Arithmetic,
                                                ),
                                            )
                                        })?;
                                }
                            }
                            counts.set((defined_functions, declarations));
                            #[cfg(test)]
                            lifecycle_probe::consumed();
                            Ok(())
                        },
                    )
                },
            ))
        },
    )
    .map_err(|error| view_error(original, error))?;
    if let Err(error) = &result {
        if let Some(observed) = error.native_diagnostic() {
            diagnostic.set(Some(observed));
        }
    }
    drop(candidate);
    drop(metadata);
    original.check_query_v18(budget)?;
    budget
        .release_storage(
            metadata_storage
                .checked_add(candidate_storage.retained_storage())
                .ok_or_else(|| retain_resource(original, Resource::Arithmetic))?,
        )
        .map_err(|error| retain_resource(original, error))?;
    result.map_err(|error| ProductionPipelineError::SourceNativeLifecycle(Box::new(error)))?;
    let (defined_functions, declarations) = counts.get();
    Ok(OptimizedSourcePolicyObservationV18 {
        invocations,
        currentness_roots,
        defined_functions,
        declarations,
    })
}
