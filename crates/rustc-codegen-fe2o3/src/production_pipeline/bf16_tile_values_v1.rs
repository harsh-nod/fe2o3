//! Increment I: live source/SSA transport in the original pre-ranked phase ledger.
//! Ordinary materialization still refuses the nominal shared DeviceMatrix helper.
//! Returning the ordinary owner does not bypass any later ranked/formal/target gate.
use super::*;
use crate::production_bf16_tile_values_source_v1::SourceOwnedBf16TileValuesRegionV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_lower_mir_kernel::Bf16CallInstanceErrorV1 as Error;

#[cfg(test)]
#[path = "bf16_tile_values_observation_v1_tests.rs"]
mod observation;

fn inspection(error: Error) -> ProductionPipelineError {
    ProductionPipelineError::Bf16TileValuesInspection(Box::new(error))
}
fn unavailable(why: &'static str) -> ProductionPipelineError {
    inspection(Error::Unavailable(why))
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    // Intentionally no public command/selector and no post-ranked repeatable view.
    // The dedicated importer result, never bytes or a bool, owns the live seed.
    pub(super) fn materialize_with_bf16_tile_values_inspection_v1<R: Copy + 'static>(
        self,
        inspect: impl for<'a, 'work> FnOnce(
            &SourceOwnedBf16TileValuesRegionV1<'a, 'tcx>,
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
        let imported: crate::collector::AuthenticatedBf16TileValuesMirV1<'tcx> =
            crate::collector::construct_production_semantic_mir_bf16_tile_values_v1(
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
            let mut occurrence_storage = None;
            #[cfg(test)]
            let normal_attempted = std::cell::Cell::new(false);
            let result = materialize_prepared_with_budget_v29(
                prepared,
                budget,
                |_, _| Ok(()),
                |mut semantic_ssa, launch, budget| {
                    #[cfg(test)]
                    materializer_storage.set(Some(budget.storage()));
                    // Attach the actual owner's occurrences exactly once. The
                    // API returns an unreserved receipt: reserve it immediately.
                    if semantic_ssa.occurrence_storage().is_some() {
                        return Err(unavailable("BF16 helper occurrence capture was not fresh"));
                    }
                    let receipt = semantic_ssa.try_capture_occurrences_with_budget_v1(budget)
                        .map_err(|error| ProductionPipelineError::PreRankedMaterialization(
                            fe2o3_lower_mir_kernel::ProductionPreRankedKirErrorV1::Occurrences(error)))?;
                    budget.reserve_storage(receipt.retained_storage())
                        .map_err(materialization_resource_error_v29)?;
                    occurrence_storage = Some(receipt.retained_storage());
                    let observed = fe2o3_lower_mir_kernel::with_checked_bf16_call_instance_v1(
                        &semantic_ssa, budget,
                        |relation, budget| source_seed.with_relation(relation, budget, inspect),
                    ).map_err(inspection)?;
                    #[cfg(test)]
                    normal_attempted.set(true);
                    // SAME owner; ordinary Preexisting capture convention. No
                    // new constructor, flattening, CalleeCollective exception,
                    // pass suppression or alternate emission graph exists here.
                    let owner = fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
                        semantic_ssa, launch,
                        fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(), budget,
                    ).map_err(ProductionPipelineError::PreRankedMaterialization)?;
                    let retained = owner.retained_analysis_storage_v1();
                    Ok(((owner, observed), retained))
                },
            );
            // On refusal the consumed SSA has already been dropped. Release
            // only this scope's actual occurrence reservation. A success keeps
            // it live under the ordinary owner's Preexisting receipt convention.
            if result.is_err() {
                if let Some(bytes) = occurrence_storage {
                    let retained_floor = protected.checked_add(bytes)
                        .ok_or_else(|| materialization_resource_error_v29(Resource::Arithmetic))?;
                    if budget.work_ledger_identity_v1() != ledger || budget.storage() < retained_floor {
                        drop(result);
                        return Err(Box::new(materialization_resource_error_v29(Resource::Accounting)));
                    }
                    budget.release_storage(bytes).map_err(materialization_resource_error_v29)?;
                }
            }
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
                occurrence_storage,
                normal_attempted: normal_attempted.get(),
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
