//! Live frontend P0 joins INSIDE the original pre-ranked phase ledger.
//! Returning the ordinary owner does not bypass any later ranked/formal/target gate.
use super::*;
use crate::production_tiled_region_source_v1::SourceOwnedBf16MfmaRegionV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_lower_mir_kernel::ProductionTiledRegionInspectionErrorV1 as Error;

#[cfg(test)]
#[path = "tiled_region_observation_v1_tests.rs"]
mod observation;

fn inspection(error: Error) -> ProductionPipelineError {
    ProductionPipelineError::TiledRegionInspection(Box::new(error))
}
fn unavailable(why: &'static str) -> ProductionPipelineError {
    inspection(Error::Unavailable(why))
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    // Intentionally no public command/selector and no post-ranked repeatable view.
    // The dedicated importer result, never bytes or a bool, owns the live seed.
    pub(super) fn materialize_with_bf16_mfma_inspection_v1<R>(
        self,
        inspect: impl for<'a, 'work> FnOnce(
            &SourceOwnedBf16MfmaRegionV1<'a, 'tcx>,
            &mut Budget<'work>,
        ) -> Result<R, Error>,
    ) -> Result<(MaterializedNeutralProductionCompilation, R), Box<ProductionPipelineError>> {
        let CollectedRustStage {
            tcx,
            closure,
            typed_descriptor_roots,
            debug_source_capture,
            transaction,
        } = self.stage;
        // This finite inspection does not acquire protected-publication policy.
        if !transaction.compiler_custody.is_extraction_only() {
            return Err(Box::new(unavailable(
                "BF16 source inspection requires extraction-only custody",
            )));
        }
        let imported: crate::collector::AuthenticatedBf16MfmaInspectionMirV1<'tcx> =
            crate::collector::construct_production_semantic_mir_bf16_inspection_v1(
                tcx,
                closure,
                debug_source_capture,
            )
            .map_err(ProductionPipelineError::SemanticImport)?;
        let (constructed, source_seed) = imported.into_parts();
        let crate::collector::ConstructedProductionSemanticMirV1 {
            semantic_mir,
            context_entries,
            rustc_identity_inventory,
            rustc_preflight_plan,
            rustc_target,
            reference_effect_bindings,
            debug_source_files,
            debug_source_scopes,
            debug_source_variables,
            debug_capture_gap,
        } = constructed;
        let typed_descriptor_roots =
            crate::compiler_descriptor::order_typed_descriptor_roots_by_semantic_v1(
                typed_descriptor_roots,
                &semantic_mir,
            )
            .map_err(ProductionPipelineError::DescriptorEvidence)?;
        let admitted: ProductionCompilation<'tcx, AdmittedSemanticMirStage> =
            ProductionCompilation {
                stage: AdmittedSemanticMirStage {
                    semantic_mir,
                    bindings: AuthenticatedProductionBindings {
                        context_entries,
                        rustc_identity_inventory,
                        rustc_preflight_plan,
                        rustc_target,
                        reference_effect_bindings,
                        debug_source_files,
                        debug_source_scopes,
                        debug_source_variables,
                        debug_capture_gap,
                        typed_descriptor_roots,
                        transaction,
                    },
                },
                invariant_session: PhantomData,
            };
        let ssa = admitted
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?;
        if source_seed.semantic_sha256()
            != ssa
                .stage
                .semantic_ssa
                .source_semantic()
                .semantic_sha256()
                .as_bytes()
            || !ssa
                .stage
                .bindings
                .rustc_target
                .rustc_layout()
                .has_exact_codegen_profile("gfx942", "-wavefrontsize32,+wavefrontsize64,-xnack")
            || !ssa
                .stage
                .bindings
                .reference_effect_bindings
                .as_slice()
                .is_empty()
            || ssa
                .stage
                .bindings
                .rustc_preflight_plan
                .rustc_identity_inventory_sha256()
                != ssa.stage.bindings.rustc_identity_inventory.sha256()
        {
            return Err(Box::new(unavailable(
                "BF16 actual source/target/lineage or refinement scope differs",
            )));
        }
        // This is the existing meter created by the ordinary phase. No sibling
        // meter is created, and no work is reconstructed from observed counters.
        let prepared = ssa.with_prepared_materialization_budget_v29(|prepared, budget| {
            let owned = source_seed.phase_storage_bytes();
            let floor = budget.storage();
            let ledger = budget.work_ledger_identity_v1();
            budget
                .reserve_storage(owned)
                .map_err(materialization_resource_error_v29)?;
            let protected = floor
                .checked_add(owned)
                .ok_or_else(|| materialization_resource_error_v29(Resource::Arithmetic))?;
            #[cfg(test)]
            let materializer_storage = std::cell::Cell::new(None);
            let result = materialize_prepared_with_budget_v29(
                prepared,
                budget,
                |_, _| Ok(()),
                |semantic_ssa, launch, budget| {
                    #[cfg(test)]
                    materializer_storage.set(Some(budget.storage()));
                    let (owner, observed) =
                        fe2o3_lower_mir_kernel::materialize_with_bf16_mfma_inspection_v1(
                            semantic_ssa,
                            launch,
                            fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
                            budget,
                            |_, emission, budget| {
                                source_seed.with_region(emission, budget, inspect)
                            },
                        )
                        .map_err(inspection)?;
                    // The ordinary helper immediately reserves this unreserved
                    // owner receipt before any other controlled allocation.
                    let retained = owner.retained_analysis_storage_v1();
                    Ok(((owner, observed), retained))
                },
            );
            drop(source_seed);
            // Drop before release; preserve callback-owned extra reservations,
            // consumed work and sticky denial history even on a callback error.
            if budget.work_ledger_identity_v1() != ledger || budget.storage() < protected {
                drop(result);
                return Err(Box::new(materialization_resource_error_v29(
                    Resource::Accounting,
                )));
            }
            budget
                .release_storage(owned)
                .map_err(materialization_resource_error_v29)?;
            #[cfg(test)]
            observation::record(observation::PhaseObservation {
                entry_storage: floor,
                source_storage: owned,
                protected_storage: protected,
                materializer_storage: materializer_storage.get(),
                final_storage: budget.storage(),
                same_ledger: budget.work_ledger_identity_v1() == ledger,
                work: Some(budget.work()),
                failed_work: budget.failed_work().is_some(),
                failed_storage: budget.failed_storage().is_some(),
                result_ok: result.is_ok(),
            });
            result
        })?;
        let PreparedMaterializationV29 {
            materialized: (materialized, observed),
            ranked_roots,
            bindings,
        } = prepared;
        Ok((
            MaterializedNeutralProductionCompilation {
                materialized,
                ranked_roots,
                bindings,
            },
            observed,
        ))
    }
}
